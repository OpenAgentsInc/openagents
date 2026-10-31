#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 161's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/onnx.coax and needs only the standard
library. It decodes the fixtures' protobuf itself, applies the reader's rules
(onnx-17-v1), evaluates admitted graphs in binary32 (each operation rounded
through struct, sums in the reader's order, Softmax in binary64), and rebuilds
the example's whole output. It also checks that each array in
psionic/fixtures/onnx/fixtures.coax is byte for byte its .onnx file, with the
recorded SHA-256. The outputs agree with onnx 1.17.0's ReferenceEvaluator, run
by psionic/tests/onnx_fixtures.py --reference.
"""
import hashlib
import math
import pathlib
import re
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIX = ROOT / "psionic/fixtures/onnx"
OPS = ("Gather", "ReduceMean", "Conv", "Relu", "Flatten", "Gemm", "Softmax")
STEMS = ("gather", "reducemean", "conv", "relu", "flatten", "gemm", "softmax")
LIMIT = 4096


class Refused(Exception):
    def __init__(self, status, node=-1):
        super().__init__(status)
        self.status, self.node = status, node


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def varint(b, i):
    v = shift = 0
    while True:
        if i >= len(b) or shift > 63:
            raise Refused("wire")
        c = b[i]
        i += 1
        if shift == 63 and c > 1:
            raise Refused("wire")
        v |= (c & 127) << shift
        if c < 128:
            return v, i
        shift += 7


def fields(b):
    """Every (number, wire type, value) of a message, or a wire refusal."""
    out, i = [], 0
    while i < len(b):
        key, i = varint(b, i)
        if key < 8:
            raise Refused("wire")
        n, wt = key >> 3, key & 7
        if wt == 0:
            v, i = varint(b, i)
        elif wt == 2:
            size, i = varint(b, i)
            if size > len(b) - i:
                raise Refused("wire")
            v, i = b[i:i + size], i + size
        elif wt in (1, 5):
            w = 8 if wt == 1 else 4
            if w > len(b) - i:
                raise Refused("wire")
            v, i = int.from_bytes(b[i:i + w], "little"), i + w
        else:
            raise Refused("wire")
        out.append((n, wt, v))
    return out


def signed(v):
    return v - (1 << 64) if v >= 1 << 63 else v


def ints(entries):
    vals = []
    for wt, v in entries:
        if wt == 0:
            vals.append(signed(v))
        elif wt == 2:
            i = 0
            while i < len(v):
                x, i = varint(v, i)
                vals.append(signed(x))
        else:
            return None
    return vals if len(vals) <= 4 else None


def count(dims):
    n = 1
    for d in dims:
        if d < 1 or d > LIMIT:
            return -1
        n *= d
        if n > LIMIT:
            return -1
    return n


def initializer(b):
    fs = fields(b)
    dims = ints([(wt, v) for n, wt, v in fs if n == 1])
    dtype = next((v for n, wt, v in fs if n == 2 and wt == 0), None)
    name = next((v for n, wt, v in fs if n == 8 and wt == 2), b"")
    raw = [v for n, wt, v in fs if n == 9 and wt == 2]
    floats = []
    other = False
    for n, wt, v in fs:
        if n == 4 and wt == 2:
            if len(v) % 4:
                raise Refused("data")
            floats += list(struct.unpack(f"<{len(v) // 4}f", v))
        elif n == 4 and wt == 5:
            floats.append(struct.unpack("<f", struct.pack("<I", v))[0])
        elif n in (1, 2, 8, 9, 12) or (n == 14 and v == 0):
            pass
        else:
            other = True
    if dims is None:
        raise Refused("shape")
    if dtype != 1:
        raise Refused("dtype")
    if other:
        raise Refused("data")
    return name, dims, raw, floats


def declaration(b):
    fs = fields(b)
    name, elem, shape, typed = b"", None, [], False
    for n, wt, v in fs:
        if n == 1 and wt == 2:
            name = v
        elif n == 2 and wt == 2:
            tensor = [x for x in fields(v) if x[0] == 1]
            if not tensor or tensor[0][1] != 2:
                raise Refused("input")
            for m, wt2, w in fields(tensor[0][2]):
                if m == 1 and wt2 == 0:
                    elem = w
                elif m == 2 and wt2 == 2:
                    typed = True
                    for k, wt3, dim in fields(w):
                        if k != 1 or wt3 != 2:
                            raise Refused("shape")
                        if len(shape) == 4:
                            raise Refused("capacity")
                        parts = fields(dim)
                        value = [x for x in parts if x[0] == 1]
                        if not value or value[0][1] != 0 or any(x[0] == 2 for x in parts) or not 1 <= value[0][2] <= LIMIT:
                            raise Refused("shape")
                        shape.append(value[0][2])
                else:
                    raise Refused("input")
        elif n not in (3, 4):
            raise Refused("field")
    if not typed:
        raise Refused("shape")
    if not name:
        raise Refused("name")
    return name, elem, shape


def attributes(node_fields, op, k):
    a = {"axis": None, "axes": None, "keepdims": 1, "transB": 0, "kernel": None}
    for n, wt, v in node_fields:
        if n != 5:
            continue
        name, kind, i, fl, lst = b"", None, 0, 0.0, []
        for m, wt2, w in fields(v):
            if m == 1 and wt2 == 2:
                name = w
            elif m == 20 and wt2 == 0:
                kind = w
            elif m == 3 and wt2 == 0:
                i = signed(w)
            elif m == 2 and wt2 == 5:
                fl = struct.unpack("<f", struct.pack("<I", w))[0]
            elif m == 8:
                lst.append((wt2, w))
            elif m != 13:
                raise Refused("attribute", k)
        lst = ints(lst)
        name = name.decode(errors="replace")
        ok = True
        if kind == 2 and name == "axis" and op in ("Gather", "Flatten", "Softmax"):
            a["axis"] = i
            ok = op != "Gather" or i == 0
        elif kind == 7 and name == "axes" and op == "ReduceMean":
            a["axes"] = lst
            ok = lst is not None and len(lst) == 1
        elif kind == 2 and name == "keepdims" and op == "ReduceMean":
            a["keepdims"] = i
            ok = i in (0, 1)
        elif kind == 7 and name == "kernel_shape" and op == "Conv":
            a["kernel"] = lst
            ok = lst is not None and len(lst) == 2
        elif kind == 7 and name in ("strides", "dilations") and op == "Conv":
            ok = lst == [1, 1]
        elif kind == 7 and name == "pads" and op == "Conv":
            ok = lst == [0, 0, 0, 0]
        elif kind == 2 and name == "group" and op == "Conv":
            ok = i == 1
        elif kind == 1 and name in ("alpha", "beta") and op == "Gemm":
            ok = fl == 1.0
        elif kind == 2 and name == "transA" and op == "Gemm":
            ok = i == 0
        elif kind == 2 and name == "transB" and op == "Gemm":
            a["transB"] = i
            ok = i in (0, 1)
        else:
            ok = False
        if not ok:
            raise Refused("attribute", k)
    return a


def stem(node_name, op, k):
    s = node_name.decode(errors="replace")
    if s.startswith("/") and s.rfind("/") > 0:
        return s[1:s.rfind("/")].replace("/", ".")
    return STEMS[OPS.index(op)] + str(k)


def read(b, capacity):
    """Returns the admitted graph as a dict, or raises Refused."""
    if len(b) > 1 << 20:
        raise Refused("capacity")
    fs = fields(b)
    ir, graphs, opsets = None, [], 0
    for n, wt, v in fs:
        if n == 1 and wt == 0:
            ir = v
        elif n == 7 and wt == 2:
            graphs.append(v)
        elif n == 8 and wt == 2:
            sub = fields(v)
            domain = next((w for m, _, w in sub if m == 1), b"")
            version = next((w for m, _, w in sub if m == 2), 0)
            if domain not in (b"", b"ai.onnx") or version != 17:
                raise Refused("opset")
            opsets += 1
        elif not 2 <= n <= 6 and n != 14:
            raise Refused("field")
    if ir is None or not 7 <= ir <= 10:
        raise Refused("ir")
    if opsets != 1:
        raise Refused("opset")
    if len(graphs) != 1:
        raise Refused("field")
    gf = fields(graphs[0])
    values, arena = {}, 0
    for n, wt, v in gf:
        if n not in (1, 2, 5, 10, 11, 12, 13, 16):
            raise Refused("field")
        if n == 5:
            name, dims, raw, floats = initializer(v)
            if not name or name in values:
                raise Refused("name")
            c = count(dims)
            if c < 0:
                raise Refused("shape")
            if bool(raw) == bool(floats) or (raw and len(raw[-1]) != 4 * c) or (not raw and len(floats) != c):
                raise Refused("data")
            if arena + c > capacity or len(values) == 32:
                raise Refused("capacity")
            data = list(struct.unpack(f"<{c}f", raw[-1])) if raw else floats
            values[name] = dict(kind="weight", shape=dims, data=data, uses=0, original=name)
            arena += c
    inputs = []
    for n, wt, v in gf:
        if n != 11:
            continue
        name, elem, shape = declaration(v)
        if name in values:
            continue
        if inputs:
            raise Refused("input")
        if elem not in (1, 7):
            raise Refused("dtype")
        c = count(shape)
        if c < 0 or arena + c > capacity or len(values) == 32:
            raise Refused("capacity")
        values[name] = dict(kind="input", shape=shape, integer=elem == 7)
        inputs.append(name)
        arena += c
    if not inputs:
        raise Refused("input")
    nodes, names, k = [], [], 0
    for n, wt, v in gf:
        if n != 1:
            continue
        if k == 16:
            raise Refused("capacity", k)
        nf = fields(v)
        ins = [w for m, _, w in nf if m == 1]
        outs = [w for m, _, w in nf if m == 2]
        node_name = next((w for m, _, w in nf if m == 3), b"")
        op = next((w for m, _, w in nf if m == 4), b"").decode(errors="replace")
        domain = next((w for m, _, w in nf if m == 7), b"")
        if len(ins) > 3:
            raise Refused("op", k)
        if sum(1 for m, _, _ in nf if m == 5) > 8:
            raise Refused("attribute", k)
        if any(m not in (1, 2, 3, 4, 5, 6, 7, 9) for m, _, _ in nf):
            raise Refused("field", k)
        if domain not in (b"", b"ai.onnx"):
            raise Refused("opset", k)
        arity_ok = {"Gather": (2,), "Conv": (2, 3), "Gemm": (2, 3)}.get(op, (1,))
        if op not in OPS or len(outs) != 1 or len(ins) not in arity_ok:
            raise Refused("op", k)
        a = attributes(nf, op, k)
        refs = []
        for s, name in enumerate(ins):
            if name not in values:
                raise Refused("order", k)
            val = values[name]
            weight_slot = (op == "Gather" and s == 0) or (op in ("Conv", "Gemm") and s >= 1)
            if (val["kind"] == "weight") != weight_slot:
                raise Refused("role", k)
            if val.get("integer", False) != (op == "Gather" and s == 1):
                raise Refused("dtype", k)
            if val["kind"] == "weight":
                val["uses"] += 1
                if val["uses"] > 1:
                    raise Refused("shared", k)
            refs.append(name)
        x = values[refs[0]]["shape"]
        axis = 0
        if op == "Gather":
            v_, i_ = x, values[refs[1]]["shape"]
            if len(v_) != 2 or len(i_) > 2:
                raise Refused("shape", k)
            shape = i_ + [v_[1]]
        elif op == "ReduceMean":
            if a["axes"] is None or len(a["axes"]) != 1:
                raise Refused("attribute", k)
            axis = a["axes"][0] + (len(x) if a["axes"][0] < 0 else 0)
            if not 0 <= axis < len(x):
                raise Refused("attribute", k)
            shape = x[:axis] + ([1] if a["keepdims"] else []) + x[axis + 1:]
        elif op == "Conv":
            w = values[refs[1]]["shape"]
            if len(x) != 4 or len(w) != 4 or w[1] != x[1] or w[2] > x[2] or w[3] > x[3]:
                raise Refused("shape", k)
            if a["kernel"] and a["kernel"] != w[2:]:
                raise Refused("attribute", k)
            if len(refs) == 3 and values[refs[2]]["shape"] != [w[0]]:
                raise Refused("shape", k)
            shape = [x[0], w[0], x[2] - w[2] + 1, x[3] - w[3] + 1]
        elif op == "Relu":
            shape = list(x)
        elif op == "Flatten":
            ax = 1 if a["axis"] is None else a["axis"]
            ax += len(x) if ax < 0 else 0
            if not 0 <= ax <= len(x):
                raise Refused("attribute", k)
            shape = [math.prod(x[:ax]), math.prod(x[ax:])]
        elif op == "Gemm":
            w = values[refs[1]]["shape"]
            if len(x) != 2 or len(w) != 2:
                raise Refused("shape", k)
            nn, kk = (w[0], w[1]) if a["transB"] == 1 else (w[1], w[0])
            if x[1] != kk:
                raise Refused("shape", k)
            if len(refs) == 3 and values[refs[2]]["shape"] != [nn]:
                raise Refused("shape", k)
            shape = [x[0], nn]
        else:
            ax = -1 if a["axis"] is None else a["axis"]
            if ax not in (-1, len(x) - 1):
                raise Refused("attribute", k)
            shape = list(x)
        for s, name in enumerate(refs):
            if values[name]["kind"] == "weight":
                full = stem(node_name, op, k) + (".bias" if s == 2 else ".weight")
                if len(full) > 64 or full in [t for t, _ in names]:
                    raise Refused("name", k)
                names.append((full, name))
        c = count(shape)
        if c < 0 or arena + c > capacity or len(values) == 32:
            raise Refused("capacity", k)
        if not outs[0] or outs[0] in values:
            raise Refused("name", k)
        values[outs[0]] = dict(kind="computed", shape=shape)
        arena += c
        nodes.append((op, refs, outs[0], a, axis))
        k += 1
    output = None
    for n, wt, v in gf:
        if n != 12:
            continue
        name, elem, shape = declaration(v)
        if output is not None or name not in values or values[name]["kind"] != "computed" or elem != 1 or values[name]["shape"] != shape:
            raise Refused("output")
        output = name
    if output is None:
        raise Refused("output")
    if any(v["kind"] == "weight" and v["uses"] == 0 for v in values.values()):
        raise Refused("unused")
    return dict(ir=ir, values=values, nodes=nodes, names=names, input=inputs[0], output=output, arena=arena)


def run(g, feed):
    data = {n: v["data"] for n, v in g["values"].items() if v["kind"] == "weight"}
    data[g["input"]] = [f32(x) for x in feed]
    for k, (op, refs, out, a, axis) in enumerate(g["nodes"]):
        x, xs = data[refs[0]], g["values"][refs[0]]["shape"]
        if op == "Gather":
            rows, d = xs
            y = []
            for i in data[refs[1]]:
                i = int(i) + (rows if i < 0 else 0)
                if not 0 <= i < rows:
                    raise Refused("input", k)
                y += x[i * d:(i + 1) * d]
        elif op == "ReduceMean":
            outer, span, inner = math.prod(xs[:axis]), xs[axis], math.prod(xs[axis + 1:])
            y = []
            for o in range(outer):
                for i in range(inner):
                    t = 0.0
                    for s in range(span):
                        t = f32(t + x[(o * span + s) * inner + i])
                    y.append(f32(t / span))
        elif op == "Conv":
            w, ws = data[refs[1]], g["values"][refs[1]]["shape"]
            n_, c, h, wd = xs
            m, _, kh, kw = ws
            oh, ow = h - kh + 1, wd - kw + 1
            y = []
            for b in range(n_):
                for f in range(m):
                    for i in range(oh):
                        for j in range(ow):
                            t = data[refs[2]][f] if len(refs) == 3 else 0.0
                            for ch in range(c):
                                for u in range(kh):
                                    for v in range(kw):
                                        t = f32(t + f32(x[((b * c + ch) * h + i + u) * wd + j + v] * w[((f * c + ch) * kh + u) * kw + v]))
                            y.append(t)
        elif op == "Relu":
            y = [v if v > 0 else 0.0 for v in x]
        elif op == "Flatten":
            y = list(x)
        elif op == "Gemm":
            w = data[refs[1]]
            rows, depth = xs
            cols = g["values"][out]["shape"][1]
            y = []
            for i in range(rows):
                for j in range(cols):
                    t = 0.0
                    for q in range(depth):
                        b = w[j * depth + q] if a["transB"] == 1 else w[q * cols + j]
                        t = f32(t + f32(x[i * depth + q] * b))
                    if len(refs) == 3:
                        t = f32(t + data[refs[2]][j])
                    y.append(t)
        else:
            width = xs[-1]
            y = []
            for o in range(len(x) // width):
                row = x[o * width:(o + 1) * width]
                top = max(row)
                total = 0.0
                for v in row:
                    total += math.exp(v - top)
                y += [f32(math.exp(v - top) / total) for v in row]
        data[out] = y
    return data[g["output"]]


def r6(v):
    return math.floor(v * 1000000 + 0.5) / 1000000


def uniform(seed, i):
    mask = (1 << 64) - 1
    z = (seed + 0x9E3779B97F4A7C15 * (i + 1)) & mask
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & mask
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & mask
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def shape_text(s):
    return "[" + ", ".join(str(d) for d in s) + "]"


def describe(lines, file, b, g):
    t = len(g["names"])
    lines.append(f"{file}: {len(b)} bytes, IR {g['ir']}, opset 17: {len(g['nodes'])} nodes, {t} weight{'' if t == 1 else 's'}, {g['arena']} floats of arena")
    for full, orig in g["names"]:
        lines.append(f"  {orig.decode()} -> {full} f32 {shape_text(g['values'][orig]['shape'])}")
    x, y = g["values"][g["input"]], g["values"][g["output"]]
    lines.append(f"  {g['input'].decode()} {'int64' if x['integer'] else 'f32'} {shape_text(x['shape'])} -> {g['output'].decode()} f32 {shape_text(y['shape'])}")


def outputs(values):
    return "".join(f" {r6(v)!r}" for v in values)


def example():
    files = {p.stem: p.read_bytes() for p in FIX.glob("*.onnx")}
    lines = []
    b = files["embedding"]
    g = read(b, 1024)
    describe(lines, "embedding.onnx", b, g)
    ids = [3, 17, 17, 42, 0, 63, 8, 21]
    lines.append("  ids 3 17 17 42 0 63 8 21 ->" + outputs(run(g, ids)))
    ids[5] = -1
    lines.append("  id -1 counts from the end, as 63 ->" + outputs(run(g, ids)))
    ids[5] = 64
    try:
        run(g, ids)
        lines.append("  id 64: ok")
    except Refused as e:
        lines.append(f"  id 64: {e.status} at node {e.node}")
    b = files["classifier"]
    g = read(b, 1024)
    describe(lines, "classifier.onnx", b, g)
    for k in range(2):
        pixels = [math.floor((2 * uniform(170 + k, i) - 1) * 32 + 0.5) / 32 for i in range(36)]
        lines.append(f"  image {k} ->" + outputs(run(g, pixels)))
    lines.append("refused:")
    for name in ("opset", "ir", "domain", "function", "sparse", "op", "trans_a", "alpha", "pads", "unknown_attr", "double",
                 "external", "short", "symbolic", "order", "unused", "shared", "role", "wrong_output", "duplicate"):
        try:
            read(files["refuse_" + name], 1024)
            lines.append(f"  refuse_{name}.onnx: ok")
        except Refused as e:
            at = f" at node {e.node}" if e.node >= 0 or name == "duplicate" else ""
            lines.append(f"  refuse_{name}.onnx: {e.status}{at}")
    for text, b, cap in (("classifier.onnx cut at 600 bytes", files["classifier"][:600], 1024),
                         ("classifier.onnx with 200 floats of arena", files["classifier"], 200)):
        try:
            read(b, cap)
            lines.append(f"  {text}: ok")
        except Refused as e:
            lines.append(f"  {text}: {e.status}")
    return "\n".join(lines) + "\n"


def check_cases():
    """Replays the generated case table: every status and node, and every
    admitted model's outputs against onnx's reference within 1e-6."""
    text = (FIX / "fixtures.coax").read_text()
    blob = bytes(int(x) for x in re.search(r"^OX_CASE_BYTES :: \[\d+\] u8\.\[([0-9,]*)\];$", text, re.M).group(1).split(","))
    feed = [float(x) for x in re.search(r"^OX_CASE_FEED :: \[\d+\] f64\.\[([^\]]*)\];$", text, re.M).group(1).split(",")]
    want = [float(x) for x in re.search(r"^OX_CASE_OUT :: \[\d+\] f64\.\[([^\]]*)\];$", text, re.M).group(1).split(",")]
    rows = re.findall(r"OxCase\.\{at=(\d+),size=(\d+),status=OxStatus\.(\w+),node=(-?\d+)(?:,feed_at=(\d+),feed=(\d+),out_at=(\d+),out=(\d+))?\}, // (\w+)", text)
    admitted = 0
    for at, size, status, node, feed_at, n, out_at, out, name in rows:
        at, size, node = int(at), int(size), int(node)
        try:
            g = read(blob[at:at + size], 8192)
            got, where = "ok", -1
        except Refused as e:
            got, where = e.status, e.node
        assert (got, where) == (status, node), (name, got, where, status, node)
        if n:
            x = feed[int(feed_at):int(feed_at) + int(n)]
            if g["values"][g["input"]]["integer"]:
                x = [int(v) for v in x]
            y = run(g, x)
            ref = want[int(out_at):int(out_at) + int(out)]
            assert len(y) == len(ref) and all(abs(a - b) <= 1e-6 for a, b in zip(y, ref)), name
            admitted += 1
    return len(rows), admitted


def check_arrays():
    text = (FIX / "fixtures.coax").read_text()
    found = re.findall(r"^OX_([A-Z_]+) :: \[(\d+)\] u8\.\[([0-9,]*)\];\nOX_\1_SHA :: \"([0-9a-f]{64})\";$", text, re.M)
    files = sorted(p.stem for p in FIX.glob("*.onnx"))
    assert sorted(n.lower() for n, *_ in found) == files, "fixtures.coax and the .onnx files list different models"
    for name, size, body, sha in found:
        data = bytes(int(x) for x in body.split(","))
        assert len(data) == int(size) and data == (FIX / f"{name.lower()}.onnx").read_bytes(), name
        assert hashlib.sha256(data).hexdigest() == sha, name
    return len(found)


if __name__ == "__main__":
    n = check_arrays()
    cases, admitted = check_cases()
    want = (ROOT / "psionic/fixtures/onnx-read.out").read_text()
    got = example()
    if got != want:
        for a, b in zip(got.splitlines(), want.splitlines()):
            if a != b:
                print("oracle :", a)
                print("fixture:", b)
        sys.exit(1)
    print(f"onnx oracle: {n} fixture arrays match their files; {cases} table cases agree ({admitted} run within 1e-6 of onnx's reference); the example's output is rebuilt line for line")
