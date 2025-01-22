use metrics::{CounterFn, GaugeFn, HistogramFn, KeyName, Metadata, SharedString};
use std::sync::Arc;
use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    mem,
    pin::Pin,
    task::Poll,
    thread,
    time::{self, Duration, SystemTime},
};

use {
    aws_sdk_cloudwatch::{
        error::SdkError,
        operation::put_metric_data::PutMetricDataError,
        primitives::DateTime,
        types::{Dimension, MetricDatum, StandardUnit, StatisticSet},
        Client,
    },
    futures_util::{
        future,
        stream::{self, Stream},
        FutureExt, StreamExt,
    },
    metrics::{GaugeValue, Key, Recorder, Unit},
    tokio::sync::mpsc,
};

use crate::{error::Error, BoxFuture};

#[doc(hidden)]
pub trait CloudWatch {
    fn put_metric_data(
        &self,
        config: &Config,
        data: Vec<MetricDatum>,
    ) -> BoxFuture<'_, Result<(), SdkError<PutMetricDataError>>>;
}

impl CloudWatch for Client {
    fn put_metric_data(
        &self,
        config: &Config,
        data: Vec<MetricDatum>,
    ) -> BoxFuture<'_, Result<(), SdkError<PutMetricDataError>>> {
        let put = self.put_metric_data();
        let namespace = config.cloudwatch_namespace.clone();
        #[cfg(feature = "gzip")]
        let gzip = config.gzip;
        #[allow(unused_mut)]
        async move {
            put.namespace(namespace)
                .set_metric_data(Some(data))
                .customize()
                .map_request(move |request| {
                    #[cfg(feature = "gzip")]
                    if gzip {
                        use std::io::Write;

                        let mut request = request;

                        // If the request is already encoded upstream has most likely implemented gzipping, so don't gzip it again
                        if let Some(content_encoding) = request.headers().get("content-encoding") {
                            log::trace!(
                                "PutMetricData request is already encoded: {:?}",
                                content_encoding
                            );
                        } else {
                            let body = request.body_mut();

                            if let Some(bytes) = body.bytes() {
                                let mut encoder = flate2::write::GzEncoder::new(
                                    Vec::new(),
                                    flate2::Compression::default(),
                                );
                                encoder.write_all(bytes)?;

                                let r = encoder.finish()?;
                                let r_len = r.len() as u64;
                                *body = r.into();
                                request.headers_mut().insert("content-encoding", "gzip");
                                request
                                    .headers_mut()
                                    .insert("content-length", r_len.to_string());
                            }
                        }
                        return Ok::<_, std::io::Error>(request);
                    }
                    Ok::<_, std::io::Error>(request)
                })
                .send()
                .await
                .map(|_| ())
        }
        .boxed()
    }
}

type Count = usize;
type HistogramValue = ordered_float::NotNan<f64>;
type Timestamp = u64;
type HashMap<K, V> = std::collections::HashMap<K, V, ahash::RandomState>;

const MAX_CW_METRICS_PER_CALL: usize = 1000;
const MAX_CLOUDWATCH_DIMENSIONS: usize = 30;
const MAX_HISTOGRAM_VALUES: usize = 150;
const MAX_CW_METRICS_PUT_SIZE: usize = 800_000; // Docs say 1Mb but we set our max lower to be safe since we only have a heuristic
const RETRY_INTERVAL: Duration = Duration::from_secs(4);

// Only public for tests/common/mock.rs
pub struct Config {
    pub cloudwatch_namespace: String,
    pub default_dimensions: BTreeMap<String, String>,
    pub storage_resolution: Resolution,
    pub send_interval_secs: u64,
    pub send_timeout_secs: u64,
    pub shutdown_signal: future::Shared<BoxFuture<'static, ()>>,
    pub metric_buffer_size: usize,
    #[cfg(feature = "gzip")]
    pub gzip: bool,
}

struct CollectorConfig {
    default_dimensions: BTreeMap<String, String>,
    storage_resolution: Resolution,
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
        emit_sender: mpsc::Sender<Vec<MetricDatum>>,
    },
}

#[derive(Debug)]
enum Value {
    Register {
        unit: Option<Unit>,
        description: Option<SharedString>,
    },
    Counter(CounterValue),
    Gauge(GaugeValue),
    Histogram(HistogramValue, usize),
}

