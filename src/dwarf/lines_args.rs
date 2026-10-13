use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use clap::Args;

use super::{Error, address_range::AddressRange, gpu_binary::GpuBinary, lines_mode::LinesMode};
use crate::text::truncate;

const SOURCE_WIDTH: usize = 110;

#[derive(Debug, Args)]
pub struct LinesArgs {
    /// GPU binary written by `metal-perf extract`
    binary: PathBuf,
    /// Report kind
    #[arg(long, value_enum, default_value_t = LinesMode::Lines)]
    mode: LinesMode,
    /// Source lines listed
    #[arg(long, default_value_t = 45)]
    top: usize,
    /// Directory that relative source paths resolve against, for printing source text
    #[arg(long)]
    root: Option<PathBuf>,
    /// Only count code in this hex address range, lo:hi
    #[arg(long)]
    range: Option<AddressRange>,
}

impl LinesArgs {
    pub fn run(&self) -> Result<(), Error> {
        let data = fs::read(&self.binary).map_err(|source| Error::Read {
            path: self.binary.clone(),
            source,
        })?;
        let mut spans = GpuBinary::parse(&data)?.line_spans()?;
        if let Some(range) = self.range {
            spans.retain(|span| range.contains(span.address));
        }
        match self.mode {
            LinesMode::Lines => {
                let mut lines: HashMap<(Rc<str>, u64), u64> = HashMap::new();
                let mut files: HashMap<&str, u64> = HashMap::new();
                for span in &spans {
                    *lines.entry((span.file.clone(), span.line)).or_default() += span.size;
                    *files.entry(base_name(&span.file)).or_default() += span.size;
                }
                let mut files: Vec<_> = files.into_iter().collect();
                files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
                let summary: Vec<String> = files.iter().take(12).map(|(file, size)| format!("{file}={size}")).collect();
                println!(
                    "bytes by file: {}, total {}",
                    summary.join(", "),
                    spans.iter().map(|span| span.size).sum::<u64>()
                );
                let mut lines: Vec<_> = lines.into_iter().collect();
                lines.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
                let mut sources = HashMap::new();
                for ((file, line), size) in lines.iter().take(self.top) {
                    let text = source_line(&mut sources, file, *line, self.root.as_deref());
                    println!("{size:6} {}:{line:<5} {}", base_name(file), truncate(text.trim(), SOURCE_WIDTH));
                }
            },
            LinesMode::Layout => {
                let mut runs: Vec<(u64, u64, String)> = Vec::new();
                for span in &spans {
                    let key = format!("{}:{}", base_name(&span.file), span.line);
                    match runs.last_mut() {
                        Some(run) if run.2 == key && run.0 + run.1 == span.address => run.1 += span.size,
                        _ => runs.push((span.address, span.size, key)),
                    }
                }
                for (address, size, key) in runs {
                    println!("0x{address:05x} {size:5} {key}");
                }
            },
        }
        Ok(())
    }
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn source_line(
    sources: &mut HashMap<PathBuf, Option<Vec<String>>>,
    file: &str,
    line: u64,
    root: Option<&Path>,
) -> String {
    let path = match root {
        Some(root) if !file.starts_with('/') => root.join(file),
        _ => PathBuf::from(file),
    };
    let lines = sources
        .entry(path)
        .or_insert_with_key(|path| fs::read_to_string(path).ok().map(|text| text.lines().map(str::to_owned).collect()));
    lines.as_ref().and_then(|lines| lines.get((line as usize).checked_sub(1)?)).cloned().unwrap_or_default()
}
