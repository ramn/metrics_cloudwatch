#![warn(rust_2018_idioms)]
#![forbid(unsafe_code)]

pub use {rusoto_cloudwatch::CloudWatch, rusoto_core::Region};

pub use {
    builder::{builder, Builder},
    collector::Resolution,
    error::Error,
};

use std::{future::Future, pin::Pin};

mod builder;
#[doc(hidden)]
pub mod collector;
mod error;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
