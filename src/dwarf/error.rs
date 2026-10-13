use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("not an object file")]
    Object(#[from] object::Error),
    #[error("malformed DWARF")]
    Dwarf(#[from] gimli::Error),
    #[error("the binary has no DWARF line table; compile the shaders with -gline-tables-only")]
    NoLineTable,
    #[error("an address range must look like 0x558:0x27b6, got {0}")]
    RangeSyntax(String),
}