#[derive(Debug)]
enum CounterValue {
    Increase(u64),
    Absolute(u64),
}

#[derive(Clone, Debug, Default)]
pub struct Counter {
    sample_count: u64,
    sum: u64,
}

#[derive(Clone, Debug, Default)]
struct Aggregate {
    counter: Counter,
    gauge: Option<Gauge>,
    histogram: HashMap<HistogramValue, Count>,
}

#[derive(Clone, Debug, Default)]
struct Gauge {
    maximum: f64,
    minimum: f64,
    current: f64,
}

struct Collector {
    metrics_data: BTreeMap<Timestamp, HashMap<Key, Aggregate>>,
    metrics_config: HashMap<Key, MetricConfig>,
    config: CollectorConfig,
}

#[derive(Clone, Debug, Default)]
struct MetricConfig {
    unit: Option<Unit>,
    description: Option<SharedString>,
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

pub(crate) fn init(
    set_global_recorder: fn(
        RecorderHandle,
    ) -> Result<(), metrics::SetRecorderError<RecorderHandle>>,
    client: impl CloudWatch + Send + 'static,
    config: Config,
    force_flush_stream: Option<Pin<Box<dyn Stream<Item = ()> + Send>>>,
) {
    let _ = thread::spawn(move || {
        // single threaded
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            match init_future(set_global_recorder, client, config, force_flush_stream).await {
                Err(e) => {
                    log::warn!("{}", e);
                }
                Ok(task) => task.await,
            }
        });
    });
}

pub(crate) async fn init_future(
    set_global_recorder: fn(
        RecorderHandle,
    ) -> Result<(), metrics::SetRecorderError<RecorderHandle>>,
    client: impl CloudWatch,
    config: Config,
    force_flush_stream: Option<Pin<Box<dyn Stream<Item = ()> + Send>>>,
) -> Result<impl Future<Output = ()>, Error> {
    let (recorder, task) = new(client, config, force_flush_stream);
    set_global_recorder(recorder).map_err(Error::SetRecorder)?;
    Ok(task)
}

pub fn new(
    client: impl CloudWatch,
    config: Config,
    force_flush_stream: Option<Pin<Box<dyn Stream<Item = ()> + Send>>>,
) -> (RecorderHandle, impl Future<Output = ()>) {
    let (collect_sender, mut collect_receiver) = mpsc::channel(1024);
    let (emit_sender, emit_receiver) = mpsc::channel(config.metric_buffer_size);

    let force_flush_stream = force_flush_stream
        .unwrap_or_else(|| {
            Box::pin(futures_util::stream::empty::<()>()) as Pin<Box<dyn Stream<Item = ()> + Send>>
        })
        .map({
            let emit_sender = emit_sender.clone();
            move |()| Message::SendBatch {
                send_all_before: u64::MAX,
                emit_sender: emit_sender.clone(),
            }
        });

    let message_stream = Box::pin(
        stream::select(
            stream::select(
                stream::poll_fn(move |cx| collect_receiver.poll_recv(cx)).map(Message::Datum),
                mk_send_batch_timer(emit_sender.clone(), &config),
            ),
            force_flush_stream,
        )
        .take_until(config.shutdown_signal.clone().map(|_| true)),
    );

    let internal_config = CollectorConfig {
        default_dimensions: config.default_dimensions.clone(),
        storage_resolution: config.storage_resolution.clone(),
    };

    let emitter = mk_emitter(emit_receiver, client, config);

    let mut collector = Collector::new(internal_config);
    let collection_fut = async move {
        collector.accept_messages(message_stream).await;
        // Send a final flush on shutdown
        collector
            .accept(Message::SendBatch {
                send_all_before: u64::MAX,
                emit_sender,
            })
            .await;
    };
    (
        RecorderHandle {
            sender: collect_sender.clone(),
            registry: metrics_util::registry::Registry::new(CloudwatchStorage {
                sender: collect_sender,
            }),
        },
        async move {
            futures_util::join!(collection_fut, emitter);
        },
    )
}

