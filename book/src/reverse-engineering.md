# Reverse engineering with retained evidence

Use binary analysis to answer a question the source and public API cannot answer. Start from an identified artifact and a falsifiable claim, then keep the returned evidence with its limitations. A backend's string table, decompiled host function and executed GPU shader answer different questions.

```text
question ──> artifact + hash ──> REA / Hopper ──> finding + unknowns
                                    │                 │
                              host compiler      GPU probe / capture
                                                      │
                                           numerical + timing evidence
```

## Current connected stack

The analysis used REA (`rea-agents` 6.0.0, 2026-10-10) with Hopper as its disassembly provider, plus Hopper's own MCP server when REA requests stalled. Findings from the Hopper MCP server carry no REA evidence ID.

The main artifact was the downloadable Metal toolchain's `usr/metal/32023/lib/libapplegpu-nt.dylib` (Metal toolchain 27.1.266.1), SHA-256 `2c1e80a0c2f716229332411903ed9b12f081668d85b59af1beae6e9f87989c3f`. The installed file still has this hash on 2026-10-13. It contains ARM64 and x86_64 host slices; the ARM64 slice was analyzed. This is a host compiler library, not an M5 GPU executable. Opening it does not establish that a particular runtime pipeline uses this library.

The Hopper databases built for this work were not kept. The JSON dossiers below are the record. The published copy keeps symbols, addresses, basic-block ranges and call lists; the assembly and pseudocode bodies of Apple's binary were removed. Before reusing any saved project, check that its input artifact hash matches the installed binary; a familiar project name does not guarantee the same compiler build.

## What the native inventory establishes

REA's `inspect_macho` returned evidence `ev_6bf8c397379f86c3d56b7269d92109d8f37eee6bdbb339b98a22536336bc3b57`. One copy of the result (header, architectures, exports, UUID) is kept in [rea-macho.json](evidence/2026-10-10/rea-macho.json).

| Observation | Location in selected host slice | Interpretation |
| --- | --- | --- |
| `_AIRNTGetSupportedArchs` export | `0x1a1b10` | An entry point to investigate supported target selection |
| `_AIRNTSupportsTarget` export | `0x1a1d20` | A target-support query exists |
| `_AIRNTEmitAssembly` export | `0x1a21cc` | A host compiler emission entry point exists |
| `_AIRNTEmitPipelineImages` export | `0x1b3308` | An entry point for pipeline-image emission exists |
| Build metadata | SDK 27.0, minimum OS 14.0 | Metadata of the inspected library, not its GPU generation |

These are symbol and layout observations. They do not establish callable public APIs, argument contracts, physical matrix instructions or supported usage. The REA result warns that stripped or hidden symbols may be absent and that only `otool` build metadata is normalized; `vtool` output is retained as provenance.

REA's `analyze_function` examined `_AIRNTGetSupportedArchs` at `0x1a1b10`, evidence `ev_61a8b5530c7bb1d0f5b1102146996dffcc59126f7cceda60f65831dfd790accc`. The [retained dossier](evidence/2026-10-10/rea-supported-architectures.json) contains the call edges and summary. It shows a call to `sub_ac2fe8`, two allocations, iteration over collected records and a returned count. That suggests dynamically assembled architecture information; it does not identify the target of the current M5 runtime pipeline. Complete function body ranges and reference-kind classifications were unavailable from Hopper's public API.

REA's `search_strings` found `air.simdgroup_async_copy_1d` at `0x4766ea5` and `air.simdgroup_async_copy_2d` at `0x4766ec1`, evidence `ev_68175cf9d812c94a512bdd20176842ac02c31ba4d884a930b6ed720ec32b76da`. The [full string-search result](evidence/2026-10-10/rea-copy-strings.json) establishes name presence only. In particular, it cannot override the [observed offline lowering failure](compiler.md#ways-to-influence-emitted-code).

## Compiler scheduling leads from the current saved project

