#!/usr/bin/env python3
# Firm reason: 2 (independent IEEE754 decoding through Python's struct and JSON codecs; computes numerical observations only, with no build/browser orchestration or driver verdict).
"""Decode recorded u32 Float32 lanes and exact reported binary64 metadata.

This oracle never compares drivers, chooses case counts or sets tolerances.
The Coax caller owns those decisions. The standard codecs provide a genuinely
independent boundary from Coax's deliberately approximate JSON scalar helper.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys


def word64(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError("nonfinite/non-numeric metadata")
    if value == 0:
        value = 0.0
    return f"{struct.unpack('<Q', struct.pack('<d', value))[0]:016x}"


def numeric_words(value):
    return [numeric_words(item) for item in value] if isinstance(value, list) else word64(value)


def observe(path):
    raw = path.read_bytes()
    document = json.loads(raw)
    rows = document["results"]
    if not isinstance(rows, list):
        raise ValueError("observations must be an array")
    channels = 0
    metadata = []
    for i, row in enumerate(rows):
        values = row["rgba"] if "rgba" in row else row["actual"]
        words = document["raw_words"][i] if "raw_words" in document else row.get("words", row.get("raw_words"))
        if not isinstance(values, list) or not isinstance(words, list) or len(values) != len(words):
            raise ValueError("lane/word shape mismatch")
        for value, word in zip(values, words):
            if type(word) is not int or not 0 <= word < 2**32:
                raise ValueError("not an unsigned32 word")
            decoded = struct.unpack("<f", struct.pack("<I", word))[0]
            if not math.isfinite(decoded) or not isinstance(value, (int, float)) or isinstance(value, bool) or value != decoded:
                raise ValueError("JSON lane differs from exact finite Float32 word")
            channels += 1
        # Preserve exact scalar metadata meaning while leaving field selection
        # and equality decisions to the Coax caller. Zero signs remain numeric
        # zeros here; observed GPU signs remain in the raw u32 lanes above.
        item = {}
        for key in ("x", "y", "lod", "pixel", "index", "wrong", "instance", "ids", "clips", "triangle", "vertex", "selector"):
            if key in row:
                item[key] = numeric_words(row[key])
        metadata.append(item)
    result = {"schema": "aethyr.independent.ieee754-observations.v1",
              "input_sha256": hashlib.sha256(raw).hexdigest(),
              "oracle_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "finite_f32_channels": channels, "numeric_metadata_binary64_words": metadata}
    if "max_error" in document:
        result["max_error_binary64_word"] = word64(document["max_error"])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observation", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = observe(args.observation)
        args.receipt.write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({"finite_f32_channels": result["finite_f32_channels"]}))
        return 0
    except (ValueError, KeyError, IndexError, OSError, TypeError) as error:
        print("independent IEEE754 decoder refused: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
