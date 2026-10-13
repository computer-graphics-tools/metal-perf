# Matmul on M5

An inference matmul multiplies an activation `A[M, K]` by a weight matrix stored as `B[N, K]` and writes `D[M, N]`, often with dequantization before the multiply and a fused transform after it. M is the number of rows in flight: 1 for plain decode, a few for speculative decode, hundreds or thousands for prefill. Small M tends to be bandwidth-bound: the kernel must stream every weight byte for little arithmetic. Large M can become compute-bound, and that is where the Neural Accelerators should pay off. We found no published M at which the M5 Max crosses over, so it has to be measured.

## Conventions

```text
A [M, K]  activations   bf16 / f16 / f32, or int8 after activation quantization
B [N, K]  weights       full precision, or packed 4/8-bit codes + per-group params
D [M, N]  output        = transform(A × Bᵀ)     (B stored transposed)
```

| Piece | Typical choices | Meaning |
| --- | --- | --- |
| A prologue | full precision, or symmetric int8 | Activations as given, or quantized to int8 before the GEMM |
| B prologue | full precision; scale + bias; scale + zero point; symmetric scale | How weight codes become values inside the kernel |
| Quantization parameter layout | `[N, G]` (per output row) or `[G, N]` (per group, N padded) | Where scales/zero points live; it affects load coalescing |
| Output transform | scale, accumulate, bias, Hadamard rotation, soft cap | Fused epilogue applied in registers before the one store |

## Work and bytes

Define **useful matmul operations** as `2·M·N·K` on real (unpadded) elements: one multiply and one add per term of the mathematical product. This is an accounting unit, not an instruction count. On a bf16/f16 route it is floating-point work, reported as FLOP/s. On integer or quantized routes (int8 activations, 4/8-bit weight codes) the hardware may run integer multiply-accumulates, conversions or dequantization instead. Report those as **FLOP-equivalent** useful operations and state the operand dtypes (for example "W4 G64 × bf16 → bf16"). Never present them as physical floating-point instructions.

Everything else is **overhead**: padded tile lanes, dequantization arithmetic, activation-quantization passes, split-K partial writes and reduction, epilogue math and launch gaps. Overhead can be necessary. It is never counted as useful work.

Worked example for one projection with `N = K = 4096`. It assumes a **DRAM-only streaming scenario**: every stated weight byte is read from DRAM exactly once per dispatch, at the 614 GB/s M5 Max **spec**. (On our M5 Max, `metal-perf bandwidth` measured a 587 GB/s peak GPU read on 2026-10-13.) The once-per-dispatch assumption has not been measured:

| Weights | Logical weight bytes | Time if streamed from DRAM at spec | Useful ops at M=1 | M=32 op rate needed to stay at that time |
| --- | --- | --- | --- | --- |
| bf16 | 33.6 MB | ≈ 55 µs | 33.6 M FLOP | ≈ 20 TFLOP/s |
| 4-bit codes (+ group params, G=64) | ≈ 8.4 MB + params | ≈ 14 µs + params | 33.6 M FLOP-equivalent | ≈ 78 T FLOP-equivalent/s |

These are illustrative estimates, not thresholds. Logical tensor bytes are not DRAM transactions: weights that stay resident in the system-level cache across back-to-back dispatches (as in a benchmark that repeats dispatches over the same weights) can make apparent bandwidth exceed any DRAM figure. Partial-line and parameter reads can also add traffic. The spec bounds DRAM streaming only. It says nothing about bandwidth served from caches.

Read the last column as a question. If the route sustains more than that rate at this shape, M=32 time should stay close to M=1 in the DRAM-streaming scenario. If it sustains less, compute matters somewhere inside 1..32. Neither the crossover nor the sustained rate is published, so both are first-experiment targets.

## A roofline per route

Build a separate **empirical model** for each route. It approximates, and does not bound:

```text
time ≈ max( dram_bytes / measured_stream_bandwidth ,  useful_ops / measured_route_rate ) + overhead
        measured_stream_bandwidth: streaming-read control, same bytes/layout/scale, data not cache-resident
        measured_route_rate:       best validated large GEMM on that route and dtype (positive control)
```

- **It is not a lower bound on time.** A rigorous lower bound needs valid *upper* bounds on bandwidth and on compute rate, plus the real DRAM traffic. Measured controls give neither: a better kernel can beat the control, and caches can serve bytes faster than the streaming probe.
- **Bandwidth control**: a streaming read of the same weight bytes, layout and dispatch scale on the same device, sized or flushed so that it actually reaches DRAM. The spec caps DRAM streaming. It does not cap cache-served traffic.
- **Compute control**: the best-performing, numerically validated large GEMM on that route and dtype (for example M = N = K = 4096 bf16 on the MXU route). It shows a rate the route can reach, not the hardware maximum.
- Report achieved bandwidth ÷ bandwidth control and achieved useful ops/s ÷ compute control, and keep in mind that both are model-relative. If both are low, that alone does not show overhead or latency as the cause. It motivates controlled experiments (cache residency, layout, resource use, launch/dispatch count) and a bounded profile ([Profiling](profiling.md)). "100% of hardware" is not a target, because neither control is known to be the hardware maximum.

## Routes: public programming surfaces

