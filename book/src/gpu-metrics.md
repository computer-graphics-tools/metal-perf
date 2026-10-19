# Reading Xcode GPU metrics

This chapter defines GPU metric categories, units and interpretation. Counter availability depends on device, shader stage, profiling mode and selected interval.

Arithmetic units execute scalar/vector operations; GPU Neural Accelerators execute supported matrix operations. Registers, private storage, threadgroup storage and caches can share physical backing under Dynamic Caching. Their logical categories remain distinct.

## First distinguish the units

| Kind of number | Question it answers | What it does not mean |
| --- | --- | --- |
| Time, ns/µs/ms | How long did this event or interval take? | CPU and GPU time are not interchangeable. |
| Source/shader cost, % | How much selected profile cost is attributed here? | The same percentage of removable application time. |
| Utilization, % | How busy was this execution unit doing work? | How much faster the whole app would be at 100%. |
| Limiter, % | How much pressure did this unit show, including stalls? | An exclusive share of total time. |
| Occupancy/residency, % | How much execution or storage capacity was occupied? | A cache-hit rate or useful arithmetic rate. |
| Occupancy influence, % | Which pressure influenced the concurrency controller? | A percentage of wall time spent stalled. |
| Bandwidth, bytes/s | How fast did data cross this particular interface? | The total traffic across all cache levels. |
| Count or bytes | How much work, code or storage is reported? | Time; check whether the number is static or measured at runtime. |

