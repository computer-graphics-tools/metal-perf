# M5 hardware and evidence levels

Apple says every M5 GPU core has a dedicated Neural Accelerator: a matrix unit inside the GPU core. It is a different thing from the CPU's matrix coprocessor and from the separate Neural Engine. Apple publishes core counts, memory bandwidth and the programming model. It does not publish absolute matrix throughput (only relative claims such as "up to 4 to 8 times faster"), register-file size, cache sizes or the instruction set. Plan experiments around what we can observe and treat everything else as a hypothesis.

## The M5 family

| | M5 | M5 Pro | M5 Max |
| --- | --- | --- | --- |
| Announced | 2025-10-15 | 2026-03-03 | 2026-03-03 |
| GPU cores | 10 | up to 20 | up to 40 |
| Neural Accelerator | one per GPU core | one per GPU core | one per GPU core |
| Unified memory bandwidth (Apple spec) | 153 GB/s | up to 307 GB/s | up to 614 GB/s |
| Unified memory | 32 GB (only figure in the release) | up to 64 GB | up to 128 GB |
| CPU | up to 4 performance + 6 efficiency | 6 super + 12 performance | 6 super + 12 performance |
| Neural Engine | 16-core | 16-core | 16-core |
| Packaging | not stated | two dies ("Fusion Architecture") | two dies |

