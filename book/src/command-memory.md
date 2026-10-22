# Command and private memory

This chapter describes how the build 26A434 G17X driver stores commands and provisions shader-private memory. Record layouts, profile indices and pool structures are implementation observations, not application ABIs. Public lifecycle requirements are in [submission and scheduling](scheduler.md).

## Host-side dispatch structure

The observed G17 host path validates dispatch dimensions, prepares resources and spill state, reserves command storage, and encodes enqueue records. Selected command records are 64 bytes, with further ring and address-space bookkeeping below them. These are implementation observations for build 26A434, not an application ABI.

The mapping from these records to actual hardware launch, retirement and all firmware synchronization remains incomplete. No host heuristic constant establishes the number of resident SIMD groups or a hardware scheduling policy.

The build 26A434 implementation separates command storage from shader-private memory provisioning:

| Layer | Verified mechanism | Interpretation limit |
| --- | --- | --- |
| Command storage | Pooled shared-memory objects hold command bytes; growth copies the used prefix into a larger object | Command-storage size is not shader-private backing size |
| Kernel validation | A bounded 616-byte compute payload carries a 72-byte private-memory descriptor, separate from another spill descriptor | Internal record layouts are not an application ABI |
| Private-memory preparation | Command preparation passes the descriptor to a pool that can reuse existing blocks or allocate additional internal resources | Descriptor requirements do not establish a new allocation for each dispatch |
| Growth control | The G17X factory selects an implicit growth engine that reports support for initial hardware growth | This capability does not specify hardware growth timing or prove that a dispatch grows the pool |
| Pool generations | A replacement freelist can retain references to backing resources from the previous generation | Replacing a freelist does not imply replacing or freeing its backing |
| Idle shrink | Reduces the retained block set and can request purge state 4 on the backing memory object; see [pool lifecycle](#private-memory-pool-lifecycle) | While that object is resident, the driver records the purge request and returns success without invoking the underlying descriptor operation |
| Scheduler retirement | The compute queue tests the descriptor event, invokes completion, releases its reference, clears the ring entry and advances the head | Device overrides and software stamp forcing exist; retirement alone does not prove successful GPU execution |
| Completion | Pool completion releases reservation bookkeeping and a freelist reference; block destruction separately releases its internal resource | Completion does not imply immediate backing destruction |

These are static implementation paths. Allocation growth, live pool size, physical placement, cache behavior and private-memory traffic are not specified by these record layouts. See [compiler statistics](gpu-metrics.md#registers-code-and-compiler-statistics) for the distinction between reported spill requirements and measured traffic.

The unoverridden normal-block sizing tables contain the following values. Internal profile indices are implementation identifiers, not Metal feature-family values.

| Internal profile | Block size | Nominal pool maximum |
| --- | ---: | ---: |
| 24 | 4 MiB | 4 GiB |
| 25 | 8 MiB | 8 GiB |
| 26 | 16 MiB | 16 GiB |

The effective maximum is capped at 4,192,256 units of 4 KiB and rounded down to a block boundary. The per-page address table reserves eight bytes per 4 KiB unit, rounded to the system page size. These accounting units do not establish physical page size. A separate small-block path returns 128 KiB; configuration overrides can change the selected sizes.

Each pool-ID slot in the selected private-memory descriptor table occupies 64 bytes. Its address fields derive from GPU virtual mappings and are encoded with right shifts of seven and six bits; these encodings do not establish translation page size.

Two backing-address tables have separate allocations and roles:

| Table | Stored representation | Published base in shared pool state |
| --- | --- | --- |
| Per-page address table | Full GPU virtual addresses at 4 KiB strides on the populated path | Byte offset 20 |
| Block address table | Eight-byte entries; host address updates replace bits 12–47 and preserve the remaining bits | Byte offsets 48 and 80 |

Growth appends block-address entries before increasing the published valid-entry count. Address publication and backing preparation are separate operations; a reserved GPU address does not establish resident physical backing.

| Descriptor operation | Scope | Ordering |
| --- | --- | --- |
| Initialization | Writes all eight 64-bit words from the shared pool state | Separate from subsequent status readback |
| Status readback | Copies selected flags and counters into the shared state; it does not reconstruct every initialization field | A returned counter is not a physical-page inventory |
| Incremental update | Publishes an increased block-address entry count and a shared-state flag in the word at byte offset 40 | `DMB ISH` before and after the write |
| Firmware ID reset | Sets bit 61 and clears selected 28-bit counters while retaining address fields | Issues an ID-specific command, polls its busy bit, and brackets descriptor writes with `DSB SY` |

A fresh pageable pool-block path initializes its VM object with zero resident pages and retains it through a memory-entry reference. The reference records the covered range separately from the descriptor's preparation state.

Backing preparation can reuse already prepared memory. When wiring is required, the descriptor obtains a page list through its memory-entry reference; existing pages and newly acquired free frames are separate paths. GPU mapping commits follow a separate step. Successful descriptor creation, preparation or descriptor-table publication does not count newly allocated physical pages.

Startup data carries separate GPU and firmware mappings of the descriptor table. The selected GFX1 initialization path writes the GPU address masked by `0x7ffffffffc0` to register offset `0x14000` within its mapped GPU register window. This programs a table reference; it does not specify physical page placement.

The firmware reset path uses its separately supplied mapping to modify the same table. Its polling helper can exit for recovery before the busy bit clears. Resetting an entry or clearing its counters therefore does not establish completed hardware work or release of physical backing.

Pool-ID retirement also compares a host-updated shared submission counter with a firmware-updated processed counter. Unequal counters, a nonzero per-ID guard or a pending reset can defer the retirement attempt; a later scan retries pending resets.

After the checked ID-command poll succeeds, firmware requests cache cleaning and invalidation over the 136-byte shared state and clears completion-pending bookkeeping. An accepted request can remain deferred, and these operations do not establish physical-page release.

## Private-memory pool lifecycle

Shader private memory — stack storage such as dynamically indexed thread-local arrays, and register spills — is backed by a per-process pool that the driver grows and shrinks. The pool exists in addition to the application's own allocations and is charged to the process's memory footprint.

| Stage | Implementation behavior | Basis |
| --- | --- | --- |
| Creation | Each process that runs compute work owns a pool; a kernel without private storage still receives 112 MiB | Observed |
| Sizing | For each command, the driver computes a minimum and a desired pool size from the pipeline's private-memory requirement, caps the desired size at the pool maximum and rounds both to the block size | Static; block size observed as 8 MiB |
| Growth | When the desired size exceeds the current pool, the host grows the pool synchronously while preparing the command. After three consecutive allocation failures, the minimum size is used instead | Static |
| Publication | New blocks are published to the GPU through the private-memory descriptor's incremental update before the command is submitted | Static |
| Idle shrink | A 1 s driver timer keeps a per-pool usage estimate, `estimate = max(peak, (peak + 4 × estimate) / 5)`, and shrinks the pool to it in whole blocks. Without submissions the pool shrinks by 20% per second | Static and observed |
| Release | After 10 s without use the pool releases all blocks and purges its backing | Static and observed |
| Accounting | Pool memory appears in the process footprint while in use; retained idle blocks are reported as reclaimable | Observed |

The following measurements use a compute kernel with a dynamically indexed 16 KiB thread-local array, dispatched as 1,048,576 threads (M5 Max, build 26A434). The desired size was 2,110,619,648 bytes and the pool grew to 2 GiB. A second process kept the GPU busy throughout, so GPU power-state transitions do not contribute to the wall-time difference.

| Idle time before the next command buffer | Extra host time before GPU execution |
| --- | ---: |
| 1.0 s or less | about 1 ms |
| 1.25 s | 15–28 ms |
| 1.5–3 s | 26–34 ms |
| 6 s | about 45 ms |
| 12 s (pool released) | about 55 ms |

Regrowing a fully released pool cost about 27 ms per GiB. Without a second process keeping the GPU active, a kernel without private storage also added 6–9 ms after gaps of 1.5 s or more, so this fixed cost dominates for small pools. Steady-state benchmarks include neither cost.

Kernel and runtime guidance:

- Keep per-thread private storage small. Pool size scales with private memory per thread times resident threads; a 16 KiB array per thread produced a 2 GiB pool.
- For latency-sensitive work that uses large private memory, measure the first command buffer after idle periods separately from steady-state timing.
- Gaps longer than about 1 s between submissions from the same process allow the pool to shrink; the next command pays for regrowth on the host.

Measure the pool with the process footprint (`footprint -p <pid>`: *IOAccelerator* and *Reclaimable*) and the GPU's `In use system memory` statistic. The IOReport channel *GPU Stats / UMA Pool Memory Usage / Total Number of UMA Pages* counts 4 KiB units; in build 26A434 it adds each growth twice and subtracts each release once, so it drifts upward and does not equal the pool size. The separate *UMA Allocation Size Stats* channels report global running minimum, maximum, and integer average of minimum and ideal sizes, plus total submissions. On this M4, saved one-probe windows included 3–95 global submissions on a baseline above 18 million, and their previous maxima did not reset. These channels cannot by themselves read back a particular probe's exact minimum or ideal size; use them only as diagnostics unless a new-record value can be isolated and attributed.

### M4 measurement control

The measurements, disassembly facts, and control results in this subsection are reproducible M4 observations. The fitted equation is an empirical hypothesis over the tested shapes, not a driver requirement.

The `idle-gap --pool-probe` path dispatches once, checks three GPU output elements against a CPU reference, prints `POOL_READY`, and keeps the process alive for an external snapshot. The current probe also records wall-clock stamps immediately before the dispatch and after completion. `scripts/private_pool_sweep.py` starts a fresh process for each shape, runs `footprint -p <pid> -f bytes` and reads `AGXAccelerator`'s `In use system memory` immediately after the marker, and writes every sample or failure to JSONL outside the repository. It requires both reads to finish within one second of **dispatch start**; this is a conservative upper bound on time since completion. The historical M4 grid predates these clock stamps. Its 36 long-workload replacement rows and 54 held-out rows have dispatch-duration measurements that, added to marker lag, stay below 0.135 s; the other 216 grid rows establish marker-to-snapshot lag but do not prove the strict completion window. The process footprint is the primary per-process measurement; the accelerator counter includes other processes and is only a cross-check. In this `footprint` format the large pool is in `Owned physical footprint (unmapped) (graphics)`, rather than the smaller `IOAccelerator (graphics)` row.

For a 4 B/thread, 65,536-thread control on the M4, a CPU reference check passed before timing. Three uninstrumented repeats per gap gave these medians; the GPU times are at the display's 0.01 ms resolution. No second process kept the GPU busy, so host extra time can include GPU wake-up as well as pool work.

| Idle gap | First wall/GPU | Warm wall/GPU | Extra host |
| ---: | ---: | ---: | ---: |
| 50 ms | 0.65 / 0.01 ms | 0.40 / 0.00 ms | 0.25 ms |
| 500 ms | 0.78 / 0.01 ms | 0.49 / 0.01 ms | 0.30 ms |
| 1,500 ms | 2.39 / 0.01 ms | 0.59 / 0.00 ms | 1.80 ms |
| 12,000 ms | 2.99 / 0.00 ms | 0.40 / 0.00 ms | 2.59 ms |

An uninstrumented A/A control used the same 4 KiB/thread, 262,144-thread `idle-gap` command as both variants, with five alternating rounds, the first discarded, and a 10 s cooldown. After a separate CPU reference check, the paired first-command GPU-time ratios were 1.011, 0.955, 0.979 and 1.001; their geometric mean was 0.986, within the required 2% control band. The individual spread limits claims smaller than these run-to-run changes. The machine was on AC power with no reported thermal or performance warning; ordinary desktop work remained active, with no intentional competing GPU benchmark.

On a 10-core Apple M4 running macOS 27.0.1 (26A434), Xcode 26.6 and Metal compiler 32023.883, a three-repeat sweep measured the following unmapped graphics footprint. Values are MiB; each of the four dispatched-thread counts (65,536, 262,144, 1,048,576 and 4,194,304) gave the same table after the long cases were rerun with the short workload described below. The M4 device loads the G16G driver path, whereas the M5 work above concerns G17X; these M4 results do not establish a G17X formula.

| Private bytes per thread | 64 threads/group | 256 threads/group | 1024 threads/group |
| ---: | ---: | ---: | ---: |
| 4 | 40 | 40 | 40 |
| 64 | 40 | 40 | 40 |
| 256 | 46 | 44 | 44 |
| 1,024 | 68 | 66 | 66 |
| 4,096 | 158 | 158 | 156 |
| 16,384 | 522 | 520 | 520 |
| 65,536 | 1,976 | 1,974 | 1,974 |

All these readings were multiples of 2 MiB. The active M4 class is `AGXAcceleratorG16G`. In its kernel collection, the pool calls `halGetUMABlockSize`, which returns 2 MiB; `getUMAPoolMaxSize` reads an internal accelerator field at byte offset 68464 and uses a 2 GiB fallback when it is zero. The field's initializer and live value are not yet verified. The pool computes its effective maximum by rounding down the smaller of that limit and `getUMARingBufferSizeMax() * 4096` to a block boundary. The G16G ring-buffer method returns `0x3ff800` pages, above the 2 GiB fallback. Disassembly also locates `getMinPoolSize`, `getIdealPoolSize`, and `getDesiredPoolSize`: they read the first two 64-bit fields of `AGXUMADescRec` and round their results to the selected block. `getRequiredRTSize` and `getSelfReserveSize` read descriptor byte offsets 60 and 64 as 32-bit page counts. The active descriptor values were captured as described below, but the active pool branch and its runtime inputs are not yet known; the measured footprint is therefore not a verified output of those equations.

The M4 code yields a partial symbolic formula. Let `D0` and `D8` be the descriptor's first two 64-bit byte counts, `B = 2 MiB`, and `roundB(x) = B * ceil(x / B)`. When the pool's sizing flag is clear, `minimum = roundB(D0)` and `ideal = roundB(max(D0, D8))`. When it is set, the G16G minimum branch uses an active-kick factor `K`, the freelist value at offset 128, and an ideal-size bound: `minimum = roundB(min(K * max(freelist[128], D0), max(ideal, pool[424])))`. The active G16G `allocatePagesEnd` method updates `pool[424]` to the unsigned maximum of its previous value and its final argument. On the no-growth branch of `prepareLocked`, that argument is the return of `getDesiredPoolSize`, so this bound retains the peak desired **bytes** observed on that path. The same completion method updates freelist offsets 128, 136, 144, 152, and 160 to unsigned maxima of `D0`, `D0−RT−SR`, `D8−RT−SR`, `RT`, and `SR`, respectively; freelist initialization zeroes them. Here `RT` and `SR` come from descriptor page fields at byte offsets 60 and 64. Freelist offset 104 is the current-size byte count compared with desired size: successful growth writes desired bytes there, while freelist replacement writes selected block size times its block count. These updates explain the freelist-history and current-size inputs in the minimum, ideal, and growth paths, though other completion branches and resets remain to be traced. The ideal branch also depends on `getAgxCrUmaIdealConstantCount()`. The desired method selects among minimum, ideal, a capped maximum of minimum/ideal/current size plus one granule, and a percentage of the difference between `D0` and `D8`; its branch choice depends on the sizing flag and device properties. These are translated code paths; the active branch and runtime values still need tracing before this becomes a numeric driver prediction.

For the observed nonnegative descriptor domain, the active G16G functions at `0xfffffe0008bf28f8` (`getDesiredPoolSize`), `0xfffffe0008bf2d48` (`getIdealPoolSize`), and `0xfffffe0008bf2fec` (`getMinPoolSize`) reduce to the following piecewise form. `P` is the freelist at pool offset `480`; `S` is pool byte `496` bit 0; `H` is accelerator word `+1672` bit 40; `C` is `getAgxCrUmaIdealConstantCount()` in 4 KiB pages; `percent` is accelerator property `+71440`, with all ones as a sentinel; `G` is the positive page granule selected by the kernel's global shift; and `effectiveMax` is pool offset `360` after the block and ring-size cap. Arithmetic outside the non-overflow domain needs the machine's unsigned widths and alignment operations.

```text
alignG(x) = G*ceil(x/G)
if S:
    base = max(D8 - RT - SR, P[144])
    ideal = roundB(base + (K-1)*C*4096
                   + K*(max(P[152],RT) + max(P[160],SR)))
    minimum = roundB(min(K*max(P[128],D0), max(ideal,pool[424])))
else:
    ideal = roundB(max(D0,D8))
    minimum = roundB(D0)

growthTarget = min(effectiveMax, max(minimum,ideal,P[104]+G))
if H:                                      desired = growthTarget
else if S and percent == UINT64_MAX:       desired = ideal
else if S:                                 desired = minimum
else if percent == UINT64_MAX or D8 <= D0: desired = ideal
else:                                      desired = alignG(D0 + floor((((D8-D0)*uint32(percent)) mod 2^64)/100))
return roundB(desired)
```

The two `H` branches use the same maximum: the `cmp`/`csel` chains at `0xfffffe0008bf2b18`–`0xfffffe0008bf2b38` and `0xfffffe0008bf2bd8`–`0xfffffe0008bf2bf0` select `ideal` only when it exceeds both `P[104]+G` and `minimum`, then cap the result. The earlier apparent ideal-size upper bound was a translation error. `AGXUSCPrivMemPool::init` gets `S` from its Boolean argument: `AGXAccelerator::createUMAPool` passes 0 for its direct new-pool path and 1 for its shared-pool path. The `K`, `C`, `H`, `percent`, `P` fields, and active `S` branch have not been read back for each dispatch.

The percentage branch's magic multiplier is exactly integer division by 100 for **every unsigned 64-bit product word**, not just the sampled range. Its instructions compute `n = product >> 2`, then `q = floor(n*M/2^66)` with `M = 0x28f5c28f5c28f5c3 = ceil(2^66/25)`; `25*M - 2^66 = 11`. Since `n < 2^62`, `11*n < 2^66`, so the rounding error cannot carry any remainder of `n/25` into the next integer: `q = floor(n/25) = floor(product/100)`. The preceding `mul` keeps the low 64 bits of `(D8-D0)*uint32(percent)`, as shown in the formula. Other additions and alignment steps also use machine-width arithmetic outside the measured non-overflow domain. The desired-size method's indirect load for `G` resolves through the kernel collection to `_PAGE_SHIFT_CONST` at `0xfffffe0007e49808`; `getconf PAGESIZE` reports 16 KiB on this M4. This supports `G = 16 KiB` if that global contains the host page shift, but its live word was not read from the kernel. The zero bytes in the on-disk global are not a runtime value.

The G16 base-class constructor in this M4 kernel collection stores a 16-byte constant `(0,1,1,1)` at accelerator offset `+1232`, so its **initial** word at `+1236` is 1. The same constructor zeros the 128-bit mask at `+1136` and initializes the word at `+1260` to 0; configuration must supply useful mask and width values later. The G16G ideal-constant getter computes `load_u32(accel+1236) * (1056 + W*T)`, where `T` comes from `+68472` when nonzero or `+68444` otherwise. For a configured width from 1 to 128 bits and `A = load_u32(accel+1236)`, it selects the mask at `+1136` if either of its 64-bit words is nonzero, or the mask at `+1152` otherwise, then sets `W` to the largest popcount among the `A` consecutive width-bit slices of that selected 128-bit mask; `W=0` when `A=0`. The same fallback `T` feeds the base HAL's default DUPM count reduction and PTC count update. G16's separate virtual DUPM max/min methods return literal 2/1; those returns do not set `T`. This is static code translation, not a readback of its runtime inputs. The constructor default narrows one input's origin but does not prove its live value or the observed 7.875 MiB footprint offset. Static stores to the same immediate offsets elsewhere in the kernel were checked against their base registers; they did not establish a later write to these accelerator fields.

#### M4 descriptor captured at runtime

LLDB broke at `AGX::SpillInfoGen3<...>::allocateUSCSpillBuffer` in the active `AGXMetalG16G_B0` user-mode plugin, saved its `AGXUMADescRec*` argument, stepped out, and read all 72 descriptor bytes. Each diagnostic run used a fresh process; its breakpoint was disabled after the first call. LLDB changes timing, so the descriptor bytes were compared with the separate, uninstrumented footprint sweep, not with a footprint sampled under the debugger.

The first descriptor call for each of the seven grid sizes and six held-out sizes at all three group sizes produced 39 valid captures. Two further private-byte sizes were selected because 7.875 and 8 MiB offsets predict different rounded footprints; their descriptor predictions were recorded before LLDB and footprint measurements. All 41 captures had zero render-target and self-reserve page fields at offsets 60 and 64. At 65,536 dispatched threads, the matching uninstrumented footprints obeyed the relation below for all 41 shapes, with three repeats each:

The plugin disassembly shows that when `SpillInfoGen3+0x2c` is zero, it writes `D0 = 4096 * round_up_to_8_pages(load_u32(SpillInfoGen3+0x4))` and `D8 = 4096 * round_up_to_8_pages(load_u32(SpillInfoGen3+0x18))`. Live entry captures at 4 KiB and 64 KiB per thread, group size 256, both took this branch. Their offset `+0x18` page counts were 37,976 and 502,976, producing the captured `D8` values exactly.

The preceding `checkSpillParamsForCompute` function fills those page counts. Its active M4 branch takes a frame-size input of zero for the tested 4 and 64 B kernels, and `private_bytes + 16` for each of the other 11 original sizes and the two discriminator sizes; all 15 inputs were captured at group size 256. The group-size argument was captured at 64, 256, and 1024. With the captured device and object fields, this branch reduces to the following source-like pseudocode for the tested shapes. `round8` rounds a page count up to eight pages:

```text
frame = 0 if private_bytes <= 64 else private_bytes + 16
a = 32 * max(frame, 0 if private_bytes == 4 else 32)
q9 = floor((a + 0x437f) / 4096) + 1
q10 = floor((a + 0x1337f) / 65536) + 1
e = threads_per_group / 32
r3 = q10 * min(e, 14) + 1
t = max(q9 * e + r3 + 1, 96)
per = 96 / e
D0 = 4096 * round8(11*t + 10*r3 + 2080)
D8 = 4096 * round8(10*(14*q10 + 96*q9 + per + min(per, 8)) + 1696)
```

This translated branch predicts both captured first-call descriptor size fields exactly for 41/41 shapes. The `D8` input rises by 31,000 pages per 4 KiB of declared private storage across the tested larger sizes because `q9` rises by 32 and `q10` by 2: `10 × (96 × 32 + 14 × 2) = 31,000`. This identifies arithmetic in the active **user-mode descriptor producer**; it does not establish the kernel pool's chosen size.

A separate LLDB capture traced `endComputePass` calling `checkSpillParamsForCompute` with its first six integer arguments zero after the ordinary enqueue call. The G16G branch treats a zero group argument as `e = 32` and selects `per = 96`; with the captured M4 device fields, its intermediate values are `q9 = 5`, `q10 = 2`, `r3 = 29`, and `t = 190`. It produces candidate page counts `raw0 = 4,460` and `raw8 = 7,816`, which `allocateUSCSpillBuffer` rounds to `D0_end = 17.4375 MiB` and `D8_end = 30.53125 MiB`. The check function keeps the larger existing page counts, so the resulting size fields are the maxima of the enqueue and end-pass candidates. For 12 small-private shapes, this predicts every captured final descriptor pair exactly. At 256 and 1,024 B/group1024, the end-pass check still ran, but the fields did not grow and no second descriptor allocation was seen.

```text
observed_unmapped_graphics_MiB = round2(max(D8_enqueue_MiB, 30.53125) + 7.875)
round2(x) = 2 * ceil(x / 2)
```

| Private bytes/thread | Threads/group | Captured `D0` | Captured `D8` | Observed footprint |
| ---: | ---: | ---: | ---: | ---: |
| 4 | 256 | 12.9375 MiB | 27.25 MiB | 40 MiB |
| 4,096 | 256 | 23.59375 MiB | 148.34375 MiB | 158 MiB |
| 12,288 | 1024 | 156.3125 MiB | 390 MiB | 398 MiB |
| 12,544 | 256 | 48.90625 MiB | 398.03125 MiB | 406 MiB |
| 16,128 | 256 | 59.84375 MiB | 504.125 MiB | 512 MiB |
| 65,536 | 256 | 208.28125 MiB | 1,964.75 MiB | 1,974 MiB |

The maximum with 30.53125 MiB is source-derived from the **user-mode descriptor producer**; it predicts the observed 40 MiB small-private result without a separate empirical floor. The additive 7.875 MiB remains a **representative observational offset**, not a recovered constant in `getIdealPoolSize` or `getDesiredPoolSize`. The first discriminator at 12,544 B/group256 measured 406 MiB and excluded an exact 8 MiB term, which would have produced 408 MiB. The second at 16,128 B/group256 measured 512 MiB and excluded offsets above 7.875 MiB, which would have produced at least 514 MiB. Together with the original shapes, the quantized samples constrain a constant offset to `(7.84375, 7.875]` MiB, assuming this relation's form. Since the descriptor producer rounds `D8` to 32 KiB increments and the measured pool to 2 MiB increments, this interval is one full observable bin: further shapes through the same relation cannot distinguish values within it. If the term is an integer count of 4 KiB pages, values 2,009–2,016 remain possible. A runtime parameter readback is needed to identify the exact term. A pre-dispatch `footprint` snapshot had no unmapped-graphics row for 4 B, 4 KiB, or 64 KiB at group size 256; after dispatch it showed 40, 158, or 1,974 MiB, respectively. `vmmap` showed 79 separate 2 MiB unmapped-graphics regions in the 158 MiB case, without naming their owner more precisely.

An independent M4 control sampled the private `IOReport` channels *GPU Stats / UMA Pool Memory Usage* before and while each fresh-process probe held its pool. In ten sequential shapes, including both offset discriminators, the *Total Number of UMA Pools* channel rose by one and the *Total Number of UMA Pages* channel rose by exactly twice the process's unmapped-graphics footprint expressed in 4 KiB pages. Representative readings are:

| Private bytes/thread | Threads/group | Footprint | UMA page-counter change | Counter change ÷ 2 × 4 KiB |
| ---: | ---: | ---: | ---: | ---: |
| 4 | 256 | 40 MiB | 20,480 | 40 MiB |
| 512 | 64 | 52 MiB | 26,624 | 52 MiB |
| 4,096 | 256 | 158 MiB | 80,896 | 158 MiB |
| 12,288 | 1024 | 398 MiB | 203,776 | 398 MiB |
| 12,544 | 256 | 406 MiB | 207,872 | 406 MiB |
| 16,128 | 256 | 512 MiB | 262,144 | 512 MiB |
| 65,536 | 256 | 1,974 MiB | 1,010,688 | 1,974 MiB |

For 4 B, 4 KiB and 64 KiB at group size 256, repeated channel and footprint reads completed within 0.023 s of the command-buffer completion stamp. Both offset discriminator counter and footprint reads also finished within 0.023 s of completion. A deliberately concurrent pair produced a pool-count change of two and a page-counter change corresponding to the **sum** of their 52 and 398 MiB footprints; that reading was excluded from the sequential checks. The counter is global and drifts after releases, so its absolute value is not a per-process pool size. These matched fresh-process changes support attributing the measured footprint, including the offset near 7.875 MiB, to private-pool growth for these probes. They still do not identify which kernel sizing term supplies the offset.

For 64 KiB/thread at group size 256, the captured descriptor has `D0 = 208.28125 MiB` and `D8 = 1,964.75 MiB`, while the measured pool footprint is 1,974 MiB. With `S=0` and `H=0`, the sentinel branch returns 1,966 MiB; finite integer percentages through 100% also return at most 1,966 MiB, and 101% returns 1,984 MiB for intermediate power-of-two granules from 4 KiB through 2 MiB. No such percentage returns 1,974 MiB under the assumption that the method return equals observed growth. The corrected `H=1` growth branch can exceed ideal through `P[104]+G`, so this measurement does **not** exclude `S=0`. A diagnostic run recorded one descriptor-producer call through process exit, but the active `S`, `H`, `P[104]`, and method return have not been read from the kernel; allocation after the method may also add capacity.

Fresh diagnostic processes captured every `allocateUSCSpillBuffer` call for 4, 64, 256, and 1,024 B/thread at each group size. Both 4 and 64 B kernels made two calls in every group size; the later descriptor always had `D8 = 30.53125 MiB`, versus first-call `D8` values of 28.65625, 27.25, and 26.71875 MiB for groups 64, 256, and 1024. For the 256 and 1,024 B kernels, groups 64 and 256 made two calls with unchanged `D8`; group 1024 made one call. Across these 12 shapes, taking the largest captured `D8` and applying `round2(D8_MiB + 7.875)` reproduced all 144 separate uninstrumented AC samples (four dispatched-thread counts × three repeats per shape). In particular, the later 30.53125 MiB descriptor rounds to the observed 40 MiB for the 4 and 64 B shapes without an explicit floor in this observational relation. This does not prove the kernel lacks a floor or that each captured call reaches the pool.

At 4 B and group size 256, the first `(D0,D8)` was `(12.9375,27.25)` MiB and the later call produced `(17.4375,30.53125)` MiB. For 4 KiB and group size 256, only one call was seen in the diagnostic run. The 4 KiB descriptor was unchanged when the dispatch grew from 65,536 to 4,194,304 threads. The 16 KiB and 64 KiB short-workload descriptors also matched the corresponding full-workload descriptors in tested controls. The descriptor helps explain why dispatch thread count did not change the measured pool size, but the additive term's device origin and the active kernel-pool branch remain unresolved.

The original full-array kernel took more than 1 s on some long grids, and its immediate-after-completion snapshot sometimes fell below the value in the table. The single-dispatch path still showed this variation. A second kernel keeps the same dynamically indexed private array but touches 64 elements per thread. At 65,536 threads, it reproduced the original footprint for both 16 KiB and 64 KiB arrays across all group sizes and three repeats. Its 1,048,576- and 4,194,304-thread dispatches completed in at most 0.095 s. All 36 long-grid samples then matched the short-grid table exactly with zero repeat range. The historical selected grid has 252 valid samples for 84 shapes.

Two further M4 grids reran all 84 selected shapes with dispatch-start and completion timestamps: the original kernel for 4–4,096 B/thread and the previously checked short kernel for 16–64 KiB/thread. In each grid, all 252 fresh-process samples matched the predictions below exactly, with zero repeat range and no invalid or late rows. The battery-power grid's latest footprint and accelerator reads completed 0.061 s and 0.129 s after command completion. Its in-run records show no thermal or performance warning, but `ProcessInfo.thermalState` was read only after the sweep. The AC-power grid records a nominal API thermal state at the start of each sweep part and matching AC power before and after every sample. Its latest footprint and accelerator reads completed 0.040 s and 0.066 s after command completion; the stricter start-to-final-read bound was 0.766 s. Neither run intentionally overlapped another GPU workload, though background desktop activity remained possible. The battery and AC grids remain separate run conditions. A battery-started A/A control had an unverified power policy during its rounds. Two monitored AC A/A controls missed the 2% geomean gate, at 1.051 and 0.979, and are retained. A longer 12-round control gave 0.991 with 56/56 AC power readings, but Xcode changed from 26.6 to 27.1 during those rounds and the final OS thermal-state read was blocked by its new license state, so it is diagnostic rather than matched-toolchain acceptance evidence. The earlier Xcode 26.6 AC A/A control above passed at 0.986.

An empirical predictor fitted to the seven private-byte sizes above is

```text
pool_MiB = max(40, 2 * ceil((A_group + 31008 * private_bytes / 1048576) / 2))
A_group = 36.5 for 64 threads/group, 35.0 for 256, 34.5 for 1024
```

The coefficient `31008` is a fit parameter, not a verified resident-thread count. Before measuring, predictions were recorded for six held-out private-byte sizes at each of the three group sizes, with 65,536 dispatched threads. All 54 fresh-process samples (18 shapes × 3 repeats) matched exactly; maximum absolute error and repeat range were both 0 MiB. The largest dispatch took 0.094 s, and the last footprint/accelerator sample completed within 0.041 s of `POOL_READY`. Each cell below is **predicted / observed MiB**; the observed value is the median of three identical repeats.

| Held-out private bytes | 64 threads/group | 256 threads/group | 1024 threads/group |
| ---: | ---: | ---: | ---: |
| 512 | 52 / 52 | 52 / 52 | 50 / 50 |
| 2,048 | 98 / 98 | 96 / 96 | 96 / 96 |
| 8,192 | 280 / 280 | 278 / 278 | 278 / 278 |
| 12,288 | 400 / 400 | 400 / 400 | 398 / 398 |
| 32,768 | 1,006 / 1,006 | 1,004 / 1,004 | 1,004 / 1,004 |
| 49,152 | 1,490 / 1,490 | 1,490 / 1,490 | 1,488 / 1,488 |

These M4 data establish a local predictor over the tested range. They do not identify which driver variables produce its coefficients or validate it on M5.

Reproduce the sweep after `cargo build --release --locked` with:

```sh
cargo build --release --locked
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-private-pool.jsonl \
  --repeats 3 --hold-ms 1200 \
  --other-gpu-work 'record observed concurrent GPU work here'
```

The default grid covers private bytes 4–65,536, thread counts 65,536–4,194,304, and group sizes 64, 256, and 1024. For the long cases and held-out checks, use the same script with these shape arguments and separate output files:

```sh
# After comparing --pool-fast with the full kernel at 65,536 threads:
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-private-pool-fast-long.jsonl \
  --private-bytes 16384,65536 --threads 1048576,4194304 \
  --threadgroup-sizes 64,256,1024 --repeats 3 --hold-ms 1200 --pool-fast \
  --other-gpu-work 'record observed concurrent GPU work here'

# Held-out sizes; record predictions before running:
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-private-pool-heldout.jsonl \
  --private-bytes 512,2048,8192,12288,32768,49152 \
  --threads 65536 --threadgroup-sizes 64,256,1024 --repeats 3 --hold-ms 1200 \
  --other-gpu-work 'record observed concurrent GPU work here'
```

Keep the JSONL and prediction file outside the repository. The script writes failures as rows and records the OS, device, toolchain, power/thermal state, sample lag, dispatch duration, every per-process graphics-footprint row, the selected pool row and column, and the global accelerator counter. The default selection is the M4 `Owned physical footprint (unmapped) (graphics)` row, summed across its dirty, clean, and reclaimable columns. On M5, inspect a pilot `footprint -p <pid> -f bytes` output for a fresh small-private and large-private probe and use an isolated UMA-counter differential, where available, to identify the category and column that track the pool. Record that selection before fitting or held-out predictions. Pass the same `--pool-row` and `--pool-component` to both the sweep and checker; they reject a missing row or inconsistent selection. The M5 baseline's `IOAccelerator` and reclaimable accounting must not be silently treated as the M4 unmapped-graphics row.

After recording predictions, check every row, including invalid or late samples, with `python3 scripts/check_private_pool.py --data <samples.jsonl> --predictions <predictions.json> --block-mib 8 --repeat-limit-mib 8 --require-completion-window --require-run-conditions`. The checker reports each shape's median, maximum prediction error, repeat range and any exception. The run-condition gate requires a stable power source, a start-of-run `ProcessInfo.thermalState` value, post-sample thermal readings, and one device/OS/Xcode/Metal-compiler identity. The sweep fails before measurement if `xcrun metal -v` cannot return a compiler version. The M4 held-out result above also passes with `--block-mib 2`; its historical logs predate the dispatch-start and API thermal-state fields, so omit the two strict flags for those files.

The M4 sweep does not complete the G17X sizing investigation. Static inspection of the same OS build's user-mode Metal plugins finds a G16G `SpillInfoGen3`/HAL200 path and a G17X `SpillInfoGen4`/HAL300 path. The G17X compute descriptor producer reads a selected active-USC mask and device count; one conditional branch combines 4 KiB and 64 KiB ceiling divisions before writing its page counts. That branch can be translated symbolically on this M4, but its selection and numeric inputs require an M5 process. A further translation of G17X `allocateUSCSpillBuffer` separates two resource-list bounds: `maxActive = min(floor(2L/N), A)` and `selectedPop = min(pop, floor(2L/N)*N)` when a resource list is present, where `L` and `N` are device-config words, `A` is `MaxActiveUSCsInMGPU()`, and `pop` counts the selected USC mask. Without a resource list, these are `A` and `pop`. The `A` method is itself resolved: it uses the selected mask at device-config offset `0x1be0` or `0x1bf0`, a width at `0x1c5c`, and `N` slices (`N` is at `0x1c44`), then returns the largest slice popcount. It writes descriptor word at byte offset 64 as `N*(11*maxActive + sum(this[0], this[4], this[8], this[12], this[16]) + 10)` in unsigned 32-bit arithmetic. The active G16G kernel's `getSelfReserveSize` treats descriptor offset 64 as a page count; the G17X kernel consumer must still be verified on M5. The user-mode descriptor arithmetic does not by itself reveal the kernel pool's minimum, ideal, and desired-size arithmetic. The G17X result still requires a formula reconstructed from its driver path and checked against an M5 Max sweep on the specified reference toolchain, including at least six held-out shapes within one 8 MiB block and a repeat range within 8 MiB. Do not transfer the M4 coefficients or its observed 2 MiB footprint steps to G17X.

Both G17X HAL300 compute-context variants in this OS build's user-mode binary call `SpillInfoGen4::checkSpillParamsForCompute` from `endComputePass` with the first six integer arguments zero. They call `allocateUSCSpillBuffer` again only when the check reports a change. The zero-input check branches converge regardless of the option byte at device-config offset `0x4327` and bit 31 of its word at `0x4328`. For nonnegative values without signed 32-bit overflow, its candidate page counts reduce to:

```text
P = popcount(selected 128-bit USC mask)
N = load_u32(config + 0x1c44)
M = load_u32(spill + 0x48)
base = load_u32(spill + 0x50)
R = load_u32(spill + 0x60)
candidate14 = round_up_8(N * (base + 65 + 10*P + M*max(170-R, 0)))
candidate18 = round_up_8(N * (base + 65 + M*max(490-R, 0)))
```

These are **static G17X user-mode code translations**, not M5 measurements or physical pool sizes. The simple integer form is valid only when each candidate still fits a signed 32-bit page count after rounding; the machine uses 32-bit arithmetic and signed SIMD maxima. The function compares the candidates with existing spill-object fields; descriptor allocation may apply further terms. The M5 run must capture the selected mask, `N`, `M`, `base`, `R`, and the before/after fields to test this path and decide whether an end-pass descriptor affects the pool.

The ordinary compute check also has a source-like reduction in the non-overflow domain. Let `f1`–`f6` be its first six unsigned input arguments, and let `P`, `N`, `M`, `base`, and `R` have the meanings above. The instructions at `0x218986590`–`0x2189866A4` yield these page-count terms:

```text
a = 32*max(f1,f2)
s = 32*f3
c = 0 if f5 == 0 else f5 + 64
P0 = floor((a + 0x537f)/4096) + ceil(s/4096)
P1 = floor((a + 0x1437f)/65536) + ceil(s/65536)
C4 = ceil(c/4096) + ceil(f4/4096)
C64 = ceil(c/65536) + ceil(f4/65536)
```

The schedule depends on config byte `0x4327` and config word `0x4328`. With `e = max(1, ceil(f6/32))`, the two branches reduce to:

```text
if config[0x4327] == 1:
    per = min(floor(96/e) + (e > 1), 96)
    baseline = (min(e,32) + 95)*P0 + 10*P1
else:
    per = floor(96/e)
    if bit31(config[0x4328]) == 0 and f4 > 0 and config[0x4328] <= 62:
        per = min(per, floor((4096*config[0x4328] + 65536)/round_up_64(f4)))
    count = floor((per + 95)/per) if f6 == 0 and per <= 95 else e
    active = count*per
    baseline = min(active,96)*P0 + min(active,10)*P1
q = baseline + per*C4 + min(per,8)*C64
g = 32 if f6 == 0 else ceil(f6/32)
z = C64 + P1*min(g,10)
v = max(P0*g + P1*min(g,10) + C64 + C4 - R,0) + C64 + C4
common = base + C64 + P1 + 64
candidate14 = round_up_8(N*(common + P*z + M*v))
candidate18 = round_up_8(N*(common + M*max(q-R,0)))
```

When `per=0`, the `count` division is ARM `UDIV` and yields zero. The quotient in `w1` at `0x2189866C0` remains uncapped for the `P0*g` term, while `w6` at `0x2189866CC` caps only the `P1` term at 10; this distinction matters when `f6` exceeds 320. `scripts/g17x_descriptor.py --compute-candidates` evaluates both candidate page fields from captured `f1`–`f6`, mask, option fields, and spill-object words. It rejects signed overflow rather than pretending Python's unbounded integers reproduce the machine's wraparound. Static analysis does not identify which branch the M5 Max selects.

### M5 reference validation runbook

The acceptance criteria in this subsection are requirements from issues #1, #9 and #15; commands are the proposed procedure for gathering their evidence.

Run this on the 40-core M5 Max with macOS 27.0.1 build 26A434 and Metal toolchain 27.1.266.1. Record `system_profiler SPDisplaysDataType`, `sw_vers`, `xcodebuild -version`, `xcrun --find metal`, `xcrun metal -v`, `pmset -g batt`, `pmset -g therm`, and concurrent GPU work before the sweep. If any reference identifier differs, label the data as a separate configuration. Check a numerical reference before A/A timing; the sweep repeats this check in every fresh process before taking a footprint.

First run a two-shape pilot to inspect this host's graphics accounting. The default selector is the M4 unmapped-graphics row; if M5 lacks that row, these pilot rows are marked invalid but still retain the raw `graphics_rows` for inspection. Choose the row and column whose change tracks the private-memory allocation, checking an isolated UMA-counter delta where available. Record the choice before fitting. Then repeat the pilot with the chosen selector and require valid snapshots before the full grid:

```sh
cargo build --release --locked
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-m5-accounting-pilot.jsonl \
  --private-bytes 4,65536 --threads 65536 --threadgroup-sizes 256 \
  --repeats 1 --hold-ms 1200 \
  --other-gpu-work 'replace with observed concurrent GPU work'
python3 - <<'PY'
import json
for line in open('/tmp/metal-perf-m5-accounting-pilot.jsonl'):
    row = json.loads(line)
    print(row['private_bytes'], row['status'], row.get('graphics_rows'), row.get('error'))
PY
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-m5-accounting-verified.jsonl \
  --private-bytes 4,65536 --threads 65536 --threadgroup-sizes 256 \
  --repeats 3 --hold-ms 1200 \
  --pool-row '<pilot-verified graphics row>' --pool-component '<pilot-verified column>' \
  --other-gpu-work 'replace with observed concurrent GPU work'
```

```sh
target/release/metal-perf idle-gap \
  --private-bytes 4096 --threads 262144 --threadgroup-size 256 \
  --pool-probe --pool-hold-ms 0
target/release/metal-perf ab run \
  --variant 'baseline=target/release/metal-perf idle-gap --private-bytes 4096 --threads 262144 --threadgroup-size 256 --gaps-ms 50 --repeats 1' \
  --variant 'baseline_repeat=target/release/metal-perf idle-gap --private-bytes 4096 --threads 262144 --threadgroup-size 256 --gaps-ms 50 --repeats 1' \
  --rounds 5 --discard 1 --cooldown 10 \
  --metric '^\s*50\s+\S+\s+/\s+([0-9.]+)' --out /tmp/metal-perf-m5-aa
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-m5-full.jsonl \
  --repeats 3 --hold-ms 1200 \
  --pool-row '<pilot-verified graphics row>' --pool-component '<pilot-verified column: dirty, clean, reclaimable, or total>' \
  --other-gpu-work 'replace with observed concurrent GPU work'
```

The A/A control must stay within 2% geometric mean across the retained rounds; keep its logs even if it fails. The JSONL records all 84 shapes, invalid samples and measurement lag. Inspect dispatch durations and repeat ranges. If a long-running shape shrinks before the post-completion snapshot, first compare the full and `--pool-fast` kernels at 65,536 threads for that private-byte size and each group size, then repeat only the affected long-grid shapes with `--pool-fast`. Keep both raw logs and mark the selected rows and their source. Do not silently discard an unstable or invalid run.

Fit the G17X formula using only a declared training subset. Store predictions **before** running at least six held-out shapes in a JSON list; each entry has `private_bytes`, `threads`, `threadgroup_size`, and an integer `predicted_bytes` from the derived driver formula. The checker also accepts the existing M4 `predicted_mib` files, but requires exactly one prediction field per shape and reports byte-level errors. A ready-to-run held-out grid is:

```sh
python3 scripts/private_pool_sweep.py \
  --output /tmp/metal-perf-m5-heldout.jsonl \
  --private-bytes 512,2048,8192,12288,32768,49152 \
  --threads 65536 --threadgroup-sizes 64,256,1024 \
  --repeats 3 --hold-ms 1200 \
  --pool-row '<same pilot-verified graphics row>' --pool-component '<same pilot-verified column>' \
  --other-gpu-work 'replace with observed concurrent GPU work'
python3 scripts/check_private_pool.py \
  --data /tmp/metal-perf-m5-heldout.jsonl \
  --predictions /tmp/metal-perf-m5-heldout-predictions.json \
  --pool-row '<same pilot-verified graphics row>' --pool-component '<same pilot-verified column>' \
  --block-mib 8 --repeat-limit-mib 8 --require-completion-window --require-run-conditions
```

Create a second prediction list for every full-grid shape and run the same checker against the selected full-grid JSONL. For #15, identify the active G17X driver binary and its device-selected constants before naming the descriptor fields or presenting a closed form. `kmutil inspect --show-fileset-entries --verbose` identifies the loaded kernel extension and prints the active boot collection path on its first output line; `kmutil emit-macho --boot-path <that printed path> --allow-missing-collections` unwraps that collection. Its symbol table can locate `AGXUSCPrivMemPoolUMA::getDesiredPoolSize`, `getIdealPoolSize`, `getMinPoolSize` and `computeAndSetUMAPoolVariables`. Use `xcrun dyld_info -disassemble /System/Library/Extensions/AGXMetalG17X.bundle/Contents/MacOS/AGXMetalG17X` for the user-mode path. The final table must show predicted and observed bytes, error, repeat range and every exception; explain each discrepancy through a verified clamp, floor, rounding rule or device constant. Run the repository formatting, Clippy, test, mdBook and internal-link gates before review.

On the M5 reference host, capture a descriptor in a **separate diagnostic process** before fitting the driver formula. The same OS build's G17X user-mode plugin has `SpillInfoGen4<HAL300>::checkSpillParamsForCompute` and `allocateUSCSpillBuffer` symbols, but M4 cannot execute that path. The following LLDB sequence reads the descriptor after its producer returns; repeat it for the training sizes and a held-out shape, preserving transcripts outside the repository:

```sh
xcrun lldb --batch \
  -o 'breakpoint set --func-regex allocateUSCSpillBuffer' \
  -o run \
  -o 'expression -l c++ -- unsigned long $spill = (unsigned long)$x0' \
  -o 'expression -l c++ -- unsigned long $desc = (unsigned long)$x1' \
  -o 'expression -l c++ -- unsigned long $resource = (unsigned long)$x2' \
  -o 'expression -l c++ -- unsigned long $config = *(unsigned long*)($spill + 0x38)' \
  -o 'memory read --format x --size 4 --count 32 $spill' \
  -o 'memory read --format x --size 8 --count 4 $config+0x1be0' \
  -o 'memory read --format x --size 4 --count 1 $config+0x1c1c' \
  -o 'memory read --format x --size 4 --count 1 $config+0x1c44' \
  -o 'memory read --format x --size 4 --count 1 $config+0x1c5c' \
  -o 'thread step-out' \
  -o 'memory read --format x --size 4 --count 18 $desc' \
  -o 'breakpoint disable 1' \
  -o continue -- target/release/metal-perf idle-gap \
  --private-bytes 4096 --threads 65536 --threadgroup-size 256 \
  --pool-probe --pool-hold-ms 0
```

Inspect the stopped frame to confirm it is `AGXMetalG17X` before interpreting the bytes. Small-private kernels can invoke this producer more than once; record each call when deciding which descriptor reaches the pool. For the kernel side, pass the *active* boot collection path reported on that host to `kmutil emit-macho --boot-path <active-kernelcache> --allow-missing-collections`; the M4 path under Preboot is not an M5 path. Resolve virtual calls against the G17X vtable, recover current values of the kick counts, ideal constant, sizing flag, percentage property, freelist fields and pool maximum, and evaluate minimum, ideal and desired separately for every measured shape. Compare those return values with the process footprint while accounting for any other charged graphics blocks. An isolated diagnostic sample of the M5 UMA pool IOReport channels, if present, can independently check pool attribution; verify G17X counter-update semantics before interpreting its values, and exclude concurrent pool activity. Preserve a prediction file made from the derived formula before measuring the held-out shapes. The M4 descriptor formula above is a method example, not an M5 parameter set.

In a separate fresh diagnostic process, break at `checkSpillParamsForCompute` and record `w1`–`w6` plus the spill object before and after the call. Repeat the `continue`/read/`thread step-out` sequence until the zero-argument `endComputePass` call is seen; the number of earlier calls can vary. For a two-call case, use:

```sh
xcrun lldb --batch \
  -o 'breakpoint set --func-regex checkSpillParamsForCompute' \
  -o run \
  -o 'expression -l c++ -- unsigned long $spill = (unsigned long)$x0' \
  -o 'expression -l c++ -- unsigned long $config = *(unsigned long*)($spill + 0x38)' \
  -o 'register read w1 w2 w3 w4 w5 w6' \
  -o 'memory read --format x --size 8 --count 4 $config+0x1be0' \
  -o 'memory read --format x --size 4 --count 1 $config+0x1c44' \
  -o 'memory read --format x --size 1 --count 1 $config+0x4327' \
  -o 'memory read --format x --size 4 --count 1 $config+0x4328' \
  -o 'memory read --format x --size 4 --count 32 $spill' \
  -o 'thread step-out' \
  -o 'memory read --format x --size 4 --count 32 $spill' \
  -o continue \
  -o 'expression -l c++ -- unsigned long $spill2 = (unsigned long)$x0' \
  -o 'expression -l c++ -- unsigned long $config2 = *(unsigned long*)($spill2 + 0x38)' \
  -o 'register read w1 w2 w3 w4 w5 w6' \
  -o 'memory read --format x --size 8 --count 4 $config2+0x1be0' \
  -o 'memory read --format x --size 4 --count 1 $config2+0x1c44' \
  -o 'memory read --format x --size 1 --count 1 $config2+0x4327' \
  -o 'memory read --format x --size 4 --count 1 $config2+0x4328' \
  -o 'memory read --format x --size 4 --count 32 $spill2' \
  -o 'thread step-out' \
  -o 'memory read --format x --size 4 --count 32 $spill2' \
  -o 'breakpoint disable 1' -o continue -- \
  target/release/metal-perf idle-gap --private-bytes 4 \
  --threads 65536 --threadgroup-size 256 --pool-probe --pool-hold-ms 0
```

Confirm the frame is `AGXMetalG17X` at each stop. Record the config fields and `w1`–`w6` at every call; the second call may use a different spill object. For the ordinary call, use `--compute-candidates` and compare post-call page fields at offsets `0x14` and `0x18` with the larger of their pre-call values and its candidates. On the zero-input call, make the same comparison with the end-pass candidates; a second descriptor allocation may be skipped if no field changes.

For each G17X descriptor capture, transcribe the LLDB values into a JSON object with `config` keys `primary_mask_0x1be0`, `alternate_mask_0x1bf0` (128-bit hex strings formed from two little-endian 64-bit words each), `device_count_0x1c44`, `resource_limit_0x1c1c`, and `usc_width_0x1c5c`; a Boolean `resource_list_present` from whether `$resource` is nonzero; and `spill_words` mapping the captured 32-bit offsets `0x00`, `0x04`, `0x08`, `0x0c`, `0x10`, `0x14`, `0x18`, `0x1c`, `0x24`, `0x28`, `0x2c`, and `0x30` to their values. Run `python3 scripts/g17x_descriptor.py /tmp/m5-spill-inputs.json` and compare its predicted values at descriptor offsets 0, 8, and 64 with the post-call memory read. To evaluate the compute-check candidates, also transcribe spill-object words `0x48`, `0x50`, and `0x60`, config byte `compute_option_0x4327`, config word `compute_word_0x4328`, and a `compute_args` object with keys `f1` through `f6` and integer values from the entry registers; run `python3 scripts/g17x_descriptor.py --compute-candidates /tmp/m5-spill-inputs.json`. The zero-input end-pass candidate uses `--zero-input-end-pass` with the same spill-object words. These evaluate only the recovered **user-mode descriptor producer**. The M5 kernel's pool methods and their device-selected parameters still need separate reconstruction before predicting physical pool bytes.

## Physical backing

Private-memory growth, descriptor preparation, accepted purge requests and physical-page release are distinct operations. Allocation reaches the system-memory and buffer-descriptor layers; preparation can wire virtual backing. A smaller freelist or a successful purge request does not establish that physical pages have already been released.

The private-memory block allocation path uses pageable buffer descriptors. Descriptor preparation and completion are separate from allocation and destruction. On unwire, ordinary backing can replay a deferred purge-state request. Backing marked always-volatile instead requests states 2 then 3; a previously recorded state-4 request is not necessarily the operation performed at this boundary. Purge results and physical-page counts must therefore be interpreted separately.

Pageable backing preparation selects among resident-page reuse, free-frame acquisition and fault resolution:

| Preparation path | Physical-backing behavior |
| --- | --- |
| Suitable resident page | Reuses the page already associated with the object and offset |
| Eligible empty object | Acquires free page records from a CPU list or the VM allocator, inserts them into the object and records their physical page numbers in the page-list result; object offsets advance by 16 KiB |
| General unresolved page | Invokes VM fault resolution and consumes the returned page according to its result |
| Unavailable free page | Can wait and retry, or fail through the request's cleanup path |

Physical-frame ownership and GPU freelist capacity are tracked separately. The examined allocator acquisition and release paths update the same frame-group bitmap table. Page clearing is controlled by the selected path and options; the page-grab helper does not unconditionally clear every acquired page.

Preparation failure aborts acquired page lists before releasing their list references. Per-page rollback depends on the saved page and list state:

| Rollback operation | Effect |
| --- | --- |
| Remove a wire reference | Decrements the page's wiring count; eligible zero-count pages can be requeued |
| Requeue a retained page | Changes queue state without returning the page through the free-page path |
| Return an eligible page | Prepares the page for batched free-page return |

Rollback does not imply that every prepared page is freed. Retained pages can remain owned by the backing VM object; page-list references and VM-object ownership have separate lifetimes.

On the examined pageable-descriptor path, completion decrements the wiring count. When that count reaches zero, outstanding direct memory access (DMA) activity causes a panic; otherwise completion finalizes the page-list state.

Destruction separately balances remaining preparation references and releases mapping references. A shared memory-reference record is destroyed only when its count drops from one to zero, releasing its stored kernel port send rights. Backing reclamation then crosses several ownership boundaries:

| Boundary | Reclamation behavior |
| --- | --- |
| Last memory-entry send right | The registered cleanup callback detaches the memory entry and destroys its port; entries carrying VM-copy backing discard that copy |
| VM-copy disposal | Releases referenced VM objects; other entry types follow separate cleanup paths |
| VM-object release | Remaining references preserve the object; final teardown can wait for paging activity or defer work to a reaper |
| Resident-page disposal | Eligible pages are detached from the object and returned through batched allocator updates |

This establishes a conditional path from descriptor destruction to physical-page reclamation. It does not imply immediate reclamation: shared references, object state and in-progress paging can retain backing after the descriptor is gone.

Deferred VM-object destruction is handled by a dedicated reaper thread. It removes objects from the deferred queue, waits for their paging and activity counts to reach zero, and then runs terminal cleanup. The wait and subsequent page reclamation are separate from GPU command completion.

Paging-end and activity-end paths can each wake this waiter when both counts reach zero. Paging-end also has a separate wake event when the paging count reaches zero. Releasing the page-list object's own reference is a separate operation; its final disposal can release an additional tracked activity reference.
