# Reading Xcode GPU metrics

This is a plain-language glossary for the M5 compute captures used in this research, plus common Xcode labels. Xcode exposes different counters for different devices, shader stages and selected time ranges. A label outside this list should be checked against its exact panel and unit before interpreting it.

Think of a GPU as a workshop. ALUs do ordinary arithmetic; Neural Accelerators do matrix arithmetic. Registers hold working values, L1 is nearby storage, the last-level cache is larger shared storage, and system memory holds the large arrays. The MMU translates the addresses used to find them. These are not entirely separate boxes: since M3, dynamic caching allocates registers and other shader-private storage from the L1 cache, and M5 has its second generation.

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

## Timing and source attribution

| Metric | Simple meaning | Interpretation |
| --- | --- | --- |
| GPU duration/time | Elapsed GPU time for the selected command, shader or interval | This is what ultimately needs to decrease; confirm the measured scope. |
| CPU encode/submit time | Time the CPU spends preparing and sending GPU work | A faster GPU kernel need not reduce this. |
| Queue/scheduling delay | Time between work being submitted and starting | May include other GPU work and dependencies. |
| Shader cost | Share of the selected profile attributed to a shader | Only compare percentages with the same parent scope. |
| Source-line/function weight | Where the profiler attributes execution cost | A slow load can place cost on a later consumer. Do not assume deleting a line saves its full weight. |
| Inclusive function cost | The function plus work in its callees | Do not add this to the costs of its child calls. |
| Invocation count | How often the selected operation ran | Check what the panel counts: dispatches, threads or shader-stage invocations. |
| Thread/SIMD-group/threadgroup counts | How much parallel work was launched | More launched work is not necessarily more useful work. |

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
| Spilled bytes | Compiler-reported temporary storage moved out of registers | It is storage size, not measured memory traffic or total stall time. |
| Stack load/store instructions | Instructions accessing private temporary storage | They support a spill/private-storage diagnosis; actual execution count matters. |
| Threadgroup memory bytes | Explicit shared scratch reserved by the shader | Zero does not mean zero L1 traffic or zero private storage. |
| Instructions | Compiled instruction count in the statistics view | Loops can execute one instruction many times. |
| ALU/FP16/FP32/INT16/INT32/branch instruction counts | Compiled code split by instruction category | Different from runtime utilization counters with similar names. |
| Device/threadgroup/texture load/store counts | Compiled memory-operation counts | Not transferred bytes; vector width, repetition and caching matter. |
| Atomic-operation count | Indivisible read/modify/write operations | Contention matters more than count alone. |
| Wait | A backend-reported instruction/statistics category | Its precise definition is not verified; do not interpret 0 as zero GPU waiting. |

`gpudebug`'s compiler statistics and Xcode's shader table can disagree on spilled bytes and report registers differently for the same shader. Keep the two sources separate; neither is a runtime spill measurement.

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
