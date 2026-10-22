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
END_PASS_OFFSETS = (0x48, 0x50, 0x60)


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


def zero_input_end_pass(record):
    """Evaluate the G17X checkSpillParamsForCompute zero-argument branch.

    Both HAL300 endComputePass variants contain this call. These are candidate
    page counts before the function compares them with the object's history;
    they are not a physical pool-size prediction. The simple integer form is
    valid for nonnegative inputs without signed 32-bit overflow.
    """
    config = record["config"]
    n = word(config["device_count_0x1c44"])
    if n == 0:
        raise ValueError("device count must be nonzero")
    primary = int(config["primary_mask_0x1be0"], 0)
    alternate = int(config["alternate_mask_0x1bf0"], 0)
    if not 0 <= primary < 1 << 128 or not 0 <= alternate < 1 << 128:
        raise ValueError("USC masks must be 128-bit unsigned values")
    pop = (primary or alternate).bit_count()
    spill = {int(offset, 0): word(value) for offset, value in record["spill_words"].items()}
    missing = set(END_PASS_OFFSETS) - spill.keys()
    if missing:
        raise ValueError(f"missing end-pass spill words at {sorted(hex(offset) for offset in missing)}")
    multiplier = spill[0x48]
    base = spill[0x50]
    reserve = spill[0x60]
    if any(value >= 1 << 31 for value in (multiplier, base, reserve)):
        raise ValueError("zero-input page-count form requires nonnegative signed inputs")
    first = n * (base + 65 + 10 * pop + multiplier * max(170 - reserve, 0))
    second = n * (base + 65 + multiplier * max(490 - reserve, 0))
    if max(first, second) > (1 << 31) - 8:
        raise ValueError("zero-input candidate rounds outside signed 32-bit range")
    return {
        "zero_input_candidate_pages_0x14": (first + 7) & ~7,
        "zero_input_candidate_pages_0x18": (second + 7) & ~7,
        "zero_input_mask_popcount": pop,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="JSON file with config, resource_list_present, and spill_words")
    parser.add_argument("--zero-input-end-pass", action="store_true",
                        help="also evaluate the static endComputePass zero-argument candidate")
    args = parser.parse_args()
    record = json.loads(args.capture.read_text())
    result = evaluate(record)
    if args.zero_input_end_pass:
        result.update(zero_input_end_pass(record))
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