async fn retry_on_throttled<T, E, F>(
    mut action: impl FnMut() -> F,
    send_timeout_secs: u64,
) -> Result<Result<T, E>, tokio::time::error::Elapsed>
where
    F: Future<Output = Result<T, E>>,
    E: fmt::Display,
{
    let send_timeout = Duration::from_secs(send_timeout_secs);
    match tokio::time::timeout(send_timeout, action()).await {
        Ok(Ok(t)) => Ok(Ok(t)),
        Ok(Err(err)) if err.to_string().contains("Throttling") => {
            tokio::time::sleep(RETRY_INTERVAL).await;
            tokio::time::timeout(send_timeout, action()).await
        }
        Ok(Err(err)) => Ok(Err(err)),
        Err(err) => Err(err),
    }
}

async fn mk_emitter(
    mut emit_receiver: mpsc::Receiver<Vec<MetricDatum>>,
    cloudwatch_client: impl CloudWatch,
    config: Config,
) {
    let cloudwatch_client = &cloudwatch_client;
    while let Some(metrics) = emit_receiver.recv().await {
        for metric_data in metrics_chunks(&metrics) {
            let send_fut = retry_on_throttled(
                || async {
                    cloudwatch_client
                        .put_metric_data(&config, metric_data.to_owned())
                        .await
                },
                config.send_timeout_secs,
            );
            match send_fut.await {
                Ok(Ok(_output)) => {
                    log::debug!("Successfully sent a metrics batch to CloudWatch.")
                }
                Ok(Err(e)) => log::warn!(
                    "Failed to send metrics: {:?}: {}",
                    metric_data
                        .iter()
                        .map(|m| &m.metric_name)
                        .collect::<Vec<_>>(),
                    e,
                ),
                Err(tokio::time::error::Elapsed { .. }) => {
                    log::warn!("Failed to send metrics: send timeout")
                }
            }
        }
    }
}

fn fit_metrics<'a>(metrics: impl IntoIterator<Item = &'a MetricDatum>) -> usize {
    let mut split = 0;

    let mut current_len = 0;
    // PutMetricData uses this really high overhead format so just take a high estimate of that.
    //
    // Assumes each value sent is ~60 bytes
    // ```
    // MetricData.member.2.Dimensions.member.2.Value=m1.small
    // ```
    for (i, metric) in metrics
        .into_iter()
        .take(MAX_CW_METRICS_PER_CALL)
        .enumerate()
    {
        current_len += metric_size(metric);
        if current_len > MAX_CW_METRICS_PUT_SIZE {
            break;
        }
        split = i + 1;
    }
    split
}

fn metrics_chunks(mut metrics: &[MetricDatum]) -> impl Iterator<Item = &[MetricDatum]> + '_ {
    std::iter::from_fn(move || {
        let split = fit_metrics(metrics);
        let (chunk, rest) = metrics.split_at(split);
        metrics = rest;
        if chunk.is_empty() {
            None
        } else {
            Some(chunk)
        }
    })
}

fn metric_size(metric: &MetricDatum) -> usize {
    60 * (
        // The 6 non Vec fields
        6 + metric.values().len() + metric.counts().len() + metric.dimensions().len()
    )
}

fn jitter_interval_at(
    start: tokio::time::Instant,
    interval: Duration,
) -> impl Stream<Item = tokio::time::Instant> {
    use rand::{rngs::SmallRng, Rng, SeedableRng};

    let rng = SmallRng::from_rng(&mut rand::rng());
    let variance = 0.1;
    let interval_secs = interval.as_secs_f64();
    let min = Duration::from_secs_f64(interval_secs * (1.0 - variance));
    let max = Duration::from_secs_f64(interval_secs * (1.0 + variance));

    let delay = Box::pin(tokio::time::sleep_until(start));
    stream::unfold((rng, delay), move |(mut rng, mut delay)| async move {
        (&mut delay).await;
        let now = delay.deadline();
        delay.as_mut().reset(now + rng.random_range(min..max));
        Some((now, (rng, delay)))
    })
}

fn mk_send_batch_timer(
    emit_sender: mpsc::Sender<Vec<MetricDatum>>,
    config: &Config,
) -> impl Stream<Item = Message> {
    let interval = Duration::from_secs(config.send_interval_secs);
    let storage_resolution = config.storage_resolution;
    jitter_interval_at(tokio::time::Instant::now(), interval).map(move |_instant| {
        let send_all_before = time_key(current_timestamp(), storage_resolution) - 1;
        Message::SendBatch {
            send_all_before,
            emit_sender: emit_sender.clone(),
        }
    })
}

