use std::{
    collections::{BTreeMap, HashMap},
    mem, thread,
    time::{self, Duration, SystemTime},
};

use {
    futures::{channel::mpsc, future, prelude::*, stream},
    metrics::{Key, Recorder},
    rusoto_cloudwatch::{CloudWatch, Dimension, MetricDatum, PutMetricDataInput, StatisticSet},
    rusoto_core::Region,
};

use crate::{error::Error, BoxFuture};

pub type ClientBuilder = Box<dyn Fn(Region) -> Box<dyn CloudWatch + Send + Sync> + Send + Sync>;
type Count = usize;
type HistogramValue = u64;
type Timestamp = u64;

const MAX_CW_METRICS_PER_CALL: usize = 20;
const MAX_CLOUDWATCH_DIMENSIONS: usize = 10;
const MAX_HISTOGRAM_VALUES: usize = 150;
const SEND_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Config {
    pub cloudwatch_namespace: String,
    pub default_dimensions: BTreeMap<String, String>,
    pub storage_resolution: Resolution,
    pub send_interval_secs: u64,
    pub region: Region,
    pub client_builder: ClientBuilder,
    pub shutdown_signal: future::Shared<BoxFuture<'static, ()>>,
}

#[derive(Clone, Copy, Debug)]
pub enum Resolution {
    Second,
    Minute,
}

enum Message {
    Datum(Datum),
    SendBatch {
        send_all_before: Timestamp,
        emit_sender: mpsc::Sender<MetricsBatch>,
    },
}

#[derive(Debug)]
enum MetricsBatch {
    Statistics(Vec<MetricDatum>),
    Histogram(MetricDatum),
}

#[derive(Debug)]
enum Value {
    Counter(u64),
    Gauge(i64),
    Histogram(u64),
}

#[derive(Clone, Debug, Default)]
struct Aggregate {
    counter: StatisticSet,
    gauge: StatisticSet,
    histogram: HashMap<HistogramValue, Count>,
}

struct Collector {
    metrics_data: BTreeMap<Timestamp, HashMap<Key, Aggregate>>,
    config: &'static Config,
}

#[derive(Debug)]
struct Datum {
    key: Key,
    value: Value,
}

#[derive(Debug, Default)]
struct Histogram {
    counts: Vec<f64>,
    values: Vec<f64>,
}

struct HistogramDatum {
    count: f64,
    value: f64,
}

struct RecorderHandle(mpsc::Sender<Datum>);

pub fn init(config: Config) {
    let _ = thread::spawn(|| {
        let mut runtime = tokio::runtime::Builder::new()
            // single threaded
            .basic_scheduler()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            if let Err(e) = init_future(config).await {
                log::warn!("{}", e);
            }
        });
    });
}

pub async fn init_future(config: Config) -> Result<(), Error> {
    let config: &'static _ = Box::leak(Box::new(config));

    let (collect_sender, collect_receiver) = mpsc::channel(1024);
    let (emit_sender, emit_receiver) = mpsc::channel(1024);
    let mut message_stream = Box::pin(
        stream::select(
            collect_receiver.map(Message::Datum),
            mk_send_batch_timer(emit_sender.clone(), config),
        )
        .take_until(config.shutdown_signal.clone().map(|_| true)),
    );
    let emitter = mk_emitter(emit_receiver, config);
    let mut collector = Collector::new(config);
    let collection_fut = async {
        while let Some(msg) = message_stream.next().await {
            collector.accept(msg);
        }
        // Need to drop this before flushing or we deadlock on shutdown
        drop(message_stream);
        // Send a final flush on shutdown
        collector.accept(Message::SendBatch {
            send_all_before: std::u64::MAX,
            emit_sender,
        });
    };
    let process_fut = future::join(collection_fut, emitter.map(|_| ()));
    let recorder = Box::new(RecorderHandle(collect_sender));
    metrics::set_boxed_recorder(recorder).map_err(Error::SetRecorder)?;
    process_fut.await;
    Ok(())
}

