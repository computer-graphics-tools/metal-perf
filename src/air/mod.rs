//! AIR (Apple's LLVM IR for Metal) call trees: which `air.*` intrinsics a kernel or a library
//! function such as an MPP tensor op lowers to.

mod air_args;
mod error;
mod ir_function;
mod ir_module;

pub use air_args::AirArgs;
pub use error::Error;
