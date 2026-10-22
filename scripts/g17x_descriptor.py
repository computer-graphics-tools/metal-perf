#!/usr/bin/env python3
"""Evaluate the statically recovered G17X USC descriptor size fields.

Input values come from an LLDB capture of SpillInfoGen4::allocateUSCSpillBuffer.
This evaluates user-mode descriptor production, not the kernel pool-size methods.
"""

import argparse
import json
from pathlib import Path


MASK32 = (1 << 32) - 1
SPILL_OFFSETS = (0x00, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C,
                 0x24, 0x28, 0x2C, 0x30)


def word(value):
    if not isinstance(value, int) or not 0 <= value <= MASK32:
        raise ValueError(f"expected a 32-bit unsigned word, got {value!r}")
    return value


def evaluate(record):
    """Return D0, D8, and descriptor offset 64 for one captured call."""
    config = record["config"]
    n = word(config["device_count_0x1c44"])
    l = word(config["resource_limit_0x1c1c"])
    width = word(config["usc_width_0x1c5c"])
    if n == 0 or not 1 <= width <= 128:
        raise ValueError("device count must be nonzero and USC width must be 1..128")
    primary = int(config["primary_mask_0x1be0"], 0)
    alternate = int(config["alternate_mask_0x1bf0"], 0)
    if not 0 <= primary < 1 << 128 or not 0 <= alternate < 1 << 128:
        raise ValueError("USC masks must be 128-bit unsigned values")
    mask = primary or alternate
    pop = mask.bit_count()
    slice_mask = (1 << width) - 1
    a = max(((mask >> (i * width)) & slice_mask).bit_count() for i in range(n))
    resource_present = record["resource_list_present"]
    if type(resource_present) is not bool:
        raise ValueError("resource_list_present must be a boolean")
    q = ((2 * l) & MASK32) // n
    max_active = min(q, a) if resource_present else a
    selected_pop = min(pop, (q * n) & MASK32) if resource_present else pop

    spill = {int(offset, 0): word(value) for offset, value in record["spill_words"].items()}
    missing = set(SPILL_OFFSETS) - spill.keys()
    if missing:
        raise ValueError(f"missing spill words at {sorted(hex(offset) for offset in missing)}")
    extra = 0
    if spill[0x2C] >= 1:
        extra = (spill[0x2C] + spill[0x30] + max(spill[0x24], spill[0x28]) + 8) * selected_pop
        extra &= MASK32
    pages0 = (spill[0x14] + extra + 7) & MASK32 & ~7
    pages8_base = spill[0x18] + (spill[0x1C] if spill[0x2C] >= 1 else 0) + extra
    pages8 = (pages8_base + 7) & MASK32 & ~7
    reserve_pages = (n * (11 * max_active + sum(spill[offset] for offset in (0, 4, 8, 12, 16)) + 10)) & MASK32
    if pages0 >= 1 << 31 or pages8 >= 1 << 31:
        raise ValueError("descriptor page count overflows its signed 32-bit byte conversion")
    return {
        "selected_mask": f"0x{mask:032x}",
        "mask_popcount": pop,
        "max_active_uscs_in_mgpu": a,
        "selected_max_active": max_active,
        "selected_popcount": selected_pop,
        "descriptor_d0_bytes": pages0 << 12,
        "descriptor_d8_bytes": pages8 << 12,
        "descriptor_pages_0x40": reserve_pages,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="JSON file with config, resource_list_present, and spill_words")
    args = parser.parse_args()
    print(json.dumps(evaluate(json.loads(args.capture.read_text())), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