Sources: [M5 announcement](https://www.apple.com/newsroom/2025/10/apple-unleashes-m5-the-next-big-leap-in-ai-performance-for-apple-silicon/), [M5 Pro/Max announcement](https://www.apple.com/newsroom/2026/03/apple-debuts-m5-pro-and-m5-max-to-supercharge-the-most-demanding-pro-workflows/). Apple's "over 4× peak GPU compute for AI" claims come from preproduction systems and "select industry-standard benchmarks". They give a direction, not a ceiling to tune against.

M5 Ultra ([announced 2026-08-25](https://www.apple.com/newsroom/2026/08/apple-introduces-m6-and-m5-ultra-for-a-big-leap-in-performance-and-ai-compute/): up to 80 GPU cores, 1.2 TB/s, up to 512 GB, four dies from two dual-die M5 Max chips) is not covered. Every local measurement in this book is from one M5 Max.

What the family table implies for tuning:

- **Measured ceiling.** On the M5 Max used here (40-core GPU), a grid-stride read kernel over a 2 GiB buffer peaked at 587 GB/s and a copy at 542 GB/s (`metal-perf bandwidth`, 2026-10-13); a Swift version of the same probe read 594 GB/s on 2026-10-12. Judge bandwidth-bound kernels against the measured number, not the 614 GB/s spec.
- **Bandwidth per core is roughly the same** across the three chips (about 15.3 GB/s per core). Bandwidth-bound decode should scale with the tier. Compute-bound prefill also scales with core count. This is a hypothesis to confirm per chip, not an established fact.
- **Pro and Max use two dies.** Whether threadgroup placement or memory locality across the die boundary affects matmul is unknown. Do not reuse a tuned table from M5 on M5 Max, or the reverse, without measuring.

## Matrix engines and their public programming surfaces

Keep two questions apart. The first is which API a kernel is written against. The second is which physical datapath executes it. Apple documents the first. For M5 the second is unpublished; AIR and profiler counters give partial answers.

```text
public programming surface (documented)    what is visible on M5 (evidence level)
GPU: Metal 4 tensor ops / MPP matmul2d ──> AIR: family-gated 16×16×16 matrix intrinsic;
                                           counters: Neural Accelerator activity (no ISA decode)
GPU: simdgroup_matrix 8×8              ──> AIR: 8×8 intrinsic; physical mapping unverified
CPU: Accelerate / BNNS (public API)    ──> CPU matrix coprocessor (ISA undocumented)
ANE: Core ML / Core AI (public API)    ──> Neural Engine
          all engines share one unified-memory bandwidth budget
```

- **GPU Neural Accelerators.** Apple says each M5 GPU core has one. Apple names Metal 4 tensor operations and [Metal Performance Primitives](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) (MPP) as the programming surface. Apple's MLX article says MLX uses "TensorOps and Metal Performance Primitives" and needs macOS 26.2 or later ([Apple ML Research, 2025-11-19](https://machinelearning.apple.com/research/exploring-llms-mlx-m5)). Apple does not document which MPP descriptors, dtypes and scopes run on the accelerator. The MPP runtime library on this machine (`MetalPerformancePrimitives.framework/.../libTensorOps.rtlib`, read with `metal-perf air`, 2026-10-13) shows:
  - The int8 × int8 → int32 cooperative `matmul2d` (`__tensorops_impl_matmul2d_op_run_cooperative_dv_i8_dv_i8_i32_v2`) issues 16 `air.simdgroup_matrix_16x16x16_widening_multiply_accumulate.s.s.v8i32.v8i8.v8i8.v8i32` calls behind an `air.supports_family` check, with an `air.simdgroup_matrix_8x8_multiply_accumulate` fp16 fallback.
  - int8 × int4b widens the nibbles to int8 in ALU code and then issues the same int8 calls. There is no int4 matrix operand.
  - Xcode's GPU profiler reports Neural Accelerator utilization of 41% for a dense BF16 4096² M = 64 MPP matmul and 0% for an MPP matmul whose 4-bit weights were dequantized to f32 in the shader ([mpp-first-profile.json](evidence/2026-10-10/mpp-first-profile.json); see [Profiling](profiling.md)).

  This is AIR and counter evidence. Without an M5 instruction decoder it does not show the physical instruction.
- **`simdgroup_matrix`** is the portable public API. Community reverse engineering of pre-M5 GPUs found that it runs on the ordinary arithmetic pipes. There is no M5 instruction-level evidence of its mapping, so on M5 it is **unverified**. Two indirect hints suggest it does not use the accelerators (hypothesis): MPP's own int8 path uses the separate 16×16×16 intrinsic and keeps the 8×8 one only as a fallback, and Apple's M5 [tech talk 111432](https://developer.apple.com/videos/play/tech-talks/111432/) says kernels written with the SIMD-group matrix API "should move … over to adopt TensorOps instead".
- **The CPU matrix coprocessor** (often called AMX) is part of the CPU cluster. Its public Apple route is Accelerate/BNNS. It competes for the same memory bandwidth, so a background Accelerate workload confounds GPU timings.
- **The Neural Engine** is a separate block. Its public Apple routes are the model frameworks. Private access paths exist but are out of scope here.

## What is verified locally

From [toolchain.txt](evidence/2026-10-10/toolchain.txt), via `MTLDevice` and system queries (no GPU work):

| Property | Value | Level |
| --- | --- | --- |
| Device / model | Apple M5 Max, `Mac17,7`, 40 GPU cores, 128 GiB | Verified locally |
| CPU performance levels | `Super` ×6, `Performance` ×12 | Verified locally |
| Metal families | `apple10` and `metal4` true, `apple11` false | Verified locally |
| `maxThreadgroupMemoryLength` | 32768 bytes | Verified locally (API limit) |
| `maxThreadsPerThreadgroup` | 1024 | Verified locally (API limit) |
| `recommendedMaxWorkingSetSize` | 107.5 GiB | Verified locally |
| Counter sets / sampling points | `counterSets` = `timestamp`; `supportsCounterSampling`: stage boundary true, dispatch/draw/tile/blit false | Verified locally (API query) |
| GPU timestamp frequency | 24 MHz (one tick ≈ 41.7 ns) | Verified locally |
| IOKit accelerator class | `AGXAcceleratorG17X` | Verified locally; internal naming, meaning not documented |

The exposed counter sets matter. The public programmatic Metal counter API on this device exposes timestamps and nothing else. The documented tool route to occupancy, stall reasons and per-kernel memory traffic is Xcode's GPU tools (`gpudebug profile` on a captured, replayed trace). See [Profiling](profiling.md).

## GPU telemetry observed without root (macmon pilot)

On 2026-10-10 (15:36:36Z–15:36:39Z) one finite `macmon pipe -s 3 -i 1000 --soc-info` run returned the values below without `sudo`. The raw lines are in [macmon-pilot.jsonl](evidence/2026-10-10/macmon-pilot.jsonl).

These are 3 one-second samples of a near-idle desktop. They are not a matmul profile, and nothing was compared with an external power meter.

| Property | Observed | What it is, and its limits |
| --- | --- | --- |
| GPU cores | 40 | Agrees with `system_profiler` and IORegistry `gpu-core-count` above |
| Nominal GPU DVFS table | 13 non-zero states, 338 … 1620 MHz | Static power-manager table of nominal frequencies, not measured clocks |
| `gpu_freq_mhz` | 338, 415, 439 MHz | Average nominal P-state frequency over the GPU's active time. It matches `scaled_ratio × 1620 / active_ratio` within 1 MHz; the integers are consistent with truncation, but rounding was not checked. It is **not** an instantaneous measured clock and **not** the 24 MHz timestamp frequency |
| `gpu_active_ratio` | ~0.48–0.50 | Share of each interval the GPU power domain was not off. This is not ALU, MXU or occupancy utilization, and 1.0 would not mean the hardware was 100% busy |
| `gpu_power` | 1.30–1.55 W | Modeled energy averaged over each interval, not a calibrated physical meter. Which IOReport energy channel feeds it is unverified |
| `gpu_temp_avg` | ~60.16 °C | Snapshot average of SMC sensors. The exact sensor key set is unverified |
| `ram_power`, `gpu_ram_power` | `0.0` | Treated as **unavailable** (inference, see below). A zero here is not a measured zero power |

The macmon v0.9.0 [README](https://github.com/vladkens/macmon/blob/v0.9.0/readme.md) says macOS 27 stopped refreshing the IOReport `Energy Model` counters without the private `com.apple.private.pmgr.nrg.reporting` entitlement, that macmon now reads hidden AppleCLPC energy reports instead, and that on an unsupported configuration "zero CPU/ANE readings can mean unavailable counters rather than zero consumption". The README says this about CPU and ANE power only. Reading the RAM zeros above as "unavailable" is an inference; no load test checked it.

Tool provenance: macmon 0.9.0, tag [`v0.9.0`](https://github.com/vladkens/macmon/tree/v0.9.0) at commit `7df49f55d9a1b9072e31fc8ba991abda84593563`. The Homebrew bottle SHA-256 is `9484530b11b84f3f9aafa1e5f1d7c582f936cc42a0ed745beabfa8aeeff349b8`, and the source tarball SHA-256 is `e3708d4da099d1e22e71384fe8ea0445aa2549d5198c569cdfb2fe75672f90c6`, which matches the formula.

## What the community measured, and why it is not M5 proof

The [metalworking glossary](metalworking-review.md) repeats widely used numbers: about 208 KB of registers per core, saturation near 24 SIMD groups, barriers costing about 2 cycles, 8 KB of L1, and threadgroup memory of "~60 KB physically". These come mostly from [metal-benchmarks](https://github.com/philipturner/metal-benchmarks) measurements on M1-era hardware. The ISA work in [applegpu](https://github.com/dougallj/applegpu) documents G13, the M1 family. For M5:

- Treat these numbers as priors for picking candidate ranges, never as limits. The autotuner must enumerate what the pipeline actually reports (see [GPU execution](gpu-execution.md)).
- Treat an M4 finding, such as "no dedicated matrix datapath" from the [Rigel paper](https://arxiv.org/abs/2606.12765), as M4 evidence only. M5 added the accelerators.
- Of the community figures, only the 32 KB threadgroup-memory limit was observed locally, as the API value above.

## Layers of truth for one matmul

| Question | Answered by | Not answered by |
| --- | --- | --- |
| Is `half × int4 → float` a legal operand combination? | `MPPTensorOpsMatMul2d.h` dtype table (local SDK) | That it runs on the accelerator |
| What did the frontend emit? | Generated AIR (see [compiler](compiler.md)) | The physical instruction |
| What does the GPU execute? | Captured, generation-matched shader decoding (not available for M5: `metal-objdump` has no agx3 instruction printer) | Throughput |
| Is the result right? | Comparison with a reference under a stated tolerance | Speed |
| Where did time go? | `gpudebug` profile with stated coverage | Whether a change helps |
| Is it faster? | Uninstrumented, calibrated A/B ([autotuning](autotuning.md)) | Why |

## Unknowns to carry forward

- Neural Accelerator peak throughput per dtype and per core. There is no Apple number. Measure a positive control instead (see [Matmul](matmul.md)).
- Whether `relaxed_precision` or particular MPP execution scopes change which datapath runs.
- Whether fp8/fp4 MPP operand formats (listed in the local header) run natively or through conversion on M5. For int8 × int4b the AIR already shows conversion (see above); fp8 and fp4 were not traced.
- Clock and power effects on sustained matmul throughput. The current GPU power limit can be read through the AGX driver's IOKit properties ([Telemetry](telemetry.md)); changing it is out of scope for this book.
