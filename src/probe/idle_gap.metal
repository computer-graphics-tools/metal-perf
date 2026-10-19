#include <metal_stdlib>
using namespace metal;

// PRIVATE_FLOATS is replaced with the per-thread array length before compilation.
constant uint count = PRIVATE_FLOATS;

// A dynamically indexed thread-local array cannot live in registers, so the driver backs it with
// private memory whose size grows with the array length and the number of resident threads.
kernel void private_array(
    device float* sink [[buffer(0)]],
    constant uint& salt [[buffer(1)]],
    uint index [[thread_position_in_grid]]
) {
  float values[PRIVATE_FLOATS];
  for (uint i = 0; i < count; i++) {
    values[i] = float(i ^ index ^ salt);
  }
  float sum = 0.0f;
  for (uint i = 0; i < 64; i++) {
    uint k = (index * 7u + i * 13u + salt) % count;
    sum += values[k];
    values[(k * 3u + 1u) % count] = sum;
  }
  sink[index] = sum;
}
