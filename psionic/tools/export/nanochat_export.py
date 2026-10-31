#!/usr/bin/env python3
"""Inert export of a nanochat model checkpoint (.pt) to safetensors plus an identity record.

Tooling only; never shipped, never a production loader. It reads the
torch.save zip archive without torch and without executing the pickle: the
unpickler resolves exactly three globals to local inert stand-ins
(collections.OrderedDict, torch._utils._rebuild_tensor_v2 and the
FloatStorage/BFloat16Storage markers) and refuses every other global, every
non-tuple persistent id, any storage that is not little-endian, contiguous
and in bounds, and any dtype other than float32 or bfloat16. Bytes are copied
unchanged. The identity record binds the pinned source revision, the
checkpoint, meta and tokenizer digests and the meta's model_config exactly as
written: missing keys are not patched here (checkpoint_manager's historical
defaults), psionic/lib/nanochat/checkpoint.coax decides and refuses them.

  python3 -I psionic/tools/export/nanochat_export.py MODEL.pt META.json TOKENIZER OUT_DIR --tokenizer-format F --tokenizer-vocab N
"""
import argparse
import hashlib
import io
import json
import os
import pickle
import struct
import zipfile

SCHEMA = "psionic.nanochat-export.v1"
REPO = "karpathy/nanochat"
REVISION = "92d63d4e8bb4df75c3b71618f31ddde2378b2bcd"
GPT_PY = "8cdedc2e418b9d4dc45884e07f59904fcdec066bb3d76664b2adf2d415f704e3"
CHECKPOINT_MANAGER_PY = "aa63399ddaf2a0412dc099287bf0dc879f9cb0cce00f861db50eeec58746f3ba"
PREFIX = "_orig_mod."
STORAGES = {"FloatStorage": ("F32", 4), "BFloat16Storage": ("BF16", 2)}
PICKLE_MAX = 1 << 20
FILE_MAX = 1 << 28


class Refused(Exception):
    pass


class Storage:
    def __init__(self, dtype, width, key, numel):
        self.dtype, self.width, self.key, self.numel = dtype, width, key, numel


class Kind:
    def __init__(self, name):
        self.name = name


class Tensor:
    def __init__(self, storage, offset, size, stride):
        self.storage, self.offset, self.size, self.stride = storage, offset, size, stride


def ordered_dict(*args):
    if args:
        raise Refused("OrderedDict with arguments")
    return {}


def rebuild_tensor_v2(storage, offset, size, stride, requires_grad, hooks, *rest):
    if not isinstance(storage, Storage) or not isinstance(offset, int) or not isinstance(size, tuple) or not isinstance(stride, tuple):
        raise Refused("malformed tensor")
    if requires_grad not in (False, True) or hooks != {} or len(rest) > 1 or (rest and rest[0] not in ({}, None)):
        raise Refused("tensor hooks or metadata")
    if len(size) != len(stride) or not all(isinstance(v, int) and v >= 0 for v in size + stride):
        raise Refused("malformed shape")
    expect = 1
    for n, s in zip(reversed(size), reversed(stride)):
        if n != 1 and s != expect:
            raise Refused("non-contiguous tensor")
        expect *= n
    if offset < 0 or offset + expect > storage.numel:
        raise Refused("tensor outside its storage")
    return Tensor(storage, offset, size, stride)


