# Faults and recovery

This chapter describes build 26A434 interrupt delivery, fault reporting, the firmware recovery handshake and the host progress watchdog. These are implementation behaviors. Applications must rely on command-buffer completion status and the public error domains.

## Interrupt delivery

Software interrupt indices and firmware-ring event types use separate namespaces. Startup creates filtered event sources for enabled provider-relative indices, attaches them to a work loop and enables them. Successful provider registration preserves the configured index; the deferred AGX action reads it from the event source.

| Event-source boundary | Behavior |
| --- | --- |
| Filter callback | A true result signals deferred work; a false result does not |
| Source signaling | Increments the 32-bit producer count and signals the work loop |
| Deferred delivery | Calls the action for a nonzero 32-bit producer-minus-consumer count, then advances the consumer count to the captured producer count |
| Auto-disable mode | On a true filter result, disables the provider interrupt before signaling; after delivery, re-enables it unless the source was explicitly disabled |

The deferred action's count argument is separate from the configured interrupt index. Provider-relative indices do not establish physical interrupt vectors. The selected build 26A434 host dispatcher uses these software indices:

| Interrupt index | Selected action |
| --- | --- |
| 0 | Configuration-dependent device hook and firmware-ring drain |
| 1–3 | Process power-controller status, subject to its power guard |
| 4 | Drain firmware rings |
| 6 | Acknowledge an ID-clear completion and reset the corresponding ID bookkeeping |
| 7 | Decode the address-protection status and terminate through a kernel panic |

Other indices enter the debugger. The selected GPU interrupt hook and the hook called before ring draining have empty bodies. An ID-clear acknowledgment does not establish shader completion or physical-page reclamation.

The power-controller callback reads logical register `0xe01014` only when its power guard permits access. A zero value ends handling; a nonzero value is written back to the same register, followed by a readback. This register operation is separate from firmware-event consumption and does not write completion stamps.

## Recovery protocol

The build 26A434 recovery path keeps fault reporting, firmware coordination and software completion separate:

| Boundary | Implementation behavior | Constraint |
| --- | --- | --- |
| Recovery event 4 | Checks the packet's recovery counter, snapshots firmware state and signals a scheduler hardware error | A stale packet is rejected; the event is not a successful command completion |
| Scheduler error request | Records a request-type bit per stamp index and schedules handling | Repeated requests with the same bit are detected instead of enqueued again |
| Firmware stop for inspection | Sends the current counter, waits for a firmware recovery count equal to that counter plus one, and checks stopped state | The wait is bounded; failure can panic |
| Recovery command records | Processes an active prefix of at most 256 eight-byte records, using stamp index and value to identify submissions | The first inactive record ends the scan |
| Pre-recovery stop | Firmware publishes shared state 1, increments its recovery count and waits for state 2 | Stopped-state publication and the counter handshake are separate fields |
| Pre-recovery resume | Host requires state 1, then publishes state 2 between memory barriers | Allows the firmware recovery phase to proceed |
| Post-recovery stop | Firmware publishes state 3 and waits for state 0; the host polls for state 3 | The host polling loop has no local timeout; a wider watchdog policy is not specified here |
| Post-recovery resume | Host requires state 3, then publishes state 0 between memory barriers | Resuming the protocol does not establish successful execution of failed work |
| Forced stamp completion | Software can advance the primary and mirror stamp arrays and wake waiters | Stamp progress alone cannot establish that the GPU executed the command |

Restart processing increments the counter used to reject stale recovery packets. Recovery also records command error status; applications must inspect completion errors before treating results as valid.

Request-queue cleanup is conditional. The selected recovery path snapshots target cursors for twelve queues arranged in four groups of three, with 24-byte records. It requests cache cleaning and invalidation over queued ranges and processes records matching each queue's expected type. Each read-cursor loop compares against its captured target and advances modulo 256.

The channel processor used by this path selects its command target from the current channel header. It can stop before all nested commands are handled and retain per-channel pending state. Progress of the outer request queue therefore does not establish completion of all nested commands.

While armed, the host progress watchdog combines a configured deadline, submitted completion stamps and a shared firmware progress value:

| Watchdog condition | Action |
| --- | --- |
| Deadline has not arrived | Reschedule the check at the existing deadline |
| Deadline expired; the sleep-state flag is set or every submitted stamp satisfies its completion comparison | Disarm the timer |
| Deadline expired; sleep-state flag is clear, work remains and the progress value changed | Update the snapshot and schedule a new deadline |
| Deadline expired; sleep-state flag is clear, work remains and the progress value is unchanged | Disarm the timer and request scheduler recovery |

The progress value can change after firmware register checks or state transitions. It does not identify a completed command, and its change alone does not establish successful execution.

Channel-error events carry separate reason and data-master fields, followed by a stamp index and value identifying the affected work. Selected GFX producer conditions in build 26A434 are:

| Internal reason | Reporting condition |
| --- | --- |
| 0, memory-failure report | A nonzero shared count and an enabled per-command reporting flag |
| 1, command-kill report | A per-command kill marker is set when completion is processed |
| 2, timeout report | A watched value is below its target and command or per-stamp state requests a failure report |

In the ordinary polling path, an unmet wait remains pending and is retained for retry. A timeout report completes that software wait and allows queue processing to advance. These reports combine command and shared-state checks; their fields are distinct from the MMU fault-register encoding.

Completion status travels separately from notification-transport success. The examined path carries the retained command's error status through a block fence and a 40-byte userspace notification, then selects an error domain during error-object construction. Selected build 26A434 classifications produced from the same internal status in the [command-buffer](https://developer.apple.com/documentation/metal/mtlcommandbuffererror-swift.struct/code) and [Metal 4 queue](https://developer.apple.com/documentation/metal/mtl4commandqueueerror-swift.struct/code) error domains are:

| Command-buffer error classification | Metal 4 queue error classification |
| --- | --- |
| Timeout | Timeout |
| Access revoked | Access revoked |
| Not permitted | Not permitted |
| Out of memory | Out of memory |
| Page fault | Not permitted |
| Invalid resource | Not permitted |
| Memoryless-resource error | Not permitted |

These selected classifications describe implementation behavior; they do not define a complete conversion between public enumerations. Always interpret the code together with its error domain.

The recovery phase also uses a separate interprocessor channel with a 16-bit selector in message bits 44–59:

| Selector | Receiver dispatch | Operation |
| --- | --- | --- |
| 11 | GFX1 work bit 34, callback argument 34 | Runs a recovery path containing zero performance-state requests, restoration inputs and register initialization |
| 7 | GFX1 work bit 15, callback argument `0x8000` | Requeues deferred work, clears deferred storage and clears the recovery flag |
| 29, payload 2 | GFX wait notification | Both handlers send this notification after their processing |

These notifications acknowledge handler progress. Configuration gates can bypass the zero-state request, and the calling recovery handler does not test its return value. The notification therefore does not establish that a complete hardware power cycle occurred.

The selected interprocessor receive path offers payloads for valid ports to their registered callbacks. A nonzero return consumes the payload; an absent callback or a zero return attempts to enqueue the eight-byte payload in a software queue. Each queue slot has a ready byte followed by the payload. On successful transfer, the producer copies the payload, executes `DMB ISH` and sets the ready byte; the consumer copies the payload, executes `DMB ISH` and clears that byte. This queue publication is separate from the firmware event ring and from GPU completion.
