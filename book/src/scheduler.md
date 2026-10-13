# Metal 4 submission and GPU scheduling

Two scheduling layers affect a matmul measurement. Metal and its driver submit command streams and enforce dependencies. The GPU then distributes threadgroups and issues instructions inside resident SIMD groups. A shorter submission path does not establish better SIMD occupancy, and a larger threadgroup does not establish more useful arithmetic per second.

```text
Host encoding ──────encode──> Metal 4 command buffer ──commit──> Queue
                                    ↑                          ↑
                       allocator: command memory    residency: resources
                                                               │
                                                    driver command stream
                                                               ↓
GPU dispatch ──threadgroups──> resident SIMD groups ──results──> output
```

## Public submission contract

Metal 4 separates command buffers, their command-memory allocator and command queues. The host must finish using an allocator before resetting it. It must include every buffer used by a submitted operator in a residency set and keep those buffers alive until completion. Residency here means resource accessibility; it is distinct from a threadgroup occupying GPU execution resources. [Apple's Metal 4 core API](https://developer.apple.com/documentation/metal/understanding-the-metal-4-core-api) and [Discover Metal 4](https://developer.apple.com/videos/play/wwdc2025/205/) explain these responsibilities.

Compute commands within a pass may run concurrently, and Metal 4 treats all resources as untracked. Encoding input transform, activation quantization, matmul and output transform in order is therefore insufficient to establish their dependencies. Insert dispatch-to-dispatch intrapass barriers between dependent components. Serialize repeated complete operators too, so that a doubled-count positive control measures additional work. Cross-pass fences and cross-queue events have different scopes. [Apple's resource synchronization](https://developer.apple.com/documentation/metal/resource-synchronization) is the governing contract.

The primary result is the complete operator span reported by uninstrumented commit feedback. Per-component diagnostic timestamps and capture/replay profiles are separate experiments. The pipeline must preserve these same dependencies during correctness, timing and capture; a profiler's replay is not an acceptance run.

## Current reverse-engineering artifact

This section is a reverse-engineering observation of the Metal driver on macOS 27.0.1 (build 26A434). The `AGXMetalG17X` executable is not an ordinary file: `/System/Library/Extensions/AGXMetalG17X.bundle/Contents/MacOS/` is empty, and the image lives in the macOS dyld shared cache. Hopper's **Read File From DYLD Cache** loaded its ARM64 image on 2026-10-10. That Hopper database was deleted in the 12 October cleanup. A cache-backed Hopper database also contains imported pages and analysis state, so its hash is not the hash of a standalone driver executable.

The routines below came from the Hopper MCP server, not from REA, so they carry no REA evidence IDs.

| Routine | Entry point | Verified host observation |
| --- | --- | --- |
| `AGXG17XFamilyCommandQueue_mtlnext commit:count:options:` | `0x21874804c` | ARM64 branch compares command-buffer count with 2 and calls different targets. Those targets are not yet resolved; this alone does not prove merging behavior. |
| `noMergeCommit:count:options:commitFeedback:error:` | `0x218747f14` | Conditionally calls `InternalSparseQueue::flushAndSubmitAllMappings`, then another submission target. |
| `ComputeContext_mtlnext dispatchThreadgroups:threadsPerThreadgroup:` | `0x218797698` | Checks threadgroup dimensions/limits and calls the next-encoding `executeKernelCommonImpl`; some conditionally enabled paths split work. Which workloads take those paths is unverified. |
| `barrierAfterEncoderStages:beforeEncoderStages:options:` | `0x218799b68` | Calls `encodeBarrier` on one path and stores deferred stage/option flags on another. Public stage-bit correspondence has not been decoded. |
| `ComputeContext<…CommandEncodingNext…>::executeKernelCommonImpl` | `0x218745934` | Writes dimension-related data into an allocated buffer and calls `performEnqueueKernel`. This is host command preparation, not GPU instruction issue. |

`search_procedures`, `procedure_info`, `procedure_assembly`, `procedure_pseudo_code` and `procedure_callees` answered these questions. Their results are summarized in [hopper-metal4-submission.json](evidence/2026-10-10/hopper-metal4-submission.json). The published copy keeps symbols, addresses, basic-block ranges and call lists; the assembly and pseudocode bodies of Apple's binary were removed. Prefer assembly when pseudocode loses argument sizes or confuses pointer indexing. Hopper's inferred `int` signatures are not a callable ABI. No private routine was invoked and no command bytes were patched.

A read-only runtime identity probe created public Metal 4 objects, ended the empty encoder/command buffer and submitted no GPU work. It reported `Apple M5 Max`, device class `AGXG17CDevice`, and queue/allocator/command/encoder classes `AGXG17XFamily…_mtlnext`. Objective-C `class_getImageName` for the queue and encoder returned the exact AGXMetalG17X bundle image selected in Hopper. This verifies the runtime class/image association on this machine; it is not a measurement of dispatch behavior. A 2026-10-13 re-check (device, queue, allocator and command buffer only, no submission) returned the same device, queue, allocator and command-buffer classes and the same image path.

The next host routine, `performEnqueueKernel` at `0x218732084`, has 329 basic blocks. Its call inventory includes `prepareForEnqueue`, `directTGSizeOptimization`, `bindResources`, `checkSpillParamsForCompute`, `reserveEnqueueDatabufferSpace`, `encodeBarrier`, `FenceToken`, `emitUscStateLoad`, and `allocateUSCSpillBuffer`. See [hopper-metal4-enqueue.json](evidence/2026-10-10/hopper-metal4-enqueue.json). Whole-function pseudocode was not requested for this large routine.

Those calls give useful experiment targets: vary threadgroup size at a fixed shader, then vary compiler unrolling at a fixed launch; inspect register/spill and traffic evidence for the resulting dispatch. Static constants in these host heuristics cannot be promoted into physical residency counts. In particular, the meaning and active-path applicability of their caps require runtime correlation.

## Evidence needed for the two layers

| Question | Evidence and controlled comparison | Current boundary |
| --- | --- | --- |
| What does a Metal 4 commit encode or submit? | Identified current driver function, call graph and observed API path | Current host routines retained; submission targets/runtime correlation remain |
| What implements a dispatch barrier? | Driver encoding routine plus captured dependency behavior | Public API contract verified; private encoding unverified |
| Does queue submission dominate small M? | Single-operator versus serialized repeated-operator spans with raw positive controls | Not established here |
| How does threadgroup size affect residency? | Same arithmetic/artifact, controlled launch sizes, supported occupancy/register/traffic profile | Pipeline limits alone are not occupancy |
| Does a tile stall on memory, dependencies or matrix issue? | Attributed dispatch counters and timeline with units and coverage | Replay can attribute dispatch costs; counters alone do not establish the bottleneck or NAX utilization |
| What is the hardware scheduling policy? | Generation-matched decoding or controlled residency/issue experiments | No M5 hardware scheduler algorithm established |

A scheduler experiment can vary launch threads and the compiled maximum workgroup bound independently. The current compiler emits both the `max-work-group-size` function attribute and the `air.max_work_group_size` kernel metadata (checked 2026-10-13), so an AIR-level change must edit both. Packaging success only verifies the AIR container contract. Read actual pipeline limits, run correctness, and profile the resulting dispatch before claiming an occupancy change. See [Compiler experiments](compiler.md#ways-to-influence-emitted-code).

Older Asahi or Apple GPU generations can suggest experiments; they do not identify M5 scheduler behavior.
