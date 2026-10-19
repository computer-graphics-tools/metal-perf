use clap::Parser;

use crate::{Error, command::Command};

#[derive(Debug, Parser)]
#[command(name = "metal-perf", version, about = "Instruments for Apple GPU (Metal) compute performance work")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub fn run(self) -> Result<(), Error> {
        self.command.run()
    }
}
