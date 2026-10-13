# Sources

The original entries were checked on **2026-10-10**; the dated WWDC25–26 audit below was checked on **2026-10-11**. "Read" means the full content was retrieved and read. "Cited only" means the book mentions the source through another source and has not read it. Treat cited-only sources as unverified. Compiler, async-copy and reverse-engineering sources are tracked in those chapters and in the [evidence ledger](evidence.md).

## Apple primary sources

| Source | Type | Status | Used for |
| --- | --- | --- | --- |
| [Investigating GPU issues with AI agents](https://developer.apple.com/documentation/xcode/investigating-gpu-issues-with-ai-agents) | Xcode docs | Read | `gpudebug` session reuse, `--oneshot` ([Profiling](profiling.md)) |
| [Metal developer tools](https://developer.apple.com/metal/tools/) | Product page | Read | Tool inventory: Metal debugger, HUD, System Trace, CLI tools |
| [GPU counters and counter sample buffers](https://developer.apple.com/documentation/metal/gpu-counters-and-counter-sample-buffers) | Metal docs | Read | Legacy counter API, timestamp conversion |
| [MTL4CounterHeap](https://developer.apple.com/documentation/metal/mtl4counterheap), [MTL4TimestampGranularity](https://developer.apple.com/documentation/metal/mtl4timestampgranularity) | Metal docs | Read | Timestamp heaps (macOS 26+) |
| [Metal Performance Primitives Programming Guide, v1](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) (2026-03-16) | Apple PDF | Read in full (13 pages) | M5 GEMM starting points: 2×2 SIMD groups, 32×32 SIMD-group tiles, `BK = 128`, Morton order, static extents, no staging needed |
| Local SDK headers: `MTL4Counters.h`, `MTL4ComputeCommandEncoder.h`, `MTLDevice.h`, `MTLComputePipeline.h`, `MetalPerformancePrimitives/MPPTensorOpsMatMul2d.h` (MacOSX 27.0 SDK, 26A425) | SDK | Read (relevant sections) | Granularity semantics, pipeline limits, MPP dtype table, scopes, `relaxed_precision` |
| `man gpucapture`, `man gpudebug`, `man metalperftrace`, and each tool's `--help` | Installed manuals | Read in full | Exact supported commands |
| [Apple M5 announcement](https://www.apple.com/newsroom/2025/10/apple-unleashes-m5-the-next-big-leap-in-ai-performance-for-apple-silicon/) (2025-10-15) | Newsroom | Read | M5 specs |
| [Apple M5 Pro and M5 Max announcement](https://www.apple.com/newsroom/2026/03/apple-debuts-m5-pro-and-m5-max-to-supercharge-the-most-demanding-pro-workflows/) (2026-03-03) | Newsroom | Read | Pro/Max specs, Fusion Architecture |
| [Exploring LLMs with MLX and the Neural Accelerators in M5](https://machinelearning.apple.com/research/exploring-llms-mlx-m5) (2025-11-19) | Apple ML Research | Read | TensorOps/MPP route; TTFT 3.33–4.06×, generation 1.19–1.27× (source report, M5 vs M4, 24 GB) |
| [apple/game-porting-toolkit @ 4f42344c](https://github.com/apple/game-porting-toolkit/tree/4f42344c90be22112f46becc3fb9687f423f2f88/game-porting-skills/skills) | Apple expert skills | Read: `using-gpucapture`, `using-gpudebug`, `using-metal-validation`; skimmed: Metal 4 sync/pipelines/resources | Capture/debug rules, Metal 4 explicit synchronization |
| [Game Porting Toolkit page](https://developer.apple.com/games/game-porting-toolkit/) | Product page | Read | Where the skills live |
| [WWDC26 session 330](https://developer.apple.com/videos/play/wwdc2026/330/) (Metal tensors, 2026-06-08) | Video transcript | Read ([audit](research-update-2026-10.md)) | macOS/iOS 27 low-bit tensor types, E8M0 block scales, cooperative reductions |
| [Tech talks 111432](https://developer.apple.com/videos/play/tech-talks/111432/) (M5/A19 ML, 2026-03-20) and [111375](https://developer.apple.com/videos/play/tech-talks/111375/) (M3 Dynamic Caching, 2023-11-09) | Video transcripts | Read ([audit](research-update-2026-10.md)) | Neural Accelerator per core; SIMD-group matrix → TensorOps; occupancy management |
| [Apple M6 and M5 Ultra announcement](https://www.apple.com/newsroom/2026/08/apple-introduces-m6-and-m5-ultra-for-a-big-leap-in-performance-and-ai-compute/) (2026-08-25) | Newsroom | Read ([audit](research-update-2026-10.md)) | M6 and M5 Ultra specs; not our hardware |
| WWDC26 sessions [388](https://developer.apple.com/videos/play/wwdc2026/388/) ("Find and fix performance issues in your Metal games") and [357](https://developer.apple.com/videos/play/wwdc2026/357/) ("Speedrun your game port with agentic coding") | Video transcripts | Read in the [WWDC audit](wwdc25-26-api-plan.md) | 388: `metalperftrace collect`/`overview`, StateReporting; 357: `gpucapture`, `gpudebug` |

## Source literature for async copy

| Source | Status | Key fact (source report) |
| --- | --- | --- |
| [0xekez/metal-matmul `async_copy.metal` L9–28 @ 04e80810](https://github.com/0xekez/metal-matmul/blob/04e80810bbf7ba96ebe26ff84a346d179ee50888/async_copy.metal#L9-L28) | Read | 12-argument `air.simdgroup_async_copy_2d.p3i8.p1i8` and `air.wait_simdgroup_events`, declared through `__asm` labels |
| [dougallj/applegpu issue #28](https://github.com/dougallj/applegpu/issues/28) | Read | 2023-02-19: MPS kernel uses the private copy; closed via PR #29; header provenance traced to the Xcode 14.2 era |
| [Reverse engineering Apple's simdgroup async copy on M4](https://ighoshsubho.bearblog.dev/reverse-engineering-apples-simdgroup-async-copy-on-m4/) (2026-05-21) | Read | M4, Xcode 26.4, toolchain 32023.883: frontend rejects `air.*` asm labels; an IR-rewrite path works with the correct 12-argument 2D form (a naive rewrite produced zeros); also reports work-group copy intrinsics as unlowered on M4 |

## Secondary and community sources

| Source | Status | Notes |
| --- | --- | --- |
| [metalworking](https://metalworking.vercel.app/) (57 content pages + README) | Read in full | Per-page review: [metalworking review](metalworking-review.md) |
| [metalworking SOURCES.md](https://github.com/Exorust/metalworking/blob/main/SOURCES.md) | Read | URLs verified by its author 2026-08-07; additions dated 2026-08-24 |
| [philipturner/metal-benchmarks](https://github.com/philipturner/metal-benchmarks) | Cited only | Origin of the register, occupancy and barrier figures (M1-era) |
| [dougallj/applegpu](https://github.com/dougallj/applegpu) | Read at `28dd8046` ([audit](research-update-2026-10.md)) | G13 ISA tooling; Oct 2026 change is README-only (`metal-tt` + `compiler_explorer.py`) |
| [Rigel (arXiv 2606.12765)](https://arxiv.org/abs/2606.12765) | Read ([audit](research-update-2026-10.md)) | M4 Max tensor API findings (source report); not M5 evidence |
| [yaroslavvb/m5-gemm](https://github.com/yaroslavvb/m5-gemm) | Cited only | FP32 GEMM numbers on M5 Max (author's numbers; not local evidence) |
| [MLX steel / NAX kernels](https://github.com/ml-explore/mlx/tree/main/mlx/backend/metal/kernels/steel) | `nax.h`, `fp_quantized_nax.h`, `precision.rst` read at `06eb7483` ([audit](research-update-2026-10.md)) | MPP-based (NAX) kernels for M5; 16×16 software fragments, software FP4 dequantization |

## October 2026 source audit

The [source audit](research-update-2026-10.md) reads about forty sources from a research synthesis at pinned commits: AIR/compiler interfaces (vertex `air`, Metal.jl, tinygrad, MetalLibraryArchive, LLAIR, floor), native ISA reports (applegpu, agx-re, Gravity Mesa/Linux, Asahi) and matrix sources (Apple talks, MLX, MFA, papers). Entries above that the audit read point to it.

## Local evidence

- [toolchain.txt](evidence/2026-10-10/toolchain.txt): OS, Xcode, compiler, tools, device capabilities, run conditions.

## WWDC25–26 materials

See the [dated session catalog and API inventory](wwdc25-26-api-plan.md): 66 sessions selected from the WWDC25 and WWDC26 catalogs, two M5 technical talks, and SDK availability checks. Use its version distinctions before treating an API as new.

