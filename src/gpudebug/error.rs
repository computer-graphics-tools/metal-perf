use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run gpudebug (it ships with Xcode 27 and later)")]
    Spawn(#[source] io::Error),
    #[error("gpudebug {arguments} failed: {output}")]
    Failed {
        arguments: String,
        output: String,
    },
    #[error("gpudebug did not report a session id: {0}")]
    NoSession(String),
    #[error("session {0} is not listed by `gpudebug -l`")]
    SessionMissing(String),
    #[error("session {0} entered the error state")]
    SessionFailed(String),
    #[error("profiling did not complete: {0}")]
    ProfileFailed(String),
    #[error("timed out waiting for {0}; raise --timeout")]
    Timeout(&'static str),
    #[error("`{command}` returned {found} JSON documents, expected {expected}: {output}")]
    UnexpectedJson {
        command: String,
        expected: usize,
        found: usize,
        output: String,
    },
    #[error("gpudebug cannot write to {0}: its command parser splits paths at spaces")]
    PathWithSpace(PathBuf),
    #[error("pass a trace or --session")]
    NoTarget,
    #[error("invalid JSON from gpudebug")]
    Json(#[from] serde_json::Error),
    #[error("could not write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
