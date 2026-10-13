//! Analysis of a profile directory written by `metal-perf profile`.

mod counter_sample;
mod counter_series;
mod counters_args;
mod dispatch;
mod encoder;
mod error;
mod interval;
mod profile_directory;
mod shader_group;
mod shader_timeline;
mod shaders_args;

pub use counters_args::CountersArgs;
pub use error::Error;
pub use shaders_args::ShadersArgs;
