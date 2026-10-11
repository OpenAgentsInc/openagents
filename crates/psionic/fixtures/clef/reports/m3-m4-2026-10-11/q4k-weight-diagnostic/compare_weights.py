#!/usr/bin/env python3
"""Bounded sampled Q4_K FFN decoder and BF16 checkpoint comparison."""
import ast
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import time

for name in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "VECLIB_MAXIMUM_THREADS"):
    os.environ[name] = "1"
import numpy as np

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
ROOT = Path("repo")
GGUF = BASE / "artifacts/Clef-Q4_K_M.gguf"
HF = BASE / "artifacts/clef-bf16"
LIMIT = 8*1024**2
read_bytes = 0


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_payload(path, offset, size):
    global read_bytes
    read_bytes += size
    assert read_bytes <= LIMIT
    with path.open("rb") as f:
        f.seek(offset)
        data = f.read(size)
    assert len(data) == size
    return data


def gguf_header():
    with GGUF.open("rb") as f:
        def number(fmt):
            return struct.unpack("<"+fmt, f.read(struct.calcsize(fmt)))[0]
        def string():
            size = number("Q")
            assert size <= 16*1024**2
            return f.read(size).decode("utf8")
        formats = {0:"B", 1:"b", 2:"H", 3:"h", 4:"I", 5:"i", 6:"f", 7:"?", 10:"Q", 11:"q", 12:"d"}
        def value(kind):
            if kind in formats:
                return number(formats[kind])
            if kind == 8:
                return string()
            assert kind == 9
            subtype, count = number("I"), number("Q")
            assert count <= 1000000
            if subtype in formats:
                f.seek(count*struct.calcsize(formats[subtype]), 1)
            else:
                for _ in range(count):
                    value(subtype)
            return None
        assert f.read(4) == b"GGUF" and number("I") == 3
        count, metadata = number("Q"), number("Q")
        assert count < 10000 and metadata < 1000
        alignment = 32
        for _ in range(metadata):
            key = string()
            item = value(number("I"))
            if key == "general.alignment":
                alignment = item
            assert f.tell() < 64*1024**2
        tensors = {}
        for _ in range(count):
            name, dimensions = string(), number("I")
            assert 1 <= dimensions <= 4
            shape = [number("Q") for _ in range(dimensions)]
            tensors[name] = {"shape": shape, "type": number("I"), "offset": number("Q")}
        first = (f.tell()+alignment-1)//alignment*alignment
    return tensors, first


def upstream_decoder():
    source = (HERE / "upstream_quants.py").read_bytes()
    identity = json.loads((HERE / "source.json").read_text())
    assert sha(source) == identity["sha256"]
    tree = ast.parse(source)
    node = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "Q4_K")
    # Preserve method bodies from the independent upstream implementation.
    # Remove only __Quant registration, which imports the rest of gguf-py.
    node.bases, node.keywords = [], []
    module = ast.fix_missing_locations(ast.Module(body=[node], type_ignores=[]))
    scope = {"np": np, "QK_K": 256}
    exec(compile(module, str(HERE / "upstream_quants.py"), "exec"), scope)
    return scope["Q4_K"], identity


