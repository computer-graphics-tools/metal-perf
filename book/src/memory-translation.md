# Memory translation

This chapter describes build 26A434 address translation for the GPU and its firmware: page granules, boot memory records, mapping, invalidation and fault decoding. Applications must use the GPU addresses, residency and lifetime contracts supplied by Metal. CPU virtual addresses, GPU virtual addresses and physical addresses are different views.

Selected CPU page-table paths on build 26A434 use 16 KiB granules. This does not establish GPU cache-line size, the alignment of every GPU allocation, or a universal GPU page geometry. Observed buffer-descriptor paths distinguish existing system mappings from wired-page walks; their returned addresses cannot unconditionally be treated as raw physical addresses.

Firmware CPU translation has its own configurable root and shared-table inputs. The examined startup code conditionally maps a UAT table range and a separate 16 KiB shared page from supplied base and length values. These translation inputs are separate from host resource allocation; values in an unmodified firmware image do not establish the running translation roots or permissions.

Boot-time GPU setup on build 26A434 reads numbered memory records with a base at byte 0 and a length at byte 16. The configuration and firmware patch routines use these records as follows:

| Memory record | Initial length | Configuration base / length properties | Firmware patch value |
| --- | ---: | --- | --- |
| 8 | 16 KiB | `gpu-region-base` / `gpu-region-size` | `GptB`: base, when the local skip flag is clear |
| 52 | 512 KiB | `gfx-shared-region-base` / `gfx-shared-region-size` | `PTPB`: base plus per-ASC offset; `PTPS`: per-ASC length |
| 217 | 16 KiB | `gfx-shared-l2-region-base` / `gfx-shared-l2-region-size` | `Gx2S`: base |
| 56 | 16 KiB | `gfx-handoff-base` / `gfx-handoff-size` | `PplB`: base |

The initial table has 255 records of 64 bytes. All four listed base fields initially contain `0xffffffffffffffff`, which the validity check rejects; the template therefore supplies no usable address for these regions. A valid record and a populated record are different states: the populated-record check also requires nonzero base and length.

The initial upper-side reservation pass visits the table in order. For each record with an unassigned base, a known length and policy bit 0 clear, it subtracts the stored length from the upper boundary and writes the result as the base. This pass performs no additional alignment rounding or length rewrite. Earlier recorded ranges can lower its starting upper boundary. The GPU base and length properties are published on the `sgx` device-tree node.

For `PTPB` and `PTPS`, the callback derives a count of one or two from the argument's upper nibble and an index from its lower nibble. It divides the shared-region length by that count and offsets the base by index times the resulting length. The 64-bit patch helper requires a matching tag with an eight-byte payload before copying the value. These boot allocations are distinct from growable command-memory pools; their metadata does not measure current physical-page residency.

The GFX and GFX1 boot descriptors select the same preparation callback and carry arguments `0x10` and `0x11`, encoding two partitions with indices zero and one. Early boot resolves and authenticates the callback pointers before the CPU translation transition. The registered-descriptor preparation path builds four 48-byte callback records. Its third record wraps the primary descriptor callback and supplies the current firmware context; its fourth record copies the descriptor's secondary callback. The dispatcher bounds-checks the list and skips null function pointers; the primary wrapper validates its callback view before forwarding. Registration alone does not establish execution during a particular boot.

The boot reservation allocator maintains lower and upper allocation boundaries. A zero size request uses the record's stored length. A zero alignment request uses the allocator granule; an explicit alignment must be a power of two at least as large as that granule.

| Record policy bit 0 at byte 28 | Base assignment | Boundary update |
| --- | --- | --- |
| Clear | Round the upper boundary minus requested size down to the requested alignment | Upper boundary becomes the assigned base |
| Set | Round the lower boundary up to the requested alignment | Lower boundary becomes base plus granule-rounded length |

Both paths store a granule-rounded length and require the lower boundary to remain below the upper boundary. The selected entry rejects an already populated record and a record with policy bit 4 set. These address-reservation operations are separate from proving that physical pages have been acquired or that a command-memory pool has grown.

On build 26A434, preloaded firmware segment metadata uses little-endian 32-byte records. Semicolon-separated segment names correspond to the records in order.

| Byte offset | Width | Segment field |
| --- | ---: | --- |
| 0 | 64 bits | Physical base |
| 8 | 64 bits | Firmware virtual base |
| 16 | 64 bits | Remapped base |
| 24 | 32 bits | Length in bytes |
| 28 | 32 bits | Flags |

Segment addresses are scoped to the selected firmware role. The host segment object's writability flag and the firmware CPU's page-table permissions describe separate access domains.

## Mapping and invalidation

The system-managed mapping path requires a 16 KiB-aligned GPU address and length. It converts backing-descriptor segments into 40-byte kernel records, then submits compact batches to the protected translation interface. Pending unmaps are collected and committed separately. The driver-managed path also drains pending unmaps before adding mappings; its selected leaf encoder combines `(address & 0x3ffffffc000)` with an unmasked flags input.

