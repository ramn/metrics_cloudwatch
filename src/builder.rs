use std::{borrow::Cow, collections::BTreeMap, fmt, pin::Pin};

use aws_sdk_cloudwatch::config::Region;
use futures_util::{future, FutureExt, Stream};

use crate::mock::MockCloudWatchClient;
use crate::{
    collector::{self, Config, Resolution},
    error::Error,
    BoxFuture, GzipSetting,
};

pub struct Builder {
    cloudwatch_namespace: Option<String>,
    default_dimensions: BTreeMap<String, String>,
    storage_resolution: Option<Resolution>,
    send_interval_secs: Option<u64>,
    shutdown_signal: Option<BoxFuture<'static, ()>>,
    metric_buffer_size: usize,
    force_flush_stream: Option<Pin<Box<dyn Stream<Item = ()> + Send>>>,
    region: Option<Region>,
    gzip: GzipSetting,
    mock: Option<MockCloudWatchClient>,
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
    pub fn new() -> Self {
        Builder {
            cloudwatch_namespace: Default::default(),
            default_dimensions: Default::default(),
            storage_resolution: Default::default(),
            send_interval_secs: Default::default(),
            shutdown_signal: Default::default(),
            metric_buffer_size: 2048,
            force_flush_stream: Default::default(),
            region: None,
            gzip: GzipSetting::Off,
            mock: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_mock(mock: MockCloudWatchClient) -> Self {
        Builder {
            mock: Some(mock),
            ..Builder::new()
        }
    }

    /// Each time an item is produced on this stream, metrics will be force-flushed to CloudWatch.
    /// Meaning, it will not respect the storage resolution aggregation but will send all currently
    /// held metric data in the same way as shutdown_signal will.
    pub fn force_flush_stream(
        mut self,
        force_flush_stream: Pin<Box<dyn Stream<Item = ()> + Send>>,
    ) -> Self {
        self.force_flush_stream = Some(force_flush_stream);
        self
    }

    /// Sets the CloudWatch namespace for all metrics sent by this backend.
    pub fn cloudwatch_namespace(self, namespace: impl Into<String>) -> Self {
        Self {
            cloudwatch_namespace: Some(namespace.into()),
            ..self
        }
    }

    /// Sets the Cloudwatch region for all metrics sent by this backend.
    pub fn region(self, region: impl Into<Cow<'static, str>>) -> Self {
        Self {
            region: Some(Region::new(region)),
            ..self
        }
    }

    /// Sets the GZIP metrics compression settings.
    #[cfg(feature = "gzip")]
    pub fn gzip(self, gzip: GzipSetting) -> Self {
        Self { gzip, ..self }
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

    /// Sets a future that acts as a shutdown signal when it completes.
    /// Completion of this future will trigger a final flush of metrics to CloudWatch.
    pub fn shutdown_signal(self, shutdown_signal: BoxFuture<'static, ()>) -> Self {
        Self {
            shutdown_signal: Some(shutdown_signal),
            ..self
        }
    }

    /// How many metrics that may be buffered before they are dropped.
    ///
    /// Metrics are buffered in a channel that is consumed by the metrics task.
    /// If there are more metrics produced than the buffer size any excess metrics will be dropped.
    ///
    /// Default: 2048
    pub fn metric_buffer_size(self, metric_buffer_size: usize) -> Self {
        Self {
            metric_buffer_size,
            ..self
        }
    }

    /// Initializes the CloudWatch metrics backend and runs it in a new thread.
    ///
    /// Expects the `metrics::set_boxed_recorder` function as an argument as a safeguard against
    /// accidentally using a different `metrics` version than is used in this crate.
    pub fn init_thread(
        self,
        set_boxed_recorder: fn(Box<dyn metrics::Recorder>) -> Result<(), metrics::SetRecorderError>,
    ) -> Result<(), Error> {
        let config = self.build_config()?;
        collector::init(set_boxed_recorder, config);
        Ok(())
    }

    /// Initializes the CloudWatch metrics and returns a Future that must be polled
    ///
    /// Expects the `metrics::set_boxed_recorder` function as an argument as a safeguard against
    /// accidentally using a different `metrics` version than is used in this crate.
    pub async fn init_future(
        self,
        set_boxed_recorder: fn(Box<dyn metrics::Recorder>) -> Result<(), metrics::SetRecorderError>,
    ) -> Result<(), Error> {
        let config = self.build_config()?;
        collector::init_future(set_boxed_recorder, config).await
    }

    fn build_config(self) -> Result<Config, Error> {
        let config = Config {
            cloudwatch_namespace: extract_namespace(self.cloudwatch_namespace)?,
            default_dimensions: self.default_dimensions,
            storage_resolution: self.storage_resolution.unwrap_or(Resolution::Minute),
            send_interval_secs: self.send_interval_secs.unwrap_or(10),
            region: self.region,
            shutdown_signal: self
                .shutdown_signal
                .unwrap_or_else(|| Box::pin(future::pending()))
                .shared(),
            metric_buffer_size: self.metric_buffer_size,
            force_flush_stream: self.force_flush_stream,
            mock: self.mock,
            gzip: self.gzip,
        };

        Ok(config)
    }
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            cloudwatch_namespace,
            default_dimensions,
            storage_resolution,
            send_interval_secs,
            shutdown_signal: _,
            metric_buffer_size,
            force_flush_stream: _,
            region,
            gzip,
            mock: _,
        } = self;
        f.debug_struct("Builder")
            .field("cloudwatch_namespace", cloudwatch_namespace)
            .field("default_dimensions", default_dimensions)
            .field("storage_resolution", storage_resolution)
            .field("send_interval_secs", send_interval_secs)
            .field("shutdown_signal", &"BoxFuture")
            .field("metric_buffer_size", metric_buffer_size)
            .field("force_flush_stream", &"dyn Stream")
            .field("gzip", gzip)
            .field("region", region)
            .finish()
    }
}