Apple documents [utilization and limiters](https://developer.apple.com/documentation/xcode/reducing-shader-bottlenecks) and [bandwidth](https://developer.apple.com/documentation/xcode/measuring-the-gpus-use-of-memory-bandwidth). Counters overlap: do not add ALU, memory and accelerator percentages to get 100%.

`metal-perf counters` preserves each counter's captured unit and reports every group cell as `mean (samples)`. Counters in the same capture can have different sample counts. Missing units are reported as `unavailable`; do not infer percent, bytes or time from a label alone. `GiB / Second` and decimal GB/s are different units.

## Timing and source attribution

| Metric | Simple meaning | Interpretation |
| --- | --- | --- |
| GPU duration/time | GPU time under the selected command, shader or interval definition | Confirm whether this is elapsed span or summed interval duration; they can differ. |
| CPU encode/submit time | Time the CPU spends preparing and sending GPU work | A faster GPU kernel need not reduce this. |
| Queue/scheduling delay | Time between work being submitted and starting | May include other GPU work and dependencies. |
| Shader cost | Share of the selected profile attributed to a shader | Only compare percentages with the same parent scope. |
| Source-line/function weight | Where the profiler attributes execution cost | A slow load can place cost on a later consumer. Do not assume deleting a line saves its full weight. |
| Inclusive function cost | The function plus work in its callees | Do not add this to the costs of its child calls. |
| API dispatch count | Compute dispatch commands in the captured command tree | Not the number of shader timeline intervals. |
| Invocation count | The producer's reported count for the selected shader or operation | Preserve that view's definition; do not derive it from timeline interval count. |
| Timeline interval count | Number of reported begin/end ranges for the selected shader | One API dispatch can have multiple intervals; their split cause may be unknown. |
| Merged interval count | Ranges remaining after the CLI's less-than-50-ns proximity merge | A heuristic timeline count, not API dispatches or shader invocations. |
| Mean interval duration | Summed merged-interval durations divided by merged interval count | The CLI's `mean interval`, not mean dispatch time. |
| API launch geometry | Thread grid and threadgroup sizes in the captured command | Combine with the pipeline's execution width; more launched work is not necessarily more useful work. |
| Reported `SIMD groups` | The value in the shader-statistics view | Aggregation scope is unverified; do not substitute it for API geometry or occupancy. |

See Apple's [source profiling explanation](https://developer.apple.com/documentation/xcode/optimizing-gpu-performance#Optimize-shaders-with-per-line-shader-profiling-statistics). A profiling replay is not the same measurement as uninstrumented live decoding.

## Arithmetic and execution units

| Metric | Simple meaning | What a high value suggests |
| --- | --- | --- |
| ALU utilization | Ordinary arithmetic units are working | Arithmetic activity; not necessarily matrix-accelerator activity. |
| FP16 / F16 | 16-bit floating-point arithmetic | Work in that arithmetic category. |
| FP32 / F32 | 32-bit floating-point arithmetic | Work such as scaling, conversion results and accumulations. |
| Integer / conditional | Integer and conditional-processing category | Address calculations, integer data processing or control work; grouping depends on hardware. |
| Integer complex | The GPU's complex-integer category | More involved integer operations; inspect generated code to identify which ones. |
| Neural Accelerator utilization | GPU matrix hardware is doing work | Matrix activity, not use of the separate Apple Neural Engine. |
| Neural Accelerator limiter | Pressure on the matrix execution path, including stalls | Matrix throughput/dependencies may constrain progress. |
| Instruction throughput utilization | Activity in processing the instruction stream | The kernel may execute much work beyond its matrix arithmetic. |
| Instruction throughput limiter | Pressure on instruction processing, including stalls | Simplifying instructions or dependencies may matter; it is not a removable-time percentage. |
| Compute shader launch limiter | GPU-side pressure from launching compute threads, including launch stalls | A GPU counter; do not read it as CPU command-submission cost. |

The utilization/limiter distinction applies to each unit. High utilization can be desirable. A high limiter is a lead to investigate, not a verdict that a unit alone causes the whole slowdown.

## Memory and caches

| Metric | Simple meaning | Common misreading |
| --- | --- | --- |
| GPU/device read or write bandwidth | Bytes per second across the reported GPU memory interface | Not the sum of every shader load/store or L1 access. |
| Read/write bytes | Total data crossing that interface in the selected measurement | Bytes are not a duration. |
| Buffer read/write utilization or limiter | Activity/pressure in the buffer-access path | Not automatically physical DRAM bandwidth. |
| L1 read/write bandwidth | Traffic through nearby on-chip cache storage | A large number alone does not prove it is saturated. |
| Stack/private L1 bandwidth | L1 traffic for shader-private temporary storage | Not all of it is a trip to system memory. |
| Stack share of L1 reads/writes | Fraction of reported L1 accesses belonging to stack/private data | 21% of accesses does not mean 21% of execution time. |
| L1 residency | L1 storage occupied by the reported category | Not the fraction of requests that hit L1. |
| L1 cache-pressure influence | L1 working-set pressure influenced the occupancy controller | Not elapsed time spent waiting for L1. |
| Last-level cache/LLC utilization or limiter | Activity/pressure at the last reported cache level | A separate place in the hierarchy from L1. |
| LLC bandwidth/read bytes | Traffic crossing that cache interface | It need not equal DRAM traffic. |
| Cache miss rate | Fraction of requests that were not found at that cache level | A miss is not necessarily a DRAM access; the next level can satisfy it. |
| MMU utilization or limiter | Activity/pressure translating GPU addresses | The GPU is finding memory, not doing matrix arithmetic. |
| TLB miss rate | Address translations absent from the translation cache | Not a data-cache miss rate, and not a time percentage. |
| Threadgroup/tile memory read/write | Accesses to shader-group shared working storage | Different from per-thread registers, despite shared physical backing on modern GPUs. |

M5's L1-backed register/private-memory behavior is described in Apple's [M5 profiling talk](https://developer.apple.com/videos/play/tech-talks/111431/). Read per-category traffic, residency and controller influences together rather than ranking counters by their numerical size.

## Occupancy and its controller

| Metric | Simple meaning | Interpretation |
| --- | --- | --- |
| Compute occupancy | Fraction of the GPU's thread capacity occupied by compute threads | Resident threads may still wait; occupancy is not useful arithmetic utilization. |
| Vertex/fragment occupancy | The corresponding capacity used by graphics shader stages | Usually irrelevant to compute-only work, but other GPU users can matter. |
| Maximum/theoretical occupancy | Potential concurrency allowed by the reported resources/model | Not a guarantee that execution will achieve it. |
| Occupancy Manager target | The controller's current ceiling on concurrency | 100% means it is not imposing a lower ceiling; it does not mean actual occupancy is 100%. |
| Register-pressure influence | Register-storage pressure contributed to lowering concurrency | Investigate how many values remain live and for how long. |
| L1-pressure influence | Cache working-set pressure contributed to lowering concurrency | Investigate the memory categories using L1. |
| Memory-request-stall influence | Backed-up requests in the memory hierarchy influenced the controller | 100% does not mean the kernel spent 100% of its time stalled. |
| Texture-decompression-stall influence | Texture decompression could not keep up with requests | Relevant to textured workloads; usually not ordinary matmul buffers. |

Lower concurrency can improve performance when it keeps working data on chip. More resident threads can also help hide latency. The correct target is lower runtime, not maximum occupancy at any cost. See [Apple's occupancy guide](https://developer.apple.com/documentation/xcode/finding-your-metal-apps-gpu-occupancy).

## Registers, code and compiler statistics

| Metric | Simple meaning | Interpretation |
| --- | --- | --- |
| Live registers at a source line | Working values that must survive at that point | A large peak or long live range can create resource pressure. |
| Allocated/temporary registers | Register resources reported for the compiled shader | Not the same as the number live at every point. Check count units for that view. |
| Uniform registers | Storage for values shared across lanes | Do not add blindly to per-thread register counts. |
| Spilled bytes | Compiler-reported spill/private-storage size | It is a size statistic, not a count of spilled values, measured memory traffic or total stall time. |
| Thread-invariant spilled bytes | A separately reported compiler storage category for thread-invariant work | Keep it distinct from ordinary spilled bytes; its physical traffic and allocation scope need separate evidence. |
| Constant-calculation registers / phase present | Compiler metadata for the constant-calculation phase | Do not add its register count to per-thread registers or infer runtime cost from its presence. |
| Stack load/store instructions | Instructions accessing private temporary storage | They support a spill/private-storage diagnosis; actual execution count matters. |
| Static threadgroup memory | Bytes reported by [`pipeline.staticThreadgroupMemoryLength`](https://developer.apple.com/documentation/metal/mtlcomputepipelinestate/staticthreadgroupmemorylength) | Add dynamic threadgroup allocation when checking the device limit. |
| Reported `Threadgroup memory (bytes)` | The field in the shader-statistics view | Allocation/aggregation scope is unverified; zero does not override the pipeline query. |
| Instructions | Compiled instruction count in the statistics view | Loops can execute one instruction many times. |
| ALU/FP16/FP32/INT16/INT32/branch instruction counts | Compiled code split by instruction category | Different from runtime utilization counters with similar names. |
| Device/threadgroup/texture load/store counts | Compiled memory-operation counts | Not transferred bytes; vector width, repetition and caching matter. |
| Atomic-operation count | Indivisible read/modify/write operations | Contention matters more than count alone. |
| Wait | A backend-reported instruction/statistics category | Its precise definition is not verified; do not interpret 0 as zero GPU waiting. |

`gpudebug`'s compiler statistics and Xcode's shader table can disagree on spilled bytes and report registers differently for the same shader. Keep the two sources separate; neither is a runtime spill measurement.

Ordinary spills, thread-invariant storage and constant-program registers are separate compiler categories. Their sizes cannot be added as if they described one per-thread register allocation.

Private-storage budgeting can quantize requirements into coarse counters. A smaller reported requirement can leave downstream counters unchanged. Budget counters and descriptor fields are not measurements of physical allocation, residency or traffic.

Static threadgroup storage, dynamic encoder allocation, compiled accesses and runtime traffic also describe different quantities. Check the pipeline query and actual dynamic allocation independently of the compiler's statistics field.

## Common graphics-only counters

| Metric | Simple meaning |
| --- | --- |
| Vertices/primitives/fragments | Geometry or pixel-shading work processed. |
| Texture sampling/filtering | Reading texture values and combining nearby samples. |
| Texture read-cache pressure | Texture requests stressing their cache path. |
| Texture compression ratio | Uncompressed bytes relative to transferred compressed bytes; above 1 saves traffic, below 1 can expose overfetch. |
| Compressed texture write inefficiency | How often writes fail to cover whole compression blocks, requiring extra read/modify/write work. |
| Depth/stencil tests and overdraw | Visibility work; repeated shading of the same pixels can waste work. |

These are not matmul costs unless the selected workload actually uses those paths.

## Common misreadings

- **A Neural Accelerator utilization of, say, 30%** means the matrix hardware is active but not continuously busy during the selected replay. It does not imply a free 3.3× speedup.
- **A high instruction-throughput limiter** is an important lead. It is not the share of time that could be removed.
- **A stack share of L1 reads of about one fifth** means one fifth of reported L1 read accesses belong to that category, not one fifth of time lost to spills.
- **A low occupancy target** means the controller capped concurrency in that replay. Check its influence counters before blaming the register count.
- **A 100% memory-stall influence** means that signal strongly influenced the occupancy decision, not 100% wall-time stalling.
- **A high source-line weight** marks a hot area worth examining. A change there must still improve complete-kernel timing under realistic cache conditions.

The practical reading order is: scope and GPU time → expensive shader/source region → unit limiters/utilization → memory hierarchy → occupancy influences → registers and generated code → a controlled before/after timing.