fn current_timestamp() -> Timestamp {
    time::UNIX_EPOCH.elapsed().unwrap().as_secs()
}

fn datetime(now: SystemTime) -> DateTime {
    use aws_smithy_types_convert::date_time::DateTimeExt;
    let dt = chrono::DateTime::<chrono::offset::Utc>::from(now);
    DateTime::from_chrono_utc(dt)
}

fn time_key(timestamp: Timestamp, resolution: Resolution) -> Timestamp {
    match resolution {
        Resolution::Second => timestamp,
        Resolution::Minute => timestamp - (timestamp % 60),
    }
}

fn get_timeslot<'a>(
    metrics_data: &'a mut BTreeMap<Timestamp, HashMap<Key, Aggregate>>,
    config: &'a CollectorConfig,
    timestamp: Timestamp,
) -> &'a mut HashMap<Key, Aggregate> {
    metrics_data
        .entry(time_key(timestamp, config.storage_resolution))
        .or_default()
}

fn accept_datum(
    slot: &mut HashMap<Key, Aggregate>,
    metrics_config: &mut HashMap<Key, MetricConfig>,
    datum: Datum,
) {
    match datum.value {
        Value::Register { unit, description } => {
            let metric_config = metrics_config.entry(datum.key).or_default();
            if unit.is_some() {
                metric_config.unit = unit;
            }
            if description.is_some() {
                metric_config.description = description;
            }
        }

        Value::Counter(value) => {
            let aggregate = slot.entry(datum.key).or_default();
            let counter = &mut aggregate.counter;
            counter.sample_count += 1;
            counter.sum = match value {
                CounterValue::Increase(value) => counter.sum + value,
                CounterValue::Absolute(value) => value,
            }
        }
        Value::Gauge(gauge_value) => {
            let aggregate = slot.entry(datum.key).or_default();

            match &mut aggregate.gauge {
                Some(gauge) => {
                    gauge.current = gauge_value.update_value(gauge.current);
                    gauge.maximum = gauge.maximum.max(gauge.current);
                    gauge.minimum = gauge.minimum.min(gauge.current);
                }
                None => {
                    let value = gauge_value.update_value(0.0);
                    aggregate.gauge = Some(Gauge {
                        current: value,
                        maximum: value,
                        minimum: value,
                    });
                }
            }
        }
        Value::Histogram(value, count) => {
            let aggregate = slot.entry(datum.key).or_default();
            *aggregate.histogram.entry(value).or_default() += count;
        }
    }
}

impl Collector {
    fn new(config: CollectorConfig) -> Self {
        Self {
            metrics_data: Default::default(),
            metrics_config: Default::default(),
            config,
        }
    }

    async fn accept_messages(&mut self, messages: impl Stream<Item = Message>) {
        futures_util::pin_mut!(messages);
        while let Some(message) = messages.next().await {
            let result = async {
                match message {
                    Message::Datum(datum) => {
                        let timestamp = current_timestamp();
                        let slot = get_timeslot(&mut self.metrics_data, &self.config, timestamp);

                        accept_datum(slot, &mut self.metrics_config, datum);

                        // `current_timestamp` can be pretty expensive when there are a lot of
                        // metrics so as long as we are immediately able to read more datums we
                        // assume that we can use the same timestamp for those, thereby amortizing
                        // the cost.
                        // 100 is arbitrarily chosen to get good a good amount of reuse without
                        // risking that this is always ready, thereby never updating the timestamp
                        for _ in 0..100 {
                            match future::lazy(|cx| messages.as_mut().poll_next(cx)).await {
                                Poll::Ready(Some(message)) => match message {
                                    Message::Datum(datum) => {
                                        accept_datum(slot, &mut self.metrics_config, datum)
                                    }
                                    Message::SendBatch {
                                        send_all_before,
                                        emit_sender,
                                    } => {
                                        self.accept_send_batch(send_all_before, emit_sender)?;
                                        break;
                                    }
                                },
                                Poll::Ready(None) => return Ok(()),
                                Poll::Pending => {
                                    tokio::task::yield_now().await;
                                    break;
                                }
                            }
                        }
                        Ok(())
                    }
                    Message::SendBatch {
                        send_all_before,
                        emit_sender,
                    } => self.accept_send_batch(send_all_before, emit_sender),
                }
            };
            if let Err(e) = result.await {
                log::warn!("Failed to accept message: {}", e);
            }
        }
    }

