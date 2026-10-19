use clap::Subcommand;

use crate::{
    Error,
    ab::AbCommand,
    air::AirArgs,
    cache::ExtractArgs,
    capture::CaptureArgs,
    dwarf::{InlinesArgs, LinesArgs},
    gpudebug::ProfileArgs,
    probe::{AccessPatternArgs, BandwidthArgs, IdleGapArgs},
    profile::{CountersArgs, ShadersArgs},
    xctrace::XctraceArgs,
};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Measure peak GPU read and copy bandwidth: the roofline ceiling
    Bandwidth(BandwidthArgs),
    /// Compare row-wise and tiled weight read orders over the same bytes
    ///
    /// `rowwise`: lane l of simdgroup s streams row 32s+l, like an MXU fragment cursor over a
    /// row-major weight matrix. `tiled`: each block of 32 rows x step bytes is contiguous and the
    /// lanes read consecutive 16-byte chunks. Both read every byte once.
    AccessPattern(AccessPatternArgs),
    /// Time the first command buffer after idle periods against back-to-back ones
    ///
    /// The kernel uses a dynamically indexed thread-local array, so the driver backs it with a
    /// private-memory pool that it shrinks while the GPU is idle and regrows on the host before the
    /// next command runs. Reports the extra GPU time and the extra host time per idle period.
    IdleGap(IdleGapArgs),
    /// Launch a command with Metal capture enabled and record a .gputrace until it exits
    ///
    /// Sets MTL_CAPTURE_ENABLED=1 and MTLCAPTURE_WAIT_FOR_SIGNAL=1, waits until the process is
    /// listed by `gpucapture list`, then runs `gpucapture start --until-exit`. Keep the workload
    /// small: every command buffer of the process lands in the trace.
    Capture(CaptureArgs),
    /// Replay a .gputrace with gpudebug and fetch timelines, counters and compiler statistics
    Profile(ProfileArgs),
    /// Per-encoder, per-shader GPU time from a fetched profile
    Shaders(ShadersArgs),
    /// Timeline counters averaged over the merged intervals of selected shaders
    Counters(CountersArgs),
    /// Extract compiled GPU binaries from the Metal shader cache
    Extract(ExtractArgs),
    /// Code bytes per source line from the DWARF line table of a GPU binary
    Lines(LinesArgs),
    /// Exclusive code bytes per inlined function from the DWARF of a GPU binary
    Inlines(InlinesArgs),
    /// AIR call trees and intrinsic counts of a metallib, AIR archive or LLVM IR file
    Air(AirArgs),
    /// Interleaved A/B benchmark runs with cooldowns, warm-up discard and paired ratios
    #[command(subcommand)]
    Ab(AbCommand),
    /// Filter, group and sum an `xctrace export` table
    Xctrace(XctraceArgs),
}

impl Command {
    pub fn run(self) -> Result<(), Error> {
        match self {
            Self::Bandwidth(arguments) => arguments.run()?,
            Self::AccessPattern(arguments) => arguments.run()?,
            Self::IdleGap(arguments) => arguments.run()?,
            Self::Capture(arguments) => arguments.run()?,
            Self::Profile(arguments) => arguments.run()?,
            Self::Shaders(arguments) => arguments.run()?,
            Self::Counters(arguments) => arguments.run()?,
            Self::Extract(arguments) => arguments.run()?,
            Self::Lines(arguments) => arguments.run()?,
            Self::Inlines(arguments) => arguments.run()?,
            Self::Air(arguments) => arguments.run()?,
            Self::Ab(command) => command.run()?,
            Self::Xctrace(arguments) => arguments.run()?,
        }
        Ok(())
    }
}
