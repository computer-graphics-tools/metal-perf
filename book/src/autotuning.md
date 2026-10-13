# Autotuning specification

**Status: proposal (draft).** This chapter specifies how an offline autotuner for Metal compute kernels could work, with matrix multiplication as the main example. It defines what the tuner may search, how a candidate proves it is correct, and when a measured difference counts as a real improvement. Every numeric gate and budget below is a proposal, not a measured requirement. The aim is simpler kernels whose few parameters are chosen per device by measurement. Counter readings are not the objective.

## Two activities, kept apart

```text
exploration (diagnostic)                     acceptance (decisive)
  profiles, counters, compiler output          uninstrumented timing only
  cheap screening, any order                   preregistered gates, randomized, calibrated
  → proposes candidates                        → accepts or rejects one candidate per workload
              └──── never feeds a number into ────┘
```

A profile can motivate a candidate. Only acceptance timing can promote it. Short paired diagnostic probes belong on the exploration side: they can drop clear losers, but they never feed a number into acceptance. No counter, occupancy estimate or "utilization" figure is ever optimized as a scalar objective.

## Workloads

A **workload** is `(route, M, N, K, A dtype, B format {bits, group, signedness, params layout}, output dtype, D transform, transposition, gathered)`: a matmul shape plus its data types and transforms.

