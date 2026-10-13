# Evidence ledger

This ledger lists findings recorded on 10 and 11 October 2026. “Verified locally” means observed on the M5 Max described in the [introduction](introduction.md), with the command recorded. “Source report” means the cited author or supplied artifact reports it. A proposal or hypothesis remains unaccepted until its required experiment runs. Evidence files not listed below are linked from the chapter that uses them ([Profiling](profiling.md), [Compiler](compiler.md), [WWDC25–26 APIs](wwdc25-26-api-plan.md)).

## Findings

| ID | Claim | Evidence | Boundary / remaining question |
| --- | --- | --- | --- |
| L01 | Local GPU identifies as M5 Max with 40 GPU cores and Metal 4 | `system_profiler SPDisplaysDataType`; [toolchain snapshot](evidence/2026-10-10/toolchain.txt) | Device identity; does not reveal unpublished execution-unit counts |
| L02 | Current compiler rejects direct private AIR asm labels | Fresh `metal -std=metal4.0` compile, exit 1; [compiler probe](evidence/2026-10-10/compiler-probe.txt) | These declarations/toolchain only |
| L03 | Typed async-copy IR assembles and packages | `air-as` and `metallib`, exit 0; same probe | No new GPU dispatch, numerical result or speedup |
| L04 | Generated library contains MTLB metadata and wrapped bitcode | `air-objdump --metallib --all-headers`, header bytes and hash; same probe | This artifact and container version |
| L05 | Installed backend is a universal (x86_64 + arm64) host Mach-O with AIRNT exports | REA `inspect_macho`; [record](evidence/2026-10-10/rea-macho.json) | Native layout, not M5 ISA or runtime backend selection |
| L06 | GPU capture/profile CLIs are installed, and one bounded compute capture was replayed | `xcrun --find`, installed manuals; one `gpudebug` replay (trace not retained) | Replay costs are diagnostic, not acceptance timing |
| L07 | The private-copy demo has existing copy/matmul contracts and reports | Supplied README and Metal shaders (not retained here) | Source/artifact inspection; historical measurements were not rerun |
| L08 | A public copy control translates to Apple10 GPU binaries, while this private copy fails offline lowering | `metal-tt`, `metal-lipo`; [compiler probe](evidence/2026-10-10/compiler-probe.txt) | This module/configuration; no new runtime conclusion |
| L09 | AGX3 registration does not provide an instruction printer for the extracted control | `air-objdump` default and explicit-AGX3 disassembly, same probe; `metal-objdump` fails the same way on cached GPU binaries (2026-10-12) | Recorded failure; no physical opcodes decoded |
| L10 | A host architecture-query routine assembles records and a count | REA `analyze_function`; [dossier](evidence/2026-10-10/rea-supported-architectures.json) | Recovered host representations; complete body ranges unavailable |
| L11 | The inspected compiler contains two base async-copy names | REA `search_strings`; [record](evidence/2026-10-10/rea-copy-strings.json) | Name presence, not successful lowering or execution |
| L12 | Loop-unroll metadata edits in AIR assemble and package | Fresh strict Metal 4 probe, `air-opt`, `air-as`, `metallib`; [Compiler](compiler.md#ways-to-influence-emitted-code) | Compile-only; numerical correctness and timing still required |
| L13 | Installed cache image has distinct Metal 4 commit/dispatch/barrier host routines | Hopper; [symbols, addresses and call lists](evidence/2026-10-10/hopper-metal4-submission.json) | Host static evidence; no hardware scheduling algorithm established |
| L14 | An unedited metallib module round-trips byte-identically through `metallib` | Module extraction and `metallib`; [Compiler](compiler.md#a-robust-custom-library-workflow) | One container form; no general container writer or native opcode patching |
| L15 | The source leads in a supplied October research report were checked against their sources | [Source audit](research-update-2026-10.md) (pinned commits or dates) | Author measurements remain source reports; Apple9 and base-M5 results do not establish local g17s behavior |
| L16 | GPU power limits can be read without root through the AGX driver's IOKit properties | Read-only IOKit property reads; [Telemetry](telemetry.md#m5-max-power-control-and-stable-profiling-2026-10-11) | Changing a limit is out of scope for this book; a power limit is not a frequency lock |
| L17 | Installed Xcode profiling CLI offers explicit GPU state levels | Installed `gpudebug` manual: `profile run --gpu-state default\|low\|medium\|high` | Later profiles used `--gpu-state high`; recovery of clocks after such a profile was not tested |

The REA evidence identifier for L05 is `ev_6bf8c397379f86c3d56b7269d92109d8f37eee6bdbb339b98a22536336bc3b57`. The target hash, selected architecture, provenance and limitations are in the record. The analysed binary (`libapplegpu-nt.dylib`) belongs to the installed Metal toolchain and is not redistributed here.

**Trimming and redaction (13 October 2026).** The Hopper and REA records of Apple binaries keep symbols, addresses, basic-block ranges, call lists and summaries; their assembly and pseudocode bodies were removed. In the kept files, a scratch directory is shown as `<scratch>/`, the deliberately invalid compiler flags used as negative controls are shown as `<invalid-flag>` and `-mllvm <invalid-backend-flag>`, and engine-specific workload labels and paths were replaced or removed. No measured value was changed.

## Claims that remain unresolved

| Question | Evidence required before promotion |
| --- | --- |
| Which M5 physical instruction implements the private copy? | Specialized executable, generation-matched decoding, controlled runtime data movement |
| Which matrix routes reach the per-core Neural Accelerators, beyond MPP int8 and dense BF16? Those two show Neural Accelerator counter activity, and MPP int8 lowers to a 16×16×16 AIR matrix intrinsic ([M5 hardware](m5-hardware.md)) | Per-route counters on a replayed capture; the physical instruction needs a decoder, and none is public |
| Can copy overlap improve a matmul? | Same arithmetic/layout, exact correctness, calibrated screening and held-out timing |
| How many threadgroups reside for a tile? | Supported occupancy evidence or a carefully scoped resource experiment, not allocation arithmetic alone |
| What fraction of an accelerator's ceiling is useful work? | Defined units/coverage, matching positive-control ceiling and route-specific operation accounting |
| Can one smaller kernel family replace several specialized kernels? | Contract coverage across dtype/packing/tails/epilogues, code deletion and accepted performance gates |

## Recording future experiments

Start with the exact hypothesis, numerical contract, baseline and preregistered acceptance rule. Record device/OS/compiler/source/artifact identities, workload shapes, dtypes, layouts, preparation costs, candidate parameters, power/thermal conditions and scheduling/randomization. Preserve diagnostic traces separately from timing records, including failures and invalid runs. Link every result to its manifest and mark missing measurements unavailable.

An accepted result needs reproducible useful-work time and an independent repeat. Counters can explain it; they cannot substitute for it. Update the claim's evidence status and search boundary whenever new data contradicts earlier notes. The [source index](sources.md) and [page review](metalworking-review.md) track reading coverage rather than local benchmark acceptance.
