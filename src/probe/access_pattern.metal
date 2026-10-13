#include <metal_stdlib>
using namespace metal;

constant uint ROWS_PER_SIMDGROUP = 32;

kernel void rowwise(
    const device uchar* base [[buffer(0)]],
    device uint* sink [[buffer(1)]],
    constant uint4& shape [[buffer(2)]],
    uint simdgroup [[threadgroup_position_in_grid]],
    ushort lane [[thread_index_in_simdgroup]]
) {
  const uint row_bytes = shape.x;
  const uint step_bytes = shape.y;
  const device uchar* row = base + size_t(simdgroup * ROWS_PER_SIMDGROUP + lane) * row_bytes;
  uint4 accumulator = 0;
  for (uint k = 0; k < row_bytes; k += step_bytes) {
    for (uint j = 0; j < step_bytes; j += 16) {
      accumulator ^= *(const device uint4*)(row + k + j);
    }
  }
  if (all(accumulator == uint4(0x12345678))) {
    sink[simdgroup] = accumulator.x;
  }
}

kernel void tiled(
    const device uchar* base [[buffer(0)]],
    device uint* sink [[buffer(1)]],
    constant uint4& shape [[buffer(2)]],
    uint simdgroup [[threadgroup_position_in_grid]],
    ushort lane [[thread_index_in_simdgroup]]
) {
  const uint row_bytes = shape.x;
  const uint step_bytes = shape.y;
  const device uchar* tile = base + size_t(simdgroup) * ROWS_PER_SIMDGROUP * row_bytes;
  const uint block_bytes = ROWS_PER_SIMDGROUP * step_bytes;
  uint4 accumulator = 0;
  for (uint k = 0; k < row_bytes; k += step_bytes) {
    const device uchar* block = tile + size_t(k) * ROWS_PER_SIMDGROUP;
    for (uint j = lane * 16; j < block_bytes; j += 32 * 16) {
      accumulator ^= *(const device uint4*)(block + j);
    }
  }
  if (all(accumulator == uint4(0x12345678))) {
    sink[simdgroup] = accumulator.x;
  }
}