- **Primary set**: the shapes, data formats and batch sizes the target application actually runs. Each value of M is tuned and accepted on its own; a winner at one M is not interpolated to others.
- **Coverage envelope**: a wider range of shapes and M values that must stay correct, even if it is tuned later or not at all.
- **Compute controls**: large-M GEMMs per route and dtype (for example M = N = K = 4096). They give an empirical route rate for the model in [Matmul](matmul.md#a-roofline-per-route). They are not upper bounds.
- **Holdout set**: shapes chosen before tuning starts and never used for selection, such as shapes from a different application at the same M values.

## Parameter space

Candidates are generated per route. A candidate is **lawful** only if it compiles, the built pipeline accepts its dispatch (`maxTotalThreadsPerThreadgroup`, `threadExecutionWidth`, static + dynamic threadgroup memory ≤ `maxThreadgroupMemoryLength`), and it meets any MPP scope contract ([GPU execution](gpu-execution.md#pipeline-time-contracts)). The tuner reads these values back from each built pipeline. It does not predict them.

| Route | Axes | Constraint source |
| --- | --- | --- |
| GEMV | SIMD groups per threadgroup, results per SIMD group, K split, reduction/group lanes, input row tile | K split divides the K blocks |
| SIMD-group GEMM | Tile shape (M × N × K) and SIMD-group layout, split-K | Existing tile configurations first; split-K divides the K steps |
| MXU GEMM (MPP) | Tile shape, split-K, MPP execution scope, relaxed vs strict precision, K step between barriers, walk order (Morton vs linear), B staging (threadgroup vs direct device) | MPP scope = dispatched SIMD groups; quantized K a multiple of the group size; starting points from Apple's [MPP guide](matmul.md#apples-m5-gemm-guidance) |
| Activation format | 16-bit vs int8 activations | Includes the cost of the quantization pass |
| Private AIR (research) | e.g. async-copy staging | Separate research build only, never shipped, never a fallback |

Proposed search order: existing tile configurations first, which costs no code; then generated variants of existing kernels; new kernels last. **Proposed** budget: at most 64 lawful candidates per workload family per device for screening, and at most 3 finalists per workload for acceptance.

## Correctness gate

Each candidate runs before any timing, on the same build that will be timed:

1. Compare with an independent CPU reference, covering the workload shape plus tails.
2. Tolerance per route, stated in the manifest. The **proposal**: exact equality where the arithmetic order is identical to the baseline. Otherwise compare against an fp32-accumulated reference with a bound per output dtype, and keep the MXU relaxed-precision bound separate from the strict one.
3. Fill output and padding with a poison value first, so that a no-op or a short write fails.
4. Reject any NaN or Inf that the reference does not produce. Run twice and require bit-identical output, unless the route documents nondeterminism.
5. A failure discards the candidate for that workload and is logged. A correctness failure never triggers a retry with a looser tolerance.

## Timing protocol

- **Unprofiled**: no capture environment variables, no validation layers, no tracing, no concurrent benchmark processes, and **no inserted counter commands**. Run conditions as in [Telemetry](telemetry.md).
- **Acceptance unit (default)**: commit-feedback GPU span (`GPUEndTime − GPUStartTime` of `MTL4CommitFeedback`) of a commit with a stated dispatch count and stated dependencies between dispatches: explicit barriers between them, or declared independence. The manifest names the unit, the dispatch count and the dependency pattern. Never compare numbers measured in different units.
- **Timestamp-heap timing is a qualified diagnostic, not acceptance**, until it passes the observer-effect gate below. An encoder `writeTimestampWithGranularity:` "ensures that any prior work finishes", and `precise` granularity "can affect runtime performance" (both `MTL4ComputeCommandEncoder.h`) and "may cause splitting of command encoders" (`MTL4Counters.h`). A commit with inserted timestamps is therefore a different workload from one without them. Stage-boundary sampling support (`supportsCounterSampling`) does not imply per-dispatch sampling support, and the M5 Max used for this book reports dispatch-boundary sampling as false ([toolchain snapshot](evidence/2026-10-10/toolchain.txt)).
- **Warm-up**: build pipelines and run at least one untimed dispatch per candidate before its first sample. Compile time is reported separately.
- **Balanced randomized order**: within each block, interleave the baseline (B) and candidate (C) as ABBA/BAAB pairs in random order across workloads, so drift and thermal trends affect both equally.
- **A/A calibration**: every block also times the baseline against itself through the identical path. Its spread defines the noise floor for that block.
- **Positive control (harness sensitivity)**: every session includes a change with an expectation that is known independently of the harness. The **default** is the same pipeline, layout, buffers and dependency pattern with the dispatch count doubled in one commit. Its expected effect is about 2× in the **raw commit span** (not span ÷ dispatch count, which normalizes the doubled work away). This expectation holds only when dispatches are serialized by barriers and caches are in the same state. Doubling K is **not** a 2× control by default, because it changes the pipeline or tile schedule, the bytes and the cache footprint. Use it only if its expected ratio has been established separately. A failed control invalidates the session.
- **Positive control unit**: the doubled-work control runs at a work-dominant dispatch count R → 2R for every timing unit. A single commit's latency legitimately includes fixed per-commit cost, so 2× at R = 1 is not a sensitivity test.
- **Sample size**: choose the number of blocks per comparison by calibration on the target device. The A/A gate must pass at the chosen count before any candidate is compared, and the count is recorded in the manifest.
- **Conditioning** (proposed): before the single-commit spans that set R and before every comparison, run untimed commits (alternating the arms) for a fixed GPU time, for example 0.5 GPU-seconds, with a commit cap. Record the count, the summed span and whether the cap was hit. Once per session, run a fixed warm-up long enough that the device's first thermal-state transition, if any, happens before measurement; measure that duration once per device and record it. Before each comparison, read `NSProcessInfo.thermalState` once per second until two consecutive reads agree and are ≤ 1 (at most 30 s); otherwise the comparison is invalid and not retried. A thermal-state change inside the measured blocks invalidates the whole comparison.

### Observer-effect gate for timestamp timing

Before any timestamp-heap measurement is used beyond diagnosis, one experiment must characterize it on the target device:

1. Time the same candidates as commits without timestamp writes, then with `relaxed` writes, then with `precise` writes, in interleaved order.
2. Report the added time per inserted timestamp and the change in commit span, against A/A noise.
3. Compare the candidate **ranking** and the paired speedups from timestamp spans against unprofiled commit-feedback timing.

**Proposed pass:** the timestamp overhead is stated, and for every candidate pair the timestamp-derived speedup lies within the commit-feedback 95% CI, with identical rank order. Until this passes for a device and granularity, acceptance uses unprofiled commit-feedback timing only. Capture and profiling runs remain separate from both.

## Acceptance

All statistics are on paired log-ratios `log(t_C / t_B)` within a block. **Proposed gates**:

| Gate | Proposal |
| --- | --- |
| A/A sanity | 95% CI of A/A median log-ratio within ±1% |
| Positive control | raw commit-span ratio for doubled dispatch count (serialized) within 2× ± 5% |
| Minimum gain | candidate median speedup ≥ 3% |
| Confidence | 95% bootstrap CI of the paired speedup excludes 1.0 and its lower bound exceeds the A/A half-width |
| Holdout | the selected candidate, frozen, repeats the gain on the holdout set in a separate session (different day or reboot) |
| No regression | no primary workload gets more than 1% slower beyond A/A noise |

A candidate that passes screening but fails holdout is rejected. It is retained in the records with the reason.

**Stop conditions**: stop a workload family when the candidate budget is used up, when two consecutive generations of candidates produce no screening pass, or when the A/A floor is wider than the minimum gain (the measurement cannot resolve the question; fix the harness first).

## Objectives beyond time

Time is the primary objective and the only acceptance gate. Among candidates within A/A noise of each other, prefer in this order:

1. **Fewer owned source lines and fewer special cases.** A table entry that lets a hard-coded branch be deleted is better than one that adds another branch.
2. **Lower pipeline compile cost and fewer distinct pipelines.**
3. **Lower energy**, only if a measurement with documented units and coverage is available. Otherwise mark it unavailable.

Report useful work per [Matmul](matmul.md#work-and-bytes), not counter "utilization". A route's achieved fraction of its measured controls is model-relative diagnostic context, not a goal. 100% of any hardware unit is not a promised or required outcome.

## Manifest

Every timing record links to a manifest that contains at least:

- **Device**: model, GPU core count, Metal families, whether the route uses the GPU matrix accelerators (as detected at run time), device name, timestamp frequency.
- **Software**: macOS build, Xcode build, Metal compiler version, Metal toolchain asset, SDK build, host compiler version.
- **Source**: commit and a dirty flag, a hash of each shader source and generated header, build features.
- **Artifacts**: metallib hash, pipeline cache key, function-constant values, and the built pipeline's `maxTotalThreadsPerThreadgroup`, `threadExecutionWidth` and `staticThreadgroupMemoryLength`.
- **Workload and candidate**: the full workload tuple, candidate parameters, measurement unit, dispatch count per sample, buffer reuse policy.
- **Conditions**: the run-condition record before and after each block ([Telemetry](telemetry.md#the-run-condition-record)).
- **Protocol**: seed, block order, sample counts, gates in force, A/A and positive-control results.
- **Outcome**: accepted, rejected or invalid, with the reason, and links to any diagnostic traces.

Keep raw samples, invalid runs, failed candidates and preflight failures. Exclude them from statistics only by an explicit, recorded rule. Never delete them.

## Caching and invalidation

A tuned choice is valid only for the key it was measured under:

```text
key = (device model, GPU core count, matrix-accelerator use, Metal family,
       macOS build, Metal compiler version, shader source hash,
       route, workload tuple, measurement unit)
```

A change in any field invalidates the entry for re-measurement. Missing telemetry, a changed thermal state within a block, or a failed positive control invalidates the block, not just the sample. An invalidated entry falls back to the documented default policy, and the fallback is reported. It is never silent.

## Non-goals for the first tuner

- No online or in-production tuning. Tables are produced offline and reviewed like code.
- No reuse across devices without re-measurement, even within the M5 family.
- No copying of published benchmark numbers into tables or manifests as if they were local evidence.
