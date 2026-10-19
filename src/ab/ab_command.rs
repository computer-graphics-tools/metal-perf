use clap::Subcommand;

use super::{Error, report_args::ReportArgs, run_args::RunArgs};

#[derive(Debug, Subcommand)]
pub enum AbCommand {
    /// Run every variant once per round in alternating order, then report
    Run(RunArgs),
    /// Summarize the logs of an earlier run, for example with another metric
    Report(ReportArgs),
}

impl AbCommand {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Self::Run(arguments) => arguments.run(),
            Self::Report(arguments) => arguments.run(),
        }
    }
}
