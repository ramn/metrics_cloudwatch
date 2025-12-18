#![cfg(feature = "integration-test")]
use std::future::Future;

use anyhow::Result;
use aws_credential_types::Credentials;
use aws_sdk_cloudwatch::{config::http::HttpRequest, error::ConnectorError};
use aws_smithy_runtime_api::{
    client::{
        http::{HttpConnector, HttpConnectorFuture, SharedHttpConnector, http_client_fn},
        identity::http::Token,
    },
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
#[cfg(feature = "integration-test")]
async fn test_gzip_mock() -> Result<()> {
    let _ = env_logger::try_init();

    let request_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let http_client = {
        let request_count = request_count.clone();
        http_client_fn(move |_settings, _runtime_components| {
            let request_count = request_count.clone();
            SharedHttpConnector::new(MockHttpConnector(move |mut request: HttpRequest| {
                let request_count = request_count.clone();
                async move {
                    log::trace!("Received request: {}", request.uri());

                    request_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
                }
            }))
        })
    };

    // An attempt to make the AWS SDK not try to fetch real credentials on /api/token.
    // Didn't get it to work but if you have valid credentials then this test will work and never
    // send any request to AWS (thanks to the mocked http client).
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .credentials_provider(Credentials::new("", "", Some("".into()), None, ""))
        .test_credentials()
        .token_provider(Token::new("test", None))
        .http_client(http_client)
        .endpoint_url("http://localhost")
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

    assert_eq!(
        request_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Expected exactly one request to CloudWatch"
    );

    Ok(())
}
