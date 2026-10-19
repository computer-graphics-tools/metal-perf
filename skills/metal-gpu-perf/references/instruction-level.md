# Compiled-code attribution

On the inspected toolchain, the public object tools cannot print AGX3 instructions from a GPU executable. Use compiler statistics, debug attribution and AIR for the questions below. Native instruction forms and their build scope are documented in the instruction reference.

## 1. Compiler statistics

`metal-perf profile` saves `info compN --all` for the costliest shaders in
`compiler_statistics.json`: instruction count, ALU, branch, FP16/FP32/INT16/INT32, device loads and
stores, threadgroup loads and stores, temp and uniform registers, and spilled bytes. `metal-perf
shaders` prints registers and spills next to each kernel. Compare them between variants: a change
that adds 20 registers can cost more occupancy than its arithmetic saves.

## 2. DWARF in the compiled GPU binary

Metal caches every compiled pipeline under
`$(getconf DARWIN_USER_CACHE_DIR)com.apple.metal/<id>/functions*.data`. The files are concatenated
GPU Mach-O images (cputype `0x01000013`). When the source was compiled with
`-gline-tables-only`, each image carries `__debug_line` and the inline tree.

- Build the shaders with line tables, at the same optimization level as release, so the code is
  identical: `-O2 -gline-tables-only` (add `-frecord-sources` to embed the source).
- Run the workload. The cache is written when the process exits, and other apps share it.
- `metal-perf extract --name <kernel substring> --path <build-hash or worktree substring> --require-debug --out bins`.
  `--path` keeps binaries from other builds out.

Then:

- `metal-perf lines X.bin` lists code bytes per source line and per file. Line 0 is
  compiler-generated code: register moves, spills and reloads, and inlined intrinsics without a
  line.
- `metal-perf lines X.bin --mode layout` lists address-ordered runs of `file:line`. A loop header
  line appears at the start of the loop body and again near its back edge. Read the two addresses
  off, then `metal-perf lines X.bin --range 0x558:0x27b6` sums only the loop body.
- `metal-perf inlines X.bin` gives exclusive code bytes per inlined function, with lexical blocks
  counted toward their function. The totals add up to the kernel's size, so the percentages show
  where the code comes from.

Bytes are a proxy for instruction count, not for time: AGX instructions vary in length, and a byte
in the K loop costs far more than a byte in the prologue. Combine the layout with the loop
structure.

## 3. AIR: what the compiler front end emitted

`metal-perf air` disassembles a `.metallib`, a `.air` file or an AIR archive with
`xcrun metal-objdump -d` and parses the LLVM IR:

- Without `--function`, it prints a summary: the largest defined functions and the `air.*` call
  sites in the module.
- With `--function regex --depth N`, it prints the call tree of each match. Calls into defined
  functions are expanded N levels deep, and the totals are `air.*` call sites weighted by call
  multiplicity.
- For archives, `--member text` selects the starting members by name or contents. Callees defined
  in other members are loaded through the archive symbol table.

Use it to answer questions such as:

- Which matrix operation does the frontend request? Look for
  `air.simdgroup_matrix_16x16x16_widening_multiply_accumulate.*` (an integer widening matrix request) or
  `air.simdgroup_matrix_8x8_multiply_accumulate.*` (the older 8x8 floating-point path).
- How does Metal Performance Primitives implement a tensor op? The implementations live in
  `/System/Library/Frameworks/MetalPerformancePrimitives.framework/Versions/A/Resources/libTensorOps.rtlib`,
  an `ar` archive of AIR modules. Wrappers are named
  `__tensorops_impl_matmul2d_op_run_cooperative_<A>_<B>_<C>[_v2]`, for example
  `cooperative_dv_i8_dv_i4_i32_v2` (device int8 × device int4 → int32). They call
  `__loweringlib.internal.N`, whose numbering changes between OS builds.
- Which conversions, shuffles and barriers sit in a kernel?

AIR pitfalls:

- AIR is the input to the GPU compiler, not its output. The backend may still fold, unroll or
  rematerialize. Example: on M5 the backend folds `float(bfloat(x))` round trips that AIR keeps, so
  rounding must be forced through bit operations (`as_type`).
- `metal -O2 -emit-llvm` output is pre-unroll. Constant-trip loops stay rolled, and
  `#pragma clang loop unroll(full)` appears only as `llvm.loop.unroll.full` metadata.
- Simdgroup builtins (`simdgroups_per_threadgroup`, `simdgroup_index_in_threadgroup`,
  `thread_index_in_simdgroup`) must share one integer type, such as `ushort`, or the compiler
  rejects the kernel.

## Probing hardware layouts

When a question is about data layout ("which lane owns which element of an MXU fragment", "is this
access pattern coalesced"), write a tiny kernel that answers it directly:

- To see element ownership, store `thread_index_in_simdgroup` into the output.
- To compare read orders, time two orders over the same bytes, as `metal-perf access-pattern` does.

Compile it at runtime with `newLibraryWithSource` and read the result back. Such a probe settles in
minutes what reading headers cannot.
