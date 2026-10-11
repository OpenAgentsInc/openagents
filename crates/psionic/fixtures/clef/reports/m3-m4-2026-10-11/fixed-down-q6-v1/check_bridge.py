#!/usr/bin/env python3
"""Check synthetic Q6 rows against upstream quantization and an independent decoder."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys
import time

import numpy as np

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
sys.path.insert(0, str(BASE / "cpu27b-diagnostic/streamed-gguf"))
from gguf_import import decoder_classes


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def image_of(function):
    class DlInfo(ctypes.Structure):
        _fields_ = [("name", ctypes.c_char_p), ("base", ctypes.c_void_p),
                    ("symbol", ctypes.c_char_p), ("address", ctypes.c_void_p)]
    dladdr = ctypes.CDLL(None).dladdr
    dladdr.argtypes = [ctypes.c_void_p, ctypes.POINTER(DlInfo)]
    dladdr.restype = ctypes.c_int
    item = DlInfo()
    require(dladdr(ctypes.cast(function, ctypes.c_void_p), ctypes.byref(item)) != 0 and item.name,
            "cannot identify loaded function")
    path = Path(os.fsdecode(item.name)).resolve()
    return {"path": str(path), "sha256": sha(path.read_bytes())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bridge-manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get("CLEF_Q6_REGEN_GUARDED") == "1", "requires owned-process guard")
    require(not args.output.exists(), "preserve existing checks")
    require(not any(k.startswith("DYLD_") or k in {"LD_PRELOAD", "LD_LIBRARY_PATH"} for k in os.environ), "library overrides remain")
    manifest = json.loads(args.bridge_manifest.read_text())
    require(manifest["status"] == "built", "bridge not built")
    library = ctypes.CDLL(manifest["bridge"])
    quantize = library.clef_q6_quantize
    quantize.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64, ctypes.c_size_t]
    quantize.restype = ctypes.c_size_t
    direct = library.ggml_quantize_chunk
    direct.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64, ctypes.c_int64, ctypes.c_void_p]
    direct.restype = ctypes.c_size_t
    c_decode = library.dequantize_row_q6_K
    c_decode.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64]
    c_decode.restype = None
    images = {name: image_of(function) for name, function in [("bridge", quantize), ("upstream_quantize", direct), ("upstream_decode", c_decode)]}
    require(images["bridge"] == {"path": str(Path(manifest["bridge"]).resolve()), "sha256": manifest["bridge_sha256"]}, "bridge image mismatch")
    expected = {str(Path(path).resolve()): digest for path, digest in manifest["runtime_libraries"].items()}
    for name in ("upstream_quantize", "upstream_decode"):
        require(expected.get(images[name]["path"]) == images[name]["sha256"], "upstream image mismatch")
    classes, decoder_identity = decoder_classes()
    decode = classes["Q6_K"].dequantize_blocks
    width = 17408
    start = time.monotonic()
    result = dict(kind="bounded_synthetic_q6_bridge_checks", status="running", model_execution=False, gpu_execution=False,
                  model_payload_bytes_read=0, independent_decoder=decoder_identity, loaded_images=images,
                  bridge_manifest_sha256=sha(args.bridge_manifest.read_bytes()), script_sha256=sha(Path(__file__).read_bytes()),
                  cases=[], refusals=[], gate_scope="Helper packing, unchanged input/guards, and decoder correctness only; no BF16 model acceptance claim.")
    try:
        for rows, mode in [(1, "zero"), (1, "constant"), (7, "dyadic"), (63, "mixed"), (64, "mixed")]:
            indices = np.arange(rows * width, dtype=np.uint32)
            if mode == "zero":
                values = np.zeros(rows * width, dtype=np.float32)
            elif mode == "constant":
                values = np.full(rows * width, -0.125, dtype=np.float32)
            else:
                values = ((indices % 257).astype(np.float32) - 128) / 128
                if mode == "mixed":
                    values *= np.array([1/4096, 1/32, 1, 16], dtype=np.float32)[(indices // 256) % 4]
                # Every source value has an exact BF16 representation, as generation requires.
                values = (values.view(np.uint32) & np.uint32(0xffff0000)).view(np.float32)
            input_storage = np.full(len(values) + 32, 0xdeadbeef, dtype=np.uint32)
            input_storage[16:-16] = values.view(np.uint32)
            before = input_storage.copy()
            pointer = input_storage.ctypes.data + 16 * 4
            size = rows * 68 * 210
            storage = np.full(size + 128, 0xa5, dtype=np.uint8)
            output = storage[64:-64]
            require(quantize(pointer, output.ctypes.data, rows, width, size) == size, "bridge rejected valid synthetic rows")
            require(np.array_equal(before, input_storage), "input or input guard mutated")
            require(np.all(storage[:64] == 0xa5) and np.all(storage[-64:] == 0xa5), "output guard mutated")
            reference = np.empty(size, dtype=np.uint8)
            require(direct(14, pointer, reference.ctypes.data, 0, rows, width, None) == size, "upstream direct quantizer failed")
            require(np.array_equal(output, reference), "bridge packed bytes differ from upstream direct call")
            # Quantizing one row at a time must not change per-block values.
            separate = np.empty(size, dtype=np.uint8)
            row_bytes = 68 * 210
            for row in range(rows):
                require(quantize(pointer + row * width * 4, separate.ctypes.data + row * row_bytes,
                                 1, width, row_bytes) == row_bytes, "row call failed")
            require(np.array_equal(output, separate), "row grouping changes packed bytes")
            independent = decode(output.reshape(-1, 210)).reshape(-1).astype(np.float32)
            native = np.empty(rows * width, dtype=np.float32)
            c_decode(output.ctypes.data, native.ctypes.data, len(native))
            require(np.isfinite(independent).all(), "independent decode contains nonfinite values")
            require(np.array_equal(independent.view(np.uint32), native.view(np.uint32)), "independent and upstream C decoders differ")
            if mode == "zero":
                require(np.all(output == 0) and np.all(independent == 0), "zero fixture changed")
            delta = independent.astype(np.float64) - values.astype(np.float64)
            denominator = float(np.linalg.norm(values.astype(np.float64)))
            result["cases"].append(dict(rows=rows, mode=mode, values=rows*width, packed_bytes=size,
                                        packed_sha256=sha(output.tobytes()), decoded_sha256=sha(independent.tobytes()),
                                        max_abs_quantization_error=float(np.abs(delta).max()),
                                        relative_l2_quantization_error=float(np.linalg.norm(delta))/denominator if denominator else 0,
                                        packed_direct_and_rowwise_identical=True, independent_decode_bitwise_equal=True,
                                        input_and_output_guards_unchanged=True))
        values = np.ones(width, dtype=np.float32)
        expected_size = 68 * 210
        for name, rows, columns, capacity, nonfinite in [
            ("zero_rows", 0, width, expected_size, False), ("too_many_rows", 65, width, expected_size, False),
            ("wrong_width", 1, width-1, expected_size, False), ("short_output", 1, width, expected_size-1, False),
            ("nonfinite", 1, width, expected_size, True),
        ]:
            values[0] = np.nan if nonfinite else 1
            output = np.full(expected_size + 128, 0xa5, dtype=np.uint8)
            require(quantize(values.ctypes.data, output.ctypes.data + 64, rows, columns, capacity) == 0, "invalid request was not refused")
            require(np.all(output == 0xa5), "refused request touched output")
            result["refusals"].append(name)
        result.update(status="passed", compared_values=sum(c["values"] for c in result["cases"]))
    except BaseException as error:
        result.update(status="failed", error_type=type(error).__name__, error=str(error))
        raise
    finally:
        result["elapsed_seconds"] = time.monotonic() - start
        args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in ("status", "compared_values", "refusals", "elapsed_seconds")}))


if __name__ == "__main__":
    main()
