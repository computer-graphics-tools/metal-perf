# Async copy: API, ABI and performance

An asynchronous tile copy separates issuing data movement from waiting for completion. That creates an opportunity to compute on one tile while another arrives. It also consumes staging memory and introduces issue/wait overhead, so it can lose to an ordinary cooperative load.

```text
device source ──issue copy──> threadgroup tile[next]
                                  │ completion event
tile[current] ──matrix work──────> accumulator
                                  │
                           wait before reuse
```

## Read the three sources in their own contexts

| Source | Evidence and relevance |
| --- | --- |
| [applegpu issue 28](https://github.com/dougallj/applegpu/issues/28) | Original 2023 observations of an MPS private copy operation and historical signatures. Useful discovery evidence; it does not decode or validate M5. |
| [Pinned `async_copy.metal`, lines 9–28](https://github.com/0xekez/metal-matmul/blob/04e80810bbf7ba96ebe26ff84a346d179ee50888/async_copy.metal#L9-L28) | Declares a 12-argument intrinsic and wait operation through asm labels. It names the copy `air.simdgroup_async_copy_2d.p3i8.p1i8`, without the `.read` that the current local compiler output uses. The pinned commit matters: current compiler acceptance must be tested independently. |
| [M4 reverse-engineering report, 2026-05-21](https://ighoshsubho.bearblog.dev/reverse-engineering-apples-simdgroup-async-copy-on-m4/) | Reports frontend rejection, recovery via typed IR, a silent no-op from an incorrect signature, and copy-throughput experiments on M4 with macOS 26.0, Xcode 26.4 SDK and Metal toolchain `32023.883`. This is the author's device-specific evidence, not a local M5 measurement. |

The M4 report motivates poisoned destinations, exact types and an independent cooperative baseline. Its throughput measurements do not predict a matmul speedup. The local [compiler experiment](compiler.md#a-fresh-local-experiment) reproduces frontend rejection and successful typed IR packaging on the newer stack (`32023.921`). The frontend rejects `air.*` asm labels with `-std=metal3.2` as well as `-std=metal4.0` (re-checked 2026-10-13), so this is a property of the compiler, not only of the Metal 4 language mode. Runtime and physical ISA claims require separate evidence.

There is a semantic disagreement worth preserving: the pinned source comments its final integer as a clamp mode, while the M4 report describes a transpose flag. The local demo passes zero. Zero-mode behavior is the only exercised contract here; other modes need targeted shape, bounds and transpose tests before being named or tuned.

## The local async-copy demo

A small local Rust + Metal 4 demo (`metal4-async-copy-demo`, not published) is the existing probe. Its copy shader stages FP32 data into threadgroup memory and waits on an event, with one SIMD group per dispatch. Its source and generated AIR were inspected; its numerical and benchmark results come from its README (dated 2026-10-10) and were not re-run for this book.

The two source-level correctness cases are concrete:

| Case | Source layout | Copied tile | Destination layout |
| --- | --- | --- | --- |
| Contiguous | FP32, row-major | `[8,64]` | 64 useful elements per row |
| Strided | FP32 `[13,80]`, origin `(column=7,row=3)` | `[8,64]` | 80 elements per row; padding preserved |

The shader poisons all 1,024 staging values before copying. A successful result must contain the expected 512 copied values and 512 untouched values for each case. Timing a dispatch or successfully creating a pipeline cannot replace that check.

The matmul experiments use FP32 `A[M,K]`, row-major `B[N,K]`, and `C[M,N] = A × Bᵀ`, with public 8×8 SIMD-group matrix arithmetic shared across cooperative, cooperative double-buffered and private-copy double-buffered variants. That isolates copy mechanisms better than comparing unrelated arithmetic implementations, but does not establish use of M5 Neural Accelerators.

The README states that the private intrinsic was verified on M4 and M5 Max. It records no candidate passing its preregistered 3% speedup gate, and records negative results for some double-buffered configurations. These are unreplicated local reports, not accepted results.

## The questions to test on M5

| Hypothesis | Controlled change | What would falsify it |
| --- | --- | --- |
| Copy can overlap arithmetic profitably | Same shape/arithmetic, next tile issued before current compute, private copy versus cooperative load | Correct output but no held-out uninstrumented time gain |
| Larger copies amortize issue overhead | Copy bytes per issue; fixed total bytes, geometry and useful work | Time fails to improve, or residency/resource pressure reverses the gain |
| Extra staging increases resource pressure | One versus two buffers, same tile and arithmetic | Pipeline/profile evidence does not show the predicted resource change; timing alone cannot diagnose occupancy |
| Tail handling changes which route wins | Exact and irregular M/N/K with identical output contract | A speedup disappears or correctness fails on held-out tails |

For a FP32 `BM×BK` A tile and `BK×BN` B tile, one staging set consumes `4×BK×(BM+BN)` bytes before padding or other shared state. Two sets double that allocation. Query actual device and pipeline caps; allocation size alone does not reveal resident threadgroups or the hardware register budget.

Keep the cooperative single-buffer path as a reference until a challenger passes correctness, calibrated timing and an independent confirmation. A private copy is a candidate parameter, not a requirement for fast matmul. Counter/profile results explain observed candidates; [performance acceptance](autotuning.md) comes from separate runs.
