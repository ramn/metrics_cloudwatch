use std::{collections::BTreeMap, fmt};

use crate::{
    collector::{self, ClientBuilder, Config, Resolution},
    error::Error,
};

use rusoto_cloudwatch::CloudWatchClient;
pub use {rusoto_cloudwatch::CloudWatch, rusoto_core::Region};

#[derive(Default)]
pub struct Builder {
    cloudwatch_namespace: Option<String>,
    default_dimensions: BTreeMap<String, String>,
    storage_resolution: Option<Resolution>,
    send_interval_secs: Option<u64>,
    region: Option<Region>,
    client_builder: Option<ClientBuilder>,
}

pub fn builder() -> Builder {
    Builder::default()
}

fn default_client_builder() -> ClientBuilder {
    Box::new(|region| Box::new(CloudWatchClient::new(region)))
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

    /// Sets the AWS Region to send metrics to
    pub fn region(self, region: Region) -> Self {
        Self {
            region: Some(region),
            ..self
        }
    }

    pub fn client_builder(self, client_builder: ClientBuilder) -> Self {
        Self {
            client_builder: Some(client_builder),
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
            region: self.region.unwrap_or(Region::UsEast1),
            client_builder: self.client_builder.unwrap_or_else(default_client_builder),
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
            region,
            client_builder: _,
        } = self;
        f.debug_struct("Builder")
            .field("cloudwatch_namespace", cloudwatch_namespace)
            .field("default_dimensions", default_dimensions)
            .field("storage_resolution", storage_resolution)
            .field("send_interval_secs", send_interval_secs)
            .field("region", region)
            .field("client_builder", &"<Fn(Region) -> Box<dyn CloudWatch>")
            .finish()
    }
}
