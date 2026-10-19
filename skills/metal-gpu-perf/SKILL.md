---
name: metal-gpu-perf
description: Measure, explain and speed up Apple GPU (Metal) compute kernels with evidence instead of guesses. Covers the bandwidth roofline, gpucapture and gpudebug traces, per-shader GPU time, hardware counters and limiters, compiler statistics (registers, spills), DWARF line attribution of compiled GPU binaries, AIR intrinsic call trees (including Metal Performance Primitives and MXU tensor ops), System Trace gaps, and interleaved A/B benchmarks, all through the `metal-perf` CLI. Use it whenever someone works on Metal shader or kernel performance, asks why a GPU kernel is slow or whether it saturates the GPU, mentions a .gputrace, gpudebug, gpucapture, xctrace, Metal System Trace, GPU counters, occupancy, register spills, memory bandwidth, the neural accelerator or MXU, or simdgroup matrix ops, or wants to compare two builds of GPU code on a Mac. Use it even when no tool is named.
---

# Metal GPU performance

GPU performance work goes wrong in two ways. People optimize what they assume is slow, and they
accept speedups that are really noise. This skill avoids both: first locate the cost with the
profiler, then explain it with counters and compiled-code evidence, then accept a change only from
interleaved, uninstrumented runs.

All instruments live in one CLI, `metal-perf`. Its source is at
<https://github.com/computer-graphics-tools/metal-perf>. Check it with `metal-perf --version`. If it
is missing, install it:

```sh
cargo install --git https://github.com/computer-graphics-tools/metal-perf --locked
```

It drives `gpucapture` and `gpudebug` (in `/usr/bin` with Xcode 27 and later), `xcrun metal-objdump`
(the Metal toolchain) and `xctrace`. These Apple tools change with each Xcode release. At the start
of a new investigation, confirm each one with `xcrun --find <tool>` (one name per call) and skim
`<tool> --help`. When `metal-perf` reports an Apple tool failure, the message carries that tool's
output: read it before retrying.

## Which instrument answers which question

| Question | Command |
| --- | --- |
| What is the memory ceiling of this GPU? | `metal-perf bandwidth` |
| Is a different weight layout worth writing? | `metal-perf access-pattern --rows R --row-bytes B` |
| What does the first command after idle cost? | `metal-perf idle-gap --private-bytes N` |
| Record a GPU trace of a run | `metal-perf capture --out run.gputrace -- <command>` |
| Profile the trace and fetch everything | `metal-perf profile run.gputrace --out prof` |
| Which shaders take the time, and their registers and spills? | `metal-perf shaders prof` |
| What limits a kernel: bandwidth, issue, cache, MXU, occupancy? | `metal-perf counters prof --encoder N --group label=regex ...` |
| Which source lines produce the most code? | `metal-perf extract ...` then `metal-perf lines X.bin` |
| Which inlined functions produce the most code? | `metal-perf inlines X.bin` |
| Which `air.*` intrinsics does a kernel or an MPP op lower to? | `metal-perf air <metallib or rtlib> --function regex --depth N` |
| Is the CPU leaving the GPU idle between command buffers? | `xctrace record` (System Trace), then `metal-perf xctrace` |
| Is change B faster than A? | `metal-perf ab run --variant a=... --variant b=... --metric regex --out dir` |

## Workflow

1. **Establish the roofline.** Run `metal-perf bandwidth` once per machine. A decode-style kernel
   that streams weights is memory-bound. Its target is bytes moved ÷ measured peak read bandwidth.
   Compute that number before tuning anything, because it shows how much headroom exists.
2. **Get a clean baseline.** Use the uninstrumented release build of the real workload. Prefer GPU
   time per command buffer (`gpuEndTime - gpuStartTime`) logged by the application over wall time
   or tokens per second. See `references/measurement.md`.
3. **Capture a small trace.** Use one or two iterations: every command buffer lands in the trace,
   and profiling time grows with it. Use `metal-perf capture`, or the application's own capture
   hook (`MTLCaptureManager`) when it has one.
4. **Profile and break down.** Run `metal-perf profile`, then `metal-perf shaders`. Take the top
   kernels by total time per encoder. Note their `regs` and `spill` columns: spills show up as
   `stack` L1 traffic and usually cost more than they look.
5. **Explain each hot kernel with counters.** Run `metal-perf counters` with one `--group` per
   kernel family and compare the columns. How to read the limiters is below. Kernels shorter than
   about 40 µs get unreliable windows: check each counter cell's sample count and unit.
6. **Go below the source.** For a kernel whose counters point at instruction issue, use `extract`,
   `lines` and `inlines` to find the source lines and inlined helpers that generate the most code.
   Use `air` to see which intrinsics are actually emitted, for example whether a matmul reaches the
   MXU (`air.simdgroup_matrix_16x16x16_*`) or falls back to ALU code. See
   `references/instruction-level.md`.
