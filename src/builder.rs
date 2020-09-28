use std::{collections::BTreeMap, fmt};

use futures::{future, prelude::*};
use rusoto_cloudwatch::CloudWatch;

use crate::{
    collector::{self, Config, Resolution},
    error::Error,
    BoxFuture,
};

pub struct Builder {
    cloudwatch_namespace: Option<String>,
    default_dimensions: BTreeMap<String, String>,
    storage_resolution: Option<Resolution>,
    send_interval_secs: Option<u64>,
    client: Box<dyn CloudWatch + Send + Sync>,
    shutdown_signal: Option<BoxFuture<'static, ()>>,
}

pub fn builder(region: rusoto_core::Region) -> Builder {
    Builder::new(region)
}

fn extract_namespace(cloudwatch_namespace: Option<String>) -> Result<String, Error> {
    match cloudwatch_namespace {
        Some(namespace) if !namespace.is_empty() => Ok(namespace),
        _ => Err(Error::BuilderIncomplete(
            "cloudwatch_namespace missing".into(),
        )),
    }
}

impl Builder {
    pub fn new(region: rusoto_core::Region) -> Self {
        Self::new_with(rusoto_cloudwatch::CloudWatchClient::new(region))
    }

    pub fn new_with(client: impl CloudWatch + Send + Sync + 'static) -> Self {
        Builder {
            cloudwatch_namespace: Default::default(),
            default_dimensions: Default::default(),
            storage_resolution: Default::default(),
            send_interval_secs: Default::default(),
            client: Box::new(client),
            shutdown_signal: Default::default(),
        }
    }

    /// Sets the CloudWatch namespace for all metrics sent by this backend.
    pub fn cloudwatch_namespace(self, namespace: impl Into<String>) -> Self {
        Self {
            cloudwatch_namespace: Some(namespace.into()),
            ..self
        }
    }

    /// Adds a default dimension (name, value), that will be sent with each MetricDatum.
    /// This method can be called multiple times.
    pub fn default_dimension(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_dimensions.insert(name.into(), value.into());
        self
    }

    /// Sets storage resolution
    pub fn storage_resolution(self, resolution: Resolution) -> Self {
        Self {
            storage_resolution: Some(resolution),
            ..self
        }
    }

    /// Sets interval (seconds) at which batches of metrics will be sent to CloudWatch
    pub fn send_interval_secs(self, secs: u64) -> Self {
        Self {
            send_interval_secs: Some(secs),
            ..self
        }
    }

    /// Sets the cloudwatch client to be used to send metrics.
    pub fn client(self, client: Box<dyn CloudWatch + Send + Sync>) -> Self {
        Self { client, ..self }
    }

    /// Sets a future that acts as a shutdown signal when it completes.
    /// Completion of this future will trigger a final flush of metrics to CloudWatch.
    pub fn shutdown_signal(self, shutdown_signal: BoxFuture<'static, ()>) -> Self {
        Self {
            shutdown_signal: Some(shutdown_signal),
            ..self
        }
    }

    /// Initializes the CloudWatch metrics backend and runs it in a new thread
    pub fn init_thread(self) -> Result<(), Error> {
        collector::init(self.build_config()?);
        Ok(())
    }

    /// Initializes the CloudWatch metrics and returns a Future that must be polled
    pub async fn init_future(self) -> Result<(), Error> {
        collector::init_future(self.build_config()?).await
    }

    fn build_config(self) -> Result<Config, Error> {
        Ok(Config {
            cloudwatch_namespace: extract_namespace(self.cloudwatch_namespace)?,
            default_dimensions: self.default_dimensions,
            storage_resolution: self.storage_resolution.unwrap_or(Resolution::Minute),
            send_interval_secs: self.send_interval_secs.unwrap_or(10),
            client: self.client,
            shutdown_signal: self
                .shutdown_signal
                .unwrap_or_else(|| Box::pin(future::pending()))
                .shared(),
        })
    }
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            cloudwatch_namespace,
            default_dimensions,
            storage_resolution,
            send_interval_secs,
            client: _,
            shutdown_signal: _,
        } = self;
        f.debug_struct("Builder")
            .field("cloudwatch_namespace", cloudwatch_namespace)
            .field("default_dimensions", default_dimensions)
            .field("storage_resolution", storage_resolution)
            .field("send_interval_secs", send_interval_secs)
            .field("client", &"Box<dyn CloudWatch>")
            .field("shutdown_signal", &"BoxFuture")
            .finish()
    }
}
