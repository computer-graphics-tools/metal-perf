# GPU execution model

A Metal compute dispatch launches a grid of threadgroups. Each threadgroup runs on one GPU core, and its threads run in SIMD groups of 32. Several of the numbers a matmul kernel depends on are fixed when its pipeline is built, not when it is dispatched. In Metal 4, ordering between dispatches is also the application's job. The search space and the measurements in this book come from these two facts.

## The levels

```text
grid ── threadgroups ── simdgroups (32 lanes) ── threads
 │          │                 │
 │          │                 └─ registers; simd_shuffle; simdgroup_matrix 8×8;
 │          │                    MPP execution_simdgroup
 │          └─ threadgroup memory (≤ maxThreadgroupMemoryLength = 32 KiB here);
 │             threadgroup_barrier; MPP execution_simdgroups<N>
 └─ device memory (unified); ordering only between dispatches, via barriers
```

- **Threadgroup**: the unit that shares threadgroup memory and barriers. It always resides on one core.
- **SIMD group**: 32 threads in lockstep. The pipeline reports this as `threadExecutionWidth`; read it instead of hard-coding 32.
- **Occupancy**: how many SIMD groups are resident on a core at once. The Metal API does not report it; a `gpudebug` replay reports occupancy counters (see the last section). It follows from register and threadgroup-memory use, which the compiler decides. More occupancy hides latency. It does not by itself raise throughput.

## Pipeline-time contracts

The register allocator runs when the pipeline is built. Anything the kernel promises at that point limits the legal dispatch shapes:

| Contract | Where it is set | Where it is observed |
| --- | --- | --- |
| Max threads per threadgroup | `max_total_threads_per_threadgroup` attribute or `MTLComputePipelineDescriptor.maxTotalThreadsPerThreadgroup` | `pipeline.maxTotalThreadsPerThreadgroup` |
| SIMD width | hardware | `pipeline.threadExecutionWidth` |
| Static threadgroup memory | `threadgroup` arrays in MSL | `pipeline.staticThreadgroupMemoryLength` |
| Dynamic threadgroup memory | `setThreadgroupMemoryLength` at encode | must keep static + dynamic ≤ `device.maxThreadgroupMemoryLength` |
| Tile shape, dtype, layout | templates, `-D` defines, generated headers | the compiled function |
| Behavioural flags (alignment, epilogue) | function constants | the specialized pipeline |

A candidate is **lawful** only if the built pipeline accepts it: the requested threads per threadgroup do not exceed `maxTotalThreadsPerThreadgroup`, the threadgroup is a multiple of `threadExecutionWidth` when the kernel assumes full SIMD groups, and threadgroup memory fits. The pipeline reports these values after compilation, so a tuner reads them back instead of predicting them. [Autotuning](autotuning.md) builds the parameter space on this rule.

MPP adds its own contract. A `matmul2d` built for `execution_simdgroups<N>` is undefined behaviour unless exactly N SIMD groups enter `run`, and all threads in the scope must call it uniformly (MPP header, local SDK). A tile shape that changes N must change the dispatch with it.

## The Metal 4 command model

A Metal 4 compute workload submits through `MTL4CommandQueue` and `MTL4CommandAllocator`, keeps its buffers resident with residency sets, and gets timing from `MTL4CommitFeedback`. Compute pipelines can be built with `MTL4Compiler` or with the classic `MTLDevice` call.

```text
MTL4CommandAllocator ─> MTL4CommandBuffer ─> MTL4ComputeCommandEncoder ─> dispatches
                                   │                         │
                                   │          barriers / fences (explicit)
                                   ▼
        MTL4CommandQueue.commit(options with feedback handler)
                                   │
                   feedback.GPUStartTime / GPUEndTime
```

The differences that matter for matmul experiments:

- **No automatic hazard tracking.** Metal 4 needs explicit barriers, fences or events between dependent work. Apple's [Game Porting Toolkit skill on Metal 4 synchronization](https://github.com/apple/game-porting-toolkit/blob/4f42344c90be22112f46becc3fb9687f423f2f88/game-porting-skills/skills/managing-metal4-synchronization/SKILL.md) states it directly: "there is no driver hazard tracking". Older guides that say Metal orders dependent dispatches automatically, including the [metalworking](metalworking-review.md) synchronization page, describe the older Metal 3 model.
- **Timing comes from commit feedback.** `GPUEndTime − GPUStartTime` (`MTL4CommitFeedback.h`) covers everything in one commit. `MTL4CounterHeap` timestamps give finer spans, but inserting them changes the workload. They stay diagnostic until they pass the [observer-effect gate](autotuning.md#observer-effect-gate-for-timestamp-timing).
- **Pipelines are built once and cached.** Build each pipeline once and cache it by a key that covers its function, constants and options. Building a pipeline is expensive. It belongs to "compile cost", never to dispatch time.

## Dispatch geometry traps

- `dispatchThreadgroups` counts the grid in threadgroups. `dispatchThreads` counts threads and may create partial threadgroups at the edges. A kernel that assumes full SIMD groups for MPP or for cooperative loads must use full threadgroups, or guard explicitly.
- Back-to-back dispatches in one encoder may overlap unless a barrier separates them. Two matmuls writing the same output buffer are a data race in Metal 4. As a benchmark, they may measure overlap rather than single-dispatch time. Record which one a harness does.

## What we cannot observe

Resident SIMD groups per core, register count per thread, spill bytes, actual core clock, and accelerator utilization are not exposed through the public Metal API on this device. For a replayed capture, `gpudebug profile run` reports compiler statistics per shader (instructions, temporary registers, spilled bytes) and timeline counters including occupancy and Neural Accelerator utilization. [Profiling](profiling.md) lists what these cover. Where nothing reports a quantity, mark it **unavailable**. Do not estimate it from allocation arithmetic.
