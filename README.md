metrics_cloudwatch
==================
[![crates.io](https://meritbadge.herokuapp.com/metrics_cloudwatch)](https://crates.io/crates/metrics_cloudwatch)
[![Docs](https://docs.rs/metrics_cloudwatch/badge.svg)](https://docs.rs/metrics_cloudwatch)
![Build status](https://github.com/ramn/metrics_cloudwatch/workflows/build/badge.svg)

Purpose
-------

Provide a backend for the [`metrics` facade
crate](https://crates.io/crates/metrics), pushing metrics to CloudWatch.


How to use
----------

Credentials for AWS needs to be available in the environment, see [AWS docs
on setting up AWS credentials](
https://github.com/awslabs/aws-sdk-rust#getting-started-with-the-sdk)

```bash
cargo add -s metrics metrics_cloudwatch
```

```rust
#[tokio::main]
async fn main() {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let client = aws_sdk_cloudwatch::Client::new(&sdk_config);
    // Initialize the backend
    tokio::spawn(metrics_cloudwatch::Builder::new()
        .cloudwatch_namespace("my-namespace")
        .init_future(client, metrics::set_global_recorder));

    metrics::counter!("requests").increment(1);
}
```

Any labels specified will map to Cloudwatch dimensions

```rust
metrics::histogram!("histogram", "dimension_name" => "dimension_value").record(100.0);
```

Specifying the empty string for the value will remove the default dimension of the same name from the metric.

```rust
metrics::histogram!("histogram", "dimension_name" => "").record(100.0);
```

The special `@unit` label accepts a `metrics_cloudwatch::Unit` which specifies the unit for the metric (the unit can also be specified when registering the metric). Other `@` prefixed labels are ignored.

```rust
metrics::histogram!("histogram", "@unit" => metrics_cloudwatch::Unit::Seconds).record(100.0);
```

Limitations
-----------

The CloudWatch metrics API imposes some limitations.

* Max 30 labels (dimensions) per metric
* Max 150 unique histogram values (used by `timing!()` and `value!()`) per API
call. Going beyond this works but will incur one API call per batch of 150
unique values. Could be a good idea to measure timing in milliseconds rather
than nanoseconds, to keep down the number of unique values.
