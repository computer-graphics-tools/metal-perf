use std::{collections::HashMap, path::PathBuf, rc::Rc};

use clap::Args;
use serde_json::Value;

use super::{Error, profile_directory::ProfileDirectory};
use crate::text::truncate;

const NAME_WIDTH: usize = 140;

#[derive(Debug, Args)]
pub struct ShadersArgs {
    /// Directory written by `metal-perf profile`
    directory: PathBuf,
    /// Shaders listed per encoder
    #[arg(long, default_value_t = 25)]
    top: usize,
}

impl ShadersArgs {
    pub fn run(&self) -> Result<(), Error> {
        let profile = ProfileDirectory::load(&self.directory)?;
        for (index, segment) in profile.segments().iter().enumerate() {
            let busy: i64 = segment.iter().map(|dispatch| dispatch.end_ns - dispatch.begin_ns).sum();
            let span = segment.last().zip(segment.first()).map_or(0, |(last, first)| last.end_ns - first.begin_ns);
            let (name, duration) = profile.encoder(index).map_or((format!("segment {index}"), 0), |encoder| {
                (encoder.name.clone(), encoder.end_ns - encoder.begin_ns)
            });
            println!(
                "\n=== {name}: encoder {:.2} ms, interval span {:.2} ms, busy {:.2} ms, dispatches {}",
                duration as f64 / 1e6,
                span as f64 / 1e6,
                busy as f64 / 1e6,
                segment.len()
            );
            let mut totals: HashMap<Rc<str>, (i64, usize)> = HashMap::new();
            for dispatch in segment {
                let total = totals.entry(dispatch.shader.clone()).or_default();
                total.0 += dispatch.end_ns - dispatch.begin_ns;
                total.1 += 1;
            }
            let mut shaders: Vec<(Rc<str>, i64, usize)> =
                totals.into_iter().map(|(shader, (total, count))| (shader, total, count)).collect();
            shaders.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            for (shader, total, count) in shaders.iter().take(self.top) {
                let statistics = profile.compiler_statistics(shader).map_or_else(String::new, |statistics| {
                    format!(
                        "regs {:>3} spill {:>3}  ",
                        text(&statistics["Temp registers"]),
                        text(&statistics["Spilled bytes"])
                    )
                });
                println!(
                    "  {:8.3} ms {:5.1}%  n={count:<4}  avg {:9.1} us  {statistics}{}",
                    *total as f64 / 1e6,
                    100.0 * *total as f64 / busy.max(1) as f64,
                    *total as f64 / *count as f64 / 1e3,
                    truncate(shader, NAME_WIDTH)
                );
            }
        }
        Ok(())
    }
}

fn text(value: &Value) -> String {
    value.as_str().map_or_else(|| value.to_string(), str::to_owned)
}
