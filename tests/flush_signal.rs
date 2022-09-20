use std::time::Duration;

use futures_util::FutureExt;

use common::MockCloudWatchClient;

mod common;

#[tokio::test]
async fn test_manual_flush() {
    let _ = env_logger::try_init();

    let client = MockCloudWatchClient::default();

    tokio::time::pause();
    let (tx, rx) = tokio::sync::oneshot::channel();

    let (metrics_force_flush_sender, metrics_force_flush_receiver) =
        tokio::sync::mpsc::channel::<Option<tokio::sync::oneshot::Sender<()>>>(1);
    let metrics_force_flush_receiver =
        tokio_stream::wrappers::ReceiverStream::from(metrics_force_flush_receiver);

    let backend_fut = Box::pin(
        metrics_cloudwatch::Builder::new_with(client.clone())
            .cloudwatch_namespace("test-ns")
            .default_dimension("dimension", "default")
            .send_interval_secs(100)
            .storage_resolution(metrics_cloudwatch::Resolution::Second)
            .shutdown_signal(Box::pin(rx.map(|_| ())))
            .force_flush_stream(Box::pin(metrics_force_flush_receiver))
            .init_future(metrics::set_boxed_recorder),
    );

    let join_handle = tokio::spawn(backend_fut);
    tokio::time::advance(Duration::from_millis(5)).await;

    for i in 0..150 {
        metrics::histogram!("test", i as f64);
        metrics::counter!("count", 1);
        metrics::gauge!("gauge", i as f64);
    }

    {
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Send flush with flush signal:
        metrics_force_flush_sender.send(Some(tx)).await.unwrap();

        // Wait for a flush signal:
        match rx.await {
            Ok(()) => {},
            _ => panic!("Expected a flush signal"),
        }
        
    }

    tx.send(()).unwrap();
    join_handle.await.unwrap().unwrap();
}
