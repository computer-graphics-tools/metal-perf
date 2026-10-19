# Firmware interface

This chapter describes the build 26A434 host–firmware boundary: firmware roles, control and event rings, completion queues and stamps. Ring layouts, command numbers and event types are implementation identifiers, not application ABIs.

## Firmware roles

The build 26A434 G17X host implementation separates control commands, firmware events and shared completion stamps. The reference configuration's published firmware UUIDs identify `armfw/a000` for the GFX scheduler role and `armfw1/a000` for the GFX1 power role. A UUID match identifies the firmware variant; it does not establish byte-for-byte identity of live firmware memory.

The reference GFX and GFX1 providers advertise `pre-loaded` and `no-firmware-service`. RTBuddy tests property presence when initializing these loader flags. Firmware-service discovery matches `RTBuddyFirmwareService` and its `Role` property; separate AGX clients attach through `RTBuddyService` for GPU endpoint communication.

The coprocessor provider nubs are constructed from device-tree entries. Preloaded segment metadata is a boot-configuration input to this path, separate from subsequent host allocations for command and event rings.

Firmware startup also imports populated memory-region descriptors through a 54-entry index table. Source and destination records have a 40-byte stride; the import copies bytes 0–35 when the source record's first 64-bit word is nonzero. These region indices are separate from control-command identifiers and event-packet types.

Deferred work in the selected GFX1 implementation uses a separate 64-bit pending mask. The worker atomically takes and clears this mask, applies state-dependent filtering and dispatches through 36 mask/callback/selector records. Matching callbacks receive their registered selector and a mutable pending mask; repeated notifications of the same bit can coalesce. This mechanism is separate from the firmware event ring.

## Control and event rings

| Boundary | Implementation behavior | Completion meaning |
| --- | --- | --- |
| Control publication | Copies a 64-byte entry into a 256-slot ring; executes `DMB ISH` before publishing the write cursor | Makes a control entry available to firmware |
| Control consumption | Executes a command handler, then advances the read cursor modulo 256 | Records consumption; invalid or ignored commands can also advance the cursor |
| Threshold response, command 33 | Accepts an ID below 256 and writes it to a register block | Does not establish successful memory growth or hardware acknowledgment |
| ID retirement, command 46 | Checks the supplied identity against a per-ID state record, removes selected dependency state and may resume blocked work | State retirement does not establish physical backing release |
| Event publication | Publishes 72-byte entries; the selected firmware producer executes `DSB SY` before and after write-cursor publication | Orders event data and notification publication |
| Stamp notification, event 1 | Carries four 32-bit stamp-index bitmaps; the host wakes the selected waiters | Notification does not write the shared stamp values |
| Private-memory events | Allocation request, grow-request completion and threshold notification have distinct host handlers | The registered allocation-request action is empty, and the selected implicit engine's grow-completion callback returns zero without changing the pool |

Control-ring progress, stamp-value publication and resource retirement are separate boundaries. The selected control-consumption tail has no explicit barrier; the event producer's barriers must not be generalized to every firmware ring. A control wait cannot replace a shader-completion dependency or the resource-lifetime contract.

## Firmware logs

Firmware log requests also have distinct effects. Size and copy select the stored log segment or a UUID-matched slice, rather than an arbitrary firmware address.

| OSLog request | Effect |
| --- | --- |
| Size | Copies the selected log length to the host caller |
| Copy | Maps the selected log backing read-only and copies its bytes to the host caller |
| Release | Advances release accounting and can free the stored log backing |
| Notify | Sends an endpoint message to firmware |

## Event ring layout

The examined event ring uses a 48-byte control region and an 18,432-byte data region containing 256 entries of 72 bytes, packed into shared backing. Its read cursor is at control offset 0 and its write cursor at offset 32. The producer advances modulo 256 and waits when the next write cursor equals the read cursor, leaving at most 255 entries outstanding.

The primary and secondary firmware event channels occupy distinct subregions of one shared allocation. The selected layout uses these byte offsets from the allocation base:

| Channel | Control offset | Data offset | Control bytes | Data bytes |
| --- | ---: | ---: | ---: | ---: |
| Primary | 62,016 | 62,656 | 48 | 18,432 |
| Secondary | 800,000 | 800,640 | 48 | 18,432 |

Each region uses the same offset in its CPU and firmware mappings, with 64-byte alignment. Separate channel addresses do not imply separate backing resources, physical contiguity or identical access permissions.

Startup publishes separate control and data addresses into the firmware's shared structure. Other adjacent ring addresses serve separate channels; sharing a record size does not make their records firmware events. The selected G17X address-conversion method preserves the numeric address, which does not establish identical translation contexts or permissions for the GPU and firmware.

## Completion queues and stamps

Hardware completion uses four separate queues with 64-byte entries. Each queue has GPU and firmware mappings of the same backing allocation; each allocation size is rounded up to 256 bytes.

| Boundary | Implementation behavior |
| --- | --- |
| Queue descriptor | Contains GPU address, firmware address, entry count, 64-byte stride, hardware selector and software read cursor |
| 3D submission | Copies the firmware-mapped command-record reference and channel context into kick-entry offsets `0x10` and `0x18`; executes `DMB ISH` before publishing the cursor |
| Queue configuration | Programs the GPU address, entry count and enable bit into the selected hardware register; initializes the software read cursor to zero |
| Completion consumption | Reads available count from the selected hardware register, performs cache invalidation and reads entries through the firmware mapping; advances the cursor modulo the entry count and reports the processed count |

The completion handler reads a command-record reference at entry offset `0x30`, clears its low bit, and reads a context reference at offset `0x38`. The exact transformation from submitted kick-entry fields to these completion fields is not specified here. Queue publication, hardware completion and stamp notification remain separate operations.

The normal compute path encodes a 32-byte completion record containing destination addresses, a value and a stamp index. The CPU current-stamp table and the firmware destination use different mappings of the same backing resource at byte offset `4 * stampIndex`. Firmware consumes the queued record, writes the value and then publishes the stamp-index notification. This describes the static completion path; fault, forced-completion and recovery paths require separate interpretation.

The producer and triggering conditions of grow-completion event 13 are not specified here. The presence of its host handler does not establish a firmware producer or use by the implicit growth engine. Firmware-ring event types, power-work callback indices and register-region identifiers are separate namespaces.
