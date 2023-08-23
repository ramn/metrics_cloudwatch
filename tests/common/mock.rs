#![allow(unused)]
use std::{future::Future, pin::Pin, sync::Arc, sync::Mutex};

use aws_sdk_cloudwatch::{
    operation::put_metric_data::{PutMetricDataError, PutMetricDataInput},
    types::MetricDatum,
};
use aws_smithy_http::body::SdkBody;
use aws_smithy_http::result::SdkError;

use futures_util::FutureExt;
use metrics_cloudwatch::collector::CloudWatch;

#[derive(Clone, Default)]
pub struct MockCloudWatchClient {
    pub put_metric_data: Arc<Mutex<Vec<PutMetricDataInput>>>,
}

impl CloudWatch for MockCloudWatchClient {
    fn put_metric_data(
        &self,
        namespace: String,
        data: Vec<MetricDatum>,
    ) -> metrics_cloudwatch::BoxFuture<
        '_,
        Result<(), SdkError<PutMetricDataError, http::Response<SdkBody>>>,
    > {
        let data = PutMetricDataInput::builder()
            .namespace(namespace)
            .set_metric_data(Some(data))
            .build()
            .unwrap();
        let mut m = self.put_metric_data.lock().unwrap();
        m.push(data);
        async { Ok(()) }.boxed()
    }
}
