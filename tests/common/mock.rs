#![allow(unused)]
use std::sync::Arc;

use {
    async_trait::async_trait, rusoto_cloudwatch::*, rusoto_core::RusotoError, tokio::sync::Mutex,
};

#[derive(Clone, Default)]
pub struct MockCloudWatchClient {
    pub put_metric_data: Arc<Mutex<Vec<PutMetricDataInput>>>,
}

#[async_trait]
impl CloudWatch for MockCloudWatchClient {
    async fn delete_alarms(
        &self,
        input: DeleteAlarmsInput,
    ) -> Result<(), RusotoError<DeleteAlarmsError>> {
        todo!()
    }

    async fn delete_anomaly_detector(
        &self,
        input: DeleteAnomalyDetectorInput,
    ) -> Result<DeleteAnomalyDetectorOutput, RusotoError<DeleteAnomalyDetectorError>> {
        todo!()
    }

    async fn delete_dashboards(
        &self,
        input: DeleteDashboardsInput,
    ) -> Result<DeleteDashboardsOutput, RusotoError<DeleteDashboardsError>> {
        todo!()
    }

    async fn delete_insight_rules(
        &self,
        input: DeleteInsightRulesInput,
    ) -> Result<DeleteInsightRulesOutput, RusotoError<DeleteInsightRulesError>> {
        todo!()
    }

    async fn describe_alarm_history(
        &self,
        input: DescribeAlarmHistoryInput,
    ) -> Result<DescribeAlarmHistoryOutput, RusotoError<DescribeAlarmHistoryError>> {
        todo!()
    }

    async fn describe_alarms(
        &self,
        input: DescribeAlarmsInput,
    ) -> Result<DescribeAlarmsOutput, RusotoError<DescribeAlarmsError>> {
        todo!()
    }

    async fn describe_alarms_for_metric(
        &self,
        input: DescribeAlarmsForMetricInput,
    ) -> Result<DescribeAlarmsForMetricOutput, RusotoError<DescribeAlarmsForMetricError>> {
        todo!()
    }

    async fn describe_anomaly_detectors(
        &self,
        input: DescribeAnomalyDetectorsInput,
    ) -> Result<DescribeAnomalyDetectorsOutput, RusotoError<DescribeAnomalyDetectorsError>> {
        todo!()
    }

    async fn describe_insight_rules(
        &self,
        input: DescribeInsightRulesInput,
    ) -> Result<DescribeInsightRulesOutput, RusotoError<DescribeInsightRulesError>> {
        todo!()
    }

    async fn disable_alarm_actions(
        &self,
        input: DisableAlarmActionsInput,
    ) -> Result<(), RusotoError<DisableAlarmActionsError>> {
        todo!()
    }

    async fn disable_insight_rules(
        &self,
        input: DisableInsightRulesInput,
    ) -> Result<DisableInsightRulesOutput, RusotoError<DisableInsightRulesError>> {
        todo!()
    }

    async fn enable_alarm_actions(
        &self,
        input: EnableAlarmActionsInput,
    ) -> Result<(), RusotoError<EnableAlarmActionsError>> {
        todo!()
    }

    async fn enable_insight_rules(
        &self,
        input: EnableInsightRulesInput,
    ) -> Result<EnableInsightRulesOutput, RusotoError<EnableInsightRulesError>> {
        todo!()
    }

    async fn get_dashboard(
        &self,
        input: GetDashboardInput,
    ) -> Result<GetDashboardOutput, RusotoError<GetDashboardError>> {
        todo!()
    }

    async fn get_insight_rule_report(
        &self,
        input: GetInsightRuleReportInput,
    ) -> Result<GetInsightRuleReportOutput, RusotoError<GetInsightRuleReportError>> {
        todo!()
    }

    async fn get_metric_data(
        &self,
        input: GetMetricDataInput,
    ) -> Result<GetMetricDataOutput, RusotoError<GetMetricDataError>> {
        todo!()
    }

    async fn get_metric_statistics(
        &self,
        input: GetMetricStatisticsInput,
    ) -> Result<GetMetricStatisticsOutput, RusotoError<GetMetricStatisticsError>> {
        todo!()
    }

    async fn get_metric_widget_image(
        &self,
        input: GetMetricWidgetImageInput,
    ) -> Result<GetMetricWidgetImageOutput, RusotoError<GetMetricWidgetImageError>> {
        todo!()
    }

    async fn list_dashboards(
        &self,
        input: ListDashboardsInput,
    ) -> Result<ListDashboardsOutput, RusotoError<ListDashboardsError>> {
        todo!()
    }

    async fn list_metrics(
        &self,
        input: ListMetricsInput,
    ) -> Result<ListMetricsOutput, RusotoError<ListMetricsError>> {
        todo!()
    }

    async fn list_tags_for_resource(
        &self,
        input: ListTagsForResourceInput,
    ) -> Result<ListTagsForResourceOutput, RusotoError<ListTagsForResourceError>> {
        todo!()
    }

    async fn put_anomaly_detector(
        &self,
        input: PutAnomalyDetectorInput,
    ) -> Result<PutAnomalyDetectorOutput, RusotoError<PutAnomalyDetectorError>> {
        todo!()
    }

    async fn put_dashboard(
        &self,
        input: PutDashboardInput,
    ) -> Result<PutDashboardOutput, RusotoError<PutDashboardError>> {
        todo!()
    }

    async fn put_insight_rule(
        &self,
        input: PutInsightRuleInput,
    ) -> Result<PutInsightRuleOutput, RusotoError<PutInsightRuleError>> {
        todo!()
    }

    async fn put_metric_alarm(
        &self,
        input: PutMetricAlarmInput,
    ) -> Result<(), RusotoError<PutMetricAlarmError>> {
        todo!()
    }

    async fn put_metric_data(
        &self,
        input: PutMetricDataInput,
    ) -> Result<(), RusotoError<PutMetricDataError>> {
        self.put_metric_data.lock().await.push(input);
        Ok(())
    }

    async fn set_alarm_state(
        &self,
        input: SetAlarmStateInput,
    ) -> Result<(), RusotoError<SetAlarmStateError>> {
        todo!()
    }

    async fn tag_resource(
        &self,
        input: TagResourceInput,
    ) -> Result<TagResourceOutput, RusotoError<TagResourceError>> {
        todo!()
    }

    async fn untag_resource(
        &self,
        input: UntagResourceInput,
    ) -> Result<UntagResourceOutput, RusotoError<UntagResourceError>> {
        todo!()
    }

    async fn put_composite_alarm(
        &self,
        input: PutCompositeAlarmInput,
    ) -> Result<(), RusotoError<PutCompositeAlarmError>> {
        todo!()
    }
}
