#include <metal_stdlib>
using namespace metal;

kernel void read4(
    const device uint4* source [[buffer(0)]],
    device uint* sink [[buffer(1)]],
    constant uint& count [[buffer(2)]],
    uint index [[thread_position_in_grid]],
    uint grid [[threads_per_grid]]
) {
  uint4 accumulator = 0;
  for (uint i = index; i < count; i += grid) {
    accumulator ^= source[i];
  }
  if (all(accumulator == uint4(0x12345678))) {
    sink[index] = accumulator.x;
  }
}

kernel void copy4(
    const device uint4* source [[buffer(0)]],
    device uint4* destination [[buffer(1)]],
    constant uint& count [[buffer(2)]],
    uint index [[thread_position_in_grid]],
    uint grid [[threads_per_grid]]
) {
  for (uint i = index; i < count; i += grid) {
    destination[i] = source[i];
  }
}
