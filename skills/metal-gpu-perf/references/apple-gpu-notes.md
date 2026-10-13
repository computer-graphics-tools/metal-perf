# Apple GPU notes

Facts measured or read from shipped binaries, each with its date and machine. Re-verify on other
chips and OS versions before relying on a number: the commands are given.

## M5 Max (macOS 27, Xcode 27)

**Bandwidth** (`metal-perf bandwidth`, 2026-10-13):

- Peak read is 587 GB/s, with a 2 GiB buffer and a grid of 65k–1M threads.
- Copy is 542 GB/s.
- The same probe in Swift measured 594 GB/s on 2026-10-12, so expect a few percent of
  run-to-run spread.

**Weight read order** (`metal-perf access-pattern` with defaults: an 85 MiB matrix,
34816 × 2560 B, rotated over 12 copies, 2026-10-13):

- Row per lane reaches 437–443 GB/s.
- Contiguous 32-row tiles reach 533–545 GB/s.
- Tiling the weights is worth about 23% of bandwidth for kernels that stream weights the
  fragment-cursor way.

**MXU intrinsic:** int8 × int8 → int32 matmuls lower to
`air.simdgroup_matrix_16x16x16_widening_multiply_accumulate.s.s.v8i32.v8i8.v8i8.v8i32`. Each
lane holds 8 of the 256 fragment elements. MPP guards it with `air.supports_family` and keeps an
`air.simdgroup_matrix_8x8_multiply_accumulate` (fp16) fallback.

**MPP int8 × int4 is not a native MXU format** (2026-10-13). Check it with:

```sh
metal-perf air .../libTensorOps.rtlib --member cooperative_dv_i8_dv_i4_i32_v2 --function 'cooperative_dv_i8_dv_i4_i32_v2$' --depth 3
```

- The int4 lowering has 4989 IR instructions, against 4103 for int8 × int8.
- Both issue the same 16 `…widening_multiply_accumulate…v8i8.v8i8.v8i32` calls.
- The int4 path loads packed nibbles with narrow loads, widens them to int8 in ALU code, then uses
  the int8 MXU operation.
- Element k is in byte k/2, low nibble first.

**MPP matmul API surface** (`MPPTensorOpsMatMul2d.h`, 2026-10-12):

- Supported: `int8 × int4b → int32`, `uint8 × uint4b → int32`, and bf16/half × int4b, int2b, fp4
  and fp8.
- There is no `int8 × uint4b`.
- The `_v2` entry points take block scales only as `fp8_ue8m0` per 32 elements along K. Arbitrary
  bf16 group scales or zero points cannot be expressed.
- Sub-byte K must be a multiple of 32.

**Compiler behavior:**

- The M5 GPU backend folds `float(bfloat(x))` round trips that AIR still contains. Force rounding
  through `as_type` bit operations.
- The architecture reported by `metal-arch` is `applegpu_g17s`.
