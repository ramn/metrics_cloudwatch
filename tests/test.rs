use std::{error::Error, time::Duration};

use aws_sdk_cloudwatch::types::{Dimension, StandardUnit, StatisticSet};
use futures_util::FutureExt;

use common::MockCloudWatchClient;

mod common;

fn dim(name: &str, value: &str) -> Dimension {
    Dimension::builder().name(name).value(value).build()
}

#[tokio::test]
async fn test_flush_on_shutdown() -> Result<(), Box<dyn Error>> {
    let client = MockCloudWatchClient::default();

    tokio::time::pause();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let backend_fut = Box::pin(
        metrics_cloudwatch::Builder::new_with_client(client.clone())
            .cloudwatch_namespace("test-ns")
            .default_dimension("dimension", "default")
            .send_interval_secs(1)
            .storage_resolution(metrics_cloudwatch::Resolution::Second)
            .shutdown_signal(Box::pin(rx.map(|_| ())))
            .init_future(metrics::set_boxed_recorder),
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
    metrics::histogram!("labels2", 111.0, "dimension" => "", "label" => "1");
    metrics::histogram!("bytes", 200.0, "@unit" => metrics_cloudwatch::Unit::Bytes);
    metrics::gauge!("gauge", 100.0);
    metrics::gauge!("gauge", 200.0);

    tokio::time::advance(Duration::from_millis(5)).await;

    // Send shutdown signal
    tx.send(()).unwrap();
    joinhandle.await??;

    let actual = client.put_metric_data.lock().unwrap();
    assert_eq!(actual.len(), 1);

    let metric_data = actual[0].metric_data.as_ref().unwrap();

    assert_eq!(metric_data.len(), 7);
    let value_metric = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("test") && m.counts.as_ref().unwrap().len() == 150)
        .unwrap();
    assert_eq!(value_metric.counts.as_ref().unwrap().len(), 150);
    assert_eq!(value_metric.values.as_ref().unwrap().len(), 150);
    assert_eq!(
        value_metric.dimensions,
        Some(vec![dim("dimension", "default")])
    );

    let dim_metric = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("labels"))
        .unwrap();
    assert_eq!(
        dim_metric.dimensions,
        Some(vec![dim("dimension", "default"), dim("label", "1")])
    );

    let dim_metric2 = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("labels2"))
        .unwrap();
    assert_eq!(dim_metric2.dimensions, Some(vec![dim("label", "1")]));

    let count_metric = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("test") && m.counts.as_ref().unwrap().len() == 1)
        .unwrap();
    assert_eq!(count_metric.counts.as_ref().unwrap().len(), 1);
    assert_eq!(count_metric.values.as_ref().unwrap().len(), 1);
    assert_eq!(
        value_metric.dimensions,
        Some(vec![dim("dimension", "default")])
    );

    let count_data = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("count"))
        .unwrap();

    assert_eq!(
        count_data.statistic_values,
        Some(
            StatisticSet::builder()
                .sample_count(1.0)
                .sum(150.0)
                .maximum(150.0)
                .minimum(150.0)
                .build()
        )
    );

    let bytes_data = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("bytes"))
        .unwrap();
    assert_eq!(bytes_data.unit(), Some(&StandardUnit::Bytes));

    let count_data = metric_data
        .iter()
        .find(|m| m.metric_name() == Some("gauge"))
        .unwrap();

    assert_eq!(
        count_data.statistic_values,
        Some(
            StatisticSet::builder()
                .sample_count(1.0)
                .sum(200.0)
                .maximum(200.0)
                .minimum(100.0)
                .build()
        )
    );

    Ok(())
}
