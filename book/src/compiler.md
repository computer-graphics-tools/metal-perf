# From Metal source to GPU instructions

A Metal kernel passes through several representations before it runs. Changing an AIR call changes what the compiler is asked to lower; it does not directly choose an M5 machine instruction. Keep the source, intermediate representation, library, pipeline and executed shader distinct when judging an optimization.

```text
MSL source ──metal frontend──> AIR module ──metallib──> shader library
                                                        │
                    specialization + GPU backend <──────┘
                                  │
                           pipeline / GPU code
                                  │
                              execution
```

Apple documents the source-to-library workflow in [Building a shader library](https://developer.apple.com/documentation/metal/building-a-shader-library-by-precompiling-source-files). Offline GPU compilation and binary archives add other paths; an extension alone does not identify a payload. Inspect the particular artifact rather than assuming every Metal library is only AIR or only machine code.

## What each representation means

| Representation | What it carries | What it cannot establish alone |
| --- | --- | --- |
| MSL (`.metal`) | C++-like shader source, address spaces, types, attributes | The final schedule or register allocation |
| Textual AIR (`.ll` in our probe) | LLVM-style typed operations, `air.*` calls, target and kernel metadata | The physical opcode or accelerator executing an operation |
| Binary AIR (`.air`) | Serialized intermediate representation | Measured execution time |
| Metal library (`.metallib`) | A versioned container of functions, metadata and modules | That a function is correct or efficient |
| Specialized pipeline | Function constants, device-specific compilation and pipeline limits | That a proposed optimization wins across workloads |
| Captured GPU executable | Target-specific shader bytes and tooling interpretation | Performance on an unmeasured device/compiler combination |

An **intrinsic** is a compiler-recognized operation with particular types and semantics. `air.simdgroup_async_copy_2d.read.p3i8.p1i8` is such a request, not a textual M5 ISA opcode. An **ABI** is the agreement about argument types, address spaces and result representation between a caller and its implementation. A familiar intrinsic name with the wrong ABI can compile while doing the wrong work.

## A fresh local experiment

Checked 2026-10-10: Apple Metal `32023.921` (`metalfe-32023.921.6`), target `air64_v29-apple-macosx27.0.0`. The host GPU is M5 Max; this experiment only compiled and inspected artifacts and did not dispatch a new kernel. Full commands, hashes and outputs are in [compiler-probe.txt](evidence/2026-10-10/compiler-probe.txt). Re-running the same steps on 2026-10-13 with the same compiler produced a byte-identical `copy.metallib` (SHA-256 `0ffe757e…`).

The input was `shaders/copy.metal` (SHA-256 `1a2c2353…`) from a small local, unpublished Metal 4 async-copy demo, described in [Async copy](async-copy.md).

| Step | Observed result | Evidence boundary |
| --- | --- | --- |
| Give declarations direct `air.*` asm labels | Four compiler errors, including `illegal string literal in 'asm'` | Current frontend rejects these declarations |
| Give declarations ordinary `demo_copy` / `demo_wait` labels | Textual AIR generated successfully | Source types and metadata preserved |
| Replace the two typed symbols in declarations and calls | Expected declaration/call presence checked first | A controlled IR mutation, not general string substitution |
| Run `air-as`, then `metallib` | Both exited 0; 5,280-byte library produced | Assembly and packaging succeed |
| Inspect with `air-objdump --metallib --all-headers` | `MTLB`, function and metadata sections, module section | Container interpretation, not physical shader disassembly |

The emitted call has 12 arguments: two `i64` size/alignment values, a pointer in address space 3, two destination layout values and a 2-vector, a pointer in address space 1, two source layout values and a 2-vector, a signed origin 2-vector, and an `i32` mode. Preserve the actual generated event-pointer type; do not copy a different toolchain's declaration. AIR pointer address spaces describe memory domains; their numbers are compiler ABI details.

The demo's Cargo build script does the same thing: it emits LLVM IR with full metadata, checks that each stub has calls and a declaration, replaces the two symbols, then runs `air-as` and `metallib`. Unexpected IR stops the build.

## How a metallib is packed

For this generated library, the first four bytes are `MTLB`. The header identifies the platform, versions and section offsets. Function records contain tags such as `NAME`, `TYPE`, `HASH`, `MDSZ`, `OFFT` and `VERS`; metadata sections describe the function, and a module region carries bitcode. These are observations of this artifact, with the independent [MetalLibraryArchive format research](https://github.com/YuAo/MetalLibraryArchive#-metal-library-archive-binary-layout) as a guide, not a guaranteed Apple file-format contract.

| Region reported by current `air-objdump` | Offset | Size |
| --- | ---: | ---: |
| Function list | `0x58` | `0x84` reported by the tool |
| Public metadata | `0xe0` | `0x8` |
| Private metadata | `0xe8` | `0x8` |
| Module list | `0xf0` | `0x13b0` |

At `0xf0`, the module starts with bytes `de c0 17 0b`: a bitcode wrapper. Its payload starts 20 bytes later with `42 43 c0 de`, the LLVM bitcode signature. The header's function-list size field is `0x80`, while the tool prints `0x84`. The 4-byte difference matches the leading `uint32` function count at `0x58` (value 1); the MetalLibraryArchive notes use the same `+ 4` adjustment.

For extraction, validate each offset and size against the actual file length, interpret offsets relative to the appropriate section, and verify payload identity. Rebuilding a container by hand must also preserve function metadata and hashes. Prefer the installed `metallib` packager for experiments.

## Ways to influence emitted code

Apple also documents [offline binary archives from a pipeline configuration](https://developer.apple.com/documentation/metal/compiling-binary-archives-from-a-custom-configuration-script). On this installation, `metal-tt -h` describes the supported options (`--help` is rejected; there is no installed `metal-tt` manual). The configuration must match function specialization and pipeline state; a different configuration is not evidence about the runtime pipeline.

A fresh positive control, a public FP32 `b[i] = a[i]` kernel, compiled with `metal-tt -target air64-apple-macosx27.0 -gpu-family apple10`. `metal-lipo -archs` found six target slices: `applegpu_g17g`, `g17s`, `g17d`, `g18g`, `g18m`, `g19p`. This establishes an offline GPU-binary path for that control, without executing it.

The private-copy module packaged above failed this offline translation with `Encountered unlowered function call to air.simdgroup_async_copy_2d.read.noholes`. Verbose output identified the first emission target as `applegpu_g17g`; selecting `applegpu_g17d` separately also failed. This negative result is specific to this module/configuration. It does not prove that every runtime M5 copy path fails. The local IORegistry reports `AGXMetalG17X`; its correspondence to an offline slice was not established.

The current `air-objdump --version` advertises AGX1/2/3 targets, but disassembling the extracted public-control `g17d` slice with `--arch-name=agx3` failed: `no instruction printer for target agx3---macho`. Without `--arch-name` it fails with the same error. `metal-objdump -d` on the whole multi-slice binary prints `couldn't initialize disassembler for target applegpu_g17g-apple-ios` for every slice and still exits 0 (re-checked 2026-10-13). Target registration is therefore not proof of usable instruction decoding. Both failures, the successful control and their artifact hashes are retained in the [compiler probe](evidence/2026-10-10/compiler-probe.txt).

1. **Public source and pipeline specialization.** Tune tile shapes, layouts, alignment, vector loads, unrolling, math mode and function constants. Compare public SIMD matrix operations with MPP cooperative tensors where the workload permits. This is the first choice because the compiler owns legal lowering and the API is maintained.
2. **Typed AIR investigation.** For an operation inaccessible from current source, generate a valid module, mutate one precisely checked intrinsic request and rebuild. Keep this in an isolated experiment with exact compiler hashes, full ABI, poisoned output, numerical tests and a baseline. It is a research route, not an implicit production fallback.
3. **Physical ISA investigation.** Obtain the specialized executable, identify its GPU generation, use a matching decoder, and connect decoded instructions to execution and counters. Unknown encodings remain unknown. Host ARM64 compiler disassembly is not GPU disassembly.

Do not enable broad fast-math transformations just to improve a benchmark: the accepted numerical contract must permit them. Likewise, more unrolling or larger tiles can increase register pressure and spills. Inspect and measure the resulting pipeline instead of treating source complexity or an intrinsic count as a throughput estimate.

Loop unrolling can also be requested at the AIR level. `metal -O2 -S -emit-llvm` output is pre-unroll: a loop marked `#pragma clang loop unroll(disable)` carries one `llvm.loop.unroll.disable` metadata node. Replacing that node with `llvm.loop.unroll.count` N and running `air-opt --passes=loop-unroll` produces N main-loop copies plus a remainder. Checked on 2026-10-13 with a dot-product kernel built with `-fno-fast-math -ffp-contract=off`: unroll 4 gave four multiply/add pairs plus one remainder pair, added no fast-math flags, and the result assembled and packaged. `[[max_total_threads_per_threadgroup(256)]]` emits **two** declarations: the function attribute `"max-work-group-size"="256"` and kernel metadata `!{!"air.max_work_group_size", i32 256}`. An edit must change both consistently and keep launch threads within the resulting pipeline limit. These are compile-only results. A safe mutation selects one declared kernel and one eligible loop, fails on missing or ambiguous matches, and preserves ABI metadata; global replacement across unrelated loops is not safe.

The host compiler's static defaults (2026-10-11, from ARM64 assembly of `libapplegpu-nt.dylib` in Metal toolchain 27.1.266.1) are `misched-regpressure` **on** and `enable-deferred-spilling` **off**. Hopper's pseudocode hides the byte stores that set them. Both are generic upstream LLVM options with these same defaults and description strings (`MachineScheduler.cpp`, `RegAllocGreedy.cpp`), so they reveal nothing Apple-specific. These are constructor values at host addresses, not the live pipeline's state. A runtime service may override them, no accepted path to set either has been verified, and no GPU opcode effect is proven ([reverse engineering](reverse-engineering.md#compiler-scheduling-leads-from-the-current-saved-project), [evidence](evidence/2026-10-10/compiler-control-defaults.json)).

## A robust custom-library workflow

A build driver around Apple's compiler and packager keeps the source, emitted LLVM/AIR, declared edits, assembled AIR and final metallib together, verifies tool identities and rejects failed edits or compilation. This preserves a readable input and a reproducible route to a device-loadable library.

```text
MSL source → Metal frontend → pristine LLVM/AIR
                                  ↓ version/ABI checks
                              declared AIR edits
                                  ↓ AIR verification
                              air-as → metallib
                                  ↓ actual device pipeline creation
                         numerical checks → calibrated comparison
```

Two approaches remain distinct:

| Approach | What is supported now | Research still needed |
| --- | --- | --- |
| Drive Apple's frontend, AIR tools and official packager | Loop-metadata AIR edits assemble and package; an unedited module round-trips byte-identically (below) | Kernel-specific numerical tests and calibrated performance acceptance for each custom kernel |
| Patch a compiled metallib container | Container/header/bitcode observations from identified artifacts | Complete versioned offsets, lengths, integrity fields and runtime-loading tests before a safe writer exists |

The first approach is the recommended foundation. Reimplementing an MSL compiler frontend would be a much larger project. Repackaging edited AIR through `metallib` lets the official packager construct its container metadata. A successfully loaded pipeline still needs a kernel-specific numerical contract; it does not establish correctness or a speedup for arbitrary custom code.

A no-edit round trip is byte-stable. Extracting the module region of the async-copy probe library above (offset `0xf0`, length `0x13b0`) and passing it back to `metallib` reproduced the original library byte for byte (SHA-256 `0ffe757e…`, checked 2026-10-13). Its stored function `HASH` equals the SHA-256 of the whole wrapped module, not of the inner bitcode. Passing a whole metallib to `metallib --function-name <name>` fails with `LLVM ERROR: Unexpected bitcode file!`. These are checks of one container form, not a general parser, a universal metallib patcher or a native-opcode writer. Binary archives and other versions require their own format validation.

Keep every pristine artifact immutable and give each mutation a new output directory. No generic byte-patching or physical-opcode capability is claimed.

## What we still need to establish

The current compilation probe establishes that the typed async-copy request can be packaged on this toolchain. It does not establish the runtime backend selected on M5, the physical instruction sequence, copy/compute overlap or a performance gain. The next compiler experiments should keep a public baseline, change one request at a time, verify output, inspect a bounded capture, then compare uninstrumented timing under the [autotuning contract](autotuning.md).

## Concrete AIR boundary of an INT8 MPP matmul

An MPP cooperative matmul with a device INT8 left operand and a threadgroup INT8 right operand (M=16, N=34816, K=5120; 32×64×64 tile, four SIMD groups) emits `__tensorops_impl_matmul2d_op_run_cooperative_dv_i8_tg_i8_i32_v2`. This is the runtime implementation symbol behind the public MPP API: device INT8 left operand, threadgroup INT8 right operand and INT32 partial output. The symbol is present in `libTensorOps.rtlib`. Scaling and FP32 accumulation stay in the calling shader. [Retained AIR identity and locations](evidence/2026-10-10/mpp-int8-air-boundary.json).

Its frontend AIR also retains descriptor/array allocations, deferred cooperative-storage sizing, and calls for valid-element checks, coordinate lookup and element-pointer access. These calls provide concrete places to investigate compiler work in the reduction loop. They are not a native instruction count or proof that a particular allocation becomes a spill: target linking and lowering occur later. The attributed Xcode profile supplies the separate evidence of NAX activity.

Do not assign `readnone`, `readonly` or alias properties to these calls from their names alone, and do not substitute a guessed cooperative layout. A custom AIR recipe must preserve a verified signature and semantics, pass full numerical checks, and earn an independent uninstrumented comparison. The robust route remains a checked edit of retained AIR followed by Apple's assembler, packager and actual device pipeline creation.

For backend evidence, continue with [Reverse engineering](reverse-engineering.md). For the async-copy case and the conflicting literature, read [Async copy](async-copy.md).

## Tensor runtime compiler controls (bounded RE)

The host-compiler observations about tensor-lowering environment flags and offline translation of MPP helpers are in [Reverse engineering](reverse-engineering.md#m5-isa-research-boundary). None of them was shown to affect a runtime pipeline. On INT4 arithmetic, later AIR inspection of `libTensorOps.rtlib` (macOS 27.0.1) found that MPP's int8 × int4 cooperative matmul widens the nibbles with ALU code and then issues the same int8 16×16×16 widening multiply-accumulate intrinsic as int8 × int8: there is no native int4 matrix operand in that AIR.

## Private compiler controls: interface presence is not backend control

These are private, undocumented interfaces observed on macOS 27.0.1. The 11 October runtime probe finds `additionalCompilerArguments` on `MTLCompileOptionsInternal`, and `driverCompilerOptions` / `gpuCompilerSPIOptions` on the legacy compute-pipeline descriptor (concrete class `MTLComputePipelineDescriptorInternal`). These dictionary selectors are absent from the inspected Metal 4 descriptor. Its `MTL4PipelineOptions` instead exposes a private unsigned `maxNumRegisters` getter/setter, with an observed default of zero. An Objective-C runtime check on 2026-10-13 confirmed all of these selector observations and the zero default. Zero's backend meaning is not established.

The controlled tests, run on a quantized GEMM test kernel, do **not** demonstrate a usable optimization switch:

| Probe | Observation | Conclusion |
|---|---|---|
| Invalid top-level compiler argument | Library creation fails with an unknown-argument error | The additional-argument channel is not wholly ignored |
| `-mllvm -enable-deferred-spilling=true` or `-mllvm -misched-regpressure=false` | Library and pipeline creation succeed | Success alone cannot establish target-backend parsing |
| An invented `-mllvm` flag name | Also succeeds | A necessary negative control fails; do not claim these LLVM flags work |
| Metal 4 `maxNumRegisters = 64` or `256` | Getter round-trip succeeds; the kernel's outputs stay byte-exact | The descriptor stores the request; this is not enforcement evidence |
| Xcode profile of the same kernel | Unchanged: 1,191 instructions, 126 temporary registers and 128 spilled bytes | No effective register-budget change demonstrated |

The register probe applies options to the pipeline and specialized function. A second 64-register probe also applies them to the underlying library function and reassigns the copied descriptors afterward; its statistics remain unchanged. Equal statistics are not proof of identical machine-code bytes. No calibrated timing sweep is justified by these results.

Further private-flag research needs to trace option serialization into the target compiler rather than assume a successful setter or compiler return means control. [Evidence](evidence/2026-10-11/private-compiler-options.json).

### Register-limit field traced, 12 October 2026

This is a reverse-engineering observation of a saved Hopper database of `AGXCompilerCore` (SHA-256 `2fa3413fb56606c7acaef876a8bc6cdd18343de192eccf5551c19ac75ba2bf8c`), not of a documented interface. No evidence file was retained; on 2026-10-13 the Hopper MCP server confirmed the procedure names and addresses below, the `AGC_TEMP_REGS_IN_BYTES` gate and the bit-field extraction in that database:

- `AGCLLVMUserComputeKernelBase::getCompilationKeyDescription` reads `maxTempRegisters` from bits 10–25 of the compute-state word at offset `0xc` (`0x1b048db4c` / `0x1b048db50`).
- `getMaxTempRegistersOverride` (`0x1b048bcd8`) first asks the base implementation, then reads that field if the base returns zero.
- The base implementation (`0x1b05935f4`) consults `AGC_TEMP_REGS_IN_BYTES` only behind `AGCEnv::isEnabled`. Nonzero byte counts are converted to 32-bit register units by `(bytes + 3) >> 2`. A missing value or a value beginning with `0` yields no override.
- `getTempRegistersLimit` (`0x1b048bd58`) consumes an override and also applies a separate resource-dependent restriction. This establishes a compiler control path in the saved artifact, not successful runtime control.

A read-only native probe loaded the current system framework. Its UUID is `e456618b-e277-31d8-8de1-cd76c58f3425`; preferred `__TEXT` address is `0x1b0404000`. `os_variant_has_internal_content("com.apple.AGXCompilerCore")` returned false, and the internal `AGCEnv::isEnabled` symbol was not exported. The database has not been matched by UUID to this live image, so its recovered offsets are not used as live function pointers. No environment override, binary patch, system setting or GPU execution was attempted. No performance gain is claimed.
