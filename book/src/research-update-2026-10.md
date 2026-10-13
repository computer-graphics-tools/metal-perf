# Source audit, October 2026

On 2026-10-10 a pasted research synthesis ("Apple GPU Internals and Reverse Engineering Research") listed about forty external sources. This chapter records what each one actually says when read directly, at a pinned commit or date. The synthesis is treated as a list of leads: a claim enters the book only after its primary source was read. Nothing here was re-run on our hardware. Author-reported numbers stay attached to the author's chip.

The sources fall into three areas that must not be merged:

```text
native AGX ISA / architecture     AIR and compiler interfaces       matrix hardware and kernels
(Dougall, agx-re, Gravity, Asahi) (vertex air, Metal.jl, tinygrad,  (MPP, MLX NAX, MFA, Apple talks,
                                   MetalLibraryArchive, LLAIR)        papers and microbenchmarks)
        │                                   │                                   │
 bytes the GPU executes           LLVM-derived SSA + air.* calls      what an API promises and what
 (per generation)                 + metadata, packed in a metallib    someone measured on their chip
```

AIR is LLVM IR with Apple intrinsics and metadata. It is not the GPU's native encoding. A metallib packs AIR modules; a pipeline is the device-specific compile of one function from it; a binary archive stores such compiled pipelines. Those are four different artifacts. See [From Metal source to GPU instructions](compiler.md) for the local evidence on each.

## Method and coverage

- GitHub repositories were read through the REST API at the commits listed below. Other pages were read over HTTP; Apple video pages carry the full transcript in their HTML.
- Of the Rust repositories, only file listings and READMEs were read.
- No third-party code was run and no GPU work was done for this audit. Links, pinned commits, arXiv IDs and patents were re-checked on 2026-10-13; all resolved.

Our machine, for scale: an M5 Max (`applegpu_g17s`, device class `AGXG17CDevice`, Metal 4 queue/encoder class `AGXG17XFamily_mtlnext`, family Apple10, see [Metal 4 submission](scheduler.md) and [toolchain.txt](evidence/2026-10-10/toolchain.txt)), macOS 27.0.1 (26A434), Metal toolchain 27.1.266.1, `metal` 32023.921 (`sw_vers`, `xcrun metal --version`). `air-dis` and `metal-dis` are not installed; `air-objdump`, `air-opt`, `air-as`, `air-validate`, `metal-objdump`, `metal-tt` and `metallib` are. The base M5 is `g17g`. A finding on `g17g`, on M4 (`G16`, Apple9) or on M1 (`G13`, Apple7) is not a finding on `g17s`.

## AIR and compiler interfaces

