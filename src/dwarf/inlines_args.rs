use std::{fs, path::PathBuf};

use clap::Args;

use super::{Error, gpu_binary::GpuBinary, inline_bytes::InlineBytes};
use crate::text::truncate;

const NAME_WIDTH: usize = 150;

#[derive(Debug, Args)]
pub struct InlinesArgs {
    /// GPU binary written by `metal-perf extract`
    binary: PathBuf,
    /// Functions listed
    #[arg(long, default_value_t = 40)]
    top: usize,
}

impl InlinesArgs {
    pub fn run(&self) -> Result<(), Error> {
        let data = fs::read(&self.binary).map_err(|source| Error::Read {
            path: self.binary.clone(),
            source,
        })?;
        let InlineBytes {
            kernels,
            exclusive,
        } = GpuBinary::parse(&data)?.inline_bytes()?;
        for (name, bytes) in &kernels {
            println!("kernel {} bytes {bytes}", truncate(name, NAME_WIDTH));
        }
        let total: u64 = exclusive.values().sum();
        println!("exclusive bytes {total}");
        let mut functions: Vec<_> = exclusive.into_iter().collect();
        functions.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (name, bytes) in functions.iter().take(self.top) {
            println!(
                "  {bytes:7} {:5.1}%  {}",
                100.0 * *bytes as f64 / total.max(1) as f64,
                truncate(name, NAME_WIDTH)
            );
        }
        Ok(())
    }
}
