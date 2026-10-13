use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not parse {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path} has an unexpected layout: {reason}")]
    Layout {
        path: PathBuf,
        reason: &'static str,
    },
    #[error("{0} holds no shader__*.json timelines; run `metal-perf profile` first")]
    NoShaders(PathBuf),
    #[error("encoder {index} does not exist; the profile has {count}")]
    NoEncoder {
        index: usize,
        count: usize,
    },
    #[error("a shader group must look like label=regex: {0}")]
    GroupSyntax(String),
    #[error("invalid regex")]
    Regex(#[from] regex::Error),
}
