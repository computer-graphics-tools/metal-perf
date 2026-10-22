#!/usr/bin/env python3
"""Measure the process private-memory pool immediately after a probe dispatch.

Each shape runs in a fresh process. JSONL output retains failures as well as successful
measurements, so an allocation failure or a noisy sample cannot disappear from a fit.
"""

import argparse
import ctypes
import datetime
import itertools
import json
import os
import plistlib
import re
import selectors
import subprocess
import sys
import time
from pathlib import Path


GRAPHICS_ROW = re.compile(
    r"^\s*(\d+) B\s+(\d+) B\s+(\d+) B\s+\d+\s+"
    r"(.+ \(graphics\))\s*$",
    re.MULTILINE,
)
DEFAULT_POOL_ROW = "Owned physical footprint (unmapped) (graphics)"
COMPONENTS = {"dirty": 0, "clean": 1, "reclaimable": 2}
READY_LINE = re.compile(r"^POOL_READY pid=(\d+)$")
DISPATCH_LINE = re.compile(r"^POOL_DISPATCH_SECONDS sample=(\d+) elapsed=([0-9.]+)$")
DISPATCH_CLOCK_LINE = re.compile(r"^POOL_DISPATCH_UNIX_NS sample=(\d+) started=(\d+) completed=(\d+)$")


def comma_ints(value):
    return [int(part) for part in value.split(",")]


def command_output(command):
    return subprocess.run(command, check=True, capture_output=True, text=True, timeout=10).stdout.strip()


def metal_compiler_version():
    result = subprocess.run(["/usr/bin/xcrun", "metal", "-v"], capture_output=True, text=True, timeout=10)
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise RuntimeError(f"Metal compiler probe failed (exit {result.returncode}): {detail}")
    version = (result.stderr or result.stdout).strip()
    if not version:
        raise RuntimeError("metal compiler returned no version")
    return version


def power_source():
    return command_output(["/usr/bin/pmset", "-g", "batt"]).splitlines()[0]


def thermal_state_raw():
    ctypes.CDLL("/System/Library/Frameworks/Foundation.framework/Foundation")
    objc = ctypes.CDLL("/usr/lib/libobjc.A.dylib")
    objc.objc_getClass.restype = ctypes.c_void_p
    objc.objc_getClass.argtypes = [ctypes.c_char_p]
    objc.sel_registerName.restype = ctypes.c_void_p
    objc.sel_registerName.argtypes = [ctypes.c_char_p]
    objc.objc_msgSend.restype = ctypes.c_void_p
    objc.objc_msgSend.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    process_info = objc.objc_msgSend(
        objc.objc_getClass(b"NSProcessInfo"), objc.sel_registerName(b"processInfo")
    )
    objc.objc_msgSend.restype = ctypes.c_long
    return int(objc.objc_msgSend(process_info, objc.sel_registerName(b"thermalState")))


def accelerator_bytes():
    raw = subprocess.run(
        ["/usr/sbin/ioreg", "-a", "-r", "-c", "AGXAccelerator"],
        check=True,
        capture_output=True,
        timeout=10,
    ).stdout
    entries = plistlib.loads(raw)
    return entries[0]["PerformanceStatistics"]["In use system memory"]


def footprint_bytes(pid):
    output = command_output(["/usr/bin/footprint", "-p", str(pid), "-f", "bytes"])
    rows = {match.group(4): tuple(map(int, match.groups()[:3])) for match in GRAPHICS_ROW.finditer(output)}
    if not rows:
        raise ValueError("footprint has no graphics rows")
    return rows


def selected_bytes(rows, pool_row, component):
    if pool_row not in rows:
        raise ValueError(f"footprint has no selected pool row {pool_row}; found {sorted(rows)}")
    values = rows[pool_row]
    return sum(values) if component == "total" else values[COMPONENTS[component]]


