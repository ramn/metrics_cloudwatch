<a name="v0.11.0"></a>
## v0.11.0 (2021-01-29)


#### Features

*   Record and translate the registered metrics units ([66a3a71f](https://github.com/ramn/metrics_cloudwatch/commit/66a3a71f808d5992053f3c886d3d12f0d6dc328e))
*   Update to metrics 0.13 ([db5c3b9d](https://github.com/ramn/metrics_cloudwatch/commit/db5c3b9d1a0b12bee38eaf528310a290ed37164f))



<a name="v0.9.0"></a>
## v0.9.0 (2020-09-28)


#### Features

*   Add support for specifying cloudwatch units ([130aecf9](https://github.com/ramn/metrics_cloudwatch/commit/130aecf9e7e3b5f24a6ed89fa2adbacdab620ee6))

#### Performance

*   Don't clone the default_dimensions btree ([045dc7b3](https://github.com/ramn/metrics_cloudwatch/commit/045dc7b3eed249f840da0af8786f9dd4d6dd1b78))

#### Bug Fixes

*   Don't assume a default region/client ([58b4aed2](https://github.com/ramn/metrics_cloudwatch/commit/58b4aed2c4d069b0968be64f870c54ec8670feaa))



<a name="0.8.2"></a>
### 0.8.2 (2020-09-11)


#### Bug Fixes

*   Display averages for count metrics ([8bfccb7e](https://github.com/ramn/metrics_cloudwatch/commit/8bfccb7e27b9cff802045a16756fe5a051fae638))



<a name="0.8.1"></a>
### 0.8.1 (2020-08-26)


#### Features

*   Add some jitter to the send interval ([cea37c14](https://github.com/ramn/metrics_cloudwatch/commit/cea37c14c5dc814da802d50608b16d428e2a84ae))

#### Bug Fixes

*   Try to avoid throttling by sending chunks one-by-one ([d6e26e34](https://github.com/ramn/metrics_cloudwatch/commit/d6e26e34acf6c3bd227a8a9326a86f04da79d1d0))



0.8.0
-----
* Allow explicit dimensions to override defaults

0.7.0
-----
* Pack metrics to be sent to CloudWatch more efficiently, to avoid hitting AWS
rate limiting
* Simplify how a CloudWatch Client is passed to the builder

0.6.0
-----
* Change: the reported min and max values of a counter is now set as equal to
  the sum for the aggregation period.

  It shouldn't matter if, within the granularity of our aggregation period, we
  increment the counter 100 times by 1 or increment it once by 100. Typically we
  increment by 1 many times, which leads the min/max to also be 1. Changes in
  increment rate is only meaningful across aggregation periods.

  For example, the max value would increase proportional to the aggregation
  period (unlike for a gauge), so it doesn't tell us anything about the measured
  phenomenon, but rather about how we measured. Thus, this effect should rather
  not be observable. With this change it is no longer observable.

  It is however useful to report min/max as the sum value, since different
  sources might still report different min/max values for the same aggregation
  period.

0.5.0
-----
* Fix conflict with StreamExt::take_until

0.4.0
-----
* Upgrade dependencies

0.3.0
-----
* timing!() and value!() metrics types now use a histogram, which enables
percentiles in CloudWatch. However this could lead to more API calls, if there
are many unique values. See Limitations in the README.
* The builder now takes a shutdown Future, which enables flush of metrics on
shutdown.

0.2.0
-----

* Breaking change: `init_future` now returns a std Future.
* Migrated to async/await, std Future, futures 0.3 and tokio 0.2
