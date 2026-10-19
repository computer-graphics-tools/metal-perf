# Profiling compute kernels

Use uninstrumented timing to establish the cost, then a bounded capture to locate and explain it. Return to uninstrumented comparison to decide whether a change helps.

## Instrument selection

| Question | Instrument | Output / scope |
| --- | --- | --- |
| How fast can this layout stream? | `metal-perf bandwidth`, `access-pattern` | Matching empirical control |
| Which shaders account for recorded time? | `metal-perf capture`, `profile`, `shaders` | Shader timeline attribution; API dispatch counts require the command tree |
| What resource pressure accompanies the cost? | `metal-perf counters` | Counter samples in the selected interval |
| Where does generated code come from? | `extract`, `lines`, `inlines` | Compiled bytes and debug attribution |
| What does an MPP call request? | `metal-perf air` | AIR functions and intrinsic calls |
| Does the CPU leave gaps? | Metal System Trace plus application feedback | System/process timeline, with observer overhead |
| What happens to presented frames? | `metalperftrace` | Layer/frame trends, not a headless per-dispatch compute metric |
| Is a candidate faster? | `metal-perf ab run` | Repeated, paired, uninstrumented comparison |

Discover the current tools and read their installed manuals before a new investigation:

```sh
xcrun --find gpucapture
xcrun --find gpudebug
xcrun --find metalperftrace
man gpucapture
man gpudebug
man metalperftrace
```

Apple's [GPU tools page](https://developer.apple.com/metal/tools/) describes the capture, debugging and profiling roles. The [agent investigation guide](https://developer.apple.com/documentation/xcode/investigating-gpu-issues-with-ai-agents) describes command-line use.

## Bounded capture

Prefer an existing application capture hook when it can select the exact operator. Otherwise identify the exact process and command-buffer boundary:

```sh
MTL_CAPTURE_ENABLED=1 MTLCAPTURE_WAIT_FOR_SIGNAL=1 <workload command> &
workload_pid=$!
gpucapture boundaries --pid "$workload_pid"
gpucapture start --pid "$workload_pid" --boundary <id> --count 1 --output <absolute trace path>
```

The wait-for-signal option holds device creation for attachment. A Device/Queue boundary count refers to complete command buffers. A process-wide until-exit capture can collect much more work; use a minimal workload.

Save traces in an owned external experiment directory and stop only the capture created for that experiment.

## Profile and inspect

```sh
metal-perf profile <trace.gputrace> --out <absolute profile directory>
metal-perf shaders <profile directory>
metal-perf counters <profile directory> --encoder <index> --group 'kernel=<pattern>'
```

The wrapper opens one debugger session, profiles serially at high GPU state, fetches data, and closes the session unless asked to keep it open.

For a manual session, start with `list`, reuse the returned session ID with `-s`, and follow advertised `go`, `info` and `fetch` actions. Use non-interactive `-c` commands, JSON where available, and an explicit output directory.

| Debugger behavior | Required handling |
| --- | --- |
| `profile load` | Reads embedded profiling data |
| `profile run` | Collects new data on the device |
| Asynchronously populated tree | Wait for structural readiness; an empty early list is not zero work |
| Textual `error:` with exit status zero | Inspect the output as well as process status |
| Different statistics views | Preserve each source's units and scope |
| Missing counter/resource | Record unavailable; do not substitute zero |
| Session cleanup | Terminate only the session created by the task |

`shaders` reports merged interval counts and mean interval duration; `counters` labels its selected merged shader intervals as `intervals`. Intervals less than 50 ns apart are merged heuristically. Neither the merged count nor its mean establishes API dispatch count or time per dispatch. Read API dispatches from the captured command tree and keep producer-reported shader invocations separate.

## Interpretation order

1. Verify the selected kernel, API dispatch count, reported invocations, timeline interval count, shape and profile mode.
2. Rank actual GPU durations or attributed costs within a common scope.
3. Read utilization and limiter counters for the suspected path.
4. Examine memory interfaces and occupancy influences.
5. Relate live registers, spills and compiled code to the hypothesis.

A compiler instruction count is static. A timeline share is interval-relative. A utilization counter measures a unit under its documented scope. These quantities are not interchangeable. See the [metric glossary](gpu-metrics.md).

Very short dispatches can have insufficient sampling coverage. Repetition can improve coverage while changing cache behavior; record both the repeat policy and its limitation.

## Acceptance boundary

Capture, counter replay, validation, timestamp writes and source instrumentation can change execution. Their timings generate hypotheses. [Calibrated comparisons](autotuning.md) decide performance claims, with [run conditions](telemetry.md) recorded separately.