7. **Form a hypothesis, change one thing, measure.** Validate a layout idea with `access-pattern`
   before implementing it. Check the result with `metal-perf ab run`: at least 4 rounds, a cooldown
   before each run, round 1 discarded, paired ratios. Report the geomean ratio, the per-round
   ratios and the best-run ratio together.
8. **Close what you opened.** `metal-perf profile` closes the `gpudebug` session it created unless
   you pass `--keep-open`. Never terminate sessions you did not create, and never run
   `gpudebug --terminate all`.

## Reading the counters

The interesting counters are limiters: the percentage of time a hardware unit was the bottleneck.

- **`bandwidth/gpu_read_bandwidth` near the roofline** (above about 75%): memory-bound. Speedups
  come from moving fewer bytes (quantization, fusion, avoiding re-reads) or from friendlier access
  patterns (`last_level_cache_limiter`, `mmu_limiter`). Fewer instructions won't help.
- **`instruction_throughput/instruction_throughput_limiter` high while bandwidth is well below the
  roofline**: issue-bound. Count what runs per byte loaded: dequantization math, address math,
  bounds checks, shuffles. `lines` and `inlines` show where the code comes from.
- **`neural_accelerator/neural_accelerator_utilization` and `_limiter`**: how busy the matrix units
  (MXU) are. Low utilization in a matmul that is not memory-bound means the MXU is starved by
  operand preparation.
- **`occupancy/kernel_occupancy` low with many temp registers**: register pressure limits the
  number of simdgroups in flight, so latency is not hidden. Spilled bytes in `shaders` and nonzero
  `l1_read_bandwidth/stack_l1_read_bandwidth` mean private-memory traffic: spills or
  dynamically indexed private arrays.
- **`l1_cache/l1_cache_limiter`, `address_generation/address_generation_limiter`**: many small or
  unaligned loads. Wider, aligned vector loads usually help.

A common pattern for a small-batch quantized matmul: read bandwidth well below the roofline,
moderate issue and last-level-cache limiters, low MXU utilization and occupancy, and a register
count high enough to spill (nonzero `stack_l1_read_bandwidth`). Such a kernel is neither memory-
nor MXU-bound. Look first at the spills and at whether each lane streams contiguous bytes
(`metal-perf access-pattern` shows what a contiguous tile layout is worth).

## Traps that cost hours

- **Profiling is not acceptance.** Counter profiles, serial replay and cold dispatches distort
  absolute times. Only interleaved, uninstrumented runs prove a speedup.
- **Thermals and page-in.** The first run after a build or a model load pays page-in and shader
  compilation. Back-to-back runs heat the chip, and the later variant loses. ABBA order, cooldowns
  and dropping round 1 handle both.
- **Some small kernels are bimodal.** Run-to-run spread can reach 1.5×. Compare minimums or medians
  over many rounds, never single runs.
- **Metal System Trace inflates CPU–GPU gaps.** Its own overhead appears as idle GPU time. Confirm
  a suspected CPU stall with a System Trace syscall table (`metal-perf xctrace`) before acting on
  it.
- **`metalperftrace` records only CAMetalLayer frame statistics.** It shows nothing for a headless
  compute process.
- **Time Profiler on debug builds** measures debug-only work, such as Objective-C runtime
  verification in binding layers. Profile release binaries.
- **Check the installed object-tool capability.** On the inspected toolchain, `metal-objdump`
  cannot print AGX3 instructions. Use DWARF attribution, compiler statistics and AIR for their
  supported questions; the book scopes the available native instruction forms.
- **The Metal toolchain can vanish after an Xcode update.** If `xcrun metal-objdump` or `metal` is
  missing, run `xcodebuild -downloadComponent MetalToolchain`.

## References

Read only what the task needs:

- `references/gpudebug.md`: what `metal-perf profile` fetches, the profile directory layout, the
  gpudebug tree and its quirks, and manual commands for anything the CLI does not cover.
- `references/measurement.md`: A/B methodology, per-command-buffer timing, choosing a metric
  regex, noise sources, and how to report a result.
- `references/instruction-level.md`: the Metal shader cache, building with line tables, reading
  `lines`, `inlines` and `air` output, MPP tensor-op lowering, and AIR pitfalls.
- `references/apple-gpu-notes.md`: which book chapter answers which hardware question (limits,
  SIMD participation, matrix forms, compilation, counters, acceptance).
- `../../book/src/` (the repository's book): a factual reference for M5 hardware, execution, memory,
  matrix kernels, compilation, submission, driver and firmware internals and performance acceptance. Read
  the chapter that matches the question; start from `SUMMARY.md`.
