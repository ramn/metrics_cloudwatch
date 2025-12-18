use std::future::Future;

use anyhow::Result;
use aws_sdk_cloudwatch::{config::http::HttpRequest, error::ConnectorError};
use aws_smithy_runtime_api::{
    client::http::{http_client_fn, HttpConnector, HttpConnectorFuture, SharedHttpConnector},
    http::{Response, StatusCode},
};
use aws_smithy_types::body::SdkBody;
use futures_util::FutureExt;
use http_body::Body;

#[tokio::test]
#[cfg(feature = "integration-test")]
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

struct MockHttpConnector<F>(F);

impl<F> std::fmt::Debug for MockHttpConnector<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockHttpConnector").finish()
    }
}

impl<F, G> HttpConnector for MockHttpConnector<F>
where
    F: Fn(HttpRequest) -> G + Send + Sync + 'static,
    G: Future<Output = Result<Response, ConnectorError>> + Send + 'static,
{
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        HttpConnectorFuture::new((self.0)(request))
    }
}

#[tokio::test]
async fn test_gzip_mock() -> Result<()> {
    let _ = env_logger::try_init();

    tokio::time::pause();

    let http_client = http_client_fn(|_settings, _runtime_components| {
        SharedHttpConnector::new(MockHttpConnector(|mut request: HttpRequest| async move {
            #[cfg(feature = "gzip")]
            {
                use std::io::Read;
                let body = request.body_mut().collect().await.unwrap().to_bytes();

                assert_eq!(
                    request.headers().get("Content-Encoding"),
                    Some("gzip"),
                    "{:#?} {:?}",
                    request.headers(),
                    str::from_utf8(&body)
                );

                flate2::read::GzDecoder::new(&body[..])
                    .read_to_string(&mut String::new())
                    .unwrap();
            }

            #[cfg(not(feature = "gzip"))]
            {
                assert_eq!(request.headers().get("Content-Encoding"), None);
                let body = request.into_body().collect().await.unwrap().to_bytes();
                str::from_utf8(&body).unwrap();
            }

            // Here you can inspect the request to verify gzip encoding
            Ok(Response::new(
                StatusCode::try_from(200).unwrap(),
                SdkBody::empty(),
            ))
        }))
    });
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .http_client(http_client)
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
