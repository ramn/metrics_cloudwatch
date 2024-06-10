use {
    criterion::{criterion_group, Criterion, Throughput},
    futures_util::FutureExt,
    metrics::Key,
};

use metrics_cloudwatch::collector;

#[path = "../tests/common/mod.rs"]
mod common;

fn simple(c: &mut Criterion) {
    const NUM_ENTRIES: usize = 2 * 1024;
    let mut group = c.benchmark_group("send_metrics");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    group
        .bench_function("full", |b| {
            b.to_async(&runtime).iter(|| async {
                let cloudwatch_client = common::MockCloudWatchClient::default();

                let (shutdown_sender, receiver) = tokio::sync::oneshot::channel();
                let (recorder, task) = collector::new(
                    cloudwatch_client.clone(),
                    collector::Config {
                        cloudwatch_namespace: "".into(),
                        default_dimensions: Default::default(),
                        storage_resolution: collector::Resolution::Second,
                        send_interval_secs: 200,
                        shutdown_signal: receiver.map(|_| ()).boxed().shared(),
                        metric_buffer_size: 1024,
                        force_flush_stream: Some(Box::pin(futures_util::stream::empty())),
                    },
                );

                let task = tokio::spawn(task);

                for i in 0..NUM_ENTRIES {
                    match i % 3 {
                        0 => {
                            let key = Key::from("counter");
                            recorder.register_counter(&key).increment(1);
                        }
                        1 => {
                            let key = Key::from("gauge");

                            recorder.register_gauge(&key).set((i as i64 % 100) as f64);
                        }
                        2 => {
                            let key = Key::from("histogram");

                            recorder
                                .register_histogram(&key)
                                .record((i as u64 % 10) as f64);
                        }
                        _ => unreachable!(),
                    }
                    if i % 100 == 0 {
                        // Give the emitter a chance to consume the entries we sent so that the
                        // buffer does not fill up
                        tokio::task::yield_now().await;
                    }
                }

                tokio::task::yield_now().await;
                shutdown_sender.send(()).unwrap();
                task.await.unwrap();

                let put_metric_data = cloudwatch_client.put_metric_data.lock().unwrap();
                assert_eq!(
                    put_metric_data
                        .iter()
                        .flat_map(|m| m.metric_data.as_ref().unwrap().iter())
                        .filter(|data| data.metric_name() == Some("counter"))
                        .map(|counter| counter.statistic_values().as_ref().unwrap().sum().unwrap())
                        .sum::<f64>(),
                    (NUM_ENTRIES as f64 / 3.0).round(),
                    "{:#?}",
                    put_metric_data,
                );
            });
        })
        .throughput(Throughput::Elements(NUM_ENTRIES as u64));

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = simple
}

fn main() {
    benches()
}
