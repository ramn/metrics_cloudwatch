use std::{error::Error, time::Duration};

use futures::prelude::*;
use rusoto_cloudwatch::CloudWatch;

use common::MockCloudWatchClient;

mod common;

#[tokio::test]
async fn test_flush_on_shutdown() -> Result<(), Box<dyn Error>> {
    let client = MockCloudWatchClient::default();
    let client_builder = {
        let client = client.clone();
        Box::new(move |_region| Box::new(client.clone()) as Box<dyn CloudWatch + Send + Sync>)
    };

    tokio::time::pause();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let backend_fut = Box::pin(
        metrics_cloudwatch::builder()
            .cloudwatch_namespace("test-ns")
            .client_builder(client_builder)
            .send_interval_secs(1)
            .storage_resolution(metrics_cloudwatch::Resolution::Second)
            .shutdown_signal(Box::pin(rx.map(|_| ())))
            .init_future(),
    );
    let joinhandle = tokio::spawn(backend_fut);
    tokio::time::advance(Duration::from_millis(5)).await;

    for i in 0..150 {
        metrics::value!("test", i);
    }
    metrics::value!("test", 0);
    metrics::value!("test", 200);
    tokio::time::advance(Duration::from_millis(5)).await;

    // Send shutdown signal
    tx.send(()).unwrap();
    joinhandle.await??;

    let actual = client.put_metric_data.lock().await;
    assert_eq!(actual.len(), 2);

    assert_eq!(actual[0].metric_data.len(), 1);
    assert_eq!(actual[0].metric_data[0].counts.as_ref().unwrap().len(), 150);
    assert_eq!(actual[0].metric_data[0].values.as_ref().unwrap().len(), 150);

    assert_eq!(actual[1].metric_data.len(), 1);
    assert_eq!(actual[1].metric_data[0].counts.as_ref().unwrap().len(), 1);
    assert_eq!(actual[1].metric_data[0].values.as_ref().unwrap().len(), 1);
    Ok(())
}
