<a name="v0.15.0"></a>
## v0.15.0 (2022-08-25)

#### Bug fixes
*   Only record value if it is finite

#### Features
*   Add option for force-flushing metrics


<a name="v0.14.1"></a>
### v0.14.1 (2021-07-14)


#### Bug Fixes

*   Increase the send timeout ([d798c0a4](https://github.com/ramn/metrics_cloudwatch/commit/d798c0a45ef3bb59195b170ea8a1832ee02b9bf4))
*   Remove the spreading out of metrics ([a48f4d24](https://github.com/ramn/metrics_cloudwatch/commit/a48f4d246b24b2a07cced86aebdf93dca04c4a06))
*   Fix don't sleep an additional interval after the last chunk ([9e2ff884](https://github.com/ramn/metrics_cloudwatch/commit/9e2ff8849033ec749cbfe85164db53ac7c068815))

#### Performance

*   Use ahash for faster hashlookups ([c1e7add1](https://github.com/ramn/metrics_cloudwatch/commit/c1e7add1b8edb377ce64bce911acb22e2a6cc01f))



<a name="v0.14.0"></a>
## v0.14.0 (2021-07-07)




<a name="v0.13.0"></a>
## v0.13.0 (2021-05-19)


#### Bug Fixes

*   Safeguard against using multiple versions of metrics ([8586904a](https://github.com/ramn/metrics_cloudwatch/commit/8586904a22f13783b648fce1a6751a3d77c34882))

#### Breaking Changes

*   Update the metrics crate to 0.15 ([c25fa9e8](https://github.com/ramn/metrics_cloudwatch/commit/c25fa9e80fbb3df3f97612d9d690b15234dc2d19), breaks [#](https://github.com/ramn/metrics_cloudwatch/issues/))

#### Features

*   Update the metrics crate to 0.15 ([c25fa9e8](https://github.com/ramn/metrics_cloudwatch/commit/c25fa9e80fbb3df3f97612d9d690b15234dc2d19), breaks [#](https://github.com/ramn/metrics_cloudwatch/issues/))



<a name="v0.12.5"></a>
### v0.12.5 (2021-05-10)


#### Features

*   Double the metric buffer size ([8040fbb6](https://github.com/ramn/metrics_cloudwatch/commit/8040fbb6104240187131a4403398777f659acb87))
*   Allow the metric channel's buffer size to be configured ([8642a767](https://github.com/ramn/metrics_cloudwatch/commit/8642a7671484dd28023af68084171b741f186240))

#### Bug Fixes

*   Spread each metrics batch out to reduce throttling ([337b23e7](https://github.com/ramn/metrics_cloudwatch/commit/337b23e7717f9bd61c09513a3e85e4559e7adac5))
*   Ensure we get a fresh timestamp after the channel returns pending ([ac6092be](https://github.com/ramn/metrics_cloudwatch/commit/ac6092becff00e85b7cfdb660403765d9dfdeddd))
*   Retry the metrics sending it gets throttled ([51eacb77](https://github.com/ramn/metrics_cloudwatch/commit/51eacb77aa482fc904c744fc2d50318198dd4622))

#### Performance

*   No need to clone the Sender ([3408e993](https://github.com/ramn/metrics_cloudwatch/commit/3408e99375176350be2b6f626e880c474ba17514))
*   Amortize the timestamp lookup ([dd5de888](https://github.com/ramn/metrics_cloudwatch/commit/dd5de8880568b6eb41650fb540d81d4c3a67fde9))
*   Amortize the cost of SystemTime::elapsed ([7ce51f90](https://github.com/ramn/metrics_cloudwatch/commit/7ce51f904f35a0db38360e46748b7ffc36b79e0c))



<a name="v0.12.4"></a>
### v0.12.4 (2021-02-19)


#### Features

*   Allow default dimensions to be removed for specific metrics ([8e99ebb5](https://github.com/ramn/metrics_cloudwatch/commit/8e99ebb55aab5896b9b0bb55e3f71e028e74d318))



<a name="v0.12.3"></a>
### v0.12.3 (2021-02-19)




<a name="v0.12.2"></a>
### v0.12.2 (2021-02-10)


#### Bug Fixes

*   rand::thread_rng is required ([98f58ab7](https://github.com/ramn/metrics_cloudwatch/commit/98f58ab7e78c9c7448aa06968ec0f0e7c37882fa))



<a name="v0.12.1"></a>
### v0.12.1 (2021-02-09)


#### Features

*   Allow rustls to be used over native-tls ([60703720](https://github.com/ramn/metrics_cloudwatch/commit/607037208aa9c8cb196f2f25c8989d4b9f1bf6d6))



<a name="v0.12.0"></a>
## v0.12.0 (2021-02-08)


#### Bug Fixes

*   Make gauges display the current value as "Average" ([4124e3e4](https://github.com/ramn/metrics_cloudwatch/commit/4124e3e4cdf967846459eae2606354e64ab2e291))

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
