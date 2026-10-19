//! Microbenchmarks that measure what the memory system can deliver on this GPU.

mod access_pattern_args;
mod bandwidth_args;
mod error;
mod gpu;
mod idle_gap_args;
mod timing;

pub use access_pattern_args::AccessPatternArgs;
pub use bandwidth_args::BandwidthArgs;
pub use error::Error;
pub use idle_gap_args::IdleGapArgs;