async fn mk_emitter(mut emit_receiver: mpsc::Receiver<MetricsBatch>, config: &Config) {
    let cloudwatch_client = (config.client_builder)(config.region.clone());
    let put = |metric_data| async {
        let send_fut = cloudwatch_client.put_metric_data(PutMetricDataInput {
            metric_data,
            namespace: config.cloudwatch_namespace.clone(),
        });
        match tokio::time::timeout(SEND_TIMEOUT, send_fut).await {
            Ok(Ok(())) => log::debug!("Successfully sent a metrics batch to CloudWatch."),
            Ok(Err(e)) => log::warn!("Failed to send metrics: {}", e),
            Err(tokio::time::Elapsed { .. }) => log::warn!("Failed to send metrics: send timeout"),
        }
    };
    while let Some(metrics) = emit_receiver.next().await {
        let mut requests = vec![];
        match metrics {
            MetricsBatch::Statistics(mut metrics) => loop {
                let mut batch = metrics.split_off(MAX_CW_METRICS_PER_CALL.min(metrics.len()));
                mem::swap(&mut batch, &mut metrics);
                if batch.is_empty() {
                    break;
                } else {
                    requests.push(put(batch));
                }
            },
            MetricsBatch::Histogram(histogram) => {
                requests.push(put(vec![histogram]));
            }
        }
        future::join_all(requests).map(|_| ()).await;
    }
}

fn mk_send_batch_timer(
    emit_sender: mpsc::Sender<MetricsBatch>,
    config: &'static Config,
) -> impl Stream<Item = Message> {
    let interval = Duration::from_secs(config.send_interval_secs);
    tokio::time::interval_at(tokio::time::Instant::now(), interval).map(move |_instant| {
        let send_all_before = time_key(current_timestamp(), config.storage_resolution) - 1;
        Message::SendBatch {
            send_all_before,
            emit_sender: emit_sender.clone(),
        }
    })
}

fn current_timestamp() -> Timestamp {
    time::UNIX_EPOCH.elapsed().unwrap().as_secs()
}

