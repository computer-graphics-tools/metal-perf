//! Drives `gpudebug` (Xcode 27+) non-interactively: replay, profile and fetch a .gputrace.

mod error;
mod profile_args;
mod session;

pub use error::Error;
pub use profile_args::ProfileArgs;
