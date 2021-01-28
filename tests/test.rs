use std::{error::Error, time::Duration};

use futures_util::FutureExt;
use rusoto_cloudwatch::StatisticSet;

use common::MockCloudWatchClient;

mod common;

#[tokio::test]
async fn test_flush_on_shutdown() -> Result<(), Box<dyn Error>> {
    let client = MockCloudWatchClient::default();

    tokio::time::pause();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let backend_fut = Box::pin(
        metrics_cloudwatch::Builder::new_with(client.clone())
            .cloudwatch_namespace("test-ns")
            .send_interval_secs(1)
            .storage_resolution(metrics_cloudwatch::Resolution::Second)
            .shutdown_signal(Box::pin(rx.map(|_| ())))
            .init_future(),
    );
    let joinhandle = tokio::spawn(backend_fut);
    tokio::time::advance(Duration::from_millis(5)).await;

    for i in 0..150 {
        metrics::histogram!("test", i as f64);
        metrics::counter!("count", 1);
    }
    metrics::histogram!("test", 0.0);
    metrics::histogram!("test", 200.0);
    metrics::histogram!("labels", 111.0, "label" => "1", "@unknown_@_label_is_skipped" => "abc");
    metrics::histogram!("bytes", 200.0, "@unit" => metrics_cloudwatch::Unit::Bytes);
    metrics::gauge!("gauge", 100.0);
    metrics::gauge!("gauge", 200.0);
    tokio::time::advance(Duration::from_millis(5)).await;

    // Send shutdown signal
    tx.send(()).unwrap();
    joinhandle.await??;

    let actual = client.put_metric_data.lock().await;
    assert_eq!(actual.len(), 1);

    let metric_data = &actual[0].metric_data;

    assert_eq!(metric_data.len(), 6);
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

    let count_data = metric_data
        .iter()
        .find(|m| m.metric_name == "gauge")
        .unwrap();
    assert_eq!(
        count_data.statistic_values,
        Some(StatisticSet {
            sample_count: 2.0,
            sum: 200.0,
            minimum: 100.0,
            maximum: 200.0,
        })
    );

    Ok(())
}
