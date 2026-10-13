use std::path::PathBuf;

use clap::Args;

use super::{Error, report_options::ReportOptions};

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Directory of <variant>_<round>.log files written by `metal-perf ab run`
    directory: PathBuf,
    /// Variant the others are compared against
    #[arg(long)]
    baseline: String,
    #[command(flatten)]
    options: ReportOptions,
}

impl ReportArgs {
    pub fn run(&self) -> Result<(), Error> {
        self.options.report(&self.directory, &self.baseline)
    }
}
