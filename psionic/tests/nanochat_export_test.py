#!/usr/bin/env python3
"""Checks for psionic/tools/export/nanochat_export.py (no torch, nothing executed).

The committed torch.save checkpoint re-exports byte for byte to the committed
safetensors and identity record; every exported value carries the oracle's
bit pattern; and crafted archives refuse: a foreign global (os.system), a
non-tuple persistent id, a big-endian archive, a non-contiguous or
out-of-bounds tensor, an unsupported storage type and mixed compile prefixes.
A uniform _orig_mod. prefix is stripped and recorded, as
checkpoint_manager.build_model strips it.
"""
import io
import json
import os
import pickle
import struct
import subprocess
import sys
import tempfile
import types
import zipfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
FX = os.path.join(ROOT, "psionic/fixtures/nanochat")
TOOL = os.path.join(ROOT, "psionic/tools/export/nanochat_export.py")
ORACLE = os.path.join(ROOT, "psionic/tests/nanochat_oracle.py")
TOKENIZER = os.path.join(ROOT, "psionic/fixtures/bpe-train/learned.pst")


def export(pt, meta, out):
    return subprocess.run([sys.executable, "-I", TOOL, pt, meta, TOKENIZER, out, "--tokenizer-format", "psionic-pst5+nanochat-specials",
                           "--tokenizer-vocab", "329"], capture_output=True, text=True)


# Stand-ins named like torch's pickled globals, used only to write crafted archives.
class Marker:
    def __init__(self, name):
        self.name = name


def fake_torch():
    torch = types.ModuleType("torch")
    utils = types.ModuleType("torch._utils")
    for name in ("FloatStorage", "BFloat16Storage", "HalfStorage"):
        cls = type(name, (), {"__module__": "torch", "__qualname__": name})
        setattr(torch, name, cls)

    def _rebuild_tensor_v2(*a):
        raise AssertionError("never called while writing")
    _rebuild_tensor_v2.__module__ = "torch._utils"
    _rebuild_tensor_v2.__qualname__ = "_rebuild_tensor_v2"
    utils._rebuild_tensor_v2 = _rebuild_tensor_v2
    torch._utils = utils
    sys.modules["torch"], sys.modules["torch._utils"] = torch, utils
    return torch, utils


class T:
    def __init__(self, kind, key, numel, offset, size, stride):
        self.kind, self.key, self.numel, self.offset, self.size, self.stride = kind, key, numel, offset, size, stride

    def __reduce__(self):
        import collections
        return (sys.modules["torch._utils"]._rebuild_tensor_v2,
                (("storage", self.kind, self.key, "cpu", self.numel), self.offset, self.size, self.stride, False, collections.OrderedDict()))


class Evil:
    def __reduce__(self):
        return (os.system, ("true",))


def write_pt(path, state, data, pid=None, byteorder=b"little"):
    class P(pickle.Pickler):
        def persistent_id(self, obj):
            if isinstance(obj, tuple) and len(obj) == 5 and obj[0] == "storage":
                return pid if pid is not None else obj
            return None
    buf = io.BytesIO()
    P(buf, protocol=2).dump(state)
    with zipfile.ZipFile(path, "w") as z:
        z.writestr("archive/data.pkl", buf.getvalue())
        z.writestr("archive/byteorder", byteorder)
        for key, blob in data.items():
            z.writestr(f"archive/data/{key}", blob)


def crafted(tmp, name, state, data, **kw):
    path = os.path.join(tmp, name + ".pt")
    write_pt(path, state, data, **kw)
    r = export(path, os.path.join(FX, "checkpoint/meta_000000.json"), os.path.join(tmp, name))
    return r


def main():
    with tempfile.TemporaryDirectory() as tmp:
        r = export(os.path.join(FX, "checkpoint/model_000000.pt"), os.path.join(FX, "checkpoint/meta_000000.json"), os.path.join(tmp, "again"))
        assert r.returncode == 0, r.stderr
        for name in ("model.safetensors", "export.json"):
            assert open(os.path.join(tmp, "again", name), "rb").read() == open(os.path.join(FX, "export", name), "rb").read(), name
        # Every stored value is the oracle's bit pattern.
        weights = json.loads(subprocess.run([sys.executable, "-I", ORACLE, "--weights", "checkpoint"], capture_output=True, text=True, check=True).stdout)
        body = open(os.path.join(FX, "export/model.safetensors"), "rb").read()
        n = struct.unpack("<Q", body[:8])[0]
        header = json.loads(body[8:8 + n])
        assert list(header) == [t["name"] for t in weights["tensors"]]
        for t in weights["tensors"]:
            h = header[t["name"]]
            assert h["dtype"] == t["dtype"] and h["shape"] == t["shape"]
            raw = body[8 + n + h["data_offsets"][0]:8 + n + h["data_offsets"][1]]
            if t["dtype"] == "BF16":
                assert list(struct.unpack(f"<{len(raw) // 2}H", raw)) == t["bits"]
            else:
                assert list(struct.unpack(f"<{len(raw) // 4}f", raw)) == t["values"]
        torch, _ = fake_torch()
        F, B, H = torch.FloatStorage, torch.BFloat16Storage, torch.HalfStorage
        good = {"0": struct.pack("<4f", 1, 2, 3, 4)}
        ok = crafted(tmp, "ok", {"_orig_mod.a": T(F, "0", 4, 0, (2, 2), (2, 1))}, good)
        assert ok.returncode == 0, ok.stderr
        rec = json.load(open(os.path.join(tmp, "ok", "export.json")))
        st = open(os.path.join(tmp, "ok", "model.safetensors"), "rb").read()
        assert rec["checkpoint"]["key_prefix"] == "_orig_mod." and b'"a":{"dtype":"F32","shape":[2,2]' in st
        refusals = {
            "evil": ({"a": Evil()}, good, {}),
            "pid": ({"a": T(F, "0", 4, 0, (4,), (1,))}, good, {"pid": "storage-0"}),
            "endian": ({"a": T(F, "0", 4, 0, (4,), (1,))}, good, {"byteorder": b"big"}),
            "strided": ({"a": T(F, "0", 4, 0, (2, 2), (1, 2))}, good, {}),
            "bounds": ({"a": T(F, "0", 4, 1, (4,), (1,))}, good, {}),
            "half": ({"a": T(H, "0", 4, 0, (4,), (1,))}, good, {}),
            "mixed": ({"_orig_mod.a": T(F, "0", 4, 0, (2,), (1,)), "b": T(F, "0", 4, 2, (2,), (1,))}, good, {}),
            "size": ({"a": T(B, "0", 4, 0, (4,), (1,))}, good, {}),
        }
        for name, (state, data, kw) in refusals.items():
            r = crafted(tmp, name, state, data, **kw)
            assert r.returncode != 0 and "refused" in r.stderr, (name, r.stdout, r.stderr)
            assert not os.path.exists(os.path.join(tmp, name, "model.safetensors")), name
    print(f"nanochat export: committed checkpoint re-exports byte for byte, {len(weights['tensors'])} tensors carry the oracle's bits, {len(refusals)} crafted archives refuse")


if __name__ == "__main__":
    main()
