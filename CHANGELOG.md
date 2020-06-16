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
