use std::{num::NonZeroUsize, time::Instant};

use metal::prelude::*;

use super::{Error, timing::Timing};

pub struct Gpu {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
}

impl Gpu {
    pub fn new() -> Result<Self, Error> {
        let device = <dyn MTLDevice>::system_default().ok_or(Error::NoDevice)?;
        let queue = device.new_command_queue().ok_or(Error::Allocation("command queue"))?;
        Ok(Self {
            device,
            queue,
        })
    }

    pub fn name(&self) -> String {
        self.device.name()
    }

    pub fn pipelines(
        &self,
        source: &str,
        functions: &[&str],
    ) -> Result<Vec<Retained<ProtocolObject<dyn MTLComputePipelineState>>>, Error> {
        let library = self.device.new_library_with_source(source, None)?;
        functions
            .iter()
            .map(|&name| {
                let function =
                    library.new_function_with_name(name).ok_or_else(|| Error::MissingFunction(name.to_owned()))?;
                Ok(self.device.new_compute_pipeline_state_with_function(&function)?)
            })
            .collect()
    }

    pub fn private_buffer(
        &self,
        length: usize,
    ) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>, Error> {
        self.device.new_buffer(length, MTLResourceOptions::STORAGE_MODE_PRIVATE).ok_or(Error::Allocation("buffer"))
    }

    pub fn timed(
        &self,
        encode: impl Fn(&ProtocolObject<dyn MTLComputeCommandEncoder>),
    ) -> Result<Timing, Error> {
        let command_buffer = self.queue.command_buffer().ok_or(Error::Allocation("command buffer"))?;
        let encoder = command_buffer.compute_command_encoder().ok_or(Error::Allocation("compute encoder"))?;
        encode(&encoder);
        encoder.end_encoding();
        let start = Instant::now();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        let wall_seconds = start.elapsed().as_secs_f64();
        if let Some(error) = command_buffer.error() {
            return Err(Error::CommandBuffer(error));
        }
        Ok(Timing {
            wall_seconds,
            gpu_seconds: command_buffer.gpu_end_time() - command_buffer.gpu_start_time(),
        })
    }

    pub fn fastest_seconds(
        &self,
        repeats: NonZeroUsize,
        encode: impl Fn(&ProtocolObject<dyn MTLComputeCommandEncoder>),
    ) -> Result<f64, Error> {
        let mut fastest = f64::INFINITY;
        for _ in 0..repeats.get() {
            fastest = fastest.min(self.timed(&encode)?.gpu_seconds);
        }
        Ok(fastest)
    }
}
