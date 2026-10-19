# Apple GPU compute reference

This reference describes Metal compute execution, matrix operations, memory movement, compilation, native instruction forms and performance measurement on Apple GPUs.

Public language and API requirements are distinguished from implementation-specific behavior. Version scope and terminology are defined in [compatibility](evidence.md). Native encodings, private interfaces and driver layouts apply only to the stated compiler and OS build.

## Organization

| Subject | Reference |
| --- | --- |
| GPU resources, device limits and memory hierarchy | [M5 hardware](m5-hardware.md) |
| Threads, SIMD groups, participation and synchronization | [Execution model](gpu-execution.md) |
| Command submission, residency and resource lifetime | [Submission and scheduling](scheduler.md) |
| Matrix layouts, execution scopes, tiling and fusion | [Matrix operations](matmul.md) |
| Specialization, intermediate representations and pipeline preparation | [Compilation](compiler.md) |
| Register views and native arithmetic, memory and matrix forms | [Instruction reference](instruction-set.md) |
| Copy interfaces, layouts, bounds and staging storage | [Copies and staging](async-copy.md) |
| Command storage, private memory, firmware rings, recovery and address translation | [Driver and firmware](command-memory.md) |
| Trace inspection and metric definitions | [Profiling](profiling.md), [GPU metrics](gpu-metrics.md) |
| Timing conditions and comparison methods | [Run conditions](telemetry.md), [performance comparison](autotuning.md) |
| Version scope and primary documentation | [Compatibility](evidence.md), [sources](sources.md) |

## Conventions

Tensor shapes name their logical axes. Dtype, accumulation precision, strides and address space are stated separately where they affect behavior. A source-level vector, matrix tile or intrinsic does not specify physical execution-unit geometry.

Device limits, compiled-pipeline limits and physical resource capacities are different quantities. A kernel must satisfy the first two; the third cannot be inferred from a compiler register class or an API maximum.

Instruction count, occupancy, utilization and memory traffic describe different aspects of execution. Performance comparisons require the same useful work, numerical requirements and measurement unit.

## Scope

Detailed cache capacities, instruction latency tables, physical matrix scheduling and preemption are outside the specified implementation coverage. Fault and recovery coverage is limited to the paths named in [faults and recovery](faults-recovery.md). The relevant chapter identifies additional limits on each mechanism.
