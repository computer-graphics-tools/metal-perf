//! Microbenchmarks that measure what the memory system can deliver on this GPU.

mod access_pattern_args;
mod bandwidth_args;
mod error;
mod gpu;

pub use access_pattern_args::AccessPatternArgs;
pub use bandwidth_args::BandwidthArgs;
pub use error::Error;
