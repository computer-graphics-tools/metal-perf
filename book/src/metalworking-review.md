# metalworking review

[metalworking](https://metalworking.vercel.app/) ([repo](https://github.com/Exorust/metalworking)) is a well-linked glossary of Apple GPU performance written for CUDA engineers. All 57 content pages, the README landing page and its [SOURCES.md](https://github.com/Exorust/metalworking/blob/main/SOURCES.md) were read on 2026-10-10. The site has changed since then: as of 2026-10-12 the glossary has 64 entries under `/glossary/`, a 29-chapter book was added under `/book/`, the old page URLs below redirect to `/glossary/...`, and several pages were edited on 2026-10-10 and 2026-10-11. The table describes the 2026-10-10 text unless noted. It is curated secondary context. Its quoted code is pinned to commits, but its hardware numbers mostly come from community measurements on pre-M5 chips. Several statements about tooling and synchronization were overtaken by Metal 4 and macOS 27.

Evidence codes: **A** consistent with Apple docs or the local SDK. **C** community measurement on pre-M5 hardware. **R** third-party measurement on M5 (source report). **Q** quoted pinned source code. **S** synthesis or opinion. **X** conflicts with current primary sources (see the conflicts section below).

## Per-page table

| Page | Takeaway | Evidence | M5 relevance |
| --- | --- | --- | --- |
| [README](https://metalworking.vercel.app/) | CUDA-to-Metal glossary; every page standalone | S | Orientation |
| **Machine** | | | |
| [NVIDIA to Metal](https://metalworking.vercel.app/machine/nvidia-to-metal/) | Term map with transfers/resized/unlearn verdicts | S, C | Medium; numbers are priors |
| [GPU Core](https://metalworking.vercel.app/machine/gpu-core/) | 10 cores/153 GB/s (M5), 40/614 (Max); ~208 KB regs, ~8 KB L1 | A (counts), C (internals) | Counts yes; internals unverified on M5 |
| [Simdgroup](https://metalworking.vercel.app/machine/simdgroup/) | 32-wide lockstep; fast shuffles | A (width), C (bandwidth) | High; read `threadExecutionWidth` |
| [Registers](https://metalworking.vercel.app/machine/registers/) | Spills are the cliff (10× in m5-gemm); `max_total_threads_per_threadgroup` matters | R, C | High for candidate design |
| [Threadgroup Memory](https://metalworking.vercel.app/machine/threadgroup-memory/) | 32 KB per threadgroup, "~60 KB physically" per core | A (32 KB), C (60 KB) | 32 KB verified locally; 60 KB unobservable |
| [Unified Memory](https://metalworking.vercel.app/machine/unified-memory/) | Bandwidth-bound platform; ~84% of spec achievable | A (spec), R (84%) | High: bandwidth floors. Locally `metal-perf bandwidth` reached 587 GB/s read and 542 GB/s copy (2026-10-12), above the site's figure |
| [Occupancy](https://metalworking.vercel.app/machine/occupancy/) | Saturation near 24 SIMD groups; no profiler reports achieved occupancy directly | C | Prior only; check what `gpudebug profile` reports |
| [F16](https://metalworking.vercel.app/machine/f16/) | 16-bit wins on stalls/registers; fp8 emulated on M4 | C | Medium; M4 fp8 result is not M5 |
| [Special Paths](https://metalworking.vercel.app/machine/special-paths/) | Fast `exp2`; float atomics emulated | C | Low |
| [AMX](https://metalworking.vercel.app/machine/amx/) | CPU matrix coprocessor shares bandwidth | C | Low; benchmark confound |
| [Neural Accelerators](https://metalworking.vercel.app/machine/neural-accelerators/) | Per-core matrix unit reached through Metal 4 tensor ops; MLX NAX 16×16 fragments | A, Q, R | High |
| **Metal, the stack** | | | |
| [Metal, the API](https://metalworking.vercel.app/metal/metal-the-api/) | Explicit command-buffer model | S | Low |
| [MSL](https://metalworking.vercel.app/metal/msl/) | MSL vs CUDA C++; `__asm("air.*")` closed by Metal 4 | A, Q | Medium; the local [compiler probe](compiler.md#a-fresh-local-experiment) agrees, and the current compiler rejects these labels with `-std=metal3.2` too |
| [Dispatch Geometry](https://metalworking.vercel.app/metal/dispatch-geometry/) | `dispatchThreads` partial groups; size is a pipeline-time contract | A | High |
| [Compilation Pipeline](https://metalworking.vercel.app/metal/compilation-pipeline/) | MSL → AIR → metallib → pipeline; "no offline path to machine code" | A, **X** | Medium; see conflicts |
| [Function Constants](https://metalworking.vercel.app/metal/function-constants/) | Pipeline-time specialization deletes branches | A, Q | High; a tuning axis |
| [Command Buffers](https://metalworking.vercel.app/metal/command-buffers/) | Batch dispatches; `GPUStartTime`/`GPUEndTime` called the only programmatic timing (since 2026-10-10 the page says "simplest") | S, R, **X** | Medium |
| [Synchronization](https://metalworking.vercel.app/metal/synchronization/) | Barriers ~2 cycles; "Metal tracks buffer hazards" | C, **X** | High (the conflict matters) |
| [simdgroup_matrix](https://metalworking.vercel.app/metal/simdgroup-matrix/) | 8×8 on ALU pipes pre-M5; 13.5 TF FP32 GEMM on M5 Max | C, R | High: portable baseline |
| [simdgroup_async_copy](https://metalworking.vercel.app/metal/simdgroup-async-copy/) | Private DMA copy; "LLVM-IR-level workarounds crash the backend" | Q, **X** | See [async copy](async-copy.md) |
| [MTLTensor and MPP](https://metalworking.vercel.app/metal/mtltensor-and-mpp/) | Descriptors, cooperative tensors, execution scopes | A | High: the MXU route on M5 |
| [MPS](https://metalworking.vercel.app/metal/mps/) | MPS as the baseline to beat | R, S | Medium: optional external control |
| [Profiling](https://metalworking.vercel.app/metal/profiling/) | No Nsight; Metal 4 counters are timestamps only through macOS 26; Xcode capture is a GUI. The page already notes macOS 27's `metalperftrace` | **X** | High (partly outdated; `metalperftrace` records only CAMetalLayer frame stats, so headless compute still needs `gpucapture`/`gpudebug`) |
| [Disassembly](https://metalworking.vercel.app/metal/disassembly/) | applegpu covers G13; NAX coverage lags | C | See [reverse engineering](reverse-engineering.md#m5-isa-research-boundary) |
| **MLX** | | | |
| [MLX Overview](https://metalworking.vercel.app/mlx/mlx-overview/) | Framework layer map | S | Low |
| [Lazy Evaluation](https://metalworking.vercel.app/mlx/lazy-evaluation/) | One sync per step | S, Q | Low |
| [How an Op Becomes a Kernel](https://metalworking.vercel.app/mlx/how-an-op-becomes-a-kernel/) | Hand-tuned tile table; M = 3–6 fell between routes | Q, R | High: small-M kernel selection |
| [Steel](https://metalworking.vercel.app/mlx/steel/) | Loader/MMA/epilogue decomposition; NAX fork | Q | Medium |
| [mx.fast](https://metalworking.vercel.app/mlx/mx-fast/) | Fused-op dispatch gate | Q | Low |
| [Quantization](https://metalworking.vercel.app/mlx/quantization/) | Group-wise affine; QMV vs QMM; mxfp4/nvfp4/mxfp8 | Q, S | Medium |
| [mx.compile](https://metalworking.vercel.app/mlx/mx-compile/) | Elementwise fusion only | S | Low |
| [Distributed](https://metalworking.vercel.app/mlx/distributed/) | Thunderbolt 5 RDMA clusters | S | None |
| **Techniques** | | | |
| [Arithmetic Intensity](https://metalworking.vercel.app/techniques/arithmetic-intensity/) | Do the bytes/FLOP arithmetic first | S | High |
| [Roofline](https://metalworking.vercel.app/techniques/roofline/) | Measure both roofs; ~516 GB/s copy on M5 Max | R, S | High; locally 542 GB/s copy (`metal-perf bandwidth`, 2026-10-12) |
| [Tiling](https://metalworking.vercel.app/techniques/tiling/) | Reuse pyramid; compile-time tiles | S, Q | Medium |
| [Cooperative Load](https://metalworking.vercel.app/techniques/cooperative-load/) | Coalesced, vectorized staging | Q | High: baseline staging |
| [Register Blocking](https://metalworking.vercel.app/techniques/register-blocking/) | Accumulator grid; spill cliff | Q, R | Medium |
| [Double Buffering](https://metalworking.vercel.app/techniques/double-buffering/) | ILP overlap; winner flips with size | Q, R | Medium; the local demo found no ≥ 3% async win |
| [Fusion and Epilogues](https://metalworking.vercel.app/techniques/fusion-and-epilogues/) | Apply epilogues in registers | Q, S | High |
| [Online Softmax](https://metalworking.vercel.app/techniques/online-softmax/) | Streaming softmax identity | Q | Low |
| [Flash Attention](https://metalworking.vercel.app/techniques/flash-attention/) | Never materialize scores | Q | Low |
| [The KV Cache](https://metalworking.vercel.app/techniques/kv-cache/) | K/V quantization asymmetry | R | Low |
| [Decode vs Prefill](https://metalworking.vercel.app/techniques/decode-vs-prefill/) | Small M is the least-tuned regime | S | High |
| **Kernels** | | | |
| [The Tiled GEMM](https://metalworking.vercel.app/kernels/gemm-tiled/) | m5-gemm FP32 13.5 vs MPS 11.7 TF at 4096² | Q, R | Medium; not local evidence |
| [The Double-Buffered GEMM](https://metalworking.vercel.app/kernels/gemm-double-buffered/) | Three sizes, three winners; published negative results | Q, R | Medium: methodology |
| [The Async-Copy Ghost](https://metalworking.vercel.app/kernels/gemm-async-ghost/) | One SIMD group issuing the copy was fastest (the page names no chip; the M4 report attributes this to M2) | Q, C | See [async copy](async-copy.md); the M4 report reverses it on M4 |
| [Steel's BlockLoader](https://metalworking.vercel.app/kernels/steel-blockloader/) | Safe/unsafe loads chosen by function constants | Q | Medium |
| [Steel's BlockMMA](https://metalworking.vercel.app/kernels/steel-blockmma/) | Fragment grid; transposition through strides | Q | Medium |
| [The Fused GEMM Kernel](https://metalworking.vercel.app/kernels/steel-gemm-fused/) | Kernel as wiring | Q | Medium |
| [Steel Attention](https://metalworking.vercel.app/kernels/steel-attention/) | Readable FA forward | Q | Low |
| [metal-flash-attention](https://metalworking.vercel.app/kernels/mfa-codegen/) | Codegen school; deliberate spills | Q, C | Low |
| [llama.cpp Attention](https://metalworking.vercel.app/kernels/llamacpp-attention/) | Enumeration school | Q | Low |
| [The NAX GEMM](https://metalworking.vercel.app/kernels/nax-gemm/) | MPP `matmul2d`, `execution_simdgroups<4>`, column-major layout friction | Q | High |
| **War stories** | | | |
| [The Three Questions](https://metalworking.vercel.app/war-stories/three-questions/) | Delete work, unlock a fast path, cut overhead | R, S | High: tuner priors |
| [Sparse-V](https://metalworking.vercel.app/war-stories/sparse-v/) | +22.8% by skipping work | R | Low |
| [The Failures](https://metalworking.vercel.app/war-stories/the-failures/) | Custom kernels came back 0.5–0.8× of stock; claim checklist | R, S | High: methodology |
| [Cheap Tricks](https://metalworking.vercel.app/war-stories/cheap-tricks/) | `sudo sysctl` wired limit; fans; bandwidth predicts decode | R | Telemetry context |

## Conflicts with current primary sources

| Site claim | Current evidence | Consequence |
| --- | --- | --- |
| Command-buffer `GPUStartTime`/`GPUEndTime` is "the platform's only programmatic GPU timing" (reworded to "simplest" late on 2026-10-10); counters are "timestamps only, through macOS 26" | `MTLCounterSampleBuffer` (macOS 10.15+), and `MTL4CounterHeap` with `relaxed`/`precise` timestamp granularity (macOS 26, local SDK headers). Locally, counter sets = `timestamp` only | Use timestamp heaps for dispatch spans ([Profiling](profiling.md)) |
| Capture is GUI-only, so there is no scripted loop | `gpucapture`/`gpudebug` CLIs on macOS 27, with Apple's agent guide | Scripted, bounded diagnosis is possible; acceptance still needs uninstrumented timing |
| Dispatches in one command buffer are ordered; "Metal tracks buffer hazards" | True for legacy Metal's tracked resources, but Apple: "In Metal 4, the framework considers all resources untracked" ([Understanding the Metal 4 core API](https://developer.apple.com/documentation/metal/understanding-the-metal-4-core-api)) | Barriers are required; benchmarks must state what they measure |
| No offline path to GPU machine code | `metal-tt` builds binary archives with native GPU slices offline (see [compiler](compiler.md#ways-to-influence-emitted-code)) | Pipeline-creation cost and artifacts can be studied separately |
| IR-level async-copy workarounds crash the backend | Typed IR assembles and packages (M4 report and the local probe). Offline `metal-tt` lowering of the local probe fails with an "unlowered function call" error rather than a crash. The local demo's README reports the intrinsic working at runtime on M4 and M5 Max | Runtime behaviour on M5 rests on one unreplicated local report ([async copy](async-copy.md)) |
| Barriers ~2 cycles, 24-SIMD-group saturation, 208 KB registers | M1-era community measurements | Priors only on M5 |
| One SIMD group issuing async copies is fastest | The M4 report attributes this to M2 and found 8 SIMD groups about 6× faster on M4 | Device-specific; re-measure |

The site does not claim that threadgroup staging is required: it calls staging "the house pattern" and its double-buffering page says winners change with problem size. Apple's MPP guidance that staging is unnecessary for optimized MPP GEMMs is discussed in [Matmul](matmul.md#apples-m5-gemm-guidance).

## Gaps

As of 2026-10-10, the site did not cover: statistical acceptance (its advice is warm-up, medians, methodology hygiene and a claim checklist), A/A calibration, correctness tolerances for `relaxed_precision`, Metal 4 explicit synchronization, timestamp heaps, small-M MPP tiling on M5, or any measured Neural Accelerator throughput.
