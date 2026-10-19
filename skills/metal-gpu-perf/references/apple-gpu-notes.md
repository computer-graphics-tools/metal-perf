# Apple GPU reference

Use the canonical book chapters for build-scoped facts and kernel guidance. Revalidate observations on a different device, OS or compiler.

| Question | Reference |
| --- | --- |
| Device limits and memory hierarchy | [Hardware](../../../book/src/m5-hardware.md) |
| SIMD participation and synchronization | [Execution](../../../book/src/gpu-execution.md) |
| Matrix formats and native forms | [Instruction reference](../../../book/src/instruction-set.md) |
| TensorOps, quantized data and inference kernels | [Kernel design](../../../book/src/matmul.md) |
| AIR and emitted-code boundaries | [Compilation](../../../book/src/compiler.md) |
| Counters and saturation claims | [Metrics](../../../book/src/gpu-metrics.md) |
| Correctness and performance acceptance | [Optimization](../../../book/src/autotuning.md) |

Historical benchmark values are not current ceilings. Run a matching control and retain its conditions before judging bandwidth or route utilization.
