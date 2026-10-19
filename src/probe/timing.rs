pub struct Timing {
    pub wall_seconds: f64,
    pub gpu_seconds: f64,
}

impl Timing {
    /// Wall time not spent executing on the GPU: submission, scheduling and host-side preparation.
    pub fn host_seconds(&self) -> f64 {
        self.wall_seconds - self.gpu_seconds
    }
}
