//! Tables exported by `xctrace export --xpath '/trace-toc/run/data/table[@schema="..."]'`.
//!
//! Exports deduplicate values: a cell either defines a value with `id` (and a display `fmt`) or
//! points at an earlier one with `ref`. Values nested inside a cell (a process inside a thread) can
//! be referenced too, so every element with an `id` is remembered.

mod cell;
mod error;
mod filter;
mod minimum;
mod open_element;
mod table;
mod xctrace_args;

pub use error::Error;
pub use xctrace_args::XctraceArgs;
