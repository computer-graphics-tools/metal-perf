# Tile copies and staging

Staging copies device data into threadgroup memory for reuse or rearrangement. An asynchronous interface separates copy issue from completion. The interface does not, by itself, specify a separate copy engine or overlap with arithmetic.

```mermaid
flowchart LR
    D[Device tile] -->|issue copy| N[Next staging tile]
    C[Current staging tile] -->|matrix operations| A[Accumulator]
    N -->|completion before consumption| C
```

## Typed copy interface

This section describes the nonpublic AIR interface in compiler `32023.921.6`, build `26A434`. It is an implementation reference, not an MSL API guarantee or a version-independent ABI.

`air.simdgroup_async_copy_2d.read.p3i8.p1i8` returns an opaque `%struct.copy_event*`.

| Argument | AIR type | Meaning |
| ---: | --- | --- |
| 1 | `i64` | Element size in bytes |
| 2 | `i64` | Element alignment in bytes |
| 3 | `i8 addrspace(3)*` | Threadgroup destination |
| 4 | `i64` | Destination row stride in elements |
| 5 | `i64` | Destination element stride |
| 6 | `<2 x i64>` | Copy extent: columns, rows |
| 7 | `i8 addrspace(1)*` | Device source |
| 8 | `i64` | Source row stride in elements |
| 9 | `i64` | Source element stride |
| 10 | `<2 x i64>` | Source extent: columns, rows |
| 11 | `<2 x i64>` | Signed source origin: column, row |
| 12 | `i32` | Edge-handling mode |

The associated wait is `air.wait_simdgroup_events(i32, %struct.copy_event**)`. Address spaces, integer widths, signed origin conversion and event representation are ABI fields. MSL `long2` and `ulong2` have the same AIR vector width; their conversion into that representation can differ.

The frontend rejects direct `air.*` assembly labels in the reference Metal 3.2 and 4.0 modes. Declaring a function with a matching name is not sufficient to implement the operation.

## Implementation selection

| Layout / mode | Reference runtime helper |
| --- | --- |
| Compile-time contiguous layout | `air.simdgroup_async_copy_1d.read` |
| Two-dimensional mode 0, unit element strides | `air.simdgroup_async_copy_2d.read.noholes` |
| Two-dimensional mode 1, unit element strides | `air.simdgroup_async_copy_2d.read.edgeclamped.noholes` |
| Non-unit element strides | `air.simdgroup_async_copy_2d.read.withholes.v1` |

The `noholes` suffix describes unit element strides in these implementations. Rows can still have padding, and a requested rectangle can cross source edges.

The mode-0 FP32 implementation uses participation loops, masked global vector loads and threadgroup stores. Its lowering includes `llvm.agx3.load.with.emask.global.v4i32` and `llvm.agx3.store.with.emask.local.v4i32`. This is a cooperative instruction sequence; the source name does not identify a physical engine.

### Compile-time layout

