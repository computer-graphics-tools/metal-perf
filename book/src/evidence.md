# Compatibility and implementation scope

This reference supplements the public Metal programming model with implementation-specific information. Public API requirements take precedence over compiler or driver observations.

## Reference configuration

Implementation-specific sections use the following configuration unless another scope is stated.

| Component | Reference configuration |
| --- | --- |
| GPU | Apple M5 Max, 40 GPU cores |
| Unified memory | 128 GiB |
| Operating system | macOS 27.0.1, build 26A434 |
| Metal toolchain | 27.1.266.1 |
| Compiler implementation | Apple LLVM 32023.921.6 |
| Native profiles | `g17s` and `g17d`; numerical behavior is scoped to M5 Max |

These values identify the implementation reference. They are not requirements or capacity guarantees for every M5-family device.

## Requirements and notation

| Category | Meaning |
| --- | --- |
| Language requirement | Behavior specified by the applicable MSL version |
| API requirement | Availability, lifetime, synchronization or resource behavior specified by Metal |
| Implementation behavior | A compiler, runtime or driver property limited to its stated build and forms |
| Numerical limitation | A restriction on supported inputs, precision, participation or accumulation behavior |
| Unspecified property | No complete contract is supplied by this reference |
| Hypothesis | A proposed explanation that is not yet established; always labelled where it appears |

An exact encoding description does not establish execution latency. A numerical result does not establish performance. A compiler statistic describes the producer's quantity and unit; it is not automatically a physical allocation or runtime counter.

## Implementation boundaries

| Mechanism | Boundary |
| --- | --- |
| Native opcodes and register names | Compiler identifiers can change between versions; register views do not define physical capacity |
| Matrix lowering | Operand type, execution scope, precision and destination representation affect instruction selection |
| Private copy interfaces | Argument types and edge behavior are build-specific; public participation rules remain applicable |
| Driver records and pool layouts | Internal implementation details, not application ABIs; see [driver and firmware](command-memory.md) |
| Resource statistics | Ordinary spills, invariant storage, threadgroup allocation and runtime traffic are separate quantities |
| Completion events | Retirement, successful execution and backing destruction are separate lifecycle conditions |
| Profiler intervals | A timeline interval is not necessarily one API dispatch |
| Performance measurements | Device, software, power state, workload and instrumentation determine applicability |

## Portability

Query device capabilities and compiled-pipeline limits. Gate language and API features by their declared availability. Revalidate implementation-specific assumptions when changing compiler, SDK, OS, GPU family, numerical mode or pipeline specialization.

Use [primary references](sources.md) for the governing language and API contracts. Private interfaces and numeric encoding identifiers are unsuitable as portable application dependencies.
