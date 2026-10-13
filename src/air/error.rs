use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not run `xcrun metal-objdump`")]
    Spawn(#[source] io::Error),
    #[error(
        "`xcrun metal-objdump -d {path}` failed: {stderr} (install the Metal toolchain with `xcodebuild -downloadComponent MetalToolchain`)"
    )]
    Disassembly {
        path: PathBuf,
        stderr: String,
    },
    #[error("malformed archive")]
    Archive(#[from] object::Error),
    #[error("invalid member text")]
    Regex(#[from] regex::Error),
    #[error("no archive member contains {0:?}")]
    NoMember(String),
    #[error("no defined function matches {0}")]
    NoFunction(String),
}