def ready_marker(process, timeout):
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    pending = b""
    lines = []
    deadline = time.monotonic() + timeout
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("probe did not reach POOL_READY")
            if not selector.select(remaining):
                raise TimeoutError("probe did not reach POOL_READY")
            chunk = os.read(process.stdout.fileno(), 4096)
            if not chunk:
                raise RuntimeError("probe exited before POOL_READY: " + " | ".join(lines))
            pending += chunk
            while b"\n" in pending:
                raw, pending = pending.split(b"\n", 1)
                line = raw.decode("utf-8", errors="replace").strip()
                lines.append(line)
                marker = READY_LINE.match(line)
                if marker:
                    return int(marker.group(1)), lines, time.monotonic(), time.time_ns()
    finally:
        selector.close()


def measure(binary, private_bytes, threads, threadgroup_size, hold_ms, timeout, pool_fast, pool_row, pool_component):
    command = [
        str(binary),
        "idle-gap",
        "--private-bytes", str(private_bytes),
        "--threads", str(threads),
        "--threadgroup-size", str(threadgroup_size),
        "--pool-probe",
        "--pool-hold-ms", str(hold_ms),
    ]
    if pool_fast:
        command.append("--pool-fast")
    row = {
        "private_bytes": private_bytes,
        "threads": threads,
        "threadgroup_size": threadgroup_size,
        "command": command,
        "pool_row": pool_row,
        "pool_component": pool_component,
    }
    process = None
    try:
        row["accelerator_before_bytes"] = accelerator_bytes()
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        pid, lines, ready_at, ready_unix_ns = ready_marker(process, timeout)
        if pid != process.pid:
            raise ValueError("ready marker PID differs from child PID")
        clocks = [match for line in lines if (match := DISPATCH_CLOCK_LINE.match(line))]
        if len(clocks) != 1 or int(clocks[0].group(1)) != 1:
            raise ValueError("probe did not report one dispatch clock pair")
        started_unix_ns, completed_unix_ns = int(clocks[0].group(2)), int(clocks[0].group(3))
        if not started_unix_ns <= completed_unix_ns <= ready_unix_ns:
            raise ValueError("dispatch and ready clock values are out of order")
        footprint_started = time.monotonic()
        graphics_rows = footprint_bytes(pid)
        row["graphics_rows"] = graphics_rows
        footprint_completed = time.monotonic()
        footprint_completed_unix_ns = time.time_ns()
        accelerator_started = time.monotonic()
        in_use = accelerator_bytes()
        accelerator_completed = time.monotonic()
        accelerator_completed_unix_ns = time.time_ns()
        if not ready_unix_ns <= footprint_completed_unix_ns <= accelerator_completed_unix_ns:
            raise ValueError("ready and snapshot clock values are out of order")
        row.update({
            "dispatch_started_unix_ns": started_unix_ns,
            "dispatch_completed_unix_ns": completed_unix_ns,
            "footprint_completed_after_dispatch_start_seconds": (footprint_completed_unix_ns - started_unix_ns) / 1e9,
            "accelerator_completed_after_dispatch_start_seconds": (accelerator_completed_unix_ns - started_unix_ns) / 1e9,
            "footprint_completed_after_dispatch_completion_seconds": (footprint_completed_unix_ns - completed_unix_ns) / 1e9,
            "accelerator_completed_after_dispatch_completion_seconds": (accelerator_completed_unix_ns - completed_unix_ns) / 1e9,
            "footprint_started_after_ready_seconds": footprint_started - ready_at,
            "footprint_completed_after_ready_seconds": footprint_completed - ready_at,
            "accelerator_started_after_ready_seconds": accelerator_started - ready_at,
            "accelerator_completed_after_ready_seconds": accelerator_completed - ready_at,
            "dispatch_seconds": {
                int(match.group(1)): float(match.group(2))
                for line in lines if (match := DISPATCH_LINE.match(line))
            },
            "pool_bytes": selected_bytes(graphics_rows, pool_row, pool_component),
            "unmapped_graphics_bytes": sum(graphics_rows.get("Owned physical footprint (unmapped) (graphics)", (0, 0, 0))),
            "graphics_dirty_bytes": sum(values[0] for values in graphics_rows.values()),
            "graphics_clean_bytes": sum(values[1] for values in graphics_rows.values()),
            "graphics_reclaimable_bytes": sum(values[2] for values in graphics_rows.values()),
            "accelerator_in_use_bytes": in_use,
            "accelerator_change_bytes": in_use - row["accelerator_before_bytes"],
            "probe_output": lines,
        })
        if row["accelerator_completed_after_dispatch_start_seconds"] >= 1:
            raise TimeoutError("snapshot did not complete within one second of dispatch start, the conservative completion-window bound")
        if process.wait(timeout=max(hold_ms / 1000 + 2, 5)) != 0:
            raise RuntimeError("probe failed after POOL_READY")
        row["status"] = "ok"
    except Exception as error:
        row["status"] = "invalid"
        row["error"] = str(error)
        if process is not None:
            if process.poll() is None:
                process.kill()
            process.wait()
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/metal-perf"))
    parser.add_argument("--output", type=Path, required=True, help="JSONL path outside the repository")
    parser.add_argument("--private-bytes", type=comma_ints, default=comma_ints("4,64,256,1024,4096,16384,65536"))
    parser.add_argument("--threads", type=comma_ints, default=comma_ints("65536,262144,1048576,4194304"))
    parser.add_argument("--threadgroup-sizes", type=comma_ints, default=comma_ints("64,256,1024"))
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--hold-ms", type=int, default=3000)
    parser.add_argument("--pool-fast", action="store_true", help="use the short volatile private-array kernel")
    parser.add_argument("--pool-row", default=DEFAULT_POOL_ROW, help="footprint graphics category attributed to the pool")
    parser.add_argument("--pool-component", choices=(*COMPONENTS, "total"), default="total", help="column of the selected footprint row")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--other-gpu-work", required=True, help="Observed other GPU work during this run")
    args = parser.parse_args()
    if args.repeats < 1 or args.hold_ms < 1000:
        parser.error("repeats must be positive and hold-ms must be at least 1000")
    if args.pool_fast and min(args.private_bytes) < 256:
        parser.error("pool-fast requires at least 256 private bytes")
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"binary does not exist: {binary}")
    if args.output.resolve().is_relative_to(Path.cwd().resolve()):
        parser.error("write raw JSONL outside the repository")
    gpu = json.loads(command_output(["/usr/sbin/system_profiler", "SPDisplaysDataType", "-json"]))["SPDisplaysDataType"][0]
    conditions = {
        "device": gpu["sppci_model"],
        "gpu_cores": gpu.get("sppci_cores"),
        "os": command_output(["/usr/bin/sw_vers"]),
        "xcode": command_output(["/usr/bin/xcodebuild", "-version"]),
        "metal_compiler": metal_compiler_version(),
        "power": power_source(),
        "thermal_state_raw": thermal_state_raw(),
        "thermal": command_output(["/usr/bin/pmset", "-g", "therm"]),
        "other_gpu_work": args.other_gpu_work,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("a", encoding="utf-8") as output:
        for private_bytes, threads, threadgroup_size in itertools.product(
            args.private_bytes, args.threads, args.threadgroup_sizes
        ):
            for repeat in range(args.repeats):
                row = measure(binary, private_bytes, threads, threadgroup_size, args.hold_ms, args.timeout, args.pool_fast, args.pool_row, args.pool_component)
                row["power_after_sample"] = power_source()
                row["thermal_after_sample"] = command_output(["/usr/bin/pmset", "-g", "therm"])
                row.update({"repeat": repeat, "conditions": conditions, "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
                output.write(json.dumps(row, sort_keys=True) + "\n")
                output.flush()
                size = row.get("pool_bytes", 0)
                print(private_bytes, threads, threadgroup_size, repeat, row["status"], f"pool row={size / 2**20:.1f} MiB", row.get("error", ""), flush=True)


if __name__ == "__main__":
    main()
