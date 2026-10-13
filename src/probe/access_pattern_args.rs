use std::{num::NonZeroUsize, ptr::NonNull};

use clap::Args;
use metal::prelude::*;

use super::{Error, gpu::Gpu};

const ROWS_PER_SIMDGROUP: usize = 32;

#[derive(Debug, Args)]
pub struct AccessPatternArgs {
    /// Matrix rows, a multiple of 32
    #[arg(long, default_value_t = 34816)]
    rows: usize,
    /// Bytes per row, a multiple of 128
    #[arg(long, default_value_t = 2560)]
    row_bytes: usize,
    /// Matrix copies to rotate through so reads miss the system-level cache
    #[arg(long, default_value_t = 12)]
    copies: usize,
    /// Bytes each lane reads per K step, multiples of 16 that divide the row
    #[arg(long, value_delimiter = ',', default_values_t = [16, 32, 64, 128])]
    steps: Vec<usize>,
    /// Timed runs per configuration; the fastest one is reported
    #[arg(long, default_value = "5")]
    repeats: NonZeroUsize,
}

impl AccessPatternArgs {
    pub fn run(&self) -> Result<(), Error> {
        if self.rows == 0
            || !self.rows.is_multiple_of(ROWS_PER_SIMDGROUP)
            || self.row_bytes == 0
            || !self.row_bytes.is_multiple_of(128)
        {
            return Err(Error::Shape("rows must be a multiple of 32 and row bytes a multiple of 128".to_owned()));
        }
        if self.copies == 0
            || self
                .steps
                .iter()
                .any(|&step| step == 0 || !step.is_multiple_of(16) || !self.row_bytes.is_multiple_of(step))
        {
            return Err(Error::Shape("copies must be positive and steps multiples of 16 dividing the row".to_owned()));
        }
        let row_bytes =
            u32::try_from(self.row_bytes).map_err(|_| Error::Shape("row bytes exceed 32 bits".to_owned()))?;
        let matrix_bytes = self.rows * self.row_bytes;
        let simdgroups = self.rows / ROWS_PER_SIMDGROUP;
        let gpu = Gpu::new()?;
        let pipelines = gpu.pipelines(include_str!("access_pattern.metal"), &["rowwise", "tiled"])?;
        let matrices = gpu.private_buffer(matrix_bytes * self.copies)?;
        let sink = gpu.private_buffer(simdgroups * 4)?;
        println!(
            "device: {}, matrix: {} x {} B = {} MiB, copies: {}",
            gpu.name(),
            self.rows,
            self.row_bytes,
            matrix_bytes >> 20,
            self.copies
        );
        for (name, pipeline) in [("rowwise", &pipelines[0]), ("tiled", &pipelines[1])] {
            for &step in &self.steps {
                let shape = [row_bytes, step as u32, 0, 0];
                let seconds = gpu.fastest_seconds(self.repeats, |encoder| {
                    encoder.set_compute_pipeline_state(pipeline);
                    encoder.set_buffer(Some(&*sink), 0, 1);
                    encoder.set_bytes(NonNull::from(&shape).cast(), size_of_val(&shape), 2);
                    for copy in 0..self.copies {
                        encoder.set_buffer(Some(&*matrices), copy * matrix_bytes, 0);
                        encoder.dispatch_threadgroups(
                            MTLSize::new(simdgroups, 1, 1),
                            MTLSize::new(ROWS_PER_SIMDGROUP, 1, 1),
                        );
                    }
                })?;
                let rate = (matrix_bytes * self.copies) as f64 / seconds / 1e9;
                let per_matrix = seconds / self.copies as f64 * 1e6;
                println!("{name:7} bytes/step/lane={step:>4}: {rate:7.1} GB/s ({per_matrix:.1} us per matrix)");
            }
        }
        Ok(())
    }
}
