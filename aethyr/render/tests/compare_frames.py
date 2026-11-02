#!/usr/bin/env python3
"""Compare complete raw GPU readbacks with the Coax CPU frame's RGBA f32.

The fixed finite-color contract is abs(gpu-cpu) <= .002*max(1,abs(cpu)).
Every channel is checked; there is no edge/depth exclusion mask in this gate.
Matching frame/resource provenance is established by the enclosing host gate.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import time

FORMATS = {"rgba32f": (16, struct.Struct("<4f")), "rgba16f": (8, struct.Struct("<4e")),
           "rgba8": (4, struct.Struct("4B")), "bgra8": (4, struct.Struct("4B"))}


def printable(value: float):
    if math.isnan(value):
        return "nan"
    if math.isinf(value):
        return "+inf" if value > 0 else "-inf"
    return value


def compare_buffers(cpu: bytes, gpu: bytes, gpu_format: str, width: int, height: int) -> dict:
    if not (1 <= width <= 16384 and 1 <= height <= 16384):
        raise ValueError("frame extent must be within 1..16384 on each axis")
    if gpu_format not in FORMATS:
        raise ValueError("unknown raw GPU readback format")
    pixels = width * height
    stride, decoder = FORMATS[gpu_format]
    if len(cpu) != pixels * 16 or len(gpu) != pixels * stride:
        raise ValueError("raw image length differs from its complete extent and format")
    failures = nonfinite = 0
    absolute = normalized = 0.0
    samples = []
    deadline = time.monotonic() + 830
    for index, reference in enumerate(struct.iter_unpack("<4f", cpu)):
        if index % 4096 == 0 and time.monotonic() > deadline:
            raise TimeoutError("image comparison exceeded its bounded command deadline")
        observed = decoder.unpack_from(gpu, index * stride)
        if gpu_format == "bgra8":
            observed = (observed[2], observed[1], observed[0], observed[3])
        if gpu_format in ("rgba8", "bgra8"):
            observed = tuple(channel / 255 for channel in observed)
        for channel, (expected, actual) in enumerate(zip(reference, observed)):
            if not (math.isfinite(expected) and math.isfinite(actual)):
                nonfinite += 1
                if len(samples) < 16:
                    samples.append(dict(x=index % width, y=index // width, channel="rgba"[channel],
                                        cpu=printable(expected), gpu=printable(actual), reason="nonfinite"))
                continue
            difference = abs(actual - expected)
            scale = max(1, abs(expected))
            absolute = max(absolute, difference)
            normalized = max(normalized, difference / scale)
            bound = .002 * scale
            if difference > bound:
                failures += 1
                if len(samples) < 16:
                    samples.append(dict(x=index % width, y=index // width, channel="rgba"[channel],
                                        cpu=expected, gpu=actual, absolute_error=difference,
                                        bound=bound, reason="colour"))
    return dict(status="pass" if failures == 0 and nonfinite == 0 else "fail", pixels=pixels,
                channels=pixels * 4, colour_failures=failures, nonfinite_failures=nonfinite,
                max_absolute_error=absolute, max_normalized_error=normalized, samples=samples,
                masked_pixels=0, rule="abs(gpu-cpu) <= 0.002 * max(1, abs(cpu)); both values finite")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpu", required=True, type=Path)
    parser.add_argument("--gpu", required=True, type=Path)
    parser.add_argument("--gpu-format", required=True, choices=FORMATS)
    parser.add_argument("--width", required=True, type=int)
    parser.add_argument("--height", required=True, type=int)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    source = sha(Path(__file__).read_bytes())
    cpu, gpu = args.cpu.read_bytes(), args.gpu.read_bytes()
    comparison = compare_buffers(cpu, gpu, args.gpu_format, args.width, args.height)
    if source != sha(Path(__file__).read_bytes()):
        raise RuntimeError("comparison tool changed during execution")
    receipt = dict(schema="aethyr.render.image-comparison.v1", status=comparison["status"],
                   width=args.width, height=args.height, source_sha256=source, comparison=comparison,
                   inputs={"cpu": dict(path=str(args.cpu.resolve()), format="rgba32f", bytes=len(cpu), sha256=sha(cpu)),
                           "gpu": dict(path=str(args.gpu.resolve()), format=args.gpu_format, bytes=len(gpu), sha256=sha(gpu))},
                   scope="Complete supplied readback/CPU image comparison, including alpha and all nonfinite values, without a mask. The enclosing host gate must establish matching frame and resource provenance.")
    args.receipt.write_text(json.dumps(receipt, indent=2, allow_nan=False) + "\n")
    print(json.dumps(comparison, allow_nan=False))
    return 0 if comparison["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