def native_scalar_translation(data):
    # A separately written scalar translation of backend-cpu/lib.rs's current
    # decode_q4_k_block_into. This is not execution of the native Rust decoder.
    decoded = []
    for start in range(0, len(data), 144):
        block = data[start:start+144]
        d, dm = np.float32(struct.unpack("<e", block[:2])[0]), np.float32(struct.unpack("<e", block[2:4])[0])
        scales, packed = block[4:16], block[16:]
        for part in range(8):
            if part < 4:
                scale, minimum = scales[part]&63, scales[part+4]&63
            else:
                scale = (scales[part+4]&15) | ((scales[part-4]>>6)<<4)
                minimum = (scales[part+4]>>4) | ((scales[part]>>6)<<4)
            sd, md = np.float32(d*scale), np.float32(dm*minimum)
            for index in range(32):
                nibble = (packed[(part//2)*32+index] >> (4*(part%2)))&15
                decoded.append(np.float32(np.float32(sd*nibble)-md))
    return np.asarray(decoded, dtype=np.float32)


def stats(candidate, reference):
    a, b = candidate.astype(np.float64), reference.astype(np.float64)
    assert a.shape == b.shape and np.isfinite(a).all() and np.isfinite(b).all()
    diff = a-b
    norm_a, norm_b = float(np.linalg.norm(a)), float(np.linalg.norm(b))
    return {"values": a.size, "cosine": float(np.dot(a,b)/(norm_a*norm_b)),
            "relative_l2_error": float(np.linalg.norm(diff)/norm_b), "rmse": float(np.sqrt(np.mean(diff*diff))),
            "max_abs_error": float(np.max(np.abs(diff))), "bf16_l2": norm_b, "q4k_l2": norm_a}


def main():
    began = time.monotonic()
    assert not (HERE / "result.json").exists(), "preserve previous diagnostic"
    decoder, decoder_identity = upstream_decoder()
    tensors, first = gguf_header()
    index = json.loads((HF / "model.safetensors.index.json").read_text())["weight_map"]
    headers = {}
    packed_samples, expected_samples = bytearray(), bytearray()
    result = {"kind": "sampled_q4k_ffn_vs_bf16", "model_execution": False, "gpu_execution": False,
              "payload_read_limit_bytes": LIMIT, "gguf_sha256_from_verified_artifact": "9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822",
              "hf_revision": "0b331204bb13fbd2ca93a64df1956f5b55478ce5", "independent_decoder": decoder_identity,
              "script_sha256": sha(Path(__file__).read_bytes()), "numpy": np.__version__, "tensors": [],
              "native_execution": "Not run; scalar Python source translation and independent upstream decoder compared. Rust harness is prepared separately.",
              "native_cpu_source_sha256": sha((ROOT / "crates/psionic/crates/psionic-backend-cpu/src/lib.rs").read_bytes()),
              "native_metal_source_sha256": sha((ROOT / "crates/psionic/crates/psionic-backend-metal/src/kernels/clef_prefill.metal").read_bytes())}
    for operation in ("gate", "up", "down"):
        available = [layer for layer in range(64) if tensors[f"blk.{layer}.ffn_{operation}.weight"]["type"] == 12]
        assert len(available) >= 6
        # Spread all three operation samples over their actual Q4_K layer set.
        layers = sorted({available[i*(len(available)-1)//5] for i in range(6)})
        for layer in layers:
            name = f"blk.{layer}.ffn_{operation}.weight"
            info = tensors[name]
            columns, rows = info["shape"]
            assert info["type"] == 12 and columns % 256 == 0
            hf_name = f"model.language_model.layers.{layer}.mlp.{operation}_proj.weight"
            shard = index[hf_name]
            if shard not in headers:
                with (HF / shard).open("rb") as f:
                    size = int.from_bytes(f.read(8), "little")
                    assert size <= 16*1024**2
                    headers[shard] = (8+size, json.loads(f.read(size)))
            header_len, header = headers[shard]
            hf = header[hf_name]
            assert hf["dtype"] == "BF16" and hf["shape"] == [rows, columns]
            record = {"gguf_tensor": name, "gguf_shape_input_output": info["shape"], "hf_tensor": hf_name,
                      "hf_shape_output_input": hf["shape"], "hf_shard": shard, "row_samples": []}
            for row in sorted({0, 31, 32, 127, 255, rows//2, rows-2, rows-1}):
                raw = read_payload(GGUF, first+info["offset"]+row*(columns//256)*144, (columns//256)*144)
                bf16 = read_payload(HF / shard, header_len+hf["data_offsets"][0]+row*columns*2, columns*2)
                reference = (np.frombuffer(bf16, dtype="<u2").astype(np.uint32)<<16).view(np.float32)
                independent = decoder.dequantize_blocks(np.frombuffer(raw, dtype=np.uint8).reshape(-1,144)).reshape(-1)
                scalar = native_scalar_translation(raw)
                equal = np.array_equal(independent.view(np.uint32), scalar.view(np.uint32))
                assert equal, f"scalar native source translation disagrees with independent decoder: {name}:{row}"
                record["row_samples"].append({"row": row, "quantized_sha256": sha(raw), "bf16_sha256": sha(bf16),
                    "independent_f32_sha256": sha(independent.astype("<f4").tobytes()), "native_scalar_translation_bitwise_exact": equal,
                    **stats(independent, reference), "f16_placement_error": stats(independent.astype(np.float16).astype(np.float32), independent)})
                packed_samples.extend(raw)
                expected_samples.extend(independent.astype("<f4").tobytes())
            result["tensors"].append(record)
    assert len(packed_samples) <= 2*1024**2 and len(expected_samples) <= 8*1024**2
    (HERE / "sample-blocks.q4k").write_bytes(packed_samples)
    (HERE / "independent-decoded.f32").write_bytes(expected_samples)
    rows = [r for tensor in result["tensors"] for r in tensor["row_samples"]]
    result.update(payload_read_bytes=read_bytes, tensors_compared=len(result["tensors"]), rows_compared=len(rows),
                  values_compared=sum(r["values"] for r in rows), minimum_cosine=min(r["cosine"] for r in rows),
                  maximum_relative_l2_error=max(r["relative_l2_error"] for r in rows),
                  maximum_f16_placement_relative_l2=max(r["f16_placement_error"]["relative_l2_error"] for r in rows),
                  native_scalar_translation_bitwise_exact=True, seconds=time.monotonic()-began,
                  packed_samples_sha256=sha(packed_samples), independent_decoded_sha256=sha(expected_samples),
                  limitation="Samples test selected rows and layout consistency. They do not prove all tensors or explain activation/answer differences.")
    (HERE / "result.json").write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    print(json.dumps({key:result[key] for key in ("tensors_compared","rows_compared","values_compared","minimum_cosine","maximum_relative_l2_error","maximum_f16_placement_relative_l2","payload_read_bytes","seconds")}),flush=True)


if __name__ == "__main__":
    main()
