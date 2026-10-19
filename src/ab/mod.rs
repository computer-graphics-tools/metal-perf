//! Interleaved A/B benchmarking for noisy GPU workloads: alternating order (ABBA), a cooldown before
//! every run, warm-up rounds dropped, and per-round paired ratios against a baseline.

mod ab_command;
mod error;
mod report_args;
mod report_options;
mod run_args;
mod run_summary;
mod variant;

pub use ab_command::AbCommand;
pub use error::Error;
