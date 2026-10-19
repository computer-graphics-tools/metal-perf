# Sources and version scope

Use the installed SDK and manuals for the build being measured. A current web page can describe features absent from an older deployment target. Implementation-specific sections use the [reference configuration](evidence.md#reference-configuration).

## Primary references

| Reference | Governs |
| --- | --- |
| [Metal Shading Language specification 4.1](https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf) | Language, active lanes, memory operations, atomics and matrix participation |
| [Metal feature tables](https://developer.apple.com/metal/capabilities/) | Family capabilities and API-visible limits |
| [Metal 4 core API](https://developer.apple.com/documentation/metal/understanding-the-metal-4-core-api) | Command allocation, submission and resource management |
| [Resource synchronization](https://developer.apple.com/documentation/metal/resource-synchronization) | Ordering and visibility scopes |
| [MPP programming guide, version 1](https://developer.apple.com/download/files/Metal-Performance-Primitives-Programming-Guide.pdf) | TensorOps execution and GEMM starting points |
| [M5/A19 machine learning overview](https://developer.apple.com/videos/play/tech-talks/111432/) | GPU Neural Accelerators and matrix-kernel design |
| [M5/A19 graphics and profiling overview](https://developer.apple.com/videos/play/tech-talks/111431/) | Dynamic Caching, occupancy influences and profiling |
| [Metal tensor operations](https://developer.apple.com/videos/play/wwdc2026/330/) | Cooperative intermediates, reductions and supported tensor representations |
| [Metal developer tools](https://developer.apple.com/metal/tools/) | Capture, inspection and performance-tool roles |
| [GPU investigation with agents](https://developer.apple.com/documentation/xcode/investigating-gpu-issues-with-ai-agents) | Scripted capture/debugger workflow |
| [Shader bottlenecks](https://developer.apple.com/documentation/xcode/reducing-shader-bottlenecks) | Utilization and limiter interpretation |
| [GPU memory bandwidth](https://developer.apple.com/documentation/xcode/measuring-the-gpus-use-of-memory-bandwidth) | Memory-interface counter interpretation |
| [GPU occupancy](https://developer.apple.com/documentation/xcode/finding-your-metal-apps-gpu-occupancy) | Concurrency and resource pressure |
| [Counter buffers](https://developer.apple.com/documentation/metal/gpu-counters-and-counter-sample-buffers) | Counter capability and timestamp conversion |
| [MTL4CounterHeap](https://developer.apple.com/documentation/metal/mtl4counterheap), [timestamp granularity](https://developer.apple.com/documentation/metal/mtl4timestampgranularity) | Timestamp storage and observer effects |

## Installed interfaces

| Interface | Read for |
| --- | --- |
| `MTLDevice.h`, `MTLComputePipeline.h` | Device and compiled-pipeline limits |
| `MTL4CommandQueue.h`, command allocator/buffer headers | Submission and lifetime |
| `MTL4ComputeCommandEncoder.h`, `MTL4Counters.h` | Dispatch barriers and timestamp semantics |
| `MPPTensorOpsMatMul2d.h` and implementation headers | Dtypes, descriptor modes, cooperative compatibility and scope constraints |
| `MTLTensor.h` | Tensor representations, usage and availability |
| Installed `gpucapture`, `gpudebug`, `metalperftrace` manuals and help | Actual supported syntax |

The implementation reference uses macOS 27.0 SDK build 26A425, Metal toolchain 27.1.266.1 and compiler 32023.921.6. API availability, storage representation, accepted operand type, native arithmetic and throughput are separate properties; each needs its own evidence.
