use std::{collections::HashSet, fs, path::PathBuf, process::Command, thread, time::Duration};

use clap::Args;

use super::{Error, report_options::ReportOptions, variant::Variant};

#[derive(Debug, Args)]
pub struct RunArgs {
    /// A variant as name=command, run with `sh -c`; repeat for each variant, the first is the baseline
    #[arg(long = "variant", required = true)]
    variants: Vec<Variant>,
    /// Rounds; the variant order reverses every round
    #[arg(long, default_value_t = 4)]
    rounds: usize,
    /// Seconds of idle time before every run, so clocks and temperature recover
    #[arg(long, default_value_t = 20)]
    cooldown: u64,
    /// Directory for the <variant>_<round>.log files (stdout and stderr)
    #[arg(long)]
    out: PathBuf,
    #[command(flatten)]
    report: ReportOptions,
}

impl RunArgs {
    pub fn run(&self) -> Result<(), Error> {
        self.report.validate()?;
        let mut names = HashSet::new();
        if let Some(duplicate) = self.variants.iter().find(|variant| !names.insert(variant.name.as_str())) {
            return Err(Error::DuplicateVariant(duplicate.name.clone()));
        }
        if self.rounds <= self.report.discard() {
            return Err(Error::TooFewRounds {
                rounds: self.rounds,
                discard: self.report.discard(),
            });
        }
        fs::create_dir_all(&self.out).map_err(|source| Error::Write {
            path: self.out.clone(),
            source,
        })?;
        for round in 1..=self.rounds {
            let mut order: Vec<&Variant> = self.variants.iter().collect();
            if round % 2 == 0 {
                order.reverse();
            }
            for variant in order {
                eprintln!("round {round}/{}: {} after {} s cooldown", self.rounds, variant.name, self.cooldown);
                thread::sleep(Duration::from_secs(self.cooldown));
                let log = self.out.join(format!("{}_{round}.log", variant.name));
                let write = |source| Error::Write {
                    path: log.clone(),
                    source,
                };
                let stdout = fs::File::create(&log).map_err(write)?;
                let stderr = stdout.try_clone().map_err(write)?;
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(&variant.command)
                    .env("METAL_PERF_VARIANT", &variant.name)
                    .env("METAL_PERF_ROUND", round.to_string())
                    .stdout(stdout)
                    .stderr(stderr)
                    .status()
                    .map_err(|source| Error::Spawn {
                        variant: variant.name.clone(),
                        source,
                    })?;
                if !status.success() {
                    return Err(Error::RunFailed {
                        variant: variant.name.clone(),
                        round,
                        status: status.to_string(),
                        log,
                    });
                }
            }
        }
        self.report.report(&self.out, &self.variants[0].name)
    }
}
