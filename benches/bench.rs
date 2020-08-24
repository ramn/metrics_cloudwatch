use {
    criterion::{criterion_group, Benchmark, Criterion, Throughput},
    futures::prelude::*,
    metrics::Recorder,
};

use metrics_cloudwatch::collector;

#[path = "../tests/common/mod.rs"]
mod common;

fn simple(c: &mut Criterion) {
    const NUM_ENTRIES: usize = 2 * 1024;
    c.bench(
        "send_metrics",
        Benchmark::new("full", |b| {
            let mut runtime = tokio::runtime::Builder::new()
                .basic_scheduler()
                .enable_all()
                .build()
                .unwrap();
            b.iter(|| {
                runtime.block_on(async {
                    let cloudwatch_client = common::MockCloudWatchClient::default();

                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    let (recorder, task) = collector::new(collector::Config {
                        cloudwatch_namespace: "".into(),
                        default_dimensions: Default::default(),
                        storage_resolution: collector::Resolution::Second,
                        send_interval_secs: 200,
                        client: Box::new(cloudwatch_client.clone()),
                        shutdown_signal: receiver.map(|_| ()).boxed().shared(),
                    });

                    let task = tokio::spawn(task);

                    for i in 0..NUM_ENTRIES {
                        match i % 3 {
                            0 => recorder.increment_counter("counter".into(), 1),
                            1 => recorder.update_gauge("gauge".into(), i as i64 % 100),
                            2 => recorder.record_histogram("histogram".into(), i as u64 % 10),
                            _ => unreachable!(),
                        }
                        if i % 100 == 0 {
                            // Give the emitter a chance to consume the entries we sent so that the
                            // buffer does not fill up
                            tokio::task::yield_now().await;
                        }
                    }

                    tokio::task::yield_now().await;
                    sender.send(()).unwrap();
                    task.await.unwrap();

                    let put_metric_data = cloudwatch_client.put_metric_data.lock().await;
                    assert_eq!(
                        put_metric_data
                            .iter()
                            .flat_map(|m| m.metric_data.iter())
                            .filter(|data| data.metric_name == "counter")
                            .map(|counter| counter.statistic_values.as_ref().unwrap().sample_count)
                            .sum::<f64>(),
                        (NUM_ENTRIES as f64 / 3.0).round(),
                        "{:#?}",
                        put_metric_data,
                    );
                });
            });
        })
        .throughput(Throughput::Elements(NUM_ENTRIES as u64)),
    );
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = simple
}

fn main() {
    benches()
}