    async fn accept(&mut self, message: Message) {
        self.accept_messages(stream::iter(Some(message))).await;
    }

    fn dimensions(&self, key: &Key) -> Vec<Dimension> {
        let mut dimensions_from_keys = key
            .labels()
            .filter(|l| !l.key().starts_with('@'))
            .map(|l| Dimension::builder().name(l.key()).value(l.value()).build())
            .peekable();

        let has_extra_dimensions = dimensions_from_keys.peek().is_some();

        let mut all_dims: Vec<_> = self
            .default_dimensions()
            .chain(dimensions_from_keys)
            .collect();

        if has_extra_dimensions {
            // Reverse order, so that the last value will override when we dedup
            all_dims.reverse();
            all_dims.sort_by(|a, b| a.name.cmp(&b.name));
            all_dims.dedup_by(|a, b| a.name == b.name);
        }
        all_dims.retain(|dim| !dim.value().unwrap_or("").is_empty());

        if all_dims.len() > MAX_CLOUDWATCH_DIMENSIONS {
            all_dims.truncate(MAX_CLOUDWATCH_DIMENSIONS);
            log::warn!(
                "Too many dimensions, taking only {}",
                MAX_CLOUDWATCH_DIMENSIONS
            );
        }

        all_dims
    }

    /// Sends a batch of the earliest collected metrics to CloudWatch
    ///
    /// # Params
    /// * send_all_before: All messages before this timestamp should be split off from the
    ///   aggregation and sent to CloudWatch
    fn accept_send_batch(
        &mut self,
        send_all_before: Timestamp,
        emit_sender: mpsc::Sender<Vec<MetricDatum>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut range = self.metrics_data.split_off(&send_all_before);
        mem::swap(&mut range, &mut self.metrics_data);

        let mut metrics_batch = vec![];

        for (timestamp, stats_by_key) in range {
            let timestamp = datetime(time::UNIX_EPOCH + Duration::from_secs(timestamp));

            for (key, aggregate) in stats_by_key {
                let Aggregate {
                    counter,
                    gauge,
                    histogram,
                } = aggregate;
                let dimensions = self.dimensions(&key);
                let unit = self
                    .metrics_config
                    .get(&key)
                    .and_then(|metric_config| metric_config.unit.as_ref())
                    .and_then(unit_cloudwatch_str)
                    .or_else(|| key.labels().find(|l| l.key() == "@unit").map(|l| l.value()))
                    .map(StandardUnit::from);

                let stats_set_datum = &mut |stats_set, unit: &Option<StandardUnit>| {
                    MetricDatum::builder()
                        .set_dimensions(Some(dimensions.clone()))
                        .metric_name(key.name())
                        .timestamp(timestamp)
                        .storage_resolution(self.config.storage_resolution.as_secs() as i32)
                        .statistic_values(stats_set)
                        .set_unit(unit.clone())
                        .build()
                };

                if counter.sample_count > 0 {
                    let sum = counter.sum as f64;
                    let stats_set = StatisticSet::builder()
                        // We aren't interested in how many `counter!` calls we did so we put 1
                        // here to allow cloudwatch to display the average between this and any
                        // other instances posting to the same metric.
                        .sample_count(1.0)
                        .sum(sum)
                        // Max and min for a count can either be the sum or the max/min of the
                        // value passed to each `increment_counter` call.
                        //
                        // In the case where we only increment by `1` each call the latter makes
                        // min and max basically useless since the end result will leave both as `1`.
                        // In the case where we sum the count first before calling
                        // `increment_counter` we do lose some granularity as the latter would give
                        // a spread in min/max.
                        // However if that is an interesting metric it would often be
                        // better modeled as the gauge (measuring how much were processed in each
                        // batch).
                        //
                        // Therefor we opt to send the sum to give a measure of how many
                        // counts *this* metrics instance observed in this time period.
                        .maximum(sum)
                        .minimum(sum)
                        .build();

                    metrics_batch.push(stats_set_datum(
                        stats_set,
                        &unit.clone().or(Some(StandardUnit::Count)),
                    ));
                }

                if let Some(Gauge {
                    current,
                    minimum,
                    maximum,
                }) = gauge
                {
                    // Gauges only submit the current value and the max and min
                    let statistic_set = StatisticSet::builder()
                        .sample_count(1.0)
                        .sum(current)
                        .maximum(maximum)
                        .minimum(minimum)
                        .build();

                    metrics_batch.push(stats_set_datum(statistic_set, &unit));
                }

                let histogram_datum =
                    &mut |Histogram { values, counts }, unit: &Option<StandardUnit>| {
                        MetricDatum::builder()
                            .metric_name(key.name())
                            .timestamp(timestamp)
                            .storage_resolution(self.config.storage_resolution.as_secs() as i32)
                            .set_dimensions(Some(dimensions.clone()))
                            .set_unit(unit.clone())
                            .set_values(Some(values))
                            .set_counts(Some(counts))
                            .build()
                    };

                if !histogram.is_empty() {
                    let histogram_data = &mut histogram.into_iter().map(|(k, v)| HistogramDatum {
                        value: f64::from(k),
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
                        metrics_batch.push(histogram_datum(histogram, &unit));
                    }
                }
            }
        }
        if !metrics_batch.is_empty() {
            emit_sender.try_send(metrics_batch)?;
        }
        Ok(())
    }

    fn default_dimensions(&self) -> impl Iterator<Item = Dimension> + '_ {
        self.config
            .default_dimensions
            .iter()
            .map(|(name, value)| Dimension::builder().name(name).value(value).build())
    }
}

