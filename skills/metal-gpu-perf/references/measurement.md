# Measuring GPU speedups honestly

## The metric

Best to worst:

1. **GPU time per command buffer**, logged by the application. Read `gpuStartTime` and
   `gpuEndTime` in a completion handler and print one line per buffer, tagged with what it did:

   ```
   GPUTIME	step 8	1234567
   ```

   It excludes CPU encoding and scheduling, and it is available in release builds without
   instrumentation. Choose the buffers that matter with the regex. For example,
   `--metric '^GPUTIME\tstep \d+\t(\S+)'` selects every step buffer.
2. **A benchmark's own GPU-side timing**, such as a kernel benchmark that times dispatches with
   command-buffer timestamps.
3. **Wall time or throughput.** These include CPU work, scheduling and synchronization. When the
   workload is data-dependent (for example sampled or speculative LLM decoding), the amount of work
   itself varies between runs. Use them only for end-to-end confirmation, with many rounds.

## The protocol `metal-perf ab run` implements

- **Alternating order (ABBA).** Round 1 runs A then B, round 2 runs B then A. A variant that always
  runs second would run hotter.
- **Cooldown before every run** (`--cooldown`, default 20 s). Apple GPUs and CPUs boost and then
  throttle. Idle time brings them back to a comparable state.
- **Discard warm-up rounds** (`--discard`, default 1). The first run pays page-in of weights,
  shader compilation and pipeline caches.
- **Per-round summaries.** For each run: the median of all samples, plus a tail quantile (p10 when
  lower is better, p90 when higher is better). The tail shows the fast mode of a bimodal
  distribution.
- **Paired ratios.** Each variant is divided by the baseline in the same round, then the geometric
  mean is taken. Pairing cancels slow drifts. Also report the best-run ratio: minimum against
  minimum.

Run at least 4 rounds. Ratios within about ±2% with inconsistent per-round signs are noise. When
small kernels are bimodal, use 8 or more rounds and compare best runs or p10.

Each variant command runs with `sh -c`. `METAL_PERF_VARIANT` and `METAL_PERF_ROUND` are set, so a
command can name its own output files. To compare two builds of the same program, copy each build
to its own path first. A rebuild between runs would change what "A" means.

## Reporting a result

State the following:

- the metric and the regex used;
- the rounds kept and discarded, and the cooldown;
- the geomean paired ratio with the per-round ratios, and the best-run ratio;
- the machine, the OS and Xcode version, and what else was running;
- what was not measured, such as end-to-end throughput, other models or other chips.

## Noise sources worth ruling out

- Another GPU client: a browser, a video call, another benchmark.
- Low Power Mode or a battery-powered laptop.
- Changing the input between variants: prompts, seeds, sampling.
- Debug builds, or shader validation left enabled (`MTL_SHADER_VALIDATION`, `MTL_DEBUG_LAYER`).
- An active GPU capture. The bandwidth probe read 209 GB/s under `gpucapture`, against 587 GB/s
  without it (M5 Max, 2026-10-13).
