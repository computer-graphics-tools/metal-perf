# Profiling playbook

Profiling explains a result. It does not decide whether a change is faster: uninstrumented, calibrated timing does that ([Autotuning](autotuning.md)). macOS 27 adds scriptable GPU tools: `gpucapture` records a trace from a running process, and `gpudebug` replays and profiles that trace from the command line. Use them on one bounded workload, write down what each number covers, and keep them out of acceptance runs.

## Discover before using

Tool syntax changes between OS and Xcode releases. At the start of every investigation, run these and read the output. Do not rely on memory or on this page.

```sh
xcrun --find gpucapture; xcrun --find gpudebug; xcrun --find metalperftrace
man gpucapture | col -b; man gpudebug | col -b; man metalperftrace | col -b
gpucapture --help; gpudebug --help; metalperftrace <collect|overview|listen|setup> --help
xcrun xctrace list templates; xcrun xctrace list instruments
```

The versions observed on 2026-10-10 are in [toolchain.txt](evidence/2026-10-10/toolchain.txt): gpucapture 2027.0.39, gpudebug 1.0, metalperftrace (man page dated May 7, 2026), xctrace 27.0. Apple's guidance is [Investigating GPU issues with AI agents](https://developer.apple.com/documentation/xcode/investigating-gpu-issues-with-ai-agents) and the [Metal developer tools](https://developer.apple.com/metal/tools/) page. Apple's [Game Porting Toolkit skills](https://github.com/apple/game-porting-toolkit/tree/4f42344c90be22112f46becc3fb9687f423f2f88/game-porting-skills/skills) `using-gpucapture`, `using-gpudebug`, `using-metal-validation`, `managing-metal4-synchronization`, `creating-metal4-shader-pipelines` and `managing-metal4-resources` apply to compute work. The porting workflow skills do not. Read them at the pinned commit.

The [`metal-perf`](https://github.com/computer-graphics-tools/metal-perf) CLI wraps the manual procedures below: `capture`, `profile`, `shaders` and `counters` for traces, `extract`, `lines`, `inlines` and `air` for compiled code, and `ab run` for interleaved timing. Run `metal-perf <command> --help` for the current options.

## What each instrument measures

| Measurement | Instrument | Unit | Attribution / coverage | Perturbation | Not available from it |
| --- | --- | --- | --- | --- | --- |
| Commit GPU span | `MTL4CommitFeedback` `gpuEndTime − gpuStartTime` | seconds | Everything in one commit, including idle gaps between dispatches | None beyond the commit itself | Per-dispatch time; CPU encode time |
| Dispatch/encoder spans | `MTL4CounterHeap` timestamps, `writeTimestamp(granularity:)` | GPU ticks; 24 MHz here, read `queryTimestampFrequency()` | Between stamp points; `relaxed` may group a pass, `precise` may split encoders. Stage-boundary support does not mean per-dispatch sampling (dispatch-boundary sampling is false here) | Observer effect: a timestamp write waits for prior work to finish, and `precise` may split command encoders (`MTL4Counters.h`, `MTL4ComputeCommandEncoder.h`). Diagnostic until the [observer-effect gate](autotuning.md#observer-effect-gate-for-timestamp-timing) passes | Anything inside a dispatch; the unmodified workload's timing |
| Host encode / submit cost | `Instant` around encode; xctrace `Time Profiler` | seconds | CPU thread | Low (sampling) | GPU time |
| Live CPU/GPU timeline | xctrace `Metal System Trace` | intervals | Command buffers and encoders of a live run | Moderate; tracing overhead | Per-shader counters |
| Per-dispatch/shader cost, counters, heat maps | `gpucapture` → `gpudebug profile run` / `profile load` | tool-defined; read each counter's unit | Replayed capture on this device; `--exec serial` or `overlapping` | Replay, not the live run; `--gpu-state` selects the GPU performance state, not cache warmth | Live-run timing; production cache state |
| Live counter samples | xctrace `Metal GPU Counters` instrument | per counter | Sampled time series | Unknown; not exercised here | Exact per-dispatch attribution |
| Per-layer frame metrics | `metalperftrace collect / overview --json / listen --json` | frame/layer metrics | `CAMetalLayer` sessions | Always-on recording | Headless compute dispatches (no layer); accelerator activity |
| API / shader misuse | `MTL_DEBUG_LAYER=1`, `MTL_SHADER_VALIDATION=1` (+ `MTL_SHADER_VALIDATION_REPORT_TO_STDERR=1`) | errors | Every validated call/pipeline | Large for shader validation | Performance of any kind |
| Programmatic hardware counters | `MTLDevice.counterSets` | n/a | Only `timestamp`, stage-boundary sampling, on this M5 Max | — | Occupancy, stalls, cache/DRAM traffic: **unavailable** via public API |
| Coarse GPU activity, nominal frequency, modeled power, temperature | pinned `macmon pipe` (no root; see [M5 hardware](m5-hardware.md#gpu-telemetry-observed-without-root-macmon-pilot)) | ratio, MHz, W, °C | Whole GPU, averaged over each `-i` interval (temperature is a snapshot) | A separate sampling process adds CPU load | Per-dispatch attribution, ALU/MXU utilization, calibrated physical power, RAM/GPU-SRAM power on macOS 27 |

Keep the quantities apart. Register statistics, cache hit rates, DRAM traffic and ALU or accelerator activity are different measurements with different coverage. A high "GPU active" fraction means work was scheduled. It does not mean the Neural Accelerators were busy. High occupancy means many SIMD groups were resident. It does not mean they made progress.

## Bounded capture of one matmul workload

Capture exactly the command buffers under investigation, from the exact process. Store traces in an owned experiment directory.

```sh
EXP=<experiment directory>
MTL_CAPTURE_ENABLED=1 MTLCAPTURE_WAIT_FOR_SIGNAL=1 <workload command> &
PID=$!
gpucapture boundaries --pid "$PID"           # pick the Device/Queue boundary id
gpucapture start --pid "$PID" --boundary <id> --count 1 --output "$EXP/matmul.gputrace"
```

- `--count` is the number of complete command buffers for Device/Queue boundaries. Bound it to the dispatches of interest.
- `MTLCAPTURE_WAIT_FOR_SIGNAL=1` holds the process at `MTLDevice` creation until `start` attaches. Use it for short CLI runs.
- `metal-perf capture --out "$EXP/run.gputrace" -- <workload command>` does the same with `gpucapture start --until-exit`: every command buffer until the process exits lands in the trace, so keep the workload small.
- `gpucapture stop --all` stops captures in every capturable process. Stop only your own PID.

## Non-interactive `gpudebug`

Open one session, reuse it, and terminate only that session. Navigate one step per `-c`, starting from `go /`. The man page documents multi-segment paths (`go cb0/re0/draw3`), but in our Xcode 27 sessions a path such as `go /performance/shaders` returned an error.

```sh
gpudebug -q --json -t "$EXP/matmul.gputrace" -o "$EXP/fetch" -c "list"   # prints the session id
gpudebug -s <id> --json -c "status"
gpudebug -s <id> --json -c "profile list"
gpudebug -s <id> --json -c "profile run --gpu-state high --exec serial"
gpudebug -s <id> --json -c "go /" -c "go performance" -c "go shaders" -c "list --all"
gpudebug -s <id> --json -c "go /" -c "go performance" -c "go timeline" -c "go counters" -c "list --all"
gpudebug --terminate <id>
```

`metal-perf profile "$EXP/matmul.gputrace" --out "$EXP/prof"` runs this sequence, waits for the tree, and fetches shader timelines, every counter group and compiler statistics; `metal-perf shaders` and `metal-perf counters` then summarize the directory.

- Follow the actions each node advertises in `list`/`go` output. Apple's agent guide describes `gpudebug` as a "self-discoverable interface" and advises: "point them at man gpudebug and a .gputrace file".
- `profile run` collects new data on the device. The man page says it requires M3/A17 or later. `profile load` reads data already embedded in the trace. `fetch` writes counter time series as JSON and buffers as `.bin` into `-o`.
- The performance tree fills asynchronously after `profile run` returns. Before it is filled, it can show "sorted by cost" with no counts and empty counter lists. Check structurally: wait until `performance` → `commands` lists `disp0` and `go shaders` → `list --all` is non-empty.
- A failed command prints `error: ...` and still exits 0, so check the output. `fetch <counter> --out <path>` needs an absolute path without spaces. `list` shows 20 children; use `list --all`.
- A shader-statistics threadgroup-memory field of 0 does not override the pipeline's `staticThreadgroupMemoryLength`.
- Name the kernel, pipeline, device, capture and profile settings next to every number you copy out.

Observations from the first counter profiles ([evidence](evidence/2026-10-10/mpp-first-profile.json), 2026-10-10, diagnostic replays):

- A dense BF16 MPP matmul, M64×N4096×K4096, showed Neural Accelerator utilization 41.09% (limiter 43.04%) and 80 spilled bytes. A dense MPP kernel like this is a useful positive control that the accelerator counters work.
- The profile requested `--gpu-state default`; the trace recorded performance state `medium`.
- A counter that reads 0 is a measured zero, not a missing counter.
- Recording `gpuStartTime` and `gpuEndTime` from `MTL4CommitFeedback` for every timed commit gives the GPU idle gap between consecutive commits (next start minus previous end) without adding timestamp commands. This is a gap measurement, not evidence about clocks.

## Order of investigation

1. **Arithmetic first.** Compute bytes and useful operations (FLOP or FLOP-equivalent) for the shape ([Matmul](matmul.md)), and compare the measured commit time with the empirical model. It is an estimate, not a floor.
2. **Timestamp heaps** to split one commit into dispatch spans when a commit holds several kernels. The stamps change the workload, so read the spans as qualified diagnostics.
3. **One bounded capture plus `gpudebug profile`** (`metal-perf capture`, `profile`, `shaders`, `counters`) when a dispatch is slower than the empirical model predicts and the reason is unclear: shader cost ranking, counters and heat maps.
4. **Compiler artifacts** ([compiler](compiler.md); `metal-perf extract`, `lines`, `inlines`, `air`) when the profile points at spills, unrolling or instruction selection.
5. **Back to uninstrumented A/B** (`metal-perf ab run`) for any change the diagnosis suggests.

## Separating diagnosis from acceptance

- Never time a run that has capture, validation or tracing enabled. A run with `MTL_CAPTURE_ENABLED=1` in its environment is a diagnostic run, even if no capture triggered.
- Cold dispatches, replayed profiles and compiler statistics can generate a hypothesis. They cannot accept one.
- System telemetry is optional context, never a timing input. If it is used, run a finite `macmon pipe -s <n> -i <ms> --soc-info` (macmon 0.9.0, pinned in [M5 hardware](m5-hardware.md#gpu-telemetry-observed-without-root-macmon-pilot)) either just before and just after an acceptance block, or inside a run that is labeled diagnostic. Never sample during acceptance. Primary GPU timing stays on `MTL4CommitFeedback` spans. Counters, activity ratios and power readings can explain a result. They do not prove a speedup.
- Retain diagnostic traces next to, but separate from, timing records. Each one is linked to the manifest of the candidate it diagnosed.
- If a tool or capability fails, record the exact command and error and say which fallback you used. Never leave a missing measurement blank or fill it with an estimate.
