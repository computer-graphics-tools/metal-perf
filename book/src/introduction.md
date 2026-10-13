# Introduction

This book collects what can be verified about GPU compute on Apple M5: the hardware, the Metal 4 execution model, how to measure and profile kernels, and what happens below the public API in the compiler, driver and runtime libraries. It grew out of a matrix-multiplication investigation in October 2026, so matrix multiplication is the running example. Every local observation comes from one M5 Max.

Some experiments described here did not keep their raw artifacts. Where that is the case the text says so, and those numbers are not acceptance results. Retained evidence is listed in the [evidence ledger](evidence.md).

The test for any optimization is useful work per unit of time on real workloads. Counter readings, intrinsic names and other people's benchmark tables cannot pass that test.

## Fact check, 13 October 2026

Before publication, every chapter was re-checked against its evidence files, the installed SDK headers and tools, Apple's cited pages and later local measurements.

- In the larger draft this book was cut from, about 1,300 claims were checked and about 180 were corrected or updated as stale.
- Measured values whose raw records were not kept are labelled as unverifiable. Do not use them as acceptance numbers.
- Process notes, material specific to one inference engine and references to deleted files were removed.

## Read claims at the right level

Most confusion about M5 performance comes from mixing up kinds of evidence. Each chapter labels its claims with one of these levels:

```text
API semantics      what Metal/MSL/MPP promise           (Apple docs, SDK headers)
compiled AIR       what the frontend asked for          (generated .ll / .air)
physical ISA       what the GPU executes                (needs a decoder; none is public for M5)
numerical result   what the kernel actually computed    (comparison with a reference)
diagnostic counter what a profiler reported             (units, coverage, perturbation)
acceptance timing  unprofiled (no capture, validation or inserted counter writes), calibrated, repeated
```

A lower level never proves a higher one. An AIR call does not prove which M5 instruction runs. A counter does not prove a speedup. One fast run does not prove a stable gain.

The [evidence ledger](evidence.md) uses the same vocabulary as these chapters:

- **Verified locally**: observed on the M5 Max below, with the command recorded.
- **Source report**: stated by a cited author, vendor page or supplied artifact, not reproduced here.
- **Proposal / hypothesis**: a design or expectation that an experiment has not yet tested.

## Local environment

Checked 2026-10-10 and recorded in [toolchain.txt](evidence/2026-10-10/toolchain.txt): Apple M5 Max (40-core GPU), macOS 27.0.1 (26A434), Xcode 27.0 (27A266a), Metal compiler 32023.921, on AC power. These are identities, not performance claims.

## How to use the book

```text
m5-hardware ─> gpu-execution ─> profiling, gpu-metrics, telemetry ─> matmul ─> autotuning
                                                                       ▲
                    compiler, scheduler, async-copy, reverse-engineering ┘
```

- Read [M5 hardware](m5-hardware.md) and [GPU execution](gpu-execution.md) for vocabulary and limits.
- Follow [Profiling](profiling.md), [Reading Xcode GPU metrics](gpu-metrics.md) and [Telemetry](telemetry.md) for any measurement.
- [Matmul on M5](matmul.md) is the worked example. The [autotuning specification](autotuning.md) proposes acceptance gates; it is a proposal, not an adopted method.
- The compiler, scheduler, async-copy and reverse-engineering chapters cover the work below the public API. [WWDC25–26 APIs](wwdc25-26-api-plan.md) surveys the recent API surface.
