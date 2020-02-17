use std::collections::BTreeMap;

use futures::{future, prelude::*};

use crate::{
    collector::{self, Config, Resolution},
    error::Error,
};

pub use rusoto_core::Region;

#[derive(Debug, Default)]
pub struct Builder {
    cloudwatch_namespace: Option<String>,
    default_dimensions: BTreeMap<String, String>,
    storage_resolution: Option<Resolution>,
    send_interval_secs: Option<u64>,
    region: Option<Region>,
}

pub fn builder() -> Builder {
    Builder::default()
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

    /// Initializes the CloudWatch metrics backend and runs it in a new thread
    pub fn init_thread(self) -> Result<(), Error> {
        collector::init(self.build_config()?);
        Ok(())
    }

    /// Initializes the CloudWatch metrics and returns a Future that must be polled
    pub fn init_future(self) -> impl Future<Item = (), Error = Error> {
        future::result(self.build_config()).and_then(collector::init_future)
    }

    fn build_config(self) -> Result<Config, Error> {
        Ok(Config {
            cloudwatch_namespace: extract_namespace(self.cloudwatch_namespace)?,
            default_dimensions: self.default_dimensions,
            storage_resolution: self.storage_resolution.unwrap_or(Resolution::Minute),
            send_interval_secs: self.send_interval_secs.unwrap_or(10),
            region: self.region.unwrap_or(Region::UsEast1),
        })
    }
}

fn extract_namespace(cloudwatch_namespace: Option<String>) -> Result<String, Error> {
    match cloudwatch_namespace {
        Some(namespace) if !namespace.is_empty() => Ok(namespace),
        _ => Err(Error::BuilderIncomplete(
            "cloudwatch_namespace missing".into(),
        )),
    }
}
