#!/usr/bin/env python3
"""Check per-shape private-pool predictions against fresh-process JSONL samples."""

import argparse
import collections
import json
import math
import statistics
from pathlib import Path


MIB = 1 << 20
KEYS = ("private_bytes", "threads", "threadgroup_size")
DEFAULT_POOL_ROW = "Owned physical footprint (unmapped) (graphics)"
COMPONENTS = {"dirty": 0, "clean": 1, "reclaimable": 2}
WINDOW_FIELDS = (
    "footprint_completed_after_dispatch_start_seconds",
    "accelerator_completed_after_dispatch_start_seconds",
)


def shape(row):
    return tuple(row[key] for key in KEYS)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", type=Path, action="append", required=True, help="JSONL file; repeat as needed")
    parser.add_argument("--predictions", type=Path, required=True, help="JSON list with shape fields and predicted_bytes or predicted_mib")
    parser.add_argument("--block-mib", type=float, default=8)
    parser.add_argument("--repeat-limit-mib", type=float, default=8)
    parser.add_argument("--min-repeats", type=int, default=3)
    parser.add_argument("--pool-row", default=DEFAULT_POOL_ROW, help="footprint graphics category used for pool predictions")
    parser.add_argument("--pool-component", choices=(*COMPONENTS, "total"), default="total")
    parser.add_argument("--require-completion-window", action="store_true", help="reject old samples without a dispatch-start upper bound")
    parser.add_argument("--require-run-conditions", action="store_true", help="require stable power, recorded thermal state, and device/toolchain identity")
    args = parser.parse_args()
    if args.block_mib <= 0 or args.repeat_limit_mib < 0 or args.min_repeats < 1:
        parser.error("block-mib must be positive, repeat-limit-mib nonnegative, and min-repeats positive")

    predictions = json.loads(args.predictions.read_text())
    if not predictions:
        parser.error("predictions must contain at least one shape")
    expected = {}
    for prediction in predictions:
        key = shape(prediction)
        if key in expected:
            parser.error(f"duplicate prediction for {key}")
        byte_prediction = "predicted_bytes" in prediction
        mib_prediction = "predicted_mib" in prediction
        if byte_prediction == mib_prediction:
            parser.error(f"prediction for {key} needs exactly one of predicted_bytes and predicted_mib")
        if byte_prediction:
            value = prediction["predicted_bytes"]
            if type(value) is not int or value < 0:
                parser.error(f"invalid predicted_bytes for {key}: {value}")
            expected[key] = value
        else:
            value = prediction["predicted_mib"]
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
                parser.error(f"invalid predicted_mib for {key}: {value}")
            expected[key] = value * MIB
    samples = collections.defaultdict(list)
    for path in args.data:
        for line_number, line in enumerate(path.read_text().splitlines(), 1):
            row = json.loads(line)
            row["_source"] = f"{path}:{line_number}"
            samples[shape(row)].append(row)

    failures = []
    legacy_total = 0
    condition_signatures = set()
    if args.require_run_conditions:
        for rows in samples.values():
            for row in rows:
                conditions = row.get("conditions") or {}
                required = ("device", "os", "xcode", "metal_compiler", "power", "thermal", "thermal_state_raw", "other_gpu_work")
                missing = [field for field in required if field not in conditions]
                if missing:
                    failures.append(f"{row['_source']}: missing run conditions {missing}")
                    continue
                if any(not isinstance(conditions[field], str) or not conditions[field].strip() for field in required if field != "thermal_state_raw"):
                    failures.append(f"{row['_source']}: empty or invalid run-condition text")
                if not (isinstance(conditions["xcode"], str) and conditions["xcode"].startswith("Xcode ")) or not (
                    isinstance(conditions["metal_compiler"], str) and conditions["metal_compiler"].startswith("Apple metal version ")
                ):
                    failures.append(f"{row['_source']}: invalid Xcode or Metal compiler version record")
                if row.get("power_after_sample") != conditions["power"]:
                    failures.append(f"{row['_source']}: power source changed or was not recorded after the sample")
                if type(conditions["thermal_state_raw"]) is not int or conditions["thermal_state_raw"] not in range(4):
                    failures.append(f"{row['_source']}: invalid ProcessInfo thermal state")
                if not row.get("thermal_after_sample"):
                    failures.append(f"{row['_source']}: missing post-sample thermal record")
                condition_signatures.add(tuple(conditions[field] for field in ("device", "os", "xcode", "metal_compiler", "power", "thermal_state_raw")))
        if len(condition_signatures) != 1:
            failures.append(f"run conditions contain {len(condition_signatures)} distinct device/OS/Xcode/compiler/power/thermal-state signatures")

    def pool_bytes(row):
        source = row["_source"]
        if row.get("pool_row", args.pool_row) != args.pool_row or row.get("pool_component", args.pool_component) != args.pool_component:
            failures.append(f"{source}: pool row/component differs from checker selection")
            return None
        graphics = row.get("graphics_rows", {})
        values = graphics.get(args.pool_row)
        if values is None or len(values) != 3:
            failures.append(f"{source}: selected graphics row missing or malformed")
            return None
        size = sum(values) if args.pool_component == "total" else values[COMPONENTS[args.pool_component]]
        if row.get("pool_bytes", size) != size:
            failures.append(f"{source}: selected pool bytes disagree with raw graphics row")
        return size

    for key in sorted(expected.keys() | samples.keys()):
        rows = samples.get(key, [])
        if key not in expected:
            failures.append(f"{key}: data has no prediction")
            continue
        invalid = [row for row in rows if row.get("status") != "ok"]
        values = [value for row in rows if row.get("status") == "ok" if (value := pool_bytes(row)) is not None]
        legacy = [
            row for row in rows if row.get("status") == "ok" and
            not all(field in row for field in WINDOW_FIELDS)
        ]
        legacy_total += len(legacy)
        lagged = [
            row for row in rows if row.get("status") == "ok" and
            (
                max(row[field] for field in WINDOW_FIELDS) if all(field in row for field in WINDOW_FIELDS)
                else row.get("accelerator_completed_after_ready_seconds", row.get("accelerator_started_after_ready_seconds", float("inf")))
            ) >= 1
        ]
        range_bytes = max(values) - min(values) if values else float("nan")
        max_error_bytes = max((abs(value - expected[key]) for value in values), default=float("nan"))
        median_bytes = statistics.median(values) if values else float("nan")
        print(
            f"{key} predicted={expected[key]} B median={median_bytes} B "
            f"max_error={max_error_bytes} B range={range_bytes} B "
            f"samples={len(rows)} invalid={len(invalid)} late={len(lagged)} legacy_window={len(legacy)}"
        )
        if len(rows) < args.min_repeats:
            failures.append(f"{key}: {len(rows)} samples, need {args.min_repeats}")
        if invalid:
            failures.extend(f"{row['_source']}: {row.get('error', 'invalid')}" for row in invalid)
        if lagged:
            failures.extend(f"{row['_source']}: sample window exceeds one second" for row in lagged)
        if args.require_completion_window and legacy:
            failures.extend(f"{row['_source']}: lacks a dispatch-start completion-window bound" for row in legacy)
        if values and max_error_bytes > args.block_mib * MIB:
            failures.append(f"{key}: maximum prediction error {max_error_bytes} B exceeds {args.block_mib} MiB")
        if values and range_bytes > args.repeat_limit_mib * MIB:
            failures.append(f"{key}: repeat range {range_bytes} B exceeds {args.repeat_limit_mib} MiB")

    print(f"checked {len(expected)} predicted shapes and {sum(map(len, samples.values()))} raw rows")
    for failure in failures:
        print("FAIL", failure)
    if failures:
        raise SystemExit(1)
    if args.require_run_conditions:
        print("PASS: one device/OS/Xcode/compiler/power/thermal-state signature; every sample has matching power and thermal records")
    if legacy_total:
        print(f"PASS: predictions and repeat ranges meet the limits; {legacy_total} legacy rows lack a completion-window bound")
    else:
        print("PASS: all predictions, repeat ranges, and completion windows meet the limits")


if __name__ == "__main__":
    main()