struct CloudwatchStorage {
    sender: mpsc::Sender<Datum>,
}

impl metrics_util::registry::Storage<Key> for CloudwatchStorage {
    type Counter = Arc<RecorderHandleKey>;
    type Gauge = Arc<RecorderHandleKey>;
    type Histogram = Arc<RecorderHandleKey>;

    fn counter(&self, key: &Key) -> Self::Counter {
        Arc::new(RecorderHandleKey {
            key: key.clone(),
            sender: self.sender.clone(),
        })
    }

    fn gauge(&self, key: &Key) -> Self::Gauge {
        Arc::new(RecorderHandleKey {
            key: key.clone(),
            sender: self.sender.clone(),
        })
    }

    fn histogram(&self, key: &Key) -> Self::Histogram {
        Arc::new(RecorderHandleKey {
            key: key.clone(),
            sender: self.sender.clone(),
        })
    }
}

pub struct RecorderHandle {
    sender: mpsc::Sender<Datum>,
    registry: metrics_util::registry::Registry<Key, CloudwatchStorage>,
}

impl RecorderHandle {
    pub fn register_counter(&self, key: &Key) -> metrics::Counter {
        self.registry
            .get_or_create_counter(key, |x| metrics::Counter::from_arc(x.clone()))
    }

    pub fn register_gauge(&self, key: &Key) -> metrics::Gauge {
        self.registry
            .get_or_create_gauge(key, |x| metrics::Gauge::from_arc(x.clone()))
    }

    pub fn register_histogram(&self, key: &Key) -> metrics::Histogram {
        self.registry
            .get_or_create_histogram(key, |x| metrics::Histogram::from_arc(x.clone()))
    }

    pub fn describe_gauge(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
        let _ = self.sender.try_send(Datum {
            key: Key::from_name(key),
            value: Value::Register {
                unit,
                description: Some(description),
            },
        });
    }

    pub fn describe_histogram(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
        let _ = self.sender.try_send(Datum {
            key: Key::from_name(key),
            value: Value::Register {
                unit,
                description: Some(description),
            },
        });
    }

    pub fn describe_counter(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
        let _ = self.sender.try_send(Datum {
            key: Key::from_name(key),
            value: Value::Register {
                unit,
                description: Some(description),
            },
        });
    }
}

#[derive(Clone)]
struct RecorderHandleKey {
    key: Key,
    sender: mpsc::Sender<Datum>,
}

