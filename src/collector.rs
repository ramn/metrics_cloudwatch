use std::{
    collections::{BTreeMap, HashMap},
    mem, thread,
    time::{self, Duration, Instant, SystemTime},
};

use {
    futures::{channel::mpsc, future, prelude::*, stream},
    metrics::{Key, Recorder},
    rusoto_cloudwatch::{
        CloudWatch, CloudWatchClient, Dimension, MetricDatum, PutMetricDataInput, StatisticSet,
    },
    rusoto_core::Region,
};

use crate::error::Error;

type MetricsBatch = Vec<MetricDatum>;
type Timestamp = u64;

const MAX_CW_METRICS_PER_CALL: usize = 20;
const MAX_CLOUDWATCH_DIMENSIONS: usize = 10;
const SEND_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Config {
    pub cloudwatch_namespace: String,
    pub default_dimensions: BTreeMap<String, String>,
    pub storage_resolution: Resolution,
    pub send_interval_secs: u64,
    pub region: Region,
}

#[derive(Clone, Copy, Debug)]
pub enum Resolution {
    Second,
    Minute,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
enum Kind {
    Counter,
    Gauge,
    Histogram,
}

struct Datum {
    key: Key,
    value: f64,
    kind: Kind,
}

enum Message {
    Datum(Datum),
    SendBatch {
        tick_ts: Instant,
        emit_sender: mpsc::Sender<MetricsBatch>,
    },
}

struct RecorderHandle(mpsc::Sender<Datum>);

struct Collector {
    metrics_data: BTreeMap<Timestamp, HashMap<(Key, Kind), StatisticSet>>,
    config: &'static Config,
}

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
    let mut stream = stream::select(
        collect_receiver.map(Message::Datum),
        mk_send_batch_timer(emit_sender, config),
    );
    let emitter = mk_emitter(emit_receiver, config);
    let mut collector = Collector::new(config);
    let collection_fut = async {
        while let Some(msg) = stream.next().await {
            collector.accept(msg);
        }
    };
    let process_fut = future::join(collection_fut, emitter.map(|_| ()));
    let recorder = Box::new(RecorderHandle(collect_sender));
    metrics::set_boxed_recorder(recorder).map_err(Error::SetRecorder)?;
    process_fut.await;
    Ok(())
}

async fn mk_emitter(mut emit_receiver: mpsc::Receiver<MetricsBatch>, config: &Config) {
    let cloudwatch_client = CloudWatchClient::new(config.region.clone());
    while let Some(mut metrics) = emit_receiver.next().await {
        let mut requests = vec![];
        loop {
            let mut batch = metrics.split_off(MAX_CW_METRICS_PER_CALL.min(metrics.len()));
            mem::swap(&mut batch, &mut metrics);
            if batch.is_empty() {
                break;
            } else {
                requests.push(async {
                    let send_fut = cloudwatch_client.put_metric_data(PutMetricDataInput {
                        metric_data: batch,
                        namespace: config.cloudwatch_namespace.clone(),
                    });
                    if let Err(e) = tokio::time::timeout(SEND_TIMEOUT, send_fut).await {
                        log::warn!("Failed to send metrics: {}", e);
                    }
                });
            }
        }
        future::join_all(requests).map(|_| ()).await;
    }
}

fn mk_send_batch_timer(
    emit_sender: mpsc::Sender<MetricsBatch>,
    config: &Config,
) -> impl Stream<Item = Message> {
    let interval = Duration::from_secs(config.send_interval_secs);
    tokio::time::interval(interval).map(move |tick_ts| Message::SendBatch {
        tick_ts: tick_ts.into_std(),
        emit_sender: emit_sender.clone(),
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
                tick_ts,
                emit_sender,
            } => self.accept_send_batch(tick_ts, emit_sender),
        };
        if let Err(e) = result {
            log::warn!("Failed to accept message: {}", e);
        }
    }

    fn accept_datum(&mut self, datum: Datum) {
        let stats_set = self
            .metrics_data
            .entry(time_key(
                current_timestamp(),
                self.config.storage_resolution,
            ))
            .or_insert_with(HashMap::new)
            .entry((datum.key, datum.kind))
            .or_default();
        let value = datum.value;
        stats_set.sample_count += 1.0;
        stats_set.sum += value;
        stats_set.maximum = stats_set.maximum.max(value);
        stats_set.minimum = stats_set.minimum.min(value);
    }

    fn accept_send_batch(
        &mut self,
        _tick_ts: Instant,
        mut emit_sender: mpsc::Sender<MetricsBatch>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let breakoff = current_timestamp() - 1;
        let mut range = self.metrics_data.split_off(&breakoff);
        mem::swap(&mut range, &mut self.metrics_data);

        let mut metrics_batch = vec![];

        for (timestamp, stats_by_key) in range {
            let timestamp = timestamp_string(time::UNIX_EPOCH + Duration::from_secs(timestamp));
            for ((key, kind), stats_set) in stats_by_key {
                let unit = match kind {
                    Kind::Counter => Some("Count".into()),
                    _ => None,
                };
                let dimensions_from_keys = key.labels().map(|l| Dimension {
                    name: l.key().to_owned(),
                    value: l.value().to_owned(),
                });
                let dimensions = self
                    .default_dimensions()
                    .chain(dimensions_from_keys)
                    .take(MAX_CLOUDWATCH_DIMENSIONS)
                    .collect();
                let metric_datum = MetricDatum {
                    dimensions: Some(dimensions),
                    metric_name: key.name().into_owned(),
                    timestamp: Some(timestamp.clone()),
                    storage_resolution: Some(self.config.storage_resolution.as_secs()),
                    statistic_values: Some(stats_set),
                    unit,
                    ..Default::default()
                };
                metrics_batch.push(metric_datum);
            }
        }

        Ok(emit_sender.try_send(metrics_batch)?)
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
            value: value as f64,
            kind: Kind::Counter,
        });
    }

    fn update_gauge(&self, key: Key, value: i64) {
        let _ = self.0.clone().try_send(Datum {
            key,
            value: value as f64,
            kind: Kind::Gauge,
        });
    }

    // TODO: impl histogram, using MetricDatum { counts, values, }
    fn record_histogram(&self, key: Key, value: u64) {
        let _ = self.0.clone().try_send(Datum {
            key,
            value: value as f64,
            kind: Kind::Histogram,
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
