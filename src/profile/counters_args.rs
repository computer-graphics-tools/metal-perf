use std::path::PathBuf;

use clap::Args;
use regex::Regex;

use super::{Error, profile_directory::ProfileDirectory, shader_group::ShaderGroup};
use crate::text::truncate;

const COLUMN_WIDTH: usize = 13;

#[derive(Debug, Args)]
pub struct CountersArgs {
    /// Directory written by `metal-perf profile`
    directory: PathBuf,
    /// Encoder index, in timeline order
    #[arg(long, default_value_t = 0)]
    encoder: usize,
    /// Column of dispatches as label=regex over shader names; repeat for more columns
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
                    .filter(|dispatch| group.pattern.is_match(&dispatch.shader))
                    .map(|dispatch| (dispatch.begin_ns as f64 + offset, dispatch.end_ns as f64 + offset))
                    .collect()
            })
            .collect();
        let counters: Vec<_> = profile
            .counters()?
            .into_iter()
            .filter(|(name, _)| self.counters.as_ref().is_none_or(|pattern| pattern.is_match(name)))
            .collect();
        let width = counters.iter().map(|(name, _)| name.len()).max().unwrap_or(8).max(10);

        println!("encoder {}: {}", self.encoder, encoder.name);
        let header: String = self
            .groups
            .iter()
            .map(|group| format!("{:>COLUMN_WIDTH$}", truncate(&group.label, COLUMN_WIDTH - 1)))
            .collect();
        println!("{:width$}{header}", "counter");
        let dispatches: String = columns.iter().map(|spans| format!("{:>COLUMN_WIDTH$}", spans.len())).collect();
        println!("{:width$}{dispatches}", "dispatches");
        for (name, series) in &counters {
            let values: String =
                columns.iter().map(|spans| format!("{:>COLUMN_WIDTH$.1}", series.window_mean(spans).0)).collect();
            println!("{name:width$}{values}");
        }
        if let Some((_, series)) = counters.first() {
            let samples: String =
                columns.iter().map(|spans| format!("{:>COLUMN_WIDTH$}", series.window_mean(spans).1)).collect();
            println!("{:width$}{samples}", "samples");
        }
        Ok(())
    }
}
