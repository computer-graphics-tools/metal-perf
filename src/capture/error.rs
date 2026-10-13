use std::{io, path::PathBuf, process::ExitStatus};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} exists; gpucapture needs a new path")]
    OutputExists(PathBuf),
    #[error("could not launch {program}")]
    Launch {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("could not run gpucapture (it ships with Xcode 27 and later)")]
    Gpucapture(#[source] io::Error),
    #[error("the target exited with {0} before it became capturable")]
    ExitedEarly(ExitStatus),
    #[error("the target was not listed by `gpucapture list` within {0} s")]
    Timeout(u64),
    #[error("gpucapture start exited with {0}")]
    CaptureFailed(ExitStatus),
    #[error("the trace was written to {path}, but the target exited with {status}")]
    TargetFailed {
        path: PathBuf,
        status: ExitStatus,
    },
    #[error("could not wait for the target")]
    Wait(#[source] io::Error),
}
