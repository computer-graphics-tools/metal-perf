//! Command-line instruments for Apple GPU (Metal) compute performance work.
//!
//! Every subcommand wraps one measurement or analysis step: bandwidth ceilings, `gpudebug` profile
//! collection and analysis, Metal shader cache extraction, DWARF attribution of GPU binaries, AIR
//! intrinsic call trees, interleaved A/B benchmarking and `xctrace` table aggregation.

mod ab;
mod air;
mod cache;
mod capture;
mod cli;
mod command;
mod dwarf;
mod error;
mod gpudebug;
mod probe;
mod profile;
mod statistics;
mod text;
mod xctrace;

pub use cli::Cli;
pub use error::Error;
