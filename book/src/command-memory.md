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
| Idle shrink | Reduces the retained block set and can request purge state 4 on the backing memory object | While that object is resident, the driver records the purge request and returns success without invoking the underlying descriptor operation |
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
