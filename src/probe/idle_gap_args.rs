use std::{
    io::Write,
    num::NonZeroUsize,
    ptr::NonNull,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clap::Args;
use metal::prelude::*;

use super::{Error, gpu::Gpu, timing::Timing};
use crate::statistics::median;

const WARM_RUNS: usize = 3;

#[derive(Debug, Args)]
pub struct IdleGapArgs {
    /// Private storage per thread in bytes, a multiple of 4: the length of a dynamically indexed array
    #[arg(long, default_value_t = 16384)]
    private_bytes: usize,
    /// Grid size in threads, a multiple of --threadgroup-size
    #[arg(long, default_value_t = 1 << 20)]
    threads: usize,
    /// Threads per threadgroup
    #[arg(long, default_value_t = 256)]
    threadgroup_size: usize,
    /// Run a correctness-checked dispatch, print a ready marker, and hold the process for an external pool snapshot
    #[arg(long)]
    pool_probe: bool,
    /// Use a short volatile private-array workload for long-grid pool probes
    #[arg(long, requires = "pool_probe")]
    pool_fast: bool,
    /// How long a pool probe holds the process after its ready marker
    #[arg(long, default_value_t = 3000)]
    pool_hold_ms: u64,
    /// Hold before the probe dispatch so an external tool can snapshot setup allocations
    #[arg(long, requires = "pool_probe", default_value_t = 0)]
    pool_pre_hold_ms: u64,
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
        if self.threadgroup_size == 0 || self.threadgroup_size > 1024 {
            return Err(Error::Shape("threadgroup size must be in 1..=1024".to_owned()));
        }
        if self.threads == 0 || !self.threads.is_multiple_of(self.threadgroup_size) {
            return Err(Error::Shape(format!("threads must be a positive multiple of {}", self.threadgroup_size)));
        }
        if self.pool_fast && self.private_bytes < 256 {
            return Err(Error::Shape("pool-fast requires at least 256 private bytes".to_owned()));
        }
        let gpu = Gpu::new()?;
        let source = include_str!("idle_gap.metal").replace("PRIVATE_FLOATS", &(self.private_bytes / 4).to_string());
        let function = if self.pool_fast {
            "private_array_fast"
        } else {
            "private_array"
        };
        let pipelines = gpu.pipelines(&source, &[function])?;
        let sink_bytes =
            self.threads.checked_mul(size_of::<f32>()).ok_or_else(|| Error::Shape("sink size overflow".to_owned()))?;
        let sink = if self.pool_probe {
            gpu.shared_buffer(sink_bytes)?
        } else {
            gpu.private_buffer(sink_bytes)?
        };
        println!(
            "device: {}, private storage: {} B per thread, {} threads, threadgroup size {}",
            gpu.name(),
            self.private_bytes,
            self.threads,
            self.threadgroup_size
        );
        if self.pool_probe {
            if self.pool_pre_hold_ms > 0 {
                println!("POOL_BEFORE pid={}", std::process::id());
                std::io::stdout().flush()?;
                thread::sleep(Duration::from_millis(self.pool_pre_hold_ms));
            }
            let sample_salt = 1u32;
            let started_unix_ns = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let started = Instant::now();
            gpu.run_untimed(|encoder| {
                encoder.set_compute_pipeline_state(&pipelines[0]);
                encoder.set_buffer(Some(&*sink), 0, 0);
                encoder.set_bytes(NonNull::from(&sample_salt).cast(), size_of::<u32>(), 1);
                encoder.dispatch_threads(MTLSize::new(self.threads, 1, 1), MTLSize::new(self.threadgroup_size, 1, 1));
            })?;
            let completed_unix_ns = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            if self.pool_fast {
                check_reference_fast(&sink, self.threads, sample_salt)?;
            } else {
                check_reference(&sink, self.threads, self.private_bytes / 4, sample_salt)?;
            }
            println!("POOL_DISPATCH_SECONDS sample={sample_salt} elapsed={:.6}", started.elapsed().as_secs_f64());
            println!(
                "POOL_DISPATCH_UNIX_NS sample={sample_salt} started={started_unix_ns} completed={completed_unix_ns}"
            );
            println!("POOL_READY pid={}", std::process::id());
            std::io::stdout().flush()?;
            thread::sleep(Duration::from_millis(self.pool_hold_ms));
            return Ok(());
        }
        let mut salt = 0u32;
        let mut run = || {
            salt = salt.wrapping_add(1);
            gpu.timed(|encoder| {
                encoder.set_compute_pipeline_state(&pipelines[0]);
                encoder.set_buffer(Some(&*sink), 0, 0);
                encoder.set_bytes(NonNull::from(&salt).cast(), size_of::<u32>(), 1);
                encoder.dispatch_threads(MTLSize::new(self.threads, 1, 1), MTLSize::new(self.threadgroup_size, 1, 1));
            })
        };
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

fn check_reference(
    sink: &Retained<ProtocolObject<dyn MTLBuffer>>,
    threads: usize,
    count: usize,
    salt: u32,
) -> Result<(), Error> {
    let output = sink.contents().as_ptr().cast::<f32>();
    for index in [0, threads / 2, threads - 1] {
        let actual = unsafe { *output.add(index) };
        let expected = reference_value(index as u32, count, salt);
        if !actual.is_finite() || (actual - expected).abs() > expected.abs().max(1.0) * 1e-5 {
            return Err(Error::Reference(format!("thread {index}: GPU={actual}, CPU={expected}")));
        }
    }
    Ok(())
}

fn reference_value(
    index: u32,
    count: usize,
    salt: u32,
) -> f32 {
    let mut values = (0..count).map(|i| ((i as u32) ^ index ^ salt) as f32).collect::<Vec<_>>();
    let mut sum = 0.0f32;
    for i in 0..64u32 {
        let k = (index.wrapping_mul(7).wrapping_add(i * 13).wrapping_add(salt) as usize) % count;
        sum += values[k];
        values[(k * 3 + 1) % count] = sum;
    }
    sum
}

fn check_reference_fast(
    sink: &Retained<ProtocolObject<dyn MTLBuffer>>,
    threads: usize,
    salt: u32,
) -> Result<(), Error> {
    let output = sink.contents().as_ptr().cast::<f32>();
    for index in [0, threads / 2, threads - 1] {
        let actual = unsafe { *output.add(index) };
        let expected = (0..64u32).rev().fold(0.0f32, |sum, i| sum + (i ^ index as u32 ^ salt) as f32);
        if !actual.is_finite() || (actual - expected).abs() > expected.abs().max(1.0) * 1e-5 {
            return Err(Error::Reference(format!("thread {index}: GPU={actual}, CPU={expected}")));
        }
    }
    Ok(())
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