impl RecorderHandleKey {
    fn increment_counter(&self, value: u64) {
        let _ = self.sender.try_send(Datum {
            key: self.key.clone(),
            value: Value::Counter(CounterValue::Increase(value)),
        });
    }

    fn set_counter(&self, value: u64) {
        let _ = self.sender.try_send(Datum {
            key: self.key.clone(),
            value: Value::Counter(CounterValue::Absolute(value)),
        });
    }

    fn increment_gauge(&self, value: f64) {
        let _ = self.sender.try_send(Datum {
            key: self.key.clone(),
            value: Value::Gauge(GaugeValue::Increment(value)),
        });
    }

    fn decrement_gauge(&self, value: f64) {
        let _ = self.sender.try_send(Datum {
            key: self.key.clone(),
            value: Value::Gauge(GaugeValue::Decrement(value)),
        });
    }

    fn set_gauge(&self, value: f64) {
        let _ = self.sender.try_send(Datum {
            key: self.key.clone(),
            value: Value::Gauge(GaugeValue::Absolute(value)),
        });
    }

    fn record_histogram(&self, value: f64, count: usize) {
        if value.is_finite() {
            let _ = self.sender.try_send(Datum {
                key: self.key.clone(),
                value: Value::Histogram(HistogramValue::new(value).unwrap(), count),
            });
        }
    }
}

impl CounterFn for RecorderHandleKey {
    fn increment(&self, value: u64) {
        self.increment_counter(value)
    }

    fn absolute(&self, value: u64) {
        self.set_counter(value);
    }
}

impl GaugeFn for RecorderHandleKey {
    fn increment(&self, value: f64) {
        self.increment_gauge(value);
    }

    fn decrement(&self, value: f64) {
        self.decrement_gauge(value);
    }

    fn set(&self, value: f64) {
        self.set_gauge(value);
    }
}

impl HistogramFn for RecorderHandleKey {
    fn record(&self, value: f64) {
        self.record_histogram(value, 1);
    }

    fn record_many(&self, value: f64, count: usize) {
        self.record_histogram(value, count);
    }
}

impl Recorder for RecorderHandle {
    fn describe_counter(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
        RecorderHandle::describe_counter(self, key, unit, description)
    }

    fn describe_gauge(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
        RecorderHandle::describe_gauge(self, key, unit, description)
    }

    fn describe_histogram(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
        RecorderHandle::describe_histogram(self, key, unit, description)
    }

    fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> metrics::Counter {
        RecorderHandle::register_counter(self, key)
    }

    fn register_gauge(&self, key: &Key, _metadata: &Metadata<'_>) -> metrics::Gauge {
        RecorderHandle::register_gauge(self, key)
    }

    fn register_histogram(&self, key: &Key, _metadata: &Metadata<'_>) -> metrics::Histogram {
        RecorderHandle::register_histogram(self, key)
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

fn unit_cloudwatch_str(unit: &Unit) -> Option<&'static str> {
    let cloudwatch_unit = match unit {
        Unit::Seconds => crate::Unit::Seconds,
        Unit::Microseconds => crate::Unit::Microseconds,
        Unit::Milliseconds => crate::Unit::Milliseconds,
        Unit::Nanoseconds => return None,
        Unit::Bytes => crate::Unit::Bytes,
        // Close enough
        Unit::Kibibytes => crate::Unit::Kilobytes,
        Unit::Mebibytes => crate::Unit::Megabytes,
        Unit::Gibibytes => crate::Unit::Gigabytes,
        Unit::Tebibytes => crate::Unit::Terabytes,

        Unit::Percent => crate::Unit::Percent,
        Unit::Count => crate::Unit::Count,
        Unit::BitsPerSecond => crate::Unit::BitsPerSecond,
        Unit::KilobitsPerSecond => crate::Unit::KilobitsPerSecond,
        Unit::MegabitsPerSecond => crate::Unit::MegabitsPerSecond,
        Unit::GigabitsPerSecond => crate::Unit::GigabitsPerSecond,
        Unit::TerabitsPerSecond => crate::Unit::TerabitsPerSecond,
        Unit::CountPerSecond => crate::Unit::CountPerSecond,
    };
    Some(cloudwatch_unit.as_str())
}

#[cfg(test)]
mod tests {
    use metrics::Label;

    use super::*;