| Route | API | Fits |
| --- | --- | --- |
| GEMV | plain loads + `simd_sum` reductions | M = 1..few, bandwidth-bound |
| SIMD-group GEMM | public `simdgroup_matrix` 8×8 (physical mapping on M5 unverified) | portable across M1–M5 |
| MXU GEMM | MPP `mpp::tensor_ops::matmul2d`, cooperative tensors | Apple’s documented surface for the M5 Neural Accelerators (AIR lowering known for int8; native instructions not decodable) |
| Private AIR | typed `air.*` intrinsics via IR rewrite | research only; see [async copy](async-copy.md) |

The MPP header in the local SDK (`MPPTensorOpsMatMul2d.h`) lists the legal operand combinations, including `half`/`bfloat` × `int8`/`int4b`/`int2b` and fp8/fp4 formats. Being listed means the API accepts it. It does not show which hardware path executes it. The list also contains `int8_t × int8_t → int32_t`, which can carry exact integer partial sums per quantization group; group scaling then accumulates in FP32. When both operands are cooperative tensors, the implementation header requires at least one of M, N or K to be 32 (`MPPTensorOpsMatMul2dImpl.h:6364` in the macOS 27.0 SDK).

In the MPP implementation library (`libTensorOps.rtlib`), the `int8 × int8 → int32` cooperative matmul lowers to sixteen `air.simdgroup_matrix_16x16x16_widening_multiply_accumulate` calls, guarded by `air.supports_family`, with an 8×8 fp16 fallback. `int8 × int4b` widens nibbles to int8 with ALU instructions and then issues the same int8 MXU operations: there is no native int4 MXU operand. The implementation symbol names the operand address spaces, for example `…_dv_i8_dv_i8_i32_v2` for two device-memory operands and `…_dv_i8_tg_i8_i32_v2` for device × threadgroup ([evidence](evidence/2026-10-10/mpp-int8-air-boundary.json)). The native AGX instructions behind these intrinsics cannot be disassembled with the available tools ([Reverse engineering](reverse-engineering.md)).

In replayed Xcode profiles (diagnostic, not timing), a dense BF16 MPP GEMM (M64 N4096 K4096) showed 41 % "Neural Accelerator Utilization", while a kernel that dequantized 4-bit weights to f32 before its MPP call (M8 N1024 K1024) showed 0 % ([evidence](evidence/2026-10-10/mpp-first-profile.json); both shaders report 80 compiler-spilled bytes).

MPP's `relaxed_precision` flag "allow[s] implementation to sacrifice accuracy for performance". A kernel that sets it must state the relaxed tolerance in its numerical contract.

Private-AIR candidates stay in an isolated research build. They cannot be shipped and never act as a fallback for a public route.

## Apple's M5 GEMM guidance

Apple's [MPP Programming Guide](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) (version 1, dated 2026-03-16) is the main primary source we found with M5-specific GEMM advice. It gives these as starting points for tuning, not as rules. They are stated for **dense 16-bit floating-point operands** with tensors read directly from device memory. The guide gives no quantized-weight or int8-activation results, so do not assume they transfer to quantized or int8 routes without measurement:

| Knob | Apple's M5 starting point | Note |
| --- | --- | --- |
| SIMD groups per threadgroup | 2 × 2 for 16-bit floats | "Smaller data types may require larger tile sizes" |
| SIMD-group tile | `SM = SN = 32` for 16-bit floats | Larger tiles help until operands no longer fit thread-local memory |
| K step between threadgroup barriers | `BK = 128` | A barrier with `mem_none` keeps SIMD groups close in K, which helps cache reuse |
| Threadgroup walk order | Morton order over a 1-D grid | Reuses last-level cache across neighbouring tiles |
| Bounds checks | Static extents for full tiles, chosen by alignment function constants; `dynamic_extent` only for partial tiles | Avoids per-load checks |
| Staging | "Threadgroup memory staging isn't necessary for the most optimized GEMM kernels"; no explicit software pipelining is needed | Contradicts the staging and double-buffering habits of pre-M5 kernels |

The guide recommends tuning by benchmark over a wide range of tile sizes, because a benchmark "accounts for all machine-specific factors". It also names two quantization losses: partial edge tiles that do wasted work, and a tile count that is not a multiple of the GPU core count. Both matter at small M. These are source reports to test, not results we have measured. For quantized routes, a direct-device-read variant must keep the same packing and numerical contract, and must charge any unpacking, dequantization or materialization it adds.

## Preparation and fusion

- **Weight preparation**: packing, layout changes and parameter layout happen once, at load. Record that cost as load-time cost. A layout change counts only if it also wins end-to-end ([Autotuning](autotuning.md)).
- **Activation preparation**: int8 activation quantization runs before the GEMM, either as its own pass or fused into the producer kernel. Its time belongs to the route's total.
- **Fused epilogues**: output transforms applied in registers avoid an extra pass. A candidate that drops a fusion and adds a pass must count that pass.

## Where to look first

Focus the first experiments on **M = 1..32** at real projection shapes, plus **positive controls** at large M. Small-M routing boundaries (when GEMV beats GEMM, when the matrix units start to pay off) are device-specific. Re-derive them from data, per device and route, rather than inherit constants measured on earlier devices.

Correctness comes before speed. Each candidate first passes exact or toleranced comparison against a CPU reference across aligned and unaligned shapes, then enters timing.
