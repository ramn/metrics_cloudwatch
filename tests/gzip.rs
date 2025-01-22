#![cfg(feature = "integration-test")]

use anyhow::Result;
use futures_util::FutureExt;

#[tokio::test]
async fn test_gzip() -> Result<()> {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;

    let client = aws_sdk_cloudwatch::Client::new(&sdk_config);

    let (shutdown_sender, receiver) = tokio::sync::oneshot::channel::<()>();

    let driver = metrics_cloudwatch::Builder::new()
        .shutdown_signal(receiver.map(|_| ()).boxed())
        .cloudwatch_namespace("metrics-cloudwatch-test")
        .init_async(client, metrics::set_global_recorder)
        .await?;
    // Initialize the backend
    let metrics_task = tokio::spawn(driver);

    for _ in 0..1000 {
        metrics::counter!("requests").increment(1);
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    drop(shutdown_sender);

    metrics_task.await?;

    Ok(())
}