class Inert(pickle.Unpickler):
    def find_class(self, module, name):
        if (module, name) == ("collections", "OrderedDict"):
            return ordered_dict
        if (module, name) == ("torch._utils", "_rebuild_tensor_v2"):
            return rebuild_tensor_v2
        if module == "torch" and name in STORAGES:
            return Kind(name)
        raise Refused(f"global {module}.{name}")

    def persistent_load(self, pid):
        if not isinstance(pid, tuple) or len(pid) != 5 or pid[0] != "storage" or not isinstance(pid[1], Kind):
            raise Refused("persistent id")
        _, kind, key, location, numel = pid
        if not isinstance(key, str) or not key.isdigit() or location != "cpu" or not isinstance(numel, int) or numel < 0:
            raise Refused("storage descriptor")
        dtype, width = STORAGES[kind.name]
        return Storage(dtype, width, key, numel)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read_pt(raw):
    z = zipfile.ZipFile(io.BytesIO(raw))
    names = z.namelist()
    pkls = [n for n in names if n.endswith("/data.pkl")]
    if len(pkls) != 1:
        raise Refused("archive without exactly one data.pkl")
    root = pkls[0][:-len("data.pkl")]
    if any(not n.startswith(root) for n in names):
        raise Refused("archive with more than one record")
    if root + "byteorder" in names and z.read(root + "byteorder") != b"little":
        raise Refused("big-endian archive")
    info = z.getinfo(pkls[0])
    if info.file_size > PICKLE_MAX:
        raise Refused("pickle too large")
    state = Inert(io.BytesIO(z.read(pkls[0]))).load()
    if not isinstance(state, dict) or not state:
        raise Refused("checkpoint is not a state dict")
    out = []
    for name, t in state.items():
        if not isinstance(name, str) or not isinstance(t, Tensor):
            raise Refused("state dict entry is not a named tensor")
        data = z.read(f"{root}data/{t.storage.key}")
        if len(data) != t.storage.numel * t.storage.width:
            raise Refused("storage size")
        n = 1
        for v in t.size:
            n *= v
        begin = t.offset * t.storage.width
        out.append((name, t.storage.dtype, list(t.size), data[begin:begin + n * t.storage.width]))
    return out


def safetensors(tensors):
    header, at = {}, 0
    for name, dtype, shape, data in tensors:
        header[name] = {"dtype": dtype, "shape": shape, "data_offsets": [at, at + len(data)]}
        at += len(data)
    text = json.dumps(header, separators=(",", ":")).encode()
    text += b" " * ((8 - len(text) % 8) % 8)
    return struct.pack("<Q", len(text)) + text + b"".join(t[3] for t in tensors)


def export(pt, meta, tokenizer, out_dir, tokenizer_format, tokenizer_vocab):
    for path in (pt, meta, tokenizer):
        if os.path.getsize(path) > FILE_MAX:
            raise Refused(f"{path} too large")
    raw, meta_raw, tok_raw = (open(p, "rb").read() for p in (pt, meta, tokenizer))
    tensors = read_pt(raw)
    prefixed = [t for t in tensors if t[0].startswith(PREFIX)]
    if prefixed and len(prefixed) != len(tensors):
        raise Refused("mixed _orig_mod. prefix")
    if prefixed:
        tensors = [(t[0][len(PREFIX):],) + t[1:] for t in tensors]
    names = [t[0] for t in tensors]
    if len(set(names)) != len(names):
        raise Refused("duplicate tensor names")
    meta_doc = json.loads(meta_raw)
    if not isinstance(meta_doc, dict) or not isinstance(meta_doc.get("model_config"), dict) or not isinstance(meta_doc.get("step"), int):
        raise Refused("meta without model_config and step")
    body = safetensors(tensors)
    record = {
        "schema": SCHEMA,
        "source": {"repo": REPO, "revision": REVISION, "gpt_py_sha256": GPT_PY, "checkpoint_manager_py_sha256": CHECKPOINT_MANAGER_PY},
        "model_config": meta_doc["model_config"],
        "pad_vocab_size_to": 64,
        "tokenizer": {"format": tokenizer_format, "sha256": sha256(tok_raw), "vocab_size": tokenizer_vocab},
        "checkpoint": {"model_pt_sha256": sha256(raw), "meta_json_sha256": sha256(meta_raw), "step": meta_doc["step"], "key_prefix": PREFIX if prefixed else ""},
        "safetensors_sha256": sha256(body),
    }
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, "model.safetensors"), "wb") as f:
        f.write(body)
    with open(os.path.join(out_dir, "export.json"), "w") as f:
        f.write(json.dumps(record, indent=1, sort_keys=True) + "\n")
    return record, tensors


def main():
    a = argparse.ArgumentParser()
    a.add_argument("pt")
    a.add_argument("meta")
    a.add_argument("tokenizer")
    a.add_argument("out")
    a.add_argument("--tokenizer-format", required=True)
    a.add_argument("--tokenizer-vocab", type=int, required=True)
    o = a.parse_args()
    try:
        record, tensors = export(o.pt, o.meta, o.tokenizer, o.out, o.tokenizer_format, o.tokenizer_vocab)
    except (Refused, zipfile.BadZipFile, pickle.UnpicklingError, KeyError, ValueError, EOFError) as e:
        raise SystemExit(f"nanochat_export: refused: {e}")
    print(f"nanochat_export: {len(tensors)} tensors, safetensors {record['safetensors_sha256'][:16]}")


if __name__ == "__main__":
    main()
