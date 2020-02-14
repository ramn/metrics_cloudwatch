#![warn(rust_2018_idioms)]
#![forbid(unsafe_code)]

pub use builder::{builder, Builder};
pub use error::Error;

mod builder;
mod collector;
mod error;