fn timestamp_string(now: SystemTime) -> String {
    let dt = chrono::DateTime::<chrono::offset::Utc>::from(now);
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn time_key(timestamp: Timestamp, resolution: Resolution) -> Timestamp {
    match resolution {
        Resolution::Second => timestamp,
        Resolution::Minute => timestamp - (timestamp % 60),
    }
}

impl Collector {
    fn new(config: &'static Config) -> Self {
        Self {
            metrics_data: Default::default(),
            config,
        }
    }

    fn accept(&mut self, message: Message) {
        let result = match message {
            Message::Datum(datum) => Ok(self.accept_datum(datum)),
            Message::SendBatch {
                send_all_before,
                emit_sender,
            } => self.accept_send_batch(send_all_before, emit_sender),
        };
        if let Err(e) = result {
            log::warn!("Failed to accept message: {}", e);
        }
    }

    fn accept_datum(&mut self, datum: Datum) {
        let aggregate = self
            .metrics_data
            .entry(time_key(
                current_timestamp(),
                self.config.storage_resolution,
            ))
            .or_insert_with(HashMap::new)
            .entry(datum.key)
            .or_default();

        let apply_stats = |stats_set: &mut StatisticSet, value: f64| {
            stats_set.sample_count += 1.0;
            stats_set.sum += value;
            stats_set.maximum = stats_set.maximum.max(value);
            stats_set.minimum = stats_set.minimum.min(value);
        };

        match datum.value {
            Value::Counter(value) => {
                apply_stats(&mut aggregate.counter, value as f64);
            }
            Value::Gauge(value) => {
                apply_stats(&mut aggregate.gauge, value as f64);
            }
            Value::Histogram(value) => {
                *aggregate.histogram.entry(value).or_default() += 1;
            }
        }
    }

    fn dimensions(&self, key: &Key) -> Vec<Dimension> {
        let dimensions_from_keys = key.labels().map(|l| Dimension {
            name: l.key().to_owned(),
            value: l.value().to_owned(),
        });
        self.default_dimensions()
            .chain(dimensions_from_keys)
            .take(MAX_CLOUDWATCH_DIMENSIONS)
            .collect()
    }

    /// Sends a batch of the earliest collected metrics to CloudWatch
    ///
    /// # Params
    /// * send_all_before: All messages before this timestamp should be split off from the aggregation and
    /// sent to CloudWatch
    fn accept_send_batch(
        &mut self,
        send_all_before: Timestamp,
        mut emit_sender: mpsc::Sender<MetricsBatch>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut range = self.metrics_data.split_off(&send_all_before);
        mem::swap(&mut range, &mut self.metrics_data);

        let mut metrics_batch = vec![];

        for (timestamp, stats_by_key) in range {
            let timestamp = timestamp_string(time::UNIX_EPOCH + Duration::from_secs(timestamp));

            for (key, aggregate) in stats_by_key {
                let Aggregate {
                    counter,
                    gauge,
                    histogram,
                } = aggregate;
                let dimensions = self.dimensions(&key);

                let stats_set_datum = &mut |stats_set, unit| MetricDatum {
                    dimensions: Some(dimensions.clone()),
                    metric_name: key.name().into_owned(),
                    timestamp: Some(timestamp.clone()),
                    storage_resolution: Some(self.config.storage_resolution.as_secs()),
                    statistic_values: Some(stats_set),
                    unit,
                    ..Default::default()
                };

                if counter.sample_count > 0.0 {
                    metrics_batch.push(stats_set_datum(counter, Some("Count".to_owned())));
                }
                if gauge.sample_count > 0.0 {
                    metrics_batch.push(stats_set_datum(gauge, None));
                }

                let histogram_datum = &mut |Histogram { values, counts }, unit| MetricDatum {
                    dimensions: Some(dimensions.clone()),
                    metric_name: key.name().into_owned(),
                    timestamp: Some(timestamp.clone()),
                    storage_resolution: Some(self.config.storage_resolution.as_secs()),
                    unit,
                    values: Some(values),
                    counts: Some(counts),
                    ..Default::default()
                };

                if !histogram.is_empty() {
                    let histogram_data = &mut histogram.into_iter().map(|(k, v)| HistogramDatum {
                        value: k as f64,
                        count: v as f64,
                    });
                    loop {
                        let histogram = histogram_data.take(MAX_HISTOGRAM_VALUES).fold(
                            Histogram::default(),
                            |mut memo, datum| {
                                memo.values.push(datum.value);
                                memo.counts.push(datum.count);
                                memo
                            },
                        );
                        if histogram.values.is_empty() {
                            break;
                        };
                        let metrics_batch =
                            MetricsBatch::Histogram(histogram_datum(histogram, None));
                        if let Err(e) = emit_sender.try_send(metrics_batch) {
                            log::debug!("error queueing: {}", e);
                        }
                    }
                }
            }
        }
        if !metrics_batch.is_empty() {
            emit_sender.try_send(MetricsBatch::Statistics(metrics_batch))?;
        }
        Ok(())
    }

    fn default_dimensions(&self) -> impl Iterator<Item = Dimension> {
        self.config
            .default_dimensions
            .clone()
            .into_iter()
            .map(|(name, value)| Dimension { name, value })
    }
}

impl Recorder for RecorderHandle {
    fn increment_counter(&self, key: Key, value: u64) {
        let _ = self.0.clone().try_send(Datum {
            key,
            value: Value::Counter(value),
        });
    }

    fn update_gauge(&self, key: Key, value: i64) {
        let _ = self.0.clone().try_send(Datum {
            key,
            value: Value::Gauge(value),
        });
    }

    fn record_histogram(&self, key: Key, value: u64) {
        let _ = self.0.clone().try_send(Datum {
            key,
            value: Value::Histogram(value),
        });
    }
}

impl Resolution {
    fn as_secs(self) -> i64 {
        match self {
            Self::Second => 1,
            Self::Minute => 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_key_should_truncate() {
        assert_eq!(time_key(370, Resolution::Second), 370);
        assert_eq!(time_key(370, Resolution::Minute), 360);
    }
}
