# Performance comparison

A kernel optimization changes one stated mechanism while preserving the workload contract. The experiment must establish correctness and a repeatable improvement on the route the application uses.

## Workload contract

| Field | Required contents |
| --- | --- |
| Operation | Mathematical result, state updates and fused transforms |
| Shape | Batch/head dimensions, M/N/K, strides, alignment and tails |
| Representation | Input/output/accumulator dtypes, packing, scales and zero points |
| Numerical behavior | Tolerance, reduction order, relaxed precision and exceptional-value policy |
| Route | Pipeline/function constants, execution scope, threadgroup size and route selector |
| Timing unit | GPU commit span, dispatch normalization and dependency pattern |
| Reuse | Buffer lifetime, cache-conditioning and repeated-input policy |
| Scope | Primary shapes, holdouts and production operator boundary |

Read each candidate pipeline's limits. Compiling successfully does not establish that its actual dispatch or MPP participation scope is valid.

## Comparison procedure

1. Build the comparison variants from fixed source and compilation options. Record executable and pipeline identities.
2. Validate the baseline and candidate with the same correctness cases, including tails and state.
3. Profile a bounded workload separately to test the proposed bottleneck.
4. Calibrate unchanged-baseline A/A runs through the acceptance harness.
5. Run interleaved baseline/candidate comparisons with the declared warm-up and cooldown.
6. Check the frozen candidate on held-out shapes and in the production operator.
7. Independently repeat the comparison with the frozen candidate, then accept, reject or invalidate it using its predeclared gates.

Choose the sample count and minimum gain from measured noise and application requirements. No numerical gate in this book is a universal hardware property.

## Correctness

| Check | Purpose |
| --- | --- |
| Independent reference | Avoid reproducing the same implementation error |
| Aligned and tail dimensions | Exercise full and partial tiles |
| Poisoned output/padding and guards | Detect short writes, out-of-range writes and no-op paths |
| Repeated execution | Expose stale state, initialization and ordering errors |
| Signedness and magnitude coverage | Expose format, conversion, overflow and accumulation mistakes |
| Per-step recurrent state | Prevent final-output agreement from hiding a wrong state update |
| Convergence and participation | Exercise active-mask and barrier contracts |
| Stated tolerance | Preserve the approved arithmetic contract; never loosen it after failure |

Bitwise comparison is useful when exact behavior is required. Arithmetic reordering or relaxed matrix precision requires an explicit error contract rather than an assumed bitwise guarantee.

## Timing

Use the existing workload harness where it measures the intended unit. Keep capture, validation, tracing, inserted counters and competing benchmark processes out of acceptance runs.

GPU start/end feedback measures a complete GPU span without adding timestamp commands. Divide by dispatch count only when the harness's dependency and repetition policy justify that normalization. Host wall time includes different costs.

`metal-perf ab run` runs every variant once per round and reverses variant order on alternating rounds. It supports an explicit cooldown and discarded leading rounds:

```sh
metal-perf ab run \
  --variant 'baseline=<baseline command>' \
  --variant 'baseline_repeat=<same baseline command>' \
  --variant 'candidate=<candidate command>' \
  --rounds 4 --discard 1 --cooldown 10 \
  --metric '<one numeric capture group>' --out <owned output directory>
```

The command is an example policy, not a sufficient sample size for every workload. Freeze a unit in the metric expression. If output can switch between ns, µs and ms, use an existing fixed-unit harness mode or explicitly validate conversion before comparison.

For paired rounds define `r = candidate_time / baseline_time`. The geometric mean of paired ratios is a useful summary; report the individual ratios and spread too. A ratio below one means less time. The unchanged-baseline control determines whether a difference is resolvable.

A positive control must change independently known work. For example, double a serialized, work-dominant dispatch count and compare raw GPU spans. A normalized per-dispatch metric would divide away this effect. Changing K is not automatically a 2× control because it can change layouts, tiles and cache behavior.

## Observer effects

Inserted timestamp commands can impose ordering. Precise timestamp granularity can split a pass; relaxed granularity can group work. If using internal timestamps beyond diagnosis, compare against uninstrumented feedback and check both overhead and candidate ranking.

Counter collection, capture replay, high-state profiling and cold dispatches remain diagnostic even when their numbers are precise.

## Result interpretation

| Outcome | Required evidence |
| --- | --- |
| Accepted | Correctness passes, calibration passes, gain exceeds the declared gate, holdouts and production route pass, independent repeat confirms the result |
| Rejected | Valid measurement does not meet the gate, or a required workload regresses |
| Invalid | Measurement conditions or harness controls fail |
| Incomplete | Required evidence is missing |

Retain raw samples and condition records, including invalid runs. If performance cannot be distinguished within the calibrated noise, the measurement supplies no basis for selecting the more complex variant.

## Reproducibility record

Retain device/OS/compiler identity; source commit and diff; executable/library hashes; pipeline/function constants and limits; shape/layout; dispatch count and dependencies; raw samples; condition records; order and discarded rounds; calibration; numerical results; and decision criteria.

Store raw traces and experiment records outside the source repository. The documentation should retain the result, scope and practical implication. [Compatibility](evidence.md) defines implementation scope.
