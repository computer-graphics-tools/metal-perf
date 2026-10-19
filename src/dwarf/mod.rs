//! Source attribution of compiled GPU binaries through their DWARF line table and inline tree.
//!
//! There is no AGX disassembler, so code bytes are the proxy for instruction count: AGX
//! instructions vary in length, and bytes measure code, not time.

mod address_range;
mod error;
mod gpu_binary;
mod inline_bytes;
mod inlines_args;
mod line_span;
mod lines_args;
mod lines_mode;
mod range_set;

pub use error::Error;
pub use inlines_args::InlinesArgs;
pub use lines_args::LinesArgs;

type Reader<'data> = gimli::EndianSlice<'data, gimli::RunTimeEndian>;
