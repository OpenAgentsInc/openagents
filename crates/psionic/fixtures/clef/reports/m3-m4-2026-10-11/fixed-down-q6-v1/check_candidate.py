#!/usr/bin/env python3
"""Check deterministic generated Q6 rows without loading or executing a model."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import time

import numpy as np

from check_bridge import decoder_classes, image_of, require
from regenerate import Header, HF, plan, state

HERE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read_exact(fd, count, offset):
    raw = os.pread(fd, count, offset)
    require(len(raw) == count, "short deterministic row read")
    return raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--bridge-manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(os.environ.get("CLEF_Q6_REGEN_GUARDED") == "1", "requires owned-process guard")
    require(not args.output.exists(), "preserve existing check evidence")
    require(not any(k.startswith("DYLD_") or k in {"LD_PRELOAD", "LD_LIBRARY_PATH"} for k in os.environ), "library overrides remain")
    manifest = json.loads(args.bridge_manifest.read_text())
    require(manifest["status"] == "built", "bridge not built")
    sidecar_path = args.candidate.with_suffix(".json")
    sidecar_raw = sidecar_path.read_bytes()
    sidecar = json.loads(sidecar_raw)
    require(sidecar["status"] == "generated_not_accepted", "candidate generation incomplete")
    require(sidecar["bridge_manifest_sha256"] == sha(args.bridge_manifest.read_bytes()), "generation used a different bridge manifest")
    _, patched, tensors, bf16, identity, expected = plan()
    candidate = Header(args.candidate)
    require(candidate.raw == patched and candidate.state[0] == expected["output_bytes"], "candidate header or size differs")
    library = ctypes.CDLL(manifest["bridge"])
    quantize = library.clef_q6_quantize
    quantize.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64, ctypes.c_size_t]
    quantize.restype = ctypes.c_size_t
    direct = library.ggml_quantize_chunk
    c_decode = library.dequantize_row_q6_K
    c_decode.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64]
    c_decode.restype = None
    images = {name: image_of(function) for name, function in [("bridge", quantize), ("upstream_quantize", direct), ("upstream_decode", c_decode)]}
    require(images["bridge"] == {"path": str(Path(manifest["bridge"]).resolve()), "sha256": manifest["bridge_sha256"]}, "bridge image mismatch")
    libraries = {str(Path(path).resolve()): digest for path, digest in manifest["runtime_libraries"].items()}
    for name in ("upstream_quantize", "upstream_decode"):
        require(libraries.get(images[name]["path"]) == images[name]["sha256"], "upstream library image mismatch")
    decoders, decoder_identity = decoder_classes()
    decode = decoders["Q6_K"].dequantize_blocks
    rows = [0, 1706, 3413, 5119]
    width, row_bytes = 17408, 68 * 210
    started = time.monotonic()
    receipt = dict(status="running", kind="fixed_q6_candidate_payload_check", model_execution=False, gpu_execution=False,
                   candidate_sha256_from_generation=sidecar["output_sha256"], full_candidate_hash_rechecked=False,
                   generation_receipt_sha256=sha(sidecar_raw), candidate_state=candidate.state,
                   bridge_manifest_sha256=sha(args.bridge_manifest.read_bytes()), script_sha256=sha(Path(__file__).read_bytes()),
                   loaded_images=images, independent_decoder=decoder_identity, sampled_rows=rows, tensors=[],
                   scope="Four fixed full rows in each replaced matrix; exact re-quantization and independent decoded values. This is not model accuracy acceptance.",
                   tensor_payload_bytes_read=0)
    candidate_fd = os.open(args.candidate, os.O_RDONLY)
    try:
        for tensor in tensors:
            if not tensor["replace"]:
                continue
            source = bf16[tensor["name"]]
            require(state(HF / source["shard"]) == identity["shards"][source["shard"]], "original BF16 shard changed")
            fd = os.open(HF / source["shard"], os.O_RDONLY)
            result = dict(name=tensor["name"], rows=[])
            try:
                for row in rows:
                    raw = read_exact(fd, width * 2, source["offset"] + row * width * 2)
                    packed_raw = read_exact(candidate_fd, row_bytes, candidate.data_start + tensor["new_offset"] + row * row_bytes)
                    receipt["tensor_payload_bytes_read"] += len(raw) + len(packed_raw)
                    values = (np.frombuffer(raw, dtype="<u2").astype(np.uint32) << 16).view(np.float32)
                    require(np.isfinite(values).all(), "nonfinite original row")
                    input_before = values.tobytes()
                    guarded = np.full(row_bytes + 128, 0xa5, dtype=np.uint8)
                    regenerated = guarded[64:-64]
                    require(quantize(values.ctypes.data, regenerated.ctypes.data, 1, width, row_bytes) == row_bytes, "row quantization failed")
                    require(values.tobytes() == input_before and np.all(guarded[:64] == 0xa5) and np.all(guarded[-64:] == 0xa5), "quantizer changed input or guard")
                    require(regenerated.tobytes() == packed_raw, "generated payload differs from original-BF16 re-quantization")
                    packed = np.frombuffer(packed_raw, dtype=np.uint8)
                    independent = decode(packed.reshape(-1, 210)).reshape(-1).astype(np.float32)
                    native = np.empty(width, dtype=np.float32)
                    c_decode(packed.ctypes.data, native.ctypes.data, width)
                    require(np.isfinite(independent).all() and np.array_equal(native.view(np.uint32), independent.view(np.uint32)), "C and independent Q6 decoded values differ")
                    delta = independent.astype(np.float64) - values.astype(np.float64)
                    result["rows"].append(dict(row=row, values=width, bf16_sha256=sha(raw), packed_sha256=sha(packed_raw),
                                               decoded_sha256=sha(independent.tobytes()), packed_recomputed_exactly=True,
                                               independent_decode_bitwise_equal=True, max_abs_quantization_error=float(np.abs(delta).max())))
            finally:
                os.close(fd)
            receipt["tensors"].append(result)
        require(len(receipt["tensors"]) == 32, "selected tensor count differs")
        require(state(args.candidate) == candidate.state and sidecar_path.read_bytes() == sidecar_raw, "candidate or generation receipt changed")
        for shard, expected_state in identity["shards"].items():
            require(state(HF / shard) == expected_state, "original BF16 shard changed during checks")
        receipt.update(status="passed", compared_values=32 * len(rows) * width, checked_rows=32 * len(rows))
    except BaseException as error:
        receipt.update(status="failed", error_type=type(error).__name__, error=str(error))
        raise
    finally:
        os.close(candidate_fd)
        receipt["elapsed_seconds"] = time.monotonic() - started
        args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({k: receipt[k] for k in ("status", "checked_rows", "compared_values", "tensor_payload_bytes_read", "elapsed_seconds")}))


if __name__ == "__main__":
    main()
