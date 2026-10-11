#!/usr/bin/env python3
"""Prepare or stream a fixed FFN-down Q6 candidate; no model execution."""
import argparse
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import sys
import time

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
GGUF = BASE / "artifacts/Clef-Q4_K_M.gguf"
GGUF_SHA = "9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822"
HF = BASE / "artifacts/clef-bf16"
REFERENCE = BASE / "reference27b/cpu-head-stage1/results/reference.json"
GGUF_REFERENCE = BASE / "cpu27b-diagnostic/streamed-gguf/full-1/results/receipt.json"
UPSTREAM_REV = "62a6f74bab748b1755f4cd8dfc0871caf865c6c8"
MIB = 1024 ** 2
BLOCK_ROWS = 64
TYPES = {0: (1, 4), 1: (1, 2), 8: (32, 34), 12: (256, 144), 14: (256, 210), 30: (1, 2)}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def state(path):
    st = Path(path).stat()
    return [st.st_size, st.st_ino, st.st_mtime_ns, st.st_ctime_ns]


def selected_layers():
    # Exact use_more_bits predicate at the pinned llama-quant.cpp, for 64 layers.
    return [i for i in range(64) if i < 64 // 8 or i >= 7 * 64 // 8 or (i - 64 // 8) % 3 == 2]


def tensor_size(kind, dims):
    require(kind in TYPES and dims and all(d > 0 for d in dims), "unsupported tensor layout")
    block, size = TYPES[kind]
    require(dims[0] % block == 0, "tensor rows do not contain whole quantization blocks")
    return math.prod(dims) // block * size


class Header:
    """Parse descriptors while preserving raw metadata and padding exactly."""
    def __init__(self, path):
        self.path = Path(path)
        self.state = state(path)
        with self.path.open("rb") as f:
            def read(n):
                require(0 <= n and f.tell() + n <= 32 * MIB, "GGUF header exceeds 32 MiB")
                data = f.read(n)
                require(len(data) == n, "truncated GGUF header")
                return data

            def number(code):
                return struct.unpack("<" + code, read(struct.calcsize(code)))[0]

            def string(keep=True):
                n = number("Q")
                require(n <= 16 * MIB, "GGUF string too large")
                raw = read(n)
                return raw.decode("utf-8") if keep else None

            def value(kind, keep=False, depth=0):
                require(depth < 4, "GGUF metadata nesting exceeds bound")
                if kind == 8:
                    return string(keep)
                if kind == 9:
                    sub, count = number("I"), number("Q")
                    require(count <= 2_000_000, "GGUF metadata array exceeds bound")
                    for _ in range(count):
                        value(sub, False, depth + 1)
                    return None
                codes = {0: "B", 1: "b", 2: "H", 3: "h", 4: "I", 5: "i", 6: "f", 7: "?", 10: "Q", 11: "q", 12: "d"}
                require(kind in codes, "unknown GGUF metadata type")
                return number(codes[kind])

            require(read(4) == b"GGUF" and number("I") == 3, "expected GGUF version 3")
            count, nmeta = number("Q"), number("Q")
            require(0 < count < 10000 and 0 < nmeta < 1000, "GGUF counts exceed bound")
            self.metadata = {}
            self.alignment = 32
            for _ in range(nmeta):
                name = string()
                require(name not in self.metadata, "duplicate GGUF metadata key")
                kind = number("I")
                keep = name in {"general.alignment", "general.architecture", "general.file_type", "clef.block_count"}
                self.metadata[name] = value(kind, keep)
            self.metadata_end = f.tell()
            self.alignment = self.metadata.get("general.alignment", 32)
            require(self.alignment == 32, "this recipe requires the original 32-byte alignment")
            self.tensors = []
            names = set()
            for _ in range(count):
                name = string()
                require(name not in names, "duplicate GGUF tensor name")
                names.add(name)
                nd = number("I")
                require(0 < nd <= 4, "invalid GGUF tensor rank")
                dims = [number("Q") for _ in range(nd)]
                kind_pos = f.tell()
                kind = number("I")
                offset_pos = f.tell()
                offset = number("Q")
                size = tensor_size(kind, dims)
                require(offset % self.alignment == 0, "unaligned GGUF tensor")
                self.tensors.append(dict(name=name, dims=dims, kind=kind, offset=offset,
                                         size=size, kind_pos=kind_pos, offset_pos=offset_pos))
            self.header_end = f.tell()
            self.data_start = (self.header_end + self.alignment - 1) // self.alignment * self.alignment
            f.seek(0)
            self.raw = read(self.data_start)
        physical = sorted(self.tensors, key=lambda t: t["offset"])
        require(physical[0]["offset"] == 0, "unexpected initial data gap")
        for i, tensor in enumerate(physical):
            end = physical[i + 1]["offset"] if i + 1 < len(physical) else self.state[0] - self.data_start
            tensor["span"] = end - tensor["offset"]
            require(0 <= tensor["span"] - tensor["size"] < self.alignment, "GGUF tensor overlaps or has an unexpected gap")
        self.physical = physical

    def replacement_plan(self, layers):
        wanted = {f"blk.{i}.ffn_down.weight" for i in layers}
        patched = bytearray(self.raw)
        delta = 0
        rows = []
        for t in self.physical:
            target = t["name"] in wanted
            new_size = t["size"]
            kind = t["kind"]
            if target:
                require(t["dims"] == [17408, 5120] and kind == 12, "replacement must be a real Q4_K 27B down matrix")
                new_size, kind = tensor_size(14, t["dims"]), 14
                require((new_size - t["size"]) % self.alignment == 0, "replacement changes tensor padding")
                wanted.remove(t["name"])
            new_offset = t["offset"] + delta
            struct.pack_into("<I", patched, t["kind_pos"], kind)
            struct.pack_into("<Q", patched, t["offset_pos"], new_offset)
            rows.append({**t, "replace": target, "new_kind": kind, "new_offset": new_offset, "new_size": new_size})
            delta += new_size - t["size"]
        require(not wanted, "selected down matrices are missing")
        require(patched[:self.metadata_end] == self.raw[:self.metadata_end], "metadata changed")
        return bytes(patched), rows, self.state[0] + delta


def bf16_inventory():
    """Read only index/header data and compare historical verified file identity."""
    reference_raw = REFERENCE.read_bytes()
    reference = json.loads(reference_raw)
    require(reference["revision"] == "0b331204bb13fbd2ca93a64df1956f5b55478ce5", "unexpected original checkpoint revision")
    index_path = HF / "model.safetensors.index.json"
    index_raw = index_path.read_bytes()
    require(sha(index_raw) == reference["index_sha256"], "checkpoint tensor index changed")
    index = json.loads(index_raw)["weight_map"]
    result, states, headers = {}, {}, {}
    for layer in selected_layers():
        name = f"model.language_model.layers.{layer}.mlp.down_proj.weight"
        shard = index[name]
        require(Path(shard).name == shard and shard in reference["shards"], "unexpected checkpoint shard")
        p = HF / shard
        if shard not in headers:
            expected = reference["artifacts"][shard]
            current = state(p)
            require(current == [expected[k] for k in ("bytes", "inode", "mtime_ns", "ctime_ns")], "checkpoint changed since verified reference")
            states[shard] = current
            with p.open("rb") as f:
                size = struct.unpack("<Q", f.read(8))[0]
                require(0 < size <= 16 * MIB, "safetensor header exceeds bound")
                raw = f.read(size)
                require(len(raw) == size, "truncated safetensor header")
            headers[shard] = (json.loads(raw), 8 + size, sha(raw))
        header, first, header_sha = headers[shard]
        item = header[name]
        require(item["dtype"] == "BF16" and item["shape"] == [5120, 17408], "BF16 replacement shape/dtype mismatch")
        start, end = item["data_offsets"]
        require(0 <= start < end <= states[shard][0] - first and end - start == 5120 * 17408 * 2, "BF16 extent invalid")
        result[f"blk.{layer}.ffn_down.weight"] = dict(name=name, shard=shard, offset=first + start,
                                                    size=end - start, header_sha256=header_sha)
    identity = dict(reference_sha256=sha(reference_raw), index_sha256=sha(index_raw), index_state=state(index_path),
                    shards=states, full_shard_hashes_rechecked=False,
                    previous_verified_shard_sha256={s: reference["artifacts"][s]["sha256"] for s in states})
    return result, identity


def check_states(header, identity):
    require(state(GGUF) == header.state, "source GGUF changed")
    require(state(HF / "model.safetensors.index.json") == identity["index_state"], "BF16 index changed")
    for shard, expected in identity["shards"].items():
        require(state(HF / shard) == expected, "source BF16 shard changed")


def plan():
    upstream = json.loads((HERE / "upstream.json").read_text())
    require(upstream["revision"] == UPSTREAM_REV, "upstream revision changed")
    for path, info in upstream["files"].items():
        require(sha((HERE / "upstream" / path).read_bytes()) == info["sha256"], "pinned upstream snapshot changed")
    source = (HERE / "upstream/src/llama-quant.cpp").read_text()
    require("return i_layer < n_layers/8 || i_layer >= 7*n_layers/8 || (i_layer - n_layers/8)%3 == 2;" in source,
            "pinned upstream selection predicate changed")
    require("if (use_more_bits(i_layer, n_layer)) new_type = GGML_TYPE_Q6_K;" in source,
            "pinned Q4_K_M down-matrix policy changed")
    layers = selected_layers()
    require(len(layers) == 32, "selection count differs")
    header = Header(GGUF)
    old = json.loads(GGUF_REFERENCE.read_text())
    require(header.state == old["gguf_file_state"] and header.state[0] == 19_232_219_200,
            "GGUF changed since verified independent reference")
    require(header.metadata["general.architecture"] == "clef" and header.metadata["general.file_type"] == 15,
            "unexpected GGUF architecture or file type")
    patched, tensors, output_bytes = header.replacement_plan(layers)
    require(output_bytes == 19_967_533_120, "candidate output size differs")
    bf16, identity = bf16_inventory()
    require(set(bf16) == {t["name"] for t in tensors if t["replace"]}, "BF16 replacement inventory differs")
    doc = dict(kind="clef_q4_fixed_q6_down_plan", status="prepared_not_generated", acceptance="not tested; original BF16 gate unchanged",
               recipe="Original Q4_K_M artifact plus the upstream fixed Q6_K down-matrix subset. All other payloads and metadata are byte-preserved.",
               upstream_revision=UPSTREAM_REV, selected_layers=layers, selected_count=len(layers),
               source_gguf_sha256=GGUF_SHA, source_gguf_bytes=header.state[0], source_state=header.state,
               source_header_sha256=sha(header.raw), patched_header_sha256=sha(patched),
               output_bytes=output_bytes, additional_bytes=output_bytes-header.state[0],
               bf16_payload_bytes=sum(t["size"] for t in bf16.values()), bf16_identity=identity,
               block_rows=BLOCK_ROWS, source_full_hash_during_generation=True, tensors=tensors,
               bounds=dict(memory_gib=2, start_available_gib=14, available_floor_gib=12, disk_gib=20,
                           free_disk_floor_gib=25, start_free_disk_gib=45, deadline_seconds=1800,
                           gpu=False, quiet_lease=False, helper_threads=1, maximum_f32_block_bytes=BLOCK_ROWS*17408*4),
               limit="The standard FFN-down selection is applied to the 64 backbone layers only; this is not a claim that all other tensor choices reproduce upstream's complete Q4_K_M preset.")
    return header, patched, tensors, bf16, identity, doc


def generate(output, bridge, bridge_manifest):
    require(os.environ.get("CLEF_Q6_REGEN_GUARDED") == "1", "generation requires the owned-process guard")
    resources = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    require({"memory", "disk"} <= resources and not ({"gpu", "quiet"} & resources), "requires memory and disk leases without GPU/quiet")
    require(output.name.endswith(".gguf") and not output.exists(), "output must be a new GGUF path")
    partial = output.with_suffix(".partial")
    receipt_path = output.with_suffix(".json")
    require(not partial.exists() and not receipt_path.exists(), "preserve existing output evidence")
    header, patched, tensors, bf16, identity, doc = plan()
    manifest = json.loads(bridge_manifest.read_text())
    require(manifest["status"] == "built" and manifest["upstream_revision"] == UPSTREAM_REV, "bridge build not verified")
    require(manifest["bridge_sha256"] == sha(bridge.read_bytes()), "bridge binary changed")
    require(manifest["bridge_source_sha256"] == sha((HERE / "q6_bridge.cpp").read_bytes()), "bridge source changed")
    for path, expected in manifest["runtime_libraries"].items():
        require(sha(Path(path).read_bytes()) == expected, "quantization runtime library changed")
    require(sys.byteorder == "little", "the pinned recipe requires a little-endian host")
    require(not any(k.startswith("DYLD_") or k in {"LD_PRELOAD", "LD_LIBRARY_PATH"} for k in os.environ),
            "dynamic-library overrides must be cleared before launching generation")
    import numpy as np
    library = ctypes.CDLL(str(bridge.resolve()))
    # Validate the images actually resolved by dyld, not only expected files.
    class DlInfo(ctypes.Structure):
        _fields_ = [("dli_fname", ctypes.c_char_p), ("dli_fbase", ctypes.c_void_p),
                    ("dli_sname", ctypes.c_char_p), ("dli_saddr", ctypes.c_void_p)]
    dladdr = ctypes.CDLL(None).dladdr
    dladdr.argtypes = [ctypes.c_void_p, ctypes.POINTER(DlInfo)]
    dladdr.restype = ctypes.c_int
    resolved = {}
    for symbol, expected in [("clef_q6_quantize", {str(bridge.resolve()): manifest["bridge_sha256"]}),
                             ("ggml_quantize_chunk", manifest["runtime_libraries"])]:
        info = DlInfo()
        pointer = ctypes.cast(getattr(library, symbol), ctypes.c_void_p)
        require(dladdr(pointer, ctypes.byref(info)) != 0 and info.dli_fname, "cannot identify loaded quantizer image")
        actual = Path(os.fsdecode(info.dli_fname)).resolve()
        expected = {str(Path(k).resolve()): v for k, v in expected.items()}
        require(str(actual) in expected and sha(actual.read_bytes()) == expected[str(actual)], "loaded quantizer image differs from pinned build")
        resolved[symbol] = {"path": str(actual), "sha256": expected[str(actual)]}
    quantize = library.clef_q6_quantize
    quantize.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64, ctypes.c_size_t]
    quantize.restype = ctypes.c_size_t
    source_hash, output_hash = hashlib.sha256(), hashlib.sha256()
    details = []
    started = time.monotonic()
    record = dict(doc, status="generating", output=str(output), bridge_manifest_sha256=sha(bridge_manifest.read_bytes()),
                  regenerate_sha256=sha(Path(__file__).read_bytes()), prior_reference_hash_rechecked_this_run=False,
                  resolved_quantizer_images=resolved)
    receipt_path.write_text(json.dumps(record, indent=2) + "\n")
    try:
        with GGUF.open("rb") as source, partial.open("xb") as destination:
            def write(raw):
                destination.write(raw)
                output_hash.update(raw)

            def consume(n, copy, digest=None):
                while n:
                    raw = source.read(min(n, 2 * MIB))
                    require(raw, "truncated original GGUF")
                    source_hash.update(raw)
                    if digest is not None:
                        digest.update(raw)
                    if copy:
                        write(raw)
                    n -= len(raw)

            consume(header.data_start, False)
            write(patched)
            for t in tensors:
                check_states(header, identity)
                require(source.tell() == header.data_start + t["offset"], "source tensor offset differs")
                require(destination.tell() == header.data_start + t["new_offset"], "output tensor offset differs")
                original_digest = hashlib.sha256()
                if t["replace"]:
                    consume(t["size"], False, original_digest)
                    replacement = bf16[t["name"]]
                    bf16_digest, q6_digest = hashlib.sha256(), hashlib.sha256()
                    fd = os.open(HF / replacement["shard"], os.O_RDONLY)
                    try:
                        for row in range(0, 5120, BLOCK_ROWS):
                            count = min(BLOCK_ROWS, 5120 - row)
                            raw = os.pread(fd, count*17408*2, replacement["offset"] + row*17408*2)
                            require(len(raw) == count*17408*2, "short BF16 tensor read")
                            bf16_digest.update(raw)
                            values = (np.frombuffer(raw, dtype="<u2").astype(np.uint32) << 16).view(np.float32)
                            require(np.isfinite(values).all(), "nonfinite BF16 down weight")
                            packed = np.empty(count*68*210, dtype=np.uint8)
                            written = quantize(values.ctypes.data, packed.ctypes.data, count, 17408, packed.nbytes)
                            require(written == packed.nbytes, "upstream Q6 quantization failed")
                            data = packed.tobytes()
                            q6_digest.update(data)
                            write(data)
                    finally:
                        os.close(fd)
                    details.append(dict(name=t["name"], action="BF16_to_Q6_K", original_sha256=original_digest.hexdigest(),
                                        output_sha256=q6_digest.hexdigest(), bf16_sha256=bf16_digest.hexdigest(), source=replacement))
                else:
                    consume(t["size"], True, original_digest)
                    details.append(dict(name=t["name"], action="byte_copy", original_sha256=original_digest.hexdigest(),
                                        output_sha256=original_digest.hexdigest()))
                consume(t["span"] - t["size"], True)
            require(source.tell() == header.state[0] and not source.read(1), "original GGUF length changed")
            require(destination.tell() == doc["output_bytes"], "candidate length differs")
            require(source_hash.hexdigest() == GGUF_SHA, "original GGUF SHA256 differs; candidate rejected")
            destination.flush()
            os.fsync(destination.fileno())
        check_states(header, identity)
        verified = Header(partial)
        require(verified.raw == patched and len(verified.tensors) == len(tensors), "candidate header verification failed")
        require(not output.exists(), "output appeared during generation")
        os.link(partial, output)
        partial.unlink()
        record.update(status="generated_not_accepted", output_sha256=output_hash.hexdigest(), original_sha256=source_hash.hexdigest(),
                      full_source_gguf_hash_verified=True, tensor_results=details, elapsed_seconds=time.monotonic()-started)
    except BaseException as error:
        record.update(status="failed", error_type=type(error).__name__, error=str(error), completed_tensors=details,
                      elapsed_seconds=time.monotonic()-started)
        raise
    finally:
        receipt_path.write_text(json.dumps(record, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    planning = sub.add_parser("plan")
    planning.add_argument("--output", type=Path, required=True)
    generating = sub.add_parser("generate")
    generating.add_argument("--output", type=Path, required=True)
    generating.add_argument("--bridge", type=Path, required=True)
    generating.add_argument("--bridge-manifest", type=Path, required=True)
    args = parser.parse_args()
    if args.mode == "plan":
        require(not args.output.exists(), "preserve previous plan")
        doc = plan()[-1]
        with args.output.open("x") as f:
            json.dump(doc, f, indent=2)
            f.write("\n")
        print(json.dumps({k: doc[k] for k in ("status", "selected_layers", "output_bytes", "additional_bytes", "bf16_payload_bytes")}))
    else:
        generate(args.output, args.bridge, args.bridge_manifest)


if __name__ == "__main__":
    main()
