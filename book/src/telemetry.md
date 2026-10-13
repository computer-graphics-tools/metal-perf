# Telemetry and run conditions

The same kernel runs at different speeds depending on power source, energy mode, temperature, display activity and other GPU users. A timing record is meaningful only together with the conditions it ran under. This chapter lists what can be read on this Mac without `sudo`, what cannot, and how to record both. It does not change fan, energy-mode or power settings.

## The run-condition record

Collect these before and after every acceptance block. They go into the run manifest ([Autotuning](autotuning.md#manifest)).

| Condition | Command / API (no sudo) | 2026-10-10 value | Status |
| --- | --- | --- | --- |
| Power source | `pmset -g batt` (first line) | AC Power | Readable |
| Energy mode | `pmset -g custom` → `powermode` per source | AC 2, Battery 1 | Readable; pmset(1) does not document `powermode` values, meaning unverified |
| Low Power Mode | `ProcessInfo.isLowPowerModeEnabled` | false | Readable |
| Thermal state | `ProcessInfo.thermalState` (0 nominal … 3 critical) | 0 | Readable; coarse four-level state |
| Thermal / performance warnings | `pmset -g therm` | none recorded | Readable |
| Thermal state over time | xctrace `Thermal State` instrument | — | Available, not exercised |
| Display / sleep assertions | `pmset -g assertions` | display-sleep prevention active | Readable; shows whether display or idle sleep is held |
| Other GPU users | Activity Monitor GPU history; xctrace `Metal System Trace` | — | Readable by inspection; no single CLI number |
| GPU / CPU power and clocks | `powermetrics`; unprivileged `macmon` | See [hardware readings](m5-hardware.md#gpu-telemetry-observed-without-root-macmon-pilot) | `powermetrics` requires root; `macmon` supplies interval estimates without root |
| Process energy | xctrace `Power Profiler` template | — | Available, not exercised; units and coverage to be checked |
| Device identity | see [toolchain.txt](evidence/2026-10-10/toolchain.txt) | M5 Max, 40 cores | Readable |
| Counter support | `MTLDevice.counterSets`, `supportsCounterSampling` | `timestamp` only, stage boundary | Readable |

Do not store serial numbers, hardware UUIDs, battery identifiers or user names in any record. `pmset -g batt` prints a battery id. Keep only its first line.

## Rules for acceptance runs

- AC power, Low Power Mode off, nominal thermal state at the start and end of each block. Record the energy mode exactly as read. Do not assume what `powermode 2` means.
- No concurrent GPU work from other benchmark processes. Desktop apps and the display stay as they are. Note them; do not hide them.
- A block whose before/after thermal state differs, or that ran on battery, is retained and marked **invalid**. It is never silently dropped.
- Sustained load can slow later runs. A community page ([metalworking, "Cheap Tricks"](https://metalworking.vercel.app/war-stories/cheap-tricks/); see the [metalworking review](metalworking-review.md)) reports that pinning fans cut long-run throughput decay from 50% to 6.7%; we have not reproduced this. We do not touch fans. Instead the protocol interleaves candidates and controls in time so that drift hits both alike, and it watches A/A drift across a block ([Autotuning](autotuning.md#timing-protocol)).

## Host-side overhead

Matmul at small M can be limited by the host rather than the GPU. Measure and report separately:

- **Encode time**: wall time around argument setup and encoder calls, from the host thread.
- **Commit-to-start latency**: the gap between commit and `gpuStartTime`, when needed. Convert host and GPU clocks with Apple's [timestamp conversion guidance](https://developer.apple.com/documentation/metal/gpu-counters-and-counter-sample-buffers).
- **Pipeline creation**: one-time `MTL4Compiler` cost per candidate, reported as compile cost and never mixed into dispatch time.

## What telemetry cannot tell you

- `macmon` supplies average nominal GPU frequency, modeled power and temperature readings without root. These do not establish instantaneous frequency, physical power accuracy or Neural Accelerator utilization. Do not infer them from thermal state or timing.
- `metalperftrace` reports per-layer frame metrics for apps that present through `CAMetalLayer`. Headless matmul benchmarks have no layer. Expect no per-dispatch compute data from it, and record "no layer sessions" instead of a zero.
- A nominal thermal state means only that the OS has not escalated. It does not mean clocks are steady.

## M5 Max power control and stable profiling (2026-10-11)

GPU power limits can be read without root through the AGX driver's IOKit properties. Changing them is out of scope for this book. A power limit is not a frequency lock, and nothing below sets a GPU frequency.

### Local evidence

The device is the 40-core M5 Max on macOS 27.0.1, build `26A434`, with driver `com.apple.AGXG17X` version `360.34.5`.

The `perf-states` property of the GPU (`sgx`) node in the device tree has four voltage tables, each with 14 entries including idle. The 13 nonzero nominal frequencies are **338, 486, 636, 796, 888, 988, 1084, 1182, 1278, 1374, 1470, 1578, 1620 MHz** (`gpu-num-perf-states` = 13). These are available operating points, not instantaneous readings or a writable clock API.

### Read-only snapshot

Bulk `ioreg` output does not list the power properties; read each key directly with `IORegistryEntryCreateCFProperty` on the `AGXAccelerator` service. This read calls no setter and no user-client method:

| Property | Raw value | Interpretation |
| --- | ---: | --- |
| `MaxGPUAbsolutePower` | 181795 | Current effective limit; 181.795 W if the unit is milliwatts |
| `PPMGPUPowerMax` | 181795 | Separately exposed power-management limit, same value in these reads |
| `GPUMaxPower` | 203110 | Separate maximum-power value |
| `CLTMPerfMax`, `PPMPerfMax` | 65536 each | Raw performance targets; not MHz |
| `FilteredGPUPower` | Changes between reads | Filtered power estimate, not a calibrated meter |
| `GPUMtrAverageTemp` | unavailable | Preserve missing status; do not interpret as zero |

The same read on 2026-10-13 returned identical values. The value 181795 is the captured state of this machine, **not a portable or factory default**.

### Use Xcode's supported profiling control first

Apple's [GPU performance guide](https://developer.apple.com/documentation/xcode/optimizing-gpu-performance#Gather-performance-data) describes choosing a GPU performance state in the profiler; by default the Metal debugger profiles with the performance state that was active at capture time. The installed `gpudebug` manual advertises:

```text
profile run --gpu-state default|low|medium|high --exec overlapping|serial
```

In the first CLI profile (2026-10-10), `--gpu-state default` was recorded as performance state `medium` ([mpp-first-profile.json](evidence/2026-10-10/mpp-first-profile.json)). `metal-perf profile` uses `--gpu-state high --exec serial`; serial execution isolates each dispatch. Keep the resulting trace/replay separate from uninstrumented acceptance timing. The states are named levels, not a requested MHz. Which frequency each state produces on this M5 Max, whether it holds under heat, and what happens after the profile ends have not been measured.