On 2026-10-10, Hopper MCP's `search_strings`, `xrefs`, `procedure_info` and `procedure_pseudo_code` connected two descriptions to option-registration routines in the ARM64 host compiler. The Hopper database SHA-256 was `7c9eb5c4f743952b703b713a374de3ae1a8d3c6879f5deb1f19c5e4f0a5e54a7`; the underlying compiler identity is the artifact hash above. [Retained registration evidence](evidence/2026-10-10/compiler-scheduling-registration.json).

| Description address | Referencing instruction / procedure | Recovered option registration |
| --- | --- | --- |
| `0x456674f` | `0x1412e7c`, `sub_1412cd8` | `misched-regpressure`; the same routine registers `misched-topdown`, `misched-bottomup`, `enable-misched` and `enable-post-misched` |
| `0x456db4c` | `0x143304c`, `sub_1432e14` | `enable-deferred-spilling` in a routine that also registers a greedy register allocator |

These are compiler instruction scheduling and allocation leads, distinct from Metal command submission and hardware threadgroup scheduling. Recovered pseudocode establishes registration, but does not establish CLI acceptance, the default value, use by our runtime pipeline, or a performance effect on M5. Generic LLVM-style options also do not establish M5-specific control. A useful experiment must first identify register or spill cost in the attributed GPU profile, then change one verified compiler option, preserve its artifact hashes, and rerun numerical and independent timing checks.

**Static defaults (2026-10-11).** The ARM64 assembly of the two constructors in the same artifact shows byte stores that Hopper's pseudocode omits.
- `misched-regpressure` (option object `0x4bdad98`, value storage `0x4bdae18`): constructor `0x1412cd8` initializes it to **true** (`0x1412cf4 mov w23,#1` → `0x1412e84 strb` → `0x1412ea4 call 0x11de26c`). It is read at `0xa790c4` (`ldrb`) and tested at `0xa790c8` (`tbz`) in `0xa78fe0`.
- `enable-deferred-spilling` (object `0x4bdd748`): constructor `0x1432e14` initializes it to **false** (`0x1433040 strb wzr`). Its consumer is not yet traced.
- A bounded string search of `AGXMetalG17X` for compiler-option or SPI-argument names found 0 matches. That search covers indexed strings only, not Metal.framework selectors or macOS interfaces.

Both values are the upstream LLVM defaults for these generic options: `misched-regpressure` is `cl::init(true)` in `MachineScheduler.cpp` and `enable-deferred-spilling` is `cl::init(false)` in `RegAllocGreedy.cpp`, with the same description strings as the recovered registrations. These are host-processor storage addresses and static constructor values. They are not the live pipeline's option state, which a runtime service may override. They are not hardware scheduler evidence either. No accepted path to set either flag has been verified, and no native GPU opcode effect is proven. [Curated evidence](evidence/2026-10-10/compiler-control-defaults.json).

`xcrun metal-tt --help` exits 1 with `unknown argument: --help`; `xcrun metal-tt -help` (or `-h`) prints the options. That help lists no option for passing these scheduling options to the backend.

## Work by question

| Question | First tool / evidence | Follow-up needed |
| --- | --- | --- |
| What file and architecture is this? | `open_binary`, `inspect_macho`, SHA-256 | Match the actual compilation/runtime artifact |
| Does the backend recognize a name? | `search_strings` and cross-references | Find the dispatch/lowering routine; string presence alone is weak evidence |
| How does a lowering routine handle operands? | `search_procedures`, `analyze_function` | Typed minimal probe, numerical tests and bounded GPU inspection |
| Which physical instructions execute? | Specialized GPU executable plus matching decoder | Connect bytes to device, compiler, dispatch and runtime evidence |
| Does the change help? | Separate uninstrumented balanced comparisons | Independent confirmation under the accepted numerical contract |

Index construction can take minutes for a large compiler. Empty results during construction are not absence evidence. Record incomplete coverage explicitly if analysis is intentionally bounded.

For each function, record the provider-returned address, inclusive body ranges, decompiler limitations, callers/callees and relevant strings. Label recovered pseudocode as analysis output rather than original source. Keep host ARM64 disassembly and Apple GPU disassembly in separate findings even when they describe the same operation.

## M5 ISA research boundary

