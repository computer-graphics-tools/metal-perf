use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a variant must look like name=command with a name of letters, digits, '.', '_' or '-': {0}")]
    VariantSyntax(String),
    #[error("variant names must be unique: {0}")]
    DuplicateVariant(String),
    #[error("{rounds} rounds leave nothing after discarding {discard} warm-up rounds")]
    TooFewRounds {
        rounds: usize,
        discard: usize,
    },
    #[error("the metric regex needs exactly one capture group, it has {0}")]
    MetricGroups(usize),
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
    #[error("could not start variant {variant}")]
    Spawn {
        variant: String,
        #[source]
        source: io::Error,
    },
    #[error("variant {variant} failed in round {round} with {status}; see {log}")]
    RunFailed {
        variant: String,
        round: usize,
        status: String,
        log: PathBuf,
    },
    #[error("the metric matched no numbers in {0}")]
    NoSamples(PathBuf),
    #[error("{0} holds no <variant>_<round>.log files after the warm-up rounds")]
    NoRuns(PathBuf),
    #[error("the baseline {0} has no runs")]
    UnknownBaseline(String),
}