The internal firmware-mapping helper accepts cache-selector values 0 and 1. For either value, an existing mapping attached to the resource is reused without reapplying changed access or extra-option arguments. A fresh mapping uses a GPU task reference separate from the resource's `kernel_task` reference. Request flags pass through backing-descriptor checks before the final UAT options are selected, so the requested access selector alone does not establish effective permissions.

The protected translation implementation walks up to three table levels using eight-byte entries. Intermediate traversal requires both low entry bits to be set. The leaf index uses address bits 14–24, the next level uses bits 25–35, and the root index uses a configured address mask shifted by 36. Returning a leaf slot does not establish that the slot contains a valid mapping.

| Protected mapping operation | State and ownership behavior |
| --- | --- |
| State acquisition | Validates the handle and allowed state-object type, then atomically changes the expected state to locked; an unexpected state is rejected |
| Mapping continuation | Registers applicable backing mappings and increments table usage before publishing leaf entries; preserves segment and page progress between bounded batches |
| Unmap first pass | Clears the two low bits of each selected leaf entry while retaining its encoded address |
| Unmap synchronization | Orders entry updates, issues translation invalidation and performs conditional permission/cache bookkeeping before the second pass |
| Unmap second pass | Unregisters applicable backing mappings and decrements leaf-table usage; preserves progress when another continuation is required |
| Table unlink | Clears the parent entry, executes `DSB OSHST`, requests invalidation, then executes `DSB OSHNXS` before reference-count updates |

Table installation requires the child table's prior reference count to be zero. Table unlink requires the detached table's reference count to be one before decrementing it. Releasing the associated frame guard releases metadata access; it does not return the backing page to the physical allocator. Mapping references, table usage, metadata guards and physical-page ownership are separate lifetimes.

With a valid context identifier, the direct-instruction invalidation path selects the following operations. These are implementation policies, not application controls.

| Policy or request | Instruction |
| --- | --- |
| Policy 0 | `TLBI VMALLE1OS` |
| Policy 2, or another nonzero policy with a zero address, zero length or length at least 32 MiB | `TLBI ASIDE1OS` |
| Policy other than 0 or 2, nonzero address, positive length below 32 MiB and no larger than the system page size | `TLBI VAE1OS` |
| Same conditions, but length larger than the system page size | `TLBI RVAE1OS` |

Context identifier `0xffffffff` skips this invalidation helper. The device-specific path delegates through an optional accelerator handler. The direct barrier helper executes `DSB SY`; translation invalidation, mapping completion and shader completion remain separate boundaries. The selected constructor initializes the direct-path flag and policy to zero; startup sets both to one before entering family startup. Later configuration and per-object snapshots must still be considered when identifying the active path.

The selected G17X MMU fault decoder accepts index 0 and uses the following fields. Other indices return no decoded fault from this method.

| Field | Encoding | Reported interpretation |
| --- | --- | --- |
| Valid | Status bit 0 | Remaining fields are reported only when set |
| Reason | Status bits 1–3 | Page Invalid, AF fault, Write Only, Read Only, No Access; remaining values map to invalid |
| Requestor | Status bits 9–16 | Index into a 256-entry name table; examples include `0xa0: UMA_CORE` and `0xd0: gCDM_CS` |
| Address | Low 42 bits of the address register, shifted left by 6 | The decoded address has six zero low bits; this is not a cache-line or page-size guarantee |
| Sideband | Status bits 23–29 | Published as `sideband` |
| Translation level | Status bits 7–8 | Published as `level` |
| Read direction | Status bit 4 | Published as the Boolean `is_read` |

The driver-managed translation check selects one of two table sets with address bit 42, then indexes with bits 36–41, 25–35 and 14–24. Its selected single-stage implementation tests entry bit 0 for validity; the leaf indexing corresponds to a 16 KiB granule.

| Reported level | Condition that matches the translation check |
| --- | --- |
| 2 | The top-level entry is invalid |
| 1 | The top-level entry is valid, but the middle table is absent or its entry is invalid |
| 0 | Both ancestors are valid, but the leaf table is absent or its entry is invalid |

This check reads translation state without repairing it. A separate configuration selects the system-managed translation backend, so these conditions do not define every active translation path or a recovery guarantee.

The system-managed backend submits a 16-byte query containing a 64-bit address, a 32-bit level and a zero reserved word. Its kernel handler returns a one-byte result. On the ordinary table-walk path, the result matches the missing-entry level against the reported level. An intermediate-table address outside the checked physical interval also produces a true result without that comparison.

Query completion and the returned Boolean are separate: a nonzero handler status makes the wrapper panic, while successful completion returns the low bit of the response. The query does not repair mappings. The selected recovery diagnostic caller invokes the configured backend's check and discards its Boolean result; this call does not determine whether recovery succeeds.
