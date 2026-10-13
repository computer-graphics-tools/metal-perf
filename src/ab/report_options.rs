use std::{collections::BTreeMap, fs, path::Path};

use clap::Args;
use regex::Regex;

use super::{Error, run_summary::RunSummary};
use crate::statistics::geometric_mean;

#[derive(Debug, Args)]
pub struct ReportOptions {
    /// Regex with one capture group that extracts a number from a log line
    #[arg(long)]
    metric: Regex,
    /// Leading rounds dropped as warm-up (page-in, shader compilation, clocks ramping)
    #[arg(long, default_value_t = 1)]
    discard: usize,
    /// Larger values are better, as for throughput [default: smaller is better, as for time]
    #[arg(long)]
    higher_is_better: bool,
}

impl ReportOptions {
    pub fn discard(&self) -> usize {
        self.discard
    }

    pub fn validate(&self) -> Result<(), Error> {
        match self.metric.captures_len() {
            2 => Ok(()),
            groups => Err(Error::MetricGroups(groups - 1)),
        }
    }

    pub fn report(
        &self,
        directory: &Path,
        baseline: &str,
    ) -> Result<(), Error> {
        self.validate()?;
        let log_name = Regex::new(r"^(.+)_(\d+)\.log$").expect("valid log name pattern");
        let (tail_fraction, tail) = if self.higher_is_better {
            (0.9, "p90")
        } else {
            (0.1, "p10")
        };
        let read = |source| Error::Read {
            path: directory.to_owned(),
            source,
        };
        let mut runs: BTreeMap<String, BTreeMap<usize, RunSummary>> = BTreeMap::new();
        for entry in fs::read_dir(directory).map_err(read)? {
            let path = entry.map_err(read)?.path();
            let file_name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let Some(parts) = log_name.captures(&file_name) else {
                continue;
            };
            let Ok(round) = parts[2].parse::<usize>() else {
                continue;
            };
            if round <= self.discard {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|source| Error::Read {
                path: path.clone(),
                source,
            })?;
            let samples: Vec<f64> = text
                .lines()
                .filter_map(|line| self.metric.captures(line)?.get(1)?.as_str().parse::<f64>().ok())
                .collect();
            if samples.is_empty() {
                return Err(Error::NoSamples(path));
            }
            runs.entry(parts[1].to_owned()).or_default().insert(round, RunSummary::new(samples, tail_fraction));
        }
        if runs.is_empty() {
            return Err(Error::NoRuns(directory.to_owned()));
        }
        let base = runs.get(baseline).ok_or_else(|| Error::UnknownBaseline(baseline.to_owned()))?;
        let best = |values: &[f64]| {
            let fold = if self.higher_is_better {
                f64::max
            } else {
                f64::min
            };
            values.iter().copied().reduce(fold).unwrap_or(f64::NAN)
        };

        for (variant, rounds) in &runs {
            let medians: Vec<f64> = rounds.values().map(|run| run.median).collect();
            let tails: Vec<f64> = rounds.values().map(|run| run.tail).collect();
            println!(
                "{variant:24} runs {}  median/run {}  best-run {:.3}  {tail}/run {}  best-{tail} {:.3}  samples/run {}",
                rounds.len(),
                list(&medians),
                best(&medians),
                list(&tails),
                best(&tails),
                rounds.values().map(|run| run.samples).min().unwrap_or(0),
            );
        }
        let direction = if self.higher_is_better {
            "higher"
        } else {
            "lower"
        };
        println!("ratios are variant / {baseline}; {direction} values are better");
        let base_medians: Vec<f64> = base.values().map(|run| run.median).collect();
        for (variant, rounds) in runs.iter().filter(|(variant, _)| variant.as_str() != baseline) {
            let pairs: Vec<(&RunSummary, &RunSummary)> =
                rounds.iter().filter_map(|(round, run)| Some((run, base.get(round)?))).collect();
            let medians: Vec<f64> = pairs.iter().map(|(run, base)| run.median / base.median).collect();
            let tails: Vec<f64> = pairs.iter().map(|(run, base)| run.tail / base.tail).collect();
            let run_medians: Vec<f64> = rounds.values().map(|run| run.median).collect();
            let best_ratio = best(&run_medians) / best(&base_medians);
            println!(
                "{variant} / {baseline}: median ratio geomean {:.3} (pairs {}), {tail} ratio geomean {:.3}, best-run ratio {best_ratio:.3}",
                geometric_mean(&medians),
                list(&medians),
                geometric_mean(&tails),
            );
        }
        Ok(())
    }
}

fn list(values: &[f64]) -> String {
    format!("[{}]", values.iter().map(|value| format!("{value:.3}")).collect::<Vec<_>>().join(", "))
}
