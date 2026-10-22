#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no Metal device is available")]
    NoDevice,
    #[error("Metal could not create a {0}")]
    Allocation(&'static str),
    #[error("the probe library has no function `{0}`")]
    MissingFunction(String),
    #[error("Metal error: {0}")]
    Metal(#[from] metal::MetalError),
    #[error("the command buffer failed: {0}")]
    CommandBuffer(metal::MetalError),
    #[error("invalid probe shape: {0}")]
    Shape(String),
    #[error("probe output disagrees with the CPU reference: {0}")]
    Reference(String),
    #[error("probe I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("system clock failed: {0}")]
    Clock(#[from] std::time::SystemTimeError),
}