Pitches, extents and origins known during specialization can select and inline a simpler implementation. A value loaded from MSL `constant` memory remains a runtime value unless compilation makes it constant. See [compile-time constants](compiler.md#constant-memory-and-compile-time-constants).

### Non-unit element strides

For logical coordinate `(column=x,row=y)`:

`element_index = y × row_stride + x × element_stride`

A logical `[rows=8,columns=64]` view with strides `(64,1)` is row-major. Strides `(1,8)` express a different permutation over the same 512 physical element indices. Source and destination mappings are independent.

Equivalent physical input/output mappings need not select equivalent code. Contiguous views expose a simpler transfer when no permutation is required.

## Edge-handling modes

The following behavior is limited to the reference FP32 implementations and supported layouts.

| Mode | In-bounds region | Right/bottom edges | Fully out-of-source rectangles |
| --- | --- | --- | --- |
| 0 | Copies source values | Zero-fills out-of-source destinations | Zero-fills the requested region in the reference forms |
| 1 | Copies source values | Edge stripes can repeat the nearest valid source row/column | Complete coordinate-wise clamping must not be assumed |

Destination padding outside the requested copy region is separate from edge handling inside the region.

### Signed origins

The mode-1 edge-stripe path uses signed division and remainder. Each visited source coordinate is clamped before loading, but not every requested destination coordinate is necessarily visited.

Its interior column width is:

`min(copy_width, source_width - origin_x) - max(-origin_x, 0)`

This expression has no nonnegative clamp. Negative widths can make top or bottom stripe counts negative and reduce the signed loop bound `L + R + T + B`.

For a `[rows=3,columns=5]` source and `[rows=3,columns=4]` copy:

| Origin: column, row | Left L | Right R | Top T | Bottom B | Stripe iterations |
| --- | ---: | ---: | ---: | ---: | ---: |
| `(-8,-4)` | 12 | 0 | -12 | 0 | 0 |
| `(-5,-4)` | 12 | 0 | -3 | 0 | 9 |
| `(8,4)` | 0 | 12 | 0 | -9 | 3 |

Unvisited destinations retain their previous values. Initialize staging storage when required values are not guaranteed by the selected bounds path. Other modes, dtypes and layouts are not specified by this table.

## Participation and synchronization

### Wider threadgroups

Copy implementation, initialization and consumption can have different work distributions. A unique owner in an initialization or readback loop does not establish unique ownership inside the copy helper.

### SIMD-group-specific copy arguments

Separate SIMD groups can select disjoint source planes and destination regions. Keep the arguments defining one cooperative transfer consistent within its required scope. This private behavior does not change public matrix-operation participation requirements.

### Partially filled SIMD groups

Nominal SIMD width, valid invocations and active lanes are different quantities. Do not substitute a physical lane index for active-lane rank in a cooperative distribution. Partial-launch and divergence behavior remain implementation-specific.

### Conditional copy participation

Private implementations can retain valid-count reads and masked memory operations inside a conditional region. That does not provide a public guarantee for arbitrary divergence, reconvergence or per-lane arguments.

A threadgroup barrier after a copy can synchronize consumers only when all required producers have written their values. Ended threads no longer blocking a barrier does not supply missing tile data.

### Vector remainders

The reference FP32 helper processes four elements per bulk vector. For positive dimensions:

`q = copy_width / 4; r = copy_width % 4`

The remainder path requires `r > 0 && copy_height > 0` and begins at column `4*q`. Let S be the source-bounds mask, including row validity, and `R = (1 << r) - 1`.

| Mode | Remainder load mask | Remainder store mask |
| --- | --- | --- |
| 0 | S | R |
| 1 | S | `S & R` |

Source-load masks and requested-width store masks have different roles. A narrow requested rectangle does not imply an equally narrow load mask or a specific physical transaction count.

The bulk loop uses previous-valid rank and total-valid count. The remainder row loop advances by one. Logical addresses alone do not establish physical worker ownership, broadcast, duplicate transactions or race freedom.

### Two copy events

The selected 1D/2D lowerings generate event tag 3; null-event lowering generates tag 0. Wait lowering can compare event categories 1 and 2 to select `llvm.agx2.syncbuf`. Tag 3 can allow event arrays and waits to be eliminated while explicit threadgroup barriers remain.

Preserve the source completion and visibility requirements. Eliminated event bookkeeping does not establish that a surrounding barrier is unnecessary. Homogeneous tags do not define other event categories, heterogeneous waits or arbitrary event-array lengths.

## Staging cost

For A `[BM,BK]` and B `[BK,BN]`, one staging set uses:

`sizeof(A_element) × BM × BK + sizeof(B_element) × BK × BN`

For FP32 operands this is `4 × BK × (BM + BN)` bytes before padding and other shared state. Two complete sets double the allocation. Static plus dynamic storage must fit the [resource limits](gpu-execution.md#pipeline-and-dispatch-contracts).

| Design | Potential benefit | Main cost |
| --- | --- | --- |
| Direct device loads | No explicit staging allocation | Repeated loads without sufficient cache reuse |
| Cooperative single buffer | Reuse and layout conversion | Transfers and synchronization |
| Cooperative double buffer | Opportunity to prepare another tile during computation | Extra storage, address work and synchronization |

Copy/compute overlap, dedicated-engine use and physical traffic require separate measurements. Use [profiling](profiling.md) for attribution and [performance comparison](autotuning.md) for complete-operation timing.
