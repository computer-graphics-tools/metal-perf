use std::{num::NonZeroUsize, ptr::NonNull};

use clap::Args;
use metal::prelude::*;

use super::{Error, gpu::Gpu};

const THREADGROUP_WIDTH: usize = 1024;

#[derive(Debug, Args)]
pub struct BandwidthArgs {
    /// Size of each buffer in GiB; keep it well above the system-level cache
    #[arg(long, default_value_t = 2.0)]
    gib: f64,
    /// Grid sizes in threads; the kernels use grid-stride loops
    #[arg(long, value_delimiter = ',', default_values_t = [16384, 65536, 262144, 1048576])]
    threads: Vec<usize>,
    /// Timed runs per configuration; the fastest one is reported
    #[arg(long, default_value = "6")]
    repeats: NonZeroUsize,
}

impl BandwidthArgs {
    pub fn run(&self) -> Result<(), Error> {
        let bytes = (self.gib * (1u64 << 30) as f64) as usize / 16 * 16;
        let count = u32::try_from(bytes / 16)
            .ok()
            .filter(|&count| count > 0)
            .ok_or_else(|| Error::Shape(format!("{} GiB does not fit a 32-bit element count", self.gib)))?;
        if self.threads.iter().any(|&threads| threads == 0 || !threads.is_multiple_of(THREADGROUP_WIDTH)) {
            return Err(Error::Shape(format!("thread counts must be positive multiples of {THREADGROUP_WIDTH}")));
        }
        let gpu = Gpu::new()?;
        let pipelines = gpu.pipelines(include_str!("bandwidth.metal"), &["read4", "copy4"])?;
        let source = gpu.private_buffer(bytes)?;
        let destination = gpu.private_buffer(bytes)?;
        let sink = gpu.private_buffer(self.threads.iter().max().copied().unwrap_or(1) * 4)?;
        println!("device: {}, buffer: {} MiB", gpu.name(), bytes >> 20);
        for (name, pipeline, output, moved) in
            [("read", &pipelines[0], &sink, bytes), ("copy", &pipelines[1], &destination, 2 * bytes)]
        {
            let mut peak = 0.0f64;
            for &threads in &self.threads {
                let seconds = gpu.fastest_seconds(self.repeats, |encoder| {
                    encoder.set_compute_pipeline_state(pipeline);
                    encoder.set_buffer(Some(&*source), 0, 0);
                    encoder.set_buffer(Some(&**output), 0, 1);
                    encoder.set_bytes(NonNull::from(&count).cast(), size_of::<u32>(), 2);
                    encoder.dispatch_threads(MTLSize::new(threads, 1, 1), MTLSize::new(THREADGROUP_WIDTH, 1, 1));
                })?;
                let rate = moved as f64 / seconds / 1e9;
                peak = peak.max(rate);
                println!("{name} threads={threads:>8}: {rate:7.1} GB/s ({:.2} ms)", seconds * 1e3);
            }
            println!("{name} peak: {peak:.1} GB/s");
        }
        Ok(())
    }
}