[dougallj/applegpu](https://github.com/dougallj/applegpu) and [Asahi Mesa](https://gitlab.freedesktop.org/mesa/mesa/-/tree/main/src/asahi) are primary reverse-engineering sources. Record a decoder revision and the generations it supports. A decoder that understood an earlier AGX generation may reject or misinterpret M5; unsupported instructions must remain undecoded. A short byte pattern or the word `matrix` in a compiler string is not enough to identify a physical Neural Accelerator operation.

Use the current [`air-objdump` capabilities](compiler.md) and [GPU profiling tools](profiling.md) before adding tooling. This host has no discoverable `air-dis` utility (`xcrun --find` exits 72). Textual IR can be generated at compilation, and `metal-objdump -d` prints the AIR of already packaged libraries, including `libTensorOps.rtlib` (checked 2026-10-13).

**INT4 lookup (2026-10-11).** The host compiler `libapplegpu-nt` contains the string `int4b`; a bounded search identified no procedure that uses it, so no lowering claim follows from the compiler binary. The question was later answered from the other side: AIR inspection of MPP's `libTensorOps.rtlib` (macOS 27.0.1) shows the int8 × int4 cooperative matmul widening nibbles in ALU code and then issuing the same int8 16×16×16 widening multiply-accumulate intrinsic as int8 × int8. There is no native int4 matrix operand in that AIR.

**Tensor runtime compiler controls (2026-10-11).** These are unverified reverse-engineering notes about `libapplegpu-nt` from Metal toolchain 27.1.266.1; their raw record was not retained.

- **Strings present.** The binary contains `AGC_ENABLE_LEGACY_PACK_UNPACK_QUANTIZATION`, `NumericPackUnpackPass` and `QuantizationHW` (string search, 2026-10-13). The 2026-10-11 analysis reported that `_AIRNTInit` reads this flag from an environment whitelist and that it selects between those two tensor passes. That mechanism is not backed by a retained evidence file. Nothing shows the flag's state or any effect in a real Metal 4 runtime pipeline.
- **Offline translation.** The default offline translator reportedly fails with `unsupported deferred-static-alloca-size` in the MPP cooperative-tensor helpers (the string exists in the binary). The alternative `-use-nt applegpu25-nt` is rejected with `air-tt: invalid 'applegpu25-nt' translator`, even for a trivial public kernel (re-checked 2026-10-13).
- **Runtime library archives.** At the time, `air-opt` refused payloads extracted from the AIR runtime-library archives (missing `air.version`). Later work read `libTensorOps.rtlib` AIR with `metal-objdump` instead, which gave the int4 result above.
- **Not shown:** native g17s decoding or hardware scheduling.

## SDK27 scheduling leads for persistent and streaming-K work

The installed headers (macOS SDK 27.0) declare `forwardProgressUsage`, `contentionRelief` and `optimizeForPersistentKernel` on both `MTLComputePipelineDescriptor` and `MTL4ComputePipelineDescriptor`, available from macOS 27. Forward-progress values are Automatic = 0, Weak = 1 and SIMDGroupParallel = 2; contention relief is Automatic = 0 or None = 1. Both descriptor types default to Automatic, Automatic and persistent optimization off (runtime check, 2026-10-13). The header declarations carry no documentation comments, and the names alone do not establish hardware guarantees.

A compile-only probe on 2026-10-13 created `MTLComputePipelineDescriptor` pipelines for a trivial FP32 copy kernel with each forward-progress setting, with persistent optimization off and on. Automatic resolves to Weak; explicitly requesting SIMDGroupParallel is reflected in the pipeline state. This does not establish greater concurrency or throughput.

The pipeline's `recommendedPersistentThreadgroupsPerGridForThreadsPerThreadgroup:` returned 0 at 32, 64 and 128 threads in every case, including with persistent optimization enabled. Treat that result as unusable for grid selection on this tested host, not as a zero-work launch plan. Persistent/streaming-K grid density still needs measured tuning.

## What to retain

Keep original binary identity, provider/version, evidence IDs, raw results, coverage and the question answered. Keep derived files and bounded `.gputrace` captures with the experiment that produced them. Do not alter installed compiler binaries to test a hypothesis. Keep typed AIR experiments outside any production compiler path.
