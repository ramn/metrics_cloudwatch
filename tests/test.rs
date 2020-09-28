use std::{error::Error, time::Duration};

use futures::prelude::*;
use rusoto_cloudwatch::StatisticSet;

use common::MockCloudWatchClient;

mod common;

#[tokio::test]
async fn test_flush_on_shutdown() -> Result<(), Box<dyn Error>> {
    let client = MockCloudWatchClient::default();

    tokio::time::pause();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let backend_fut = Box::pin(
        metrics_cloudwatch::builder()
            .cloudwatch_namespace("test-ns")
            .client(Box::new(client.clone()))
            .send_interval_secs(1)
            .storage_resolution(metrics_cloudwatch::Resolution::Second)
            .shutdown_signal(Box::pin(rx.map(|_| ())))
            .init_future(),
    );
    let joinhandle = tokio::spawn(backend_fut);
    tokio::time::advance(Duration::from_millis(5)).await;

    for i in 0..150 {
        metrics::value!("test", i);
        metrics::counter!("count", 1);
    }
    metrics::value!("test", 0);
    metrics::value!("test", 200);
    metrics::value!("labels", 111, "label" => "1", "@unknown_@_label_is_skipped" => "abc");
    metrics::value!("bytes", 200, "@unit" => metrics_cloudwatch::Unit::Bytes);
    tokio::time::advance(Duration::from_millis(5)).await;

    // Send shutdown signal
    tx.send(()).unwrap();
    joinhandle.await??;

    let actual = client.put_metric_data.lock().await;
    assert_eq!(actual.len(), 1);

    let metric_data = &actual[0].metric_data;

    assert_eq!(metric_data.len(), 5);
    let value_metric = metric_data
        .iter()
        .find(|m| m.metric_name == "test" && m.counts.as_ref().unwrap().len() == 150)
        .unwrap();
    assert_eq!(value_metric.counts.as_ref().unwrap().len(), 150);
    assert_eq!(value_metric.values.as_ref().unwrap().len(), 150);
    assert_eq!(value_metric.dimensions, Some(vec![]));

    let dim_metric = metric_data
        .iter()
        .find(|m| m.metric_name == "labels")
        .unwrap();
    assert_eq!(
        dim_metric.dimensions,
        Some(vec![rusoto_cloudwatch::Dimension {
            name: "label".into(),
            value: "1".into()
        }])
    );

    let count_metric = metric_data
        .iter()
        .find(|m| m.metric_name == "test" && m.counts.as_ref().unwrap().len() == 1)
        .unwrap();
    assert_eq!(count_metric.counts.as_ref().unwrap().len(), 1);
    assert_eq!(count_metric.values.as_ref().unwrap().len(), 1);
    assert_eq!(value_metric.dimensions, Some(vec![]));

    let count_data = metric_data
        .iter()
        .find(|m| m.metric_name == "count")
        .unwrap();
    assert_eq!(
        count_data.statistic_values,
        Some(StatisticSet {
            sample_count: 1.0,
            sum: 150.0,
            maximum: 150.0,
            minimum: 150.0
        })
    );

    let bytes_data = metric_data
        .iter()
        .find(|m| m.metric_name == "bytes")
        .unwrap();
    assert_eq!(bytes_data.unit, Some("Bytes".to_string()));

    Ok(())
}
