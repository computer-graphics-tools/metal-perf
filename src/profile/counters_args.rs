use std::path::PathBuf;

use clap::Args;
use regex::Regex;

use super::{Error, profile_directory::ProfileDirectory, shader_group::ShaderGroup};
use crate::text::truncate;

const COLUMN_WIDTH: usize = 18;

#[derive(Debug, Args)]
pub struct CountersArgs {
    /// Directory written by `metal-perf profile`
    directory: PathBuf,
    /// Encoder index, in timeline order
    #[arg(long, default_value_t = 0)]
    encoder: usize,
    /// Column of shader timeline intervals as label=regex over shader names; repeat for more columns
    #[arg(long = "group", required = true)]
    groups: Vec<ShaderGroup>,
    /// Regex over group/counter names [default: every fetched counter]
    #[arg(long)]
    counters: Option<Regex>,
}

impl CountersArgs {
    pub fn run(&self) -> Result<(), Error> {
        let profile = ProfileDirectory::load(&self.directory)?;
        let no_encoder = || Error::NoEncoder {
            index: self.encoder,
            count: profile.encoder_count(),
        };
        let encoder = profile.encoder(self.encoder).ok_or_else(no_encoder)?;
        let segment =
            profile.segments().get(self.encoder).filter(|segment| !segment.is_empty()).ok_or_else(no_encoder)?;
        let offset = (encoder.begin_ns - segment[0].begin_ns) as f64;
        let columns: Vec<Vec<(f64, f64)>> = self
            .groups
            .iter()
            .map(|group| {
                segment
                    .iter()
                    .filter(|interval| group.pattern.is_match(&interval.shader))
                    .map(|interval| (interval.begin_ns as f64 + offset, interval.end_ns as f64 + offset))
                    .collect()
            })
            .collect();
        let counters: Vec<_> = profile
            .counters()?
            .into_iter()
            .filter(|(name, _)| self.counters.as_ref().is_none_or(|pattern| pattern.is_match(name)))
            .collect();
        let width = counters.iter().map(|(name, _)| name.len()).max().unwrap_or(8).max(10);
        let unit_width =
            counters.iter().filter_map(|(_, series)| series.unit.as_deref()).map(str::len).max().unwrap_or(0).max(11);

        println!("encoder {}: {}", self.encoder, encoder.name);
        println!(
            "interval counts are merged shader intervals; values are mean (samples); units are from capture metadata"
        );
        let header: String = self
            .groups
            .iter()
            .map(|group| format!("{:>COLUMN_WIDTH$}", truncate(&group.label, COLUMN_WIDTH - 1)))
            .collect();
        println!("{:width$} {:unit_width$}{header}", "counter", "unit");
        let intervals: String = columns.iter().map(|spans| format!("{:>COLUMN_WIDTH$}", spans.len())).collect();
        println!("{:width$} {:unit_width$}{intervals}", "intervals", "");
        for (name, series) in &counters {
            let values: String = columns
                .iter()
                .map(|spans| {
                    let (mean, count) = series.window_mean(spans);
                    let value = format!("{mean:.1} ({count})");
                    format!("{value:>COLUMN_WIDTH$}")
                })
                .collect();
            let unit = series.unit.as_deref().filter(|unit| !unit.is_empty()).unwrap_or("unavailable");
            println!("{name:width$} {unit:unit_width$}{values}");
        }
        Ok(())
    }
}
