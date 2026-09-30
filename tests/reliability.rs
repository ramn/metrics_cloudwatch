//! Tests that metrics are not silently lost under bursts of updates or transient send failures.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use aws_sdk_cloudwatch::{
    error::SdkError,
    operation::put_metric_data::{PutMetricDataError, PutMetricDataInput},
    types::MetricDatum,
};
use aws_smithy_runtime_api::http::{Response, StatusCode};
use aws_smithy_types::{body::SdkBody, error::ErrorMetadata};
use futures_util::{FutureExt, future};
use metrics_cloudwatch::{
    BoxFuture, Resolution,
    collector::{self, CloudWatch, Config},
};

type PutResult = Result<(), SdkError<PutMetricDataError>>;

/// What the mock should do on a call
enum Behavior {
    Hang,
    Fail(Box<dyn Fn() -> SdkError<PutMetricDataError> + Send>),
}

/// Mock client that plays back a script of failures before succeeding
#[derive(Clone, Default)]
struct ScriptedClient {
    script: Arc<Mutex<VecDeque<Behavior>>>,
    calls: Arc<Mutex<usize>>,
    sent: Arc<Mutex<Vec<PutMetricDataInput>>>,
}

impl ScriptedClient {
    fn with_script(script: impl IntoIterator<Item = Behavior>) -> Self {
        Self {
            script: Arc::new(Mutex::new(script.into_iter().collect())),
            ..Default::default()
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().unwrap()
    }

    fn sent_datums(&self) -> Vec<MetricDatum> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .flat_map(|input| input.metric_data.clone().unwrap_or_default())
            .collect()
    }
}

impl CloudWatch for ScriptedClient {
    fn put_metric_data(&self, config: &Config, data: Vec<MetricDatum>) -> BoxFuture<'_, PutResult> {
        *self.calls.lock().unwrap() += 1;
        match self.script.lock().unwrap().pop_front() {
            Some(Behavior::Hang) => future::pending().boxed(),
            Some(Behavior::Fail(err)) => {
                let err = err();
                async move { Err(err) }.boxed()
            }
            None => {
                let input = PutMetricDataInput::builder()
                    .namespace(config.cloudwatch_namespace.clone())
                    .set_metric_data(Some(data))
                    .build()
                    .unwrap();
                self.sent.lock().unwrap().push(input);
                async { Ok(()) }.boxed()
            }
        }
    }
}

fn config(
    metric_buffer_size: usize,
    send_retries: u32,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Config {
    Config {
        cloudwatch_namespace: "test-ns".into(),
        default_dimensions: Default::default(),
        storage_resolution: Resolution::Minute,
        send_interval_secs: 1,
        send_timeout_secs: 4,
        send_retries,
        shutdown_signal: shutdown.boxed().shared(),
        metric_buffer_size,
    }
}

fn service_error(status: u16, code: &str) -> SdkError<PutMetricDataError> {
    let raw = Response::new(StatusCode::try_from(status).unwrap(), SdkBody::from(""));
    SdkError::service_error(
        PutMetricDataError::generic(ErrorMetadata::builder().code(code).build()),
        raw,
    )
}

fn counter_sum(datums: &[MetricDatum], name: &str) -> f64 {
    datums
        .iter()
        .filter(|d| d.metric_name() == Some(name))
        .map(|d| d.statistic_values().unwrap().sum().unwrap())
        .sum()
}

/// Records `updates` counter increments in one burst, without giving the collector a chance to
/// run in between, then shuts down and returns everything that was sent.
async fn record_burst(metric_buffer_size: usize, updates: u64) -> Vec<MetricDatum> {
    let client = ScriptedClient::default();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let (recorder, driver) = collector::new(
        client.clone(),
        config(metric_buffer_size, 0, rx.map(|_| ())),
        None,
    );
    let driver = tokio::spawn(driver);

    metrics::with_local_recorder(&recorder, || {
        for _ in 0..updates {
            metrics::counter!("burst").increment(1);
        }
    });

    tx.send(()).unwrap();
    driver.await.unwrap();
    client.sent_datums()
}

#[tokio::test]
async fn burst_within_buffer_size_is_not_dropped() {
    let datums = record_burst(50_000, 50_000).await;
    assert_eq!(counter_sum(&datums, "burst"), 50_000.0);
}

#[tokio::test]
async fn burst_larger_than_buffer_size_is_dropped() {
    // The test runtime is single threaded, so the collector cannot drain the buffer while the
    // burst is being recorded: exactly `metric_buffer_size` updates make it through.
    let datums = record_burst(100, 50_000).await;
    assert_eq!(counter_sum(&datums, "burst"), 100.0);
}

async fn send_one_counter(client: ScriptedClient, send_retries: u32) {
    tokio::time::pause();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let (recorder, driver) = collector::new(
        client.clone(),
        config(1024, send_retries, rx.map(|_| ())),
        None,
    );
    let driver = tokio::spawn(driver);

    metrics::with_local_recorder(&recorder, || metrics::counter!("count").increment(7));

    tx.send(()).unwrap();
    // Time is paused, so timeouts and backoff sleeps complete instantly
    tokio::time::timeout(Duration::from_secs(3600), driver)
        .await
        .expect("driver should finish")
        .unwrap();
}

#[tokio::test]
async fn retries_after_send_timeout() {
    let client = ScriptedClient::with_script([Behavior::Hang, Behavior::Hang]);
    send_one_counter(client.clone(), 3).await;
    assert_eq!(client.calls(), 3);
    assert_eq!(counter_sum(&client.sent_datums(), "count"), 7.0);
}

#[tokio::test]
async fn retries_on_transient_errors() {
    let client = ScriptedClient::with_script([
        Behavior::Fail(Box::new(|| SdkError::timeout_error("timed out"))),
        Behavior::Fail(Box::new(|| service_error(400, "Throttling"))),
        Behavior::Fail(Box::new(|| service_error(500, "InternalServiceFault"))),
    ]);
    send_one_counter(client.clone(), 3).await;
    assert_eq!(client.calls(), 4);
    assert_eq!(counter_sum(&client.sent_datums(), "count"), 7.0);
}

#[tokio::test]
async fn gives_up_after_configured_retries() {
    let client = ScriptedClient::with_script([Behavior::Hang, Behavior::Hang, Behavior::Hang]);
    send_one_counter(client.clone(), 2).await;
    assert_eq!(client.calls(), 3);
    assert!(client.sent_datums().is_empty());
}

#[tokio::test]
async fn does_not_retry_client_errors() {
    let client = ScriptedClient::with_script([Behavior::Fail(Box::new(|| {
        service_error(400, "InvalidParameterValue")
    }))]);
    send_one_counter(client.clone(), 3).await;
    assert_eq!(client.calls(), 1);
    assert!(client.sent_datums().is_empty());
}

#[tokio::test]
async fn driver_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let (_recorder, driver) = collector::new(
        ScriptedClient::default(),
        config(1, 0, future::pending()),
        None,
    );
    assert_send(&driver);
}
