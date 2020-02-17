metrics_cloudwatch
==================
[![crates.io](http://meritbadge.herokuapp.com/metrics_cloudwatch)](https://crates.io/crates/metrics_cloudwatch)
[![Docs](https://docs.rs/metrics_cloudwatch/badge.svg)](http://docs.rs/metrics_cloudwatch)
![Build status](https://github.com/ramn/metrics_cloudwatch/workflows/build/badge.svg)

Purpose
-------

Provide a backend for the [`metrics` facade
crate](https://crates.io/crates/metrics), pushing metrics to CloudWatch.


How to use
----------

Credentials for AWS needs to be available in the environment, see [Rusoto docs
on AWS credentials](
https://github.com/rusoto/rusoto/blob/master/AWS-CREDENTIALS.md)

```bash
cargo add -s metrics metrics_cloudwatch
```

```rust
fn main() {
    // Initialize the backend
    metrics_cloudwatch::builder()
        .cloudwatch_namespace("my-namespace")
        .init_thread()
        .unwrap();

    metrics::counter!("requests", 1);
}
```