| Source | Pinned at | What it is | Boundary |
| --- | --- | --- | --- |
| [vertex-language/air](https://github.com/vertex-language/air) | `3d277d16`, 2026-09-24 | Pure Go (no cgo) AIR builder, `.air` bitcode writer and `.metallib` reader/writer. Apple tools only as a test oracle: `xcrun metal -x ir`, `air-opt`, `air-validate`, `xcrun metallib`, `newLibraryWithData:` | Compute kernels only, no MSL frontend. Version table ends at macOS 26 → AIR 2.8; families Apple7–Apple9 only. GPU tests ran on an M1 |
| [MetalLibraryArchive](https://github.com/YuAo/MetalLibraryArchive) | `de573ba4`, 2023-05-24 | Swift metallib parser with a documented byte layout | Predates Metal 4. Throws on unknown platform, library type or OS byte; does not document `RBUF`/`AIRR` reflection |
| [Metal.jl `simd.jl`](https://github.com/JuliaGPU/Metal.jl/blob/8c421b88521c74322dd943d4d499bf05665d1fc7/src/device/intrinsics/simd.jl) | `8c421b88`, 2026-10-09 | `air.simdgroup_matrix_8x8_*` calls on `<64 x T>` values | AIR 2.8 load/store take dims/strides/origin vectors; older AIR is rewritten to the legacy signature |
| [Metal.jl `tensor.jl`](https://github.com/JuliaGPU/Metal.jl/blob/8c421b88521c74322dd943d4d499bf05665d1fc7/src/device/intrinsics/tensor.jl) | same | Tensor descriptors and `matmul2d` from Julia | See below; 4-bit formats not exposed |
| [Metal.jl intrinsics directory](https://github.com/JuliaGPU/Metal.jl/tree/8c421b88521c74322dd943d4d499bf05665d1fc7/src/device/intrinsics) | same | `arguments`, `atomics`, `math`, `memory`, `output`, `simd`, `synchronization`, `tensor`, `version` | Only `simd.jl` and `tensor.jl` read in full |
| [Metal.jl hacking FAQ](https://github.com/JuliaGPU/Metal.jl/blob/main/docs/src/faq/hacking.md) | `main` = `8c421b88` | AIR reading, metallib round trip, backend crash reduction, offline `metal-tt` | `metallib-dis`/`-as`/`-load` are Metal.jl repo scripts, not Apple tools |
| [tinygrad `ops_metal.py`](https://github.com/tinygrad/tinygrad/blob/master/tinygrad/runtime/ops_metal.py) | `8a4e8c23`, 2026-10-10 | In-process MSL → metallib through `MTLCompiler` | Request type `13` is undocumented ("there aren't any docs or symbols") |
| [gzorin/LLAIR](https://github.com/gzorin/LLAIR) | `b7828e97`, 2026-09-19 | C++ over `llvm::Module`; runs Apple `metal`/`metallib` | Needs the `gzorin/floor_llvm` fork; README only read |
| [a2flo/floor_llvm](https://github.com/a2flo/floor_llvm) | `4b173e3d`, 2026-10-10 (`floor_toolchain_1406`) | LLVM fork with `llvm/lib/Bitcode/MetalLib/MetalLibWriterPass.cpp` and `llvm/tools/metallib-dis` | Paths and commit log only; source not read |
| [iOS26-Runtime-Headers, Metal](https://github.com/qingralf/iOS26-Runtime-Headers/blob/cbf1d4c4122f3c6807eba36693581882640e5c6c/Frameworks/Metal.framework/MTLCompileOptionsInternal.h) | `cbf1d4c4`, 2025-09-22 | RuntimeBrowser dump: `_additionalCompilerArguments`, `_optimizationLevel`; [pipeline descriptor](https://github.com/qingralf/iOS26-Runtime-Headers/blob/cbf1d4c4122f3c6807eba36693581882640e5c6c/Frameworks/Metal.framework/MTLComputePipelineDescriptorInternal.h) has `driverCompilerOptions`, `gpuCompilerSPIOptions` | iOS 26 declarations. Not proof that macOS 27 honors them |

### Version boundaries

The three readers disagree on what to call the container version. The local tools settle it for this machine:

| Deployment | metallib | AIR | MSL | Source |
| --- | --- | --- | --- | --- |
| macOS 14 / 15 / 26 | 1.2.7 / 1.2.8 / 1.2.9 | 2.6 / 2.7 / 2.8 | 3.1 / 3.2 / 4.0 | Metal.jl FAQ |
| macOS 26 | "2.9" | 2.8 | — | vertex `fileVersion()`; it names a different field split of the same header bytes |
| macOS 27 (local) | 1.2.9 (`METALLIB_EXECUTE`) | 2.9, triple `air64_v29-apple-macosx27.0.0` | 4.0 as requested with `-std=metal4.0`; the compiler default is 4.1 (`__METAL_VERSION__` 410) | Local [round-trip probe](compiler.md) (`air-objdump --metallib --all-headers`, extracted `.ll`) |

So AIR moved to 2.9 on macOS 27 while the container stayed 1.2.9. Neither vertex nor Metal.jl tables cover macOS 27 yet, and vertex has no Apple10 family. Pin the deployment target before comparing any generated module with these projects.

### What each tool chain does with a library

- **vertex**: writes one bitcode module per kernel, SHA-256 per function in `HASH`, pads bitcode to 16 bytes. vertex reports that `air-validate` refuses unpadded modules though Metal loads them, and treats any `air-validate` output as failure rather than trusting the exit code. (Locally, toolchain 27.1.266.1, `air-validate` on an unparseable file prints `error: cannot parse` and exits 1.)
- **MetalLibraryArchive**: checks `MTLB`, file size, `ENDT` terminators and each function's SHA-256 (`invalidBitcodeHash`). Header extension tags it knows: `HDYN`, `VLST`, `ILST`, `HSRD`/`HSRC`, `UUID`.
- **Metal.jl**: reads and round-trips `RBUF` reflection (flatbuffer with `AIRR` identifier, since 1.2.7) but writes only the sections it has data for. `metallib-load` builds a pipeline, which is where backend crashes appear. Crash logs: `MTLCompilerService-*.ips` is symbolicated against `AGXCompilerCore`; the offline `air-nt-*.ips` is not.
- **tinygrad**: `MTLCodeGenServiceCreate("tinygrad")`, then `MTLCodeGenServiceBuildRequest(service, NULL, 13, request, len, block)`. The request is two little-endian `u64` lengths, the source padded to 4 bytes with at least one NUL, then NUL-terminated flags (`-std=metal4.0 --driver-mode=metal -x metal -fno-fast-math -fmodules-cache-path=…` on macOS 26+). The block receives `(error, data, len, message)`. The library starts at the sum of the two `u32` at reply bytes 8–16 (header plus warnings) and must begin `MTLB` and end `ENDT`. tinygrad notes that `MTLCompiler` loads its own LLVM with `RTLD_GLOBAL`, so load order matters.

### Tensor intrinsics as Metal.jl spells them

Metal.jl's `tensor.jl` is the most current public reading of the Metal 4 tensor ABI:

- A tensor descriptor is opaque thread-private storage; Metal.jl reserves a conservative 128 bytes instead of querying `air.get_descriptor_size_tensor`.
- `air.init_strided_private_tensor.i32.{global,local}` takes an AIR-internal element code (`f32 = 0`, `f16 = 1`, `bf16 = 2`, `i8 = 3`, … `u32 = 8`). The code is distinct from `MTLTensorDataType`. On macOS 27, `air.slice_*` uses it to scale origins to bytes, so a wrong code misaddresses non-4-byte types.
- Metal.jl reports that the macOS 27 frontend appends two arguments (a null device pointer and `i32 0`) to that call, guesses they are block-scale data, and says the backend accepts them omitted. Our local compiler (`metal` 32023.921, `-std=metal4.0` and `4.1`) appends three after the element code: `i8 addrspace(1)* null`, `i32 0` and `i8 -1`; the `half` element code is `1`, matching Metal.jl's table.
- `matmul2d` is not an `air.*` intrinsic. It lowers to a call of `__tensorops_impl_matmul2d_op_run_<as>_<type>_…` with a descriptor struct (`m, n, k, transpose_left, transpose_right, relaxed_precision, mode`) and the thread count.
- Metal.jl's docstring says the loop trip count must stay dynamic to avoid a backend crash.

These are Metal.jl's observations on its own toolchain. They are leads for our AIR probes, not facts about `g17s` code.

## Native ISA and scheduling (source reports only)

| Source | Pinned at | Chip in the source | What it establishes there |
| --- | --- | --- | --- |
| [dougallj/applegpu](https://github.com/dougallj/applegpu) | `28dd8046`, 2026-10-06 | G13 (M1) | The October change is README-only (PR #66): `xcrun metal-tt -arch applegpu_g13g` with a `.mtlp-json`, then `compiler_explorer.py`. No ISA change |
| [applegpu docs](https://dougallj.github.io/applegpu/docs.html) | live | G13 | G13 register, occupancy and encoding reference |
| [agx-re `APPLE9_SCOREBOARD_SLOTS.md`](https://github.com/niklassheth/agx-re/blob/master/APPLE9_SCOREBOARD_SLOTS.md) | `e25433b1`; file changed 2026-09-04. This repository is a fork of `ADevWithAnIdea/agx-re` (below); the file exists only in the fork | T8132 M4 Mac mini, macOS 26.6.2 | Six reusable scoreboard slots (1–6) link async producers to their first consumer; slot 0 is the plain register path; slot 7 unproven. Retain/release is independent of the slot selector |
| [agx-re `apple9_isa_explainer.md`](https://github.com/ADevWithAnIdea/agx-re/blob/master/apple9_isa_explainer.md) | `c677ff97`; file changed 2026-08-28 | Apple9 (device not named) | Per-source retain bits. A wrong release still completes the command but corrupts later consumers (`gid = 1` gave `0x64` not `0x65`; all-released floats gave zeros) |
| [agx-re EXP-M5-20 splice log](https://github.com/ADevWithAnIdea/agx-re/blob/d9cb313553f11b4172fef40c2b3b641a2f8c2984/experiments/EXP-M5-20-coopmatrix-operands/logs/splice_evidence.txt) | `d9cb3135`, 2026-07-13 | base M5, `G17g`, T8142 | See below |
| [GravityLinux/mesa Apple9 backend](https://github.com/GravityLinux/mesa/blob/main/src/asahi/compiler/README.apple9-ir.md) | `87ac51d4`, 2026-09-22 | T8132 (M4) | Apple9 compiler: `agx_compile_apple9.c`, `agx_apple9_ir.{c,h}`, `agx_apple9_machine.{c,h}`, `agx_apple9_ra.c`, `agx_apple9_encoding.h`. No path in the tree names Apple10, G17, T8142 or T6050 |
| [GravityLinux/linux `drivers/gpu/drm/asahi`](https://github.com/GravityLinux/linux/tree/gravity-m4/drivers/gpu/drm/asahi) | `4fc326f6`, 2026-10-09 | G16 | Inventory only: `g16.rs` and `g16_{alloc,compute,drm,fw,notify,platform,power,queue,render,runtime,stamp,tvb,vm}.rs` |

Two of these reports had to be reconciled. The explainer calls bits 45–47 a "consumer route" that identifies the producer kind. The later scoreboard document reinterprets the same field as a slot selector and calls "route" the older name. Use the later reading, and keep it an M4 result.

**The M5 splice log.** On a base M5 (`G17g`), the author compiled their own `mad_f32` kernel (`R = A·B + C`, one SIMD group), then edited single bytes and read back the matrix. With `A = 2I`, `B = 3I`, `C = 5I` the result is `11I`. The 14-byte MAC is `2f 00 05 9a 20 20 af 0a 30 08 10 00 10 19`. Byte +3 selects the A and B tile registers, byte +13 bits 4:3 select C, and bit 6 negates the product: `-(A·B) + C = -1`. The author marks the exact A/B sub-bit packing as unresolved ("kept RAW").

This is a set of byte-to-effect observations for one compiled kernel on `g17g`. It is not a decoder. A shared leading byte (the synthesis points to a `cf` prefix) is not proof of an opcode class, and `g17s` may differ. The rule stays: no raw opcode patch on our machine before a validated decoder for that generation exists.

**Do not transfer:** G13 register counts, occupancy tables and scoreboard numbers describe M1 (or M4 for agx-re). They do not give M3–M6 resource counts. M3 replaced fixed allocation with Dynamic Caching (below).

## Matrix hardware, precision and kernels

### Apple primary sources

| Source | Date | Verified content |
| --- | --- | --- |
| [Tech talk 111432, "Accelerate your machine learning workloads with the M5 and A19 GPUs"](https://developer.apple.com/videos/play/tech-talks/111432/) | 2026-03-20 | "each shader core has its own neural accelerator"; matrix multiplies "up to 4 to 8 times faster, depending on precision"; custom SIMD-group matrix kernels should "move … to adopt TensorOps". It says "occupancy management" and adjusting occupancy "based on cache usage". The phrase "occupancy manager" does not appear |
| [Tech talk 111375, "Explore GPU advancements in M3 and A17 Pro"](https://developer.apple.com/videos/play/tech-talks/111375/) | 2023-11-09 | Dynamic Caching: register, threadgroup, tile, stack and buffer data share on-chip caches; the core adjusts occupancy |
| [WWDC26 session 330, "Optimize custom machine learning operations with Metal tensors"](https://developer.apple.com/videos/play/wwdc2026/330/) | 2026-06-08 | Cooperative reductions (`reduce_rows`): confirmed (the API is already in the macOS 26.5 SDK headers). macOS/iOS 27 add 4- and 8-bit float and 2-bit integer types, multi-plane tensors: confirmed. FP8 E8M0 block scales, block 32×1: confirmed. "Fusion" is a FlashAttention pattern using cooperative tensors as `matmul` inputs, not a fusion API |
| [Metal capabilities](https://developer.apple.com/metal/capabilities/) | 301 → Feature Set Tables PDF, edition dated 2026-05-21 | Apple10 = A19 and M5 series. No M6 row and no Apple11 in this edition. Acquire/release atomics: Apple7+, Metal 3 and 4; no MSL minimum stated in the table |
| [MPP Programming Guide](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) | Version 1, 2026-03-16 | Unchanged (Last-Modified 2026-03-17, 13 pages) |
| [Apple newsroom: M6 and M5 Ultra](https://www.apple.com/newsroom/2026/08/apple-introduces-m6-and-m5-ultra-for-a-big-leap-in-performance-and-ai-compute/) | 2026-08-25 | Exists (HTTP 200, both the US and `/mz/` URLs). M6: 12-core GPU with a Neural Accelerator per core, up to 170 GB/s, ~30% more peak GPU AI compute than M5. M5 Ultra: two dual-die M5 Max chips joined by UltraFusion (four dies), up to 80 GPU cores, up to 1.2 TB/s |

The Feature Set Tables predate M6, so M6's GPU family is not stated by Apple yet. Do not assume it.

### Community kernels

| Source | Pinned at | What the code shows | What it does not show |
| --- | --- | --- | --- |
| [MLX `precision.rst`](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/docs/src/usage/precision.rst) | `06eb7483`, 2026-10-10 | fp32 matmul, conv and attention may run at reduced precision on matrix units by default; `MLX_ENABLE_TF32=0` keeps fp32 | Which hardware path is taken |
| [MLX `steel/gemm/nax.h`](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/kernels/steel/gemm/nax.h) | same | 16×16 fragment, 8 elements per lane over 32 lanes; `matmul2d_descriptor(16, 32, 16, …, relaxed_precision = true, multiply_accumulate)` per SIMD group; `static_assert` messages say "MXU tile matmul" | The physical array shape. The 16×16 fragment and the 16×32×16 descriptor are a software layout |
| [MLX `fp_quantized_nax.h`](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/kernels/fp_quantized_nax.h) | same (path is `kernels/`, not `steel/gemm/`) | `fp4_e2m1` weights and `fp8_e4m3`/`fp8_e8m0` scales dequantized in software into threadgroup tiles (default `bfloat`) before the matmul | Native FP4 multiplication |
| [MFA `GEMMHeaders.swift`](https://github.com/philipturner/metal-flash-attention/blob/main/Sources/FlashAttention/GEMM/GEMMHeaders.swift) | `8671cddc`, 2024-09-22 | `__asm("air.simdgroup_async_copy_2d.p3i8.p1i8")` declarations, a `clamp_mode` enum, and an M1 hang workaround (read the copied data and pass another barrier) | That this compiles today. The May 2026 M4 report and our local probe find the current frontend rejects `air.*` asm labels; see [Async copy](async-copy.md) |
| [pplx-garden `lily`](https://github.com/perplexityai/pplx-garden/tree/main/lily) | `1ed972ed`, 2026-09-02 | Rust Metal server for one mixture-of-experts checkpoint (MLX affine Q4, group 64), Apple10+, macOS 26+, runtime source compile | Reference only |

### Five axes for "FP4 support"

A claim like "supports FP4" can mean five different things. Each needs its own evidence:

| Axis | Question | Current evidence |
| --- | --- | --- |
| Storage | Can tensors hold FP4 bytes? | WWDC26-330, the Feature Set Tables and `MTLTensorDataTypeMetalFloat4E2M1` (macOS 27.0 in `MTLTensor.h`): yes, macOS/iOS 27 |
| Accepted operand type | Does `matmul2d` accept FP4 directly? | Yes in the macOS 27.0 SDK: `MPPTensorOpsMatMul2d.h` lists `half` × `metal_fp4_e2m1_format` and FP4 × FP4 with `half` or `float` destinations (absent from the 26.5 SDK). MLX still dequantizes in software |
| Multiplication precision | At what precision are products formed? | Not stated |
| Accumulation | What is the accumulator type? | Not stated for FP4 |
| Native throughput | Is FP4 faster than FP8/FP16 on the hardware? | Not measured by anyone in this audit on `g17s` |

## Benchmarks and design leads

Every number below is the author's, on the author's chip, and was not re-run here. None is an M5 Max (`g17s`) measurement except where the chip column says M5 Max, and even then it is someone else's run.

| Source | Chip in the source | Reported result | Status and caveat |
| --- | --- | --- | --- |
| [Bandwidth, Not FLOPS (arXiv 2609.32237)](https://arxiv.org/abs/2609.32237), [code `m6/`](https://github.com/aminems/AppleSiliconFFT/tree/main/m6) at `14d3558f` | "Apple M6", 12-core GPU, 32 GiB, macOS 27.0 (26A428) | ~150 GB/s GPU streaming; GEMM (FP16 in, FP32 accumulate, Metal 4 tensor ops) 19.6 / 17.3 / 11.0 TFLOPS at n = 1024 / 2048 / 4096 for one kernel in the paper's Table VII (raw output 17.25 and 10.98; another tile reached 17.75 at n = 2048); ~6 TF inside the FFT | Submitted 2026-09-26. The exact Mac model is not stated. The only M6 paper an arXiv API query returned |
| [SLAC (arXiv 2608.09075v2)](https://arxiv.org/html/2608.09075v2) | M1, 8 GB | System-level cache: 12-bit hashed set index, 4096 sets × 16 ways | M1 only; the paper itself flags M3 Dynamic Caching as unresolved |
| [arXiv 2606.27098](https://arxiv.org/abs/2606.27098) | M4 Pro, 14-core | GPU work evicts CPU data from the shared cache, observed after the GPU finishes, contrasted with simultaneous DRAM contention | Abstract only. A residual effect after GPU work is not the same as contention while both run |
| [Rigel (arXiv 2606.12765v1)](https://arxiv.org/html/2606.12765v1) | M4 Max, 40-core, beta macOS 27 | FP16 `matmul2d` 14.8 TF; FP8 matmul emulated at 0.94× FP16; no dedicated matrix unit | M4 has no Neural Accelerators; not M5 evidence |
| [al-engr NAX coverage map](https://www.al-engr.com/m5-nax-coverage-map.html) | M5 Max 40-core, 128 GB, MLX 0.32.3 | ~98% of prefill matmul + attention time in NAX kernels (not all op time) | **Predicted**, not measured: kernels are inferred from MLX gating rules, and the authors say no capture was checked. Not utilization |
| [tzakharko neural-accelerator benchmark](https://tzakharko.github.io/apple-neural-accelerators-benchmark/) | A19, 5-core GPU (iPhone 17) | 7.4 TFLOPS FP16, 13.4 TOPS INT8, cache-resident peaks | The M5 figures on the page are estimates. The INT8 accumulator type is stated two ways |
| [dwijen GPU kernels](https://dwijen.com/experiments/apple-silicon-gpu-kernels-architecture/) | base M5, 10-core GPU, 24 GB | 15.4 TF Neural Accelerator FP16, 6.1 TF packed `half2`, 120.4 GB/s | Base M5 (`g17g`). The 6.1 TF value replaced a retracted 4.24 |
| [US20250103292A1](https://patents.google.com/patent/US20250103292A1/en) "Matrix Multiplier Caching" | — | 4-wide dot products with an accumulator cache; separate integer and float circuits | Apple patent, filed 2024-01-29. Design inference only |
| [US20250110734A1](https://patents.google.com/patent/US20250110734A1/en) "Operand Selection Circuitry" | — | Operand routing with transposed and non-transposed modes | Filed 2024-08-28. Design inference only |
| [US20260087093A1](https://patents.google.com/patent/US20260087093A1/en) "Rate Control for Matrix Accelerator Hardware" | — | Thermal rate throttling of a matrix unit | Filed 2025-01-09. Not a precision feature |

Patents describe a possible design. They do not prove that M5 ships that datapath.

Platform status pages read for context: [Gravity Linux early alpha](https://gravitylinux.org/blog/early-alpha-m4-mac-mini/) (2026-09-18, M4 Mac mini, OpenGL ES 3.0 / OpenGL 3.3), [codyho.dev GPU driver](https://codyho.dev/blog/gpu-driver/) (undated, M4; compute submission works, Vulkan/OpenCL future), [Asahi AGX notes](https://asahilinux.org/docs/hw/soc/agx/) (M1-era kernel/firmware view), [Asahi M3 support](https://asahilinux.org/docs/platform/feature-support/m3/) (GPU work in progress), [Mesa Asahi](https://docs.mesa3d.org/drivers/asahi.html) (M1 OpenGL). None covers M5.

Kept separate from the GPU: [corsix/amx](https://github.com/corsix/amx) (CPU AMX, last updated 2024-12-26 for M4) and [arXiv 2606.22283](https://arxiv.org/abs/2606.22283) (Neural Engine, M1 and M5). [maderix/apple-gpu-dvfs](https://github.com/maderix/apple-gpu-dvfs) (`4f903a72`) is a `sudo` power-cap tool; read as a reference only, never run here.

### Further leads from the full report (existence checked only)

The full synthesis cites these extra sources. Each one resolved on 2026-10-10, but none was read for content, so none supports a claim in this book.

| Source | Last push / date | Report's use |
| --- | --- | --- |
| [AsahiLinux/m1n1](https://github.com/AsahiLinux/m1n1) | 2026-10-10 | Tracing and firmware experiments behind driver bring-up |
| [YuAo/MetalLibraryExplorer](https://github.com/YuAo/MetalLibraryExplorer) | 2023-07-24 | Browser metallib viewer over MetalLibraryArchive |
| [zhuowei/MetalShaderTools](https://github.com/zhuowei/MetalShaderTools) | 2023-06-29 | Historical extraction techniques |
| [Slang `slang-metal-compiler.cpp`](https://github.com/shader-slang/slang/blob/master/source/compiler-core/slang-metal-compiler.cpp) | 2026-10-10 | Another driver of Apple's Metal compiler |
| [imperatormk/metal-profiler](https://github.com/imperatormk/metal-profiler) | 2026-04-08 | Static native-code triage; estimates unvalidated |
| [dwijenpatel/gpu-kernel](https://github.com/dwijenpatel/gpu-kernel) | 2026-09-24 | Code behind the base-M5 measurements above |
| [tzakharko/apple-neural-accelerators-benchmark](https://github.com/tzakharko/apple-neural-accelerators-benchmark) | 2025-10-19 | Code behind the A19 measurements above |
| [BaseRT (arXiv 2607.19438)](https://arxiv.org/abs/2607.19438), [repository](https://github.com/basecompute/baseRT) | 2026-07-21 / 2026-10-08 | M5 inference study; the report says the engine ships as binaries |
| [percisely.xyz/gemm](https://percisely.xyz/gemm) | — | GEMM essay on an M2 MacBook Air |
| [Asahi GPU part 6](https://alyssarosenzweig.ca/blog/asahi-gpu-part-6.html) | 2022 | OpenGL clip-space compatibility mode |

## Contradictions and unavailable links

| Item | What was found | Resolution |
| --- | --- | --- |
| M6 and M5 Ultra "announced Aug 25" | Apple newsroom page exists (HTTP 200, dated 2026-08-25, US and `/mz/` URLs) | Confirmed. The May 2026 Feature Set Tables do not list M6 or an Apple11 family yet |
| "Sep M6 papers" | One arXiv paper (2609.32237, 2026-09-26) on an "Apple M6" with a 12-core GPU; the arXiv API returned no other | One paper, not several. Exact Mac model unstated |
| Metallib version "2.9" (vertex) vs "1.2.9" (Metal.jl) | Same header bytes, different field split | Local `air-objdump` reports 1.2.9 with platform major 27 |
| AIR version on macOS 27 | vertex and Metal.jl tables stop at macOS 26 / AIR 2.8 | Local extract: AIR 2.9 (`air64_v29`); MSL 4.0 because the probe passed `-std=metal4.0` (default is 4.1) |
| "Occupancy manager" (M5 talk) | Transcript says "occupancy management" | Phrase corrected |
| WWDC26-330 "fusion support" | A pattern (cooperative tensors as `matmul` inputs), no fusion API | Partial |
| "98% NAX coverage" | Share of prefill matmul + attention time, predicted from MLX dispatch rules; no capture checked | Not utilization, not all op time |
| arXiv 2606.27098 "residual cache" | GPU work evicts CPU data, seen after GPU finishes | Not simultaneous contention; abstract only |
| agx-re "consumer route" vs "slot selector" | Older explainer vs later scoreboard document | Use the later reading; M4 only |
| MFA `__asm("air.*")` async copy | 2024 source | Current frontend rejects it ([Async copy](async-copy.md)) |
| `fp_quantized_nax.h` path | Synthesis placed it in `steel/gemm/` | Actual path is `mlx/backend/metal/kernels/` |
| Asahi M3 page | Read through a fetch summary | GPU "WIP" status taken as stated; other rows not verified |
| The synthesis itself | Read from a saved copy | Every source link was checked directly |

No source link in the synthesis returned an error.
