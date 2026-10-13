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
}
