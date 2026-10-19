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
    #[error("`getconf DARWIN_USER_CACHE_DIR` failed")]
    CacheDirectory(#[source] io::Error),
    #[error("no Metal shader cache under {0}; run the workload first or pass --cache")]
    NoCache(PathBuf),
    #[error("invalid pattern")]
    Regex(#[from] regex::Error),
}