    use proptest::prelude::*;
    fn dim(name: &str, value: &str) -> Dimension {
        Dimension::builder().name(name).value(value).build()
    }
    #[test]
    fn time_key_should_truncate() {
        assert_eq!(time_key(370, Resolution::Second), 370);
        assert_eq!(time_key(370, Resolution::Minute), 360);
    }

    fn metrics() -> impl Strategy<Value = Vec<MetricDatum>> {
        let values = || {
            proptest::collection::vec(proptest::num::f64::ANY, 1..MAX_HISTOGRAM_VALUES)
                .prop_map(Some)
        };
        let timestamp = datetime(time::UNIX_EPOCH);
        let datum = (
            values(),
            values(),
            proptest::collection::vec(
                ("name", "value").prop_map(|(name, value)| dim(&name, &value)),
                1..6,
            )
            .prop_map(Some),
        )
            .prop_map(move |(counts, values, dimensions)| {
                MetricDatum::builder()
                    .set_counts(counts)
                    .set_values(values)
                    .set_dimensions(dimensions)
                    .metric_name("test")
                    .timestamp(timestamp)
                    .storage_resolution(1)
                    .statistic_values(StatisticSet::builder().build())
                    .unit(StandardUnit::Count)
                    .build()
            });

        proptest::collection::vec(datum, 1..100)
    }

    #[test]
    fn chunks_fit_in_cloudwatch_constraints() {
        proptest! {
            proptest::prelude::ProptestConfig { cases: 30, .. Default::default() },
            |(metrics in metrics())| {
                let mut total_chunks = 0;
                for metric_data in metrics_chunks(&metrics) {
                    assert!(metric_data.len() > 0 && metric_data.len() < MAX_CW_METRICS_PER_CALL, "Sending too many metrics per call: {}", metric_data.len());
                    let estimated_size = metric_data.iter().map(metric_size).sum::<usize>();
                    assert!(estimated_size < MAX_CW_METRICS_PUT_SIZE, "{} >= {}", estimated_size, MAX_CW_METRICS_PUT_SIZE);
                    total_chunks += metric_data.len();
                }
                assert_eq!(total_chunks, metrics.len());
            }
        }
    }

    #[test]
    fn should_override_default_dimensions() {
        let collector = Collector::new(CollectorConfig {
            default_dimensions: vec![
                ("second".to_owned(), "123".to_owned()),
                ("first".to_owned(), "initial-value".to_owned()),
            ]
            .into_iter()
            .collect(),
            storage_resolution: Resolution::Minute,
        });

        let key = Key::from_parts(
            "my-metric",
            vec![
                Label::new("zzz", "123"),
                Label::new("first", "override-value"),
                Label::new("aaa", "123"),
            ],
        );

        let actual = collector.dimensions(&key);
        let dim = |name, value| Dimension::builder().name(name).value(value).build();
        assert_eq!(
            actual,
            vec![
                dim("aaa", "123"),
                dim("first", "override-value"),
                dim("second", "123"),
                dim("zzz", "123")
            ],
        );
    }

    macro_rules! assert_between {
        ($x: expr, $min: expr, $max: expr, $(,)?) => {
            match ($x, $min, $max) {
                (x, min, max) => {
                    assert!(
                        x > min && x < max,
                        "{:?} is not in the range {:?}..{:?}",
                        x,
                        min,
                        max
                    );
                }
            }
        };
    }

    #[tokio::test]
    async fn jitter_interval_test() {
        let interval = Duration::from_millis(10);
        let start = tokio::time::Instant::now();
        let mut interval_stream = Box::pin(jitter_interval_at(start, interval));
        assert_eq!(interval_stream.next().await, Some(start));
        let x = interval_stream.next().await.unwrap();
        assert_between!(
            x,
            start.checked_add(interval / 2).unwrap(),
            start.checked_add(interval + interval / 2).unwrap(),
        );
    }

    #[test]
    fn should_handle_nan_in_record_histogram() {
        let (sender, _receiver) = mpsc::channel(1);
        let recorder = RecorderHandle {
            sender: sender.clone(),
            registry: metrics_util::registry::Registry::new(CloudwatchStorage { sender }),
        };
        recorder
            .register_histogram(&Key::from_static_name("my_metric"))
            .record(f64::NAN);
    }
}
