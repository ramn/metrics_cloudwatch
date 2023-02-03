#![warn(rust_2018_idioms)]
#![forbid(unsafe_code)]
// False positives on `metrics::Key` which uses interior mutability to cache the hash
#![allow(clippy::mutable_key_type)]

pub use {builder::Builder, collector::Resolution, error::Error, metrics};

use std::{borrow::Cow, future::Future, pin::Pin};

mod builder;
#[doc(hidden)]
pub mod collector;
mod error;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// From the CloudWatch docs:
// Seconds | Microseconds | Milliseconds | Bytes | Kilobytes | Megabytes | Gigabytes | Terabytes | Bits | Kilobits | Megabits | Gigabits | Terabits | Percent | Count | Bytes/Second | Kilobytes/Second | Megabytes/Second | Gigabytes/Second | Terabytes/Second | Bits/Second | Kilobits/Second | Megabits/Second | Gigabits/Second | Terabits/Second | Count/Second | None
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Unit {
    Seconds,
    Microseconds,
    Milliseconds,
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
    Bits,
    Kilobits,
    Megabits,
    Gigabits,
    Terabits,
    Percent,
    Count,
    BytesPerSecond,
    KilobytesPerSecond,
    MegabytesPerSecond,
    GigabytesPerSecond,
    TerabytesPerSecond,
    BitsPerSecond,
    KilobitsPerSecond,
    MegabitsPerSecond,
    GigabitsPerSecond,
    TerabitsPerSecond,
    CountPerSecond,
}

impl Unit {
    pub fn as_str(self) -> &'static str {
        use self::Unit::*;
        match self {
            Seconds => "Seconds",
            Microseconds => "Microseconds",
            Milliseconds => "Milliseconds",
            Bytes => "Bytes",
            Kilobytes => "Kilobytes",
            Megabytes => "Megabytes",
            Gigabytes => "Gigabytes",
            Terabytes => "Terabytes",
            Bits => "Bits",
            Kilobits => "Kilobits",
            Megabits => "Megabits",
            Gigabits => "Gigabits",
            Terabits => "Terabits",
            Percent => "Percent",
            Count => "Count",
            BytesPerSecond => "Bytes/Second",
            KilobytesPerSecond => "Kilobytes/Second",
            MegabytesPerSecond => "Megabytes/Second",
            GigabytesPerSecond => "Gigabytes/Second",
            TerabytesPerSecond => "Terabytes/Second",
            BitsPerSecond => "Bits/Second",
            KilobitsPerSecond => "Kilobits/Second",
            MegabitsPerSecond => "Megabits/Second",
            GigabitsPerSecond => "Gigabits/Second",
            TerabitsPerSecond => "Terabits/Second",
            CountPerSecond => "Count/Second",
        }
    }
}

impl From<Unit> for Cow<'static, str> {
    fn from(unit: Unit) -> Self {
        unit.as_str().into()
    }
}

impl From<Unit> for metrics::SharedString {
    fn from(unit: Unit) -> Self {
        unit.as_str().into()
    }
}
