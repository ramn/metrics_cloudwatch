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
