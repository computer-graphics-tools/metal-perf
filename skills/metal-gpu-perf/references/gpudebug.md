# gpudebug and the profile directory

Verified with Xcode 27 on macOS 27 (M5 Max), October 2026.

## What `metal-perf profile` does

1. It opens a session with `gpudebug -q --timeout 0 -t X.gputrace -c list`, or attaches to
   `--session N`. It waits until `gpudebug -l` lists the session as `ready`.
2. It runs `profile run --gpu-state high --exec serial`. Serial execution isolates every dispatch,
   and the high performance state pins clocks. This takes seconds for a 100 MB trace and minutes
   for a 20 GB one.
3. The performance tree fills in asynchronously after `profile run` returns. It polls until
   `performance/commands` lists `disp0`.
4. It writes the profile directory:

   | File | Content | Time base |
   | --- | --- | --- |
   | `timeline_encoders.json` | encoder name, begin, end, duration | encoder time, continuous from 0 |
   | `timeline_shaders.json` | shaders on the timeline | |
   | `shaders.json` | shaders sorted by cost, with producer-reported invocation counts | |
   | `shader__compN.json` | `{name, intervals: [{begin_ns, end_ns}]}` | separate, with gaps between command buffers |
   | `compiler_statistics.json` | `info compN --all` for the `--top` costliest shaders: instructions, ALU, FP16/FP32/INT16/INT32, device loads and stores, temp and uniform registers, spilled bytes, threadgroup memory | |
   | `counters/<group>__<counter>.json` | `{name, unit, average, samples: [{t, v}]}`, about every 20 µs | encoder time |

5. It closes a session it created, unless `--keep-open` is given.

`metal-perf shaders` splits the shader intervals into encoders at the N−1 largest gaps. N is the
number of encoders. `metal-perf counters` shifts each encoder's intervals by
`encoder.begin − first interval begin` so they line up with the counter samples.

The CLI reports merged shader timeline intervals: `merged intervals` in the encoder summary,
`intervals=N` and `mean interval` per shader, and `intervals` in counter groups. These are separate
from API dispatches and producer-reported shader invocation counts.

## The tree, for manual queries

Navigate one step per `-c`: `-c "go /" -c "go performance" -c "go shaders" -c "list --all"`. A
path such as `go /performance/shaders` is an error. With `--json` every command prints one JSON
document. Listings are single lines, and `info` is pretty-printed over several lines.

```
/
├── commands, api_calls, resources
└── performance              (after `profile run`)
    ├── encoders, commands   (dispN sorted by cost; each has a `d` link such as commands/cb0/ce0/disp1)
    │   └── dispN/counters/compute_kernel   per-dispatch counters (kernel_invocations, ...)
    ├── shaders              compN: function, pipeline, cost %, invocations; `info compN --all`
    └── timeline
        ├── encoders
        ├── shaders          compN: `fetch compN --out file.json`
        └── counters/<group> `fetch <counter> --out file.json`
```

Counter groups seen on M5 include `bandwidth`, `gpu_peak_bandwidth`, `occupancy`,
`occupancy_manager`, `occupancy_target_influences`, `instruction_throughput`,
`shader_launch_limiter`, `alu`, `f32`, `f16`, `integer_and_conditional`, `integer_and_complex`,
`neural_accelerator`, `control_flow`, `address_generation`, `mmu`, `last_level_cache`,
`l2_bandwidth`, `l1_cache`, `l1_read_bandwidth`, `l1_residency`, `simd_groups_inflight_per_core`
and `active_cores`. List them with `go counters` + `list --all`. They differ between GPU
generations.

Per-dispatch counter nodes (`commands/dispN/counters/compute_kernel`) also exist. The GEMM
instruction count there looks scaled, so compare it only relatively.

## Quirks

- **Errors exit 0.** A failed command prints `error: ...` and still exits successfully. Check the
  output. `metal-perf` turns those lines into errors.
- **Paths cannot contain spaces.** The command parser splits `fetch X --out "/a b/c"` at the space,
  and quotes do not help. `metal-perf` fetches into a temporary path, then moves the file.
- **Several fetches in one invocation do not all complete.** Use one fetch per call.
- **Device names contain spaces.** Parse `gpudebug -l` rows by their first word (the id) and look
  for the status word `ready` or `error`, not by column position.
- **`--terminate` can succeed while `-l` still shows the session as `error`.** The next `-l` is
  clean.
- **Sessions are anchored to the parent process.** A session opened by an agent's shell lives
  until it is terminated or that process exits.
- **One API dispatch can have multiple timeline intervals.** Merging intervals less than 50 ns
  apart is a heuristic, not a dispatch mapping. A validated one-dispatch copy reports invocation
  count 1 but two intervals separated by 320 ns; both remain after merging. The split cause is unknown.
- **Short kernels get few counter samples.** At about 20 µs between samples, a 5 µs kernel may get
  none (the column shows NaN). Treat columns with few samples as unreliable.
- **Large traces work.** 15–27 GB traces replay on a 128 GB machine. `profile run` then takes
  80–210 s.
- **Heatmaps:** `performance/commands/dispN` offers a fetchable `heatmap/compute` PNG.

## Capturing without metal-perf

```sh
MTL_CAPTURE_ENABLED=1 MTLCAPTURE_WAIT_FOR_SIGNAL=1 ./workload &
gpucapture list                       # wait until the PID appears
gpucapture start --pid <PID> --until-exit --output run.gputrace
```

With `MTLCAPTURE_WAIT_FOR_SIGNAL=1` the process waits for the capture to start, so nothing is
missed. `gpucapture start --count N --boundary ID` captures N iterations of a boundary object
instead: list boundaries with `gpucapture boundaries --pid P`.
