use std::{num::NonZeroUsize, ptr::NonNull, thread, time::Duration};

use clap::Args;
use metal::prelude::*;

use super::{Error, gpu::Gpu, timing::Timing};
use crate::statistics::median;

const THREADGROUP_WIDTH: usize = 256;
const WARM_RUNS: usize = 3;

#[derive(Debug, Args)]
pub struct IdleGapArgs {
    /// Private storage per thread in bytes, a multiple of 4: the length of a dynamically indexed array
    #[arg(long, default_value_t = 16384)]
    private_bytes: usize,
    /// Grid size in threads, a multiple of 256
    #[arg(long, default_value_t = 1 << 20)]
    threads: usize,
    /// Idle times in milliseconds to wait before the measured command buffer
    #[arg(long, value_delimiter = ',', default_values_t = [50, 500, 1000, 1500, 3000, 12000])]
    gaps_ms: Vec<u64>,
    /// Measured command buffers per idle time; the median is reported
    #[arg(long, default_value = "3")]
    repeats: NonZeroUsize,
}

impl IdleGapArgs {
    pub fn run(&self) -> Result<(), Error> {
        if self.private_bytes == 0 || !self.private_bytes.is_multiple_of(4) {
            return Err(Error::Shape("private bytes must be a positive multiple of 4".to_owned()));
        }
        if self.threads == 0 || !self.threads.is_multiple_of(THREADGROUP_WIDTH) {
            return Err(Error::Shape(format!("threads must be a positive multiple of {THREADGROUP_WIDTH}")));
        }
        let gpu = Gpu::new()?;
        let source = include_str!("idle_gap.metal").replace("PRIVATE_FLOATS", &(self.private_bytes / 4).to_string());
        let pipelines = gpu.pipelines(&source, &["private_array"])?;
        let sink = gpu.private_buffer(self.threads * 4)?;
        let mut salt = 0u32;
        let mut run = || {
            salt = salt.wrapping_add(1);
            gpu.timed(|encoder| {
                encoder.set_compute_pipeline_state(&pipelines[0]);
                encoder.set_buffer(Some(&*sink), 0, 0);
                encoder.set_bytes(NonNull::from(&salt).cast(), size_of::<u32>(), 1);
                encoder.dispatch_threads(MTLSize::new(self.threads, 1, 1), MTLSize::new(THREADGROUP_WIDTH, 1, 1));
            })
        };
        println!(
            "device: {}, private storage: {} B per thread, {} threads",
            gpu.name(),
            self.private_bytes,
            self.threads
        );
        for _ in 0..WARM_RUNS {
            run()?;
        }
        println!("{:>8}  {:>26}  {:>26}  {:>16}", "idle", "first command buffer", "warm command buffer", "extra");
        println!("{:>8}  {:>26}  {:>26}  {:>16}", "ms", "wall / GPU ms", "wall / GPU ms", "GPU / host ms");
        for &gap_ms in &self.gaps_ms {
            let mut first = Vec::new();
            let mut warm = Vec::new();
            for _ in 0..self.repeats.get() {
                let mut warm_runs = (0..WARM_RUNS).map(|_| run()).collect::<Result<Vec<_>, _>>()?;
                warm_runs.sort_by(|left, right| left.wall_seconds.total_cmp(&right.wall_seconds));
                warm.push(warm_runs.swap_remove(WARM_RUNS / 2));
                thread::sleep(Duration::from_millis(gap_ms));
                first.push(run()?);
            }
            let (first, warm) = (medians(first), medians(warm));
            println!(
                "{gap_ms:>8}  {:>12.2} / {:>11.2}  {:>12.2} / {:>11.2}  {:>+7.2} / {:>+6.2}",
                first.wall_seconds * 1e3,
                first.gpu_seconds * 1e3,
                warm.wall_seconds * 1e3,
                warm.gpu_seconds * 1e3,
                (first.gpu_seconds - warm.gpu_seconds) * 1e3,
                (first.host_seconds() - warm.host_seconds()) * 1e3,
            );
        }
        Ok(())
    }
}

fn medians(samples: Vec<Timing>) -> Timing {
    let column = |select: fn(&Timing) -> f64| {
        let mut values = samples.iter().map(select).collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        median(&values)
    };
    Timing {
        wall_seconds: column(|timing| timing.wall_seconds),
        gpu_seconds: column(|timing| timing.gpu_seconds),
    }
}
