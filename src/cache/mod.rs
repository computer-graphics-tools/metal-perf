//! The Metal shader cache: every compiled pipeline as a GPU Mach-O, with DWARF when the source
//! was compiled with line tables.

mod error;
mod extract_args;

pub use error::Error;
pub use extract_args::ExtractArgs;
