# Run conditions

Power, temperature, background activity and resource reuse can change the time of an unchanged kernel. Record conditions before and after a comparison so that a candidate is compared with its baseline under the same policy.

## Condition record

| Condition | Read-only source | Interpretation |
| --- | --- | --- |
| Power source | First line of `pmset -g batt` | AC and battery runs are different conditions |
| Energy mode | `pmset -g custom` | Preserve the raw setting; do not infer an undocumented numeric meaning |
| Low Power Mode | `ProcessInfo.isLowPowerModeEnabled` | Record explicitly |
| Thermal state | `ProcessInfo.thermalState` | Coarse OS state, not a clock measurement |
| Thermal/performance warnings | `pmset -g therm` | Missing warnings do not prove stable frequency |
| Other GPU work | Process inventory, GPU history or a separate system trace | Desktop and other applications can interfere |
| Device/software identity | Device query, OS build, SDK and compiler versions | Results are build- and device-scoped |
| Resource reuse | Workload manifest | Reusing the same buffers can change cache behavior |
| Capture/validation state | Process environment and harness configuration | Instrumented runs are diagnostics |

Store the power-source line without battery identifiers. Hardware serial numbers and user identifiers are unnecessary for performance records.

## Acceptance conditions

Use the declared power policy consistently, with Low Power Mode off and a stable thermal policy. Interleave baseline and candidate runs, include an unchanged-baseline control, and cool down according to the recorded protocol.

Retain runs that violate the policy and mark them invalid with the reason. Do not silently remove unfavorable results. Do not change fans, power caps or system settings to repair a comparison without an explicit experiment contract.

## Frequency and power readings

| Reading | What it can describe | What it cannot establish |
| --- | --- | --- |
| Nominal DVFS table | Available operating-point labels | Instantaneous measured frequency |
| Interval-average frequency | Averages over the tool's sampling definition | Constant clocks throughout a short dispatch |
| GPU active ratio | Activity under the producer's definition | ALU/MXU utilization or occupancy |
| Modeled GPU power | Tool-provided energy estimate over an interval | Calibrated physical power |
| Missing / unsupported energy channel | Unavailable information | Zero consumption |
| Temperature average | Selected sensors at a moment or interval | Full-chip temperature or absence of throttling |

Collect optional telemetry outside acceptance timing unless its observer effect has been characterized. Per-second system telemetry cannot attribute a microsecond shader bottleneck.

## Profiling performance state

The installed debugger supports `profile run --gpu-state default|low|medium|high --exec overlapping|serial`. `metal-perf profile` selects high state and serial execution to obtain diagnostic attribution.

These are named profiling states, not requested MHz. Their exact frequency, thermal behavior and restoration behavior are not established here. Profiled timing remains separate from [acceptance timing](autotuning.md).

## Host time

Report pipeline creation, host encoding, submission/wait overhead and GPU execution separately. Compilation can improve startup without changing dispatch performance. A lower GPU span can leave application latency unchanged when another stage dominates.
