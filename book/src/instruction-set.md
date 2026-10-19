# G17 instruction reference

Native instructions explain which work a compiled shader performs. They do not, by themselves, establish instruction latency, memory traffic or GPU utilization. Use this reference to interpret compiled code alongside the [compiler](compiler.md), [execution model](gpu-execution.md) and [profiling](profiling.md) chapters.

## Scope and terminology

The encodings and numerical forms in this chapter apply to the [reference configuration](evidence.md#reference-configuration), including compiler `32023.921.6` and `g17s`/`g17d` profiles. The default encoding mode is 1, named `g17g-encodings`. Numeric identifiers are implementation details, not stable assembly mnemonics or public APIs.

| Term | Meaning | Does not establish |
| --- | --- | --- |
| AIR intrinsic | A typed operation in Metal's intermediate representation | A single physical instruction |
| Machine opcode | An identifier used during compiler passes | The identifier a byte decoder will choose later |
| MC opcode | The compiler's identifier for an encoded instruction form | A stable hardware mnemonic |
| Encoding family | A group of native bit layouts | One operation, one latency or one execution unit |
| Register name | A view in the compiler's register model | Available physical capacity or shader residency |
| Exact byte round trip | Decoding and encoding reproduce the original bytes | Execution correctness or unique source operands |

```mermaid
flowchart LR
    S[Metal source] --> A[Typed AIR]
    A --> M[Machine instructions]
    M --> B[Native shader bytes]
    B --> D[Decoded families and operands]
```

The decoded opcode can be a canonical alternative to the compiler's earlier machine opcode. A tested special-register read remains machine opcode 14977 through the machine passes, while its final bytes decode as 14981. Their scope-predication metadata differs. Properties of 14981 therefore cannot explain which properties an earlier pass saw.

Packed IR bitcasts, shuffles and subregister moves do not establish numerical unpacking. Interpret conversions along the complete operand path rather than treating every frontend cast as arithmetic.

## Encoding and control flow

| Property | Observed rule | Limit |
| --- | --- | --- |
| Instruction lengths | Even byte lengths from 2 through 24 occur in the format tables | A descriptor's nominal size of 8 is not its native instruction length |
| Byte order | Encoded 16-bit words are emitted low byte first | Individual fields can span scattered bit positions |
| Padding | The NOP writer repeats `06 00`, a family-115 encoding | Other family-115 field combinations have no assigned meaning here |
| Profile | CPU subtypes 355 and 387 map to `g17s` and `g17d` | A container subtype alone does not identify a live dispatch |
| Operand reconstruction | Multiple compiler records can encode the same bytes | A matching record need not be the original record |
| Atomic flag preservation | A native MC conversion can omit a flag needed to preserve encoded bit 68 | The field's hardware meaning is unknown |

Three observed ten-byte relative branch forms use the current instruction's byte offset as their origin:

```text
displacement = sign_extend_48(reconstructed_bits_1_through_47 << 1)
target       = current_instruction_offset + displacement
```

Here the reconstructed value is the packed 47-bit field; the left shift supplies its omitted zero low bit. Do not add instruction length or apply a second halfword scale.

| Encoding family | Compiler fixup name | Fixup kind | Validated form |
| --- | --- | ---: | --- |
| 3 | `b` | 144 | Relative form with instruction bit 61 clear |
| 15 | `bl` | 153 | Same displacement rule |
| 16 | `bn` | 162 | Same displacement rule |

The set-bit-61 form, indirect return targets, execution-mask effects and reconvergence semantics are not specified here. A branch target landing on a decoded boundary is a structural check, not an execution trace.

## Register views and thread attributes

The compiler models overlapping whole and half registers. For `n` from 0 through 143, `Rn` and `RnL` use encoding value `2*n`; `RnH` uses `2*n+1`. Width comes from the operand form, so equal encoding values do not mean equal-width operands.

| Register view | Compiler ID | Encoding value | Relationship |
| --- | ---: | ---: | --- |
| `R0` | 106 | 0 | Whole-register view |
| `R0L` | 426 | 0 | Bits 0–15 of `R0` |
| `R0H` | 282 | 1 | Bits 16–31 of `R0` |
| `R1` | 107 | 2 | Does not overlap `R0` |
| `R1L` | 427 | 2 | Bits 0–15 of `R1` |
| `R1H` | 283 | 3 | Bits 16–31 of `R1` |

`GPR16` contains 288 half-register names and `GPR32` contains 144 whole-register names. These class sizes are not physical per-thread allocation limits. Use pipeline statistics and occupancy measurements to determine the cost of the registers a shader actually uses.

The following public attributes correlate with special-input encodings in isolated compiled programs:

| Public value | Compiler register | Encoding |
| --- | --- | --- |
| Global position X/Y/Z | `SR_TP_IN_GRID_X/Y/Z` | 23/24/25 |
| Local position X/Y/Z | `SR_LOCAL_X/Y/Z` | 9/10/11 |
| Threadgroup position X/Y/Z | `SR_TG_X/Y/Z` | 26/27/28 |
| Threads per threadgroup X/Y/Z | `SR_TG_X/Y/Z_SIZE` | 17/18/19 |
| Linear local index | `SR_LIN_ID` | 13 |
| Lane index | `SR_SIMD_ELEM` | 7 |
| SIMD-group index | `SR_SIMD_GRP` | 6 |

For the reference uniform dispatch mapping, global coordinates satisfy `global = group * group_size + local`. The local linear index is `x + size_x * (y + size_y * z)`; lane and SIMD-group indices correspond to its remainder and quotient by 32. Query pipeline execution width; this is not a portable lane-order guarantee.

Indexed source operands require separate interpretation:

| Representation | Observed behavior | Unresolved meaning |
| --- | --- | --- |
| Backend tag 4 | Whole-to-high-half rewrite increments its source index by one, supporting 16-bit index units | Physical storage bank, capacity and arbitrary index contents |
| Backend tag 3, printed `u` | Raw index 12 with width 4 prints `u6`; index 8 with width 8 prints `{ u4, u5 }` | The prefix alone does not establish a memory space |
| Full grid dimensions | Compiled loads have byte displacements 0, 4 and 8 from register-pair bases | Driver-side identities of the preceding pointer inputs |

## Scalar integer operations

These result-producing forms are tied to tested source expressions and stored outputs. Inputs include boundary values, wraparound cases, signed/unsigned comparisons and shift counts 0–31. The table is not a mnemonic assignment for every instruction in each family.

| Expression or operation | MC opcode | Encoding family |
| --- | --- | --- |
| `a + b` | 11082 | 78 |
| `a - b` | 12592 | 101 |
| `a * b` | 11756, immediate addend 0 | 82 |
| `a * b + constant` | 11756, example addend 5 | 82 |
| `a * b + c` | 11757, register addend | 82 |
| AND / OR / XOR | 443 / 14496 / 17880 | 5 / 11 / 14 |
| Bitwise complement | 12115 | 92 |
| Left / logical-right / arithmetic-right shift | 15312 / 17166 / 16959 | 178 / 185 / 183 |
| Population count | 485 | 18 |
| Bit reversal | 14968 | 144 |
| Equality / less-than | 12387 with different conditions, followed by high-half clearing | Form-dependent |
| Unsigned minimum / maximum | 12300, conditions 9 / 10 | Form-dependent |
| Leading-zero count | Sequence 10891 → 12589 → 12288 | Multiple instructions |

A source expression can require several instructions; conversely, multiplication and addition can share a native form. Count the compiled result path, including operand preparation and stores. Static instruction count is not an estimate of cycles.

## SIMD operations

The active lanes are part of a collective operation's input. A collective inside divergent flow control operates on the lanes active at that point according to its API contract. A compiler transformation that changes that set can change the result without changing the arithmetic expression.

| Full-group operation | Core family | MC opcode or sequence |
| --- | ---: | --- |
| Unsigned sum | 155 | 17026 |
| Exclusive prefix sum | 155 | 17025 |
| Inclusive prefix sum | 155 | 17025 followed by addition |
| Unsigned minimum / maximum | 159 / 158 | 17066 / 17058 |
| Signed minimum / maximum | 157 / 156 | 17050 / 17042 |
| AND / OR / XOR reduction | 146 / 147 / 148 | 16982 / 17034 / 17074 |
| Broadcast / general shuffle | 170 | 15077 or 15078 |
| XOR shuffle | 171 | 15089 |
| Shuffle down / up | 176 / 177 | 15203 / 15215 |
| Rotate or fill down / up | 174 / 175 | 15155 / 15179 |
| Ballot / active mask | 173 | 15131 or 15128 |
| Half sum / minimum / maximum | Form-dependent | 16990 / 17020 / 17012 |

A source-level shuffle can require multiple native instructions. Prefix products can expand into repeated shuffle/arithmetic sequences. Static code size does not specify latency.

### Active-lane count implementations

| Result | Reference implementation | Identifier |
| --- | --- | --- |
| Public prefix/sum of integer one | Integer scan/reduction | Family 155, MC 17025/17026 |
| Previous-valid count | Special-register read of `SR_PVSIMD` | Register encoding 34 |
| Total-valid count | Special-register read of `SR_TVSIMD` | Register encoding 36 |

The direct count-read form uses machine opcode 14977 and canonical native MC opcode 14981, family 145. In the reference masked forms, previous/total counts correspond to active-lane rank/count. Arbitrary read modifiers and reconvergence behavior are not specified. See [active lanes](gpu-execution.md#active-lane-rank-and-count).

Register encodings, machine opcodes and encoding-family identifiers are separate namespaces. The private intrinsic interface is not a portable kernel dependency.

### Optimized divergent-collective correctness limit

On compiler 32023.921.6, selected optimized conditional half/SIMD/quad operations incorrectly include inactive lanes or identify the wrong active subset. Exact matching compiled programs lose the branch in `gpu-flattencfg`. The [collective-correctness reference](gpu-execution.md#collective-correctness) defines the affected source shape and constraints; the [compiler reference](compiler.md#participation-sensitive-optimization) explains the transformation. Revalidate masked participation after changing source order, dtype or optimization settings.

## Memory operations and atomics

Compiler memory descriptors model dependencies; they do not measure physical transactions. In the general vector-memory path, element size is 1, 2 or 4 bytes and a four-bit component mask selects the number of elements. Modeled size is element size multiplied by the mask's population count; observed sizes include 3, 6 and 12 bytes.

| Compiler target-flag field | Modeled LLVM address space |
| --- | ---: |
| `0x400` | 1 |
| `0x800`, or override bit 26 set | 3 |
| `0xc00` | 0 |
| `0x1000` | 5 |
| Other field values | 4 |

Address-space numbers, load/store flags and raw alignment fields do not alone identify cache levels, cache-line traffic or physical alignment restrictions. Some opcodes carry load/store flags without entering the compiler's ordinary memory-descriptor path.

| Tested atomic form | Family | MC opcode |
| --- | ---: | --- |
| Device integer update returning old value | 1 | 10962 |
| Device compare-exchange | 1 | 10963 |
| Device addition, tested constant address | 1 | 10966 |
| Device update with discarded result | 1 | 10914 |
| Threadgroup addition, varying / constant address | 2 | 12668 / 12666 |
| Device float addition | 1 | 10962, MC operation 12; decoded operation 11 |
| Device float subtraction | 1 plus scalar operation | Sign-bit XOR followed by float atomic addition |
| Threadgroup float addition | 2 plus scalar operations | Float addition and a compare-exchange retry loop |
| Non-returning unsigned 64-bit minimum / maximum | 1 | 10899, operation 4 / 6 |

The integer operation selector is 0 add, 1 subtract, 2 exchange, 3 compare-exchange, 4/5 unsigned/signed minimum, 6/7 unsigned/signed maximum, and 8/9/10 AND/OR/XOR. These values apply to the tested forms.

Discarding an atomic result can select a form with no destination. It does not imply that every other policy bit clears: discarded exchange is a counterexample. Contended test results include lane-selection and result-distribution code, so source thread count is not an executed atomic-instruction count. Threadgroup float addition's retry loop can repeat under contention; its source API name does not imply one native float-atomic instruction.

### Language support and ordering

The installed compiler accepts device float atomic addition in Metal 4.0 and 4.1. Threadgroup float atomic addition and atomic calls combining non-relaxed order with memory flags are accepted in 4.1 and rejected in the tested 4.0 forms. Returning 64-bit fetch-min/fetch-add and float fetch-min are rejected in both tested language modes. An acquire load without memory flags is also rejected in these tests.

| Ordered device-memory form | Observed generated code |
| --- | --- |
| Acquire load | Fence after the load |
| Release store | Fence before the store |
| Acquire/release read-modify-write | Fences before and after |
| Explicit acquire fence, device scope | Family 4 / opcode 466, control `0xa6` |
| Explicit release fence, device scope | Family 4 / opcode 466, control `0xb8` |
| Explicit acq-rel or sequentially consistent fence, device scope | Family 4 / opcode 466, control `0xba` |

Device-scope explicit fences occur with device-memory flags in the tested programs. The tested thread, SIMD-group and threadgroup scopes, relaxed order, and `mem_none` emit no corresponding explicit device fence. This does not establish an absence of hardware ordering within those scopes.

Identical generated code for selected acquire/release and sequentially consistent forms does not make their API contracts interchangeable. Cross-group polling also requires a scheduling-progress guarantee.

## Floating-point behavior

The tested float32 add/subtract and atomic-add paths flush subnormal inputs and results to signed zero, round normal arithmetic to nearest with ties to even, and canonicalize arithmetic NaNs to `0x7fc00000`. Load/store/exchange preserve the tested bit patterns; compare-exchange compares those bit patterns, including zero signs and NaN payloads.

| Float32 bit-pattern example | Observed result |
| --- | --- |
| `00000001 + 00800000` | `00800000`, rather than gradual-underflow result `00800001` |
| `00800001 + 80800000` | `00000000` |
| `80000001 + 80000001` | `80000000` |
| `7fc12345 + 3f800000` | `7fc00000` |
| `7f7fffff + 7f7fffff` | `7f800000` |

The tested safe/strict/no-fast-math controls emit identical plain-add/subtract bytes and results. Safe math therefore does not imply gradual underflow on this target. These are bounded observations; signaling-NaN arithmetic is not a portable contract.

### FP8 conversion

The reference typed pack/unpack forms have the following conversion modes. They do not define general FP8 arithmetic or the lowering of every MSL expression.

| Immediate mode | Format | Reference pack overflow behavior |
| --- | --- | --- |
| 97 (`0x61`) | E4M3FN | Unsaturated |
| 98 (`0x62`) | E5M2 | Unsaturated |
| 113 (`0x71`) | E4M3FN | Saturates to signed maximum finite |
| 114 (`0x72`) | E5M2 | Saturates to signed maximum finite |

Pack places input component 0 in bits 0-7 and component 1 in bits 8-15. Unpack component 0 reads the low byte and component 1 reads the high byte.

Finite conversions match nearest-even rounding. Tested NaNs pack to `0x7f` for E4M3FN and `0x7e` for E5M2; unpack NaNs become FP32 `0x7fc00000`. Unsaturated E4M3FN maps infinity to signed NaN, while unsaturated E5M2 preserves signed infinity. Saturated maximum finite codes are `0x7e/0xfe` and `0x7b/0xfb`, respectively.

Rounding precedes overflow classification: E4M3FN 464 rounds to 448, while its next larger FP32 value becomes NaN in unsaturated mode. E5M2 61,440 rounds to infinity, while its next smaller FP32 value rounds to 57,344.

| Native conversion form | Observed encoding |
| --- | --- |
| Pack | Family 120, tested length 12 bytes; selected MC IDs 14539–14547 |
| Unpack | Family 194, tested length 10 bytes; selected MC IDs 17737–17740 and 17745–17748 |
| Pack mode bits 0 / 1 / 4 | Instruction bits 86 / 94 / 87 |
| Unpack mode bits 0 / 1 | Instruction bits 73 / 70 |

Saturated pack programs also contain additional instructions and field changes. The whole-program saturation observation cannot be assigned solely to bit 87. Safe-math conversion programs still declare denormal flushing; FP32 subnormal-to-FP8-zero results do not distinguish flushing from rounding to the same zero.

## Matrix operations

Public 8×8 SIMD-group operations and MPP can generate different native families for the same 16×16×16 numerical product. The source route, input representation, accumulator placement and output placement all matter.

| Tested source route | Family / MC opcode | Native format operands |
| --- | --- | --- |
| Public 8×8 SIMD-group matrix tiles | 56 / 2884 and 2886 | Form-dependent |
| MPP FP16 × FP16 → float32, zero matrix accumulator | 64 / 5698 | `0x2`, `0x2` |
| MPP BF16 × BF16 → float32, zero matrix accumulator | 64 / 5698 | `0x3`, `0x3` |
| MPP INT8 × INT8 → int32, zero matrix accumulator | 77 / 11189 | `0x4b`, `0x4b` |
| MPP UINT8 × UINT8 → int32, zero matrix accumulator | 77 / 11189 | `0xb`, `0xb` |
| MPP FP16 × INT8 → float32 | 64 / 5698 | Integer input converted to half; `0x2`, `0x2` |
| MPP floating product with loaded cooperative C | 64 / 5697 | Tested FP16/BF16 and converted mixed inputs |
| MPP integer product with loaded cooperative C | 77 / 11188 | Tested signed/unsigned INT8 inputs |
| MPP strict FP32 × FP32 → FP32, loaded cooperative C | 56 / 2864 | 8×8 float matrix form |
| MPP relaxed FP32 × FP32 → FP32, loaded cooperative C | 64 / 5689 | `0x1`, `0x1`; bounded precision behavior below |

For tested FP16 operands, transposition adds `0x20` to the corresponding format operand: left-only `0x22,0x2`, right-only `0x2,0x22`, both `0x22,0x22`. Signed and unsigned INT8 formats differ by `0x40`. These are validated combinations, not a legality table for arbitrary flags.

Source multiply-accumulate mode does not guarantee a nonzero native matrix accumulator. In the tested device-destination programs, the matrix instruction receives zero; separate scalar additions combine its result with the previous destination. Loaded cooperative C programs instead use the accumulating forms above. Their numerical results and exact matching compiler outputs are verified; separate constant/zero cooperative-C programs pass numerical checks but their alternate compiler outputs do not match exactly.

The FP16/BF16/integer tests use 16×16×16 products and small exactly representable values. They establish route and format distinctions, not arbitrary-value accuracy, latency, accelerator throughput or saturation. Measure preparation, matrix work and output conversion together when comparing routes. See [matmul](matmul.md) for kernel design.

### Strict and relaxed FP32

The reference 16×32×16 product with a loaded cooperative accumulator has the following lowerings:

| Property | Strict | Relaxed |
| --- | --- | --- |
| Matrix forms | Sixteen family-56 / MC 2864 forms | Two family-64 / MC 5689 forms |
| Length per form | 14 bytes | 10 bytes |

These are static code properties, not a throughput ratio. Selected finite-input relaxed forms exhibit behavior consistent with clearing the low 13 FP32 fraction bits of both multiplicands toward zero, while retaining the loaded FP32 accumulator. This is different from conversion to FP16, whose exponent range is narrower.

The implementation coverage does not define universal accumulator precision, association, exceptional-value behavior or rounding at every boundary. Relaxed matrix precision requires a numerical error contract; BF16 input storage does not by itself permit reduced precision for FP32 intermediates.

## Copy operations

The reference mode-0 two-dimensional FP32 implementation uses lane-participation loops, masked global vector loads and threadgroup vector stores. Compile-time contiguous views can select an included one-dimensional implementation. Non-unit strides can select scalar masked accesses.

Whole-text counts include separate program regions and padding. The event/wait source form does not identify a separate engine, copy/compute overlap or physical transaction count.

The [copy reference](async-copy.md) defines the typed interface, layout selection, [signed origins](async-copy.md#signed-origins), [participation constraints](async-copy.md#conditional-copy-participation), [remainder masks](async-copy.md#vector-remainders) and event representation.

## Limits relevant to kernel design

| Unknown | Evidence needed for a usable performance model |
| --- | --- |
| Instruction latency and issue rate | Dependency-chain and independent-chain timings with matching native forms |
| Physical register capacity and residency | Resource sweeps with supported occupancy data |
| Cache/bank mapping and transaction granularity | Controlled address/layout experiments with attributed traffic counters |
| Matrix execution scheduling and peak useful throughput | Route-specific positive controls and operation accounting |
| Copy overlap | Same numerical work with explicit synchronization and uninstrumented comparisons |
| All-schedules memory ordering and forward progress | Applicable API guarantees plus tests that can distinguish competing models |
| Unobserved encodings and physical storage meanings | Independent form-specific evidence; numeric labels alone are insufficient |

Use native code to test a concrete explanation: bounds-check overhead, duplicated loads, expanded scans, scalar accumulation, retry loops or unnecessary conversions. Confirm the performance effect with [calibrated comparisons](autotuning.md); fewer static instructions alone do not prove a faster kernel.
