"""Chapter 219's independent check, with onnx 1.17.0 and numpy.

With no arguments, builds the writer's test network with onnx's own helpers
and writes its bytes to psionic/fixtures/onnx/write.coax, so the Coax tests
compare the writer against onnx's serializer.

With --example, reads the files the example printed (in hex) from
psionic/fixtures/onnx-write.out, runs onnx.checker on each, rebuilds each from
its weights with onnx's helpers and requires the same bytes, and runs each with
onnx's ReferenceEvaluator on the example's sample rows, requiring the outputs
the example printed within 1e-6.
"""
import pathlib
import re
import sys

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper
from onnx.reference import ReferenceEvaluator

ROOT = pathlib.Path(__file__).resolve().parents[2]
F = TensorProto.FLOAT


def gemm(stem, ins, out, trans_b=False):
    attrs = {"transB": 1} if trans_b else {}
    return helper.make_node("Gemm", ins, [out], name=f"/{stem}/Gemm", **attrs)


def build(input_name, features, output_name, out_shape, nodes, inits):
    g = helper.make_graph(nodes, "psionic", [helper.make_tensor_value_info(input_name, F, [1, features])],
                          [helper.make_tensor_value_info(output_name, F, out_shape)], inits)
    return helper.make_model(g, opset_imports=[helper.make_opsetid("", 17)], ir_version=8, producer_name="psionic")


def mlp():
    v = np.array([1, -1, 0.5, 2, 0.25, -3, 0, 0.5, 1, 1, -1, 0.5, 2, 0, -0.25], dtype=np.float32)
    nodes = [gemm("fc1", ["x", "fc1.weight", "fc1.bias"], "t1"),
             helper.make_node("Relu", ["t1"], ["t2"], name="/Relu"),
             gemm("fc2", ["t2", "fc2.weight"], "t3"),
             helper.make_node("Softmax", ["t3"], ["p"], name="/Softmax", axis=1)]
    inits = [numpy_helper.from_array(v[0:6].reshape(2, 3), "fc1.weight"), numpy_helper.from_array(v[6:9], "fc1.bias"),
             numpy_helper.from_array(v[9:15].reshape(3, 2), "fc2.weight")]
    return build("x", 2, "p", [1, 2], nodes, inits)


def chain():
    names = ["x"] + [f"t{k}" for k in range(1, 16)] + ["y"]
    nodes = [helper.make_node("Relu", [names[k]], [names[k + 1]], name="/Relu") for k in range(16)]
    return build("x", 2, "y", [1, 2], nodes, [])


def coax_bytes(name, data):
    rows = [", ".join(str(b) for b in data[i:i + 32]) for i in range(0, len(data), 32)]
    return f"{name} :: [{len(data)}]u8.[\n    " + ",\n    ".join(rows) + "\n];\n"


def fixture():
    m, c = mlp(), chain()
    for x in (m, c):
        onnx.checker.check_model(x, full_check=True)
    data = m.SerializeToString()
    out = ROOT / "psionic/fixtures/onnx/write.coax"
    out.write_text("// Written by psionic/tests/onnx_write_check.py with onnx " + onnx.__version__ + "; do not edit.\n"
                   "// The writer's test networks, serialized by onnx's own helpers.\n" + coax_bytes("OW_T_MLP_FILE", data)
                   + coax_bytes("OW_T_CHAIN_FILE", c.SerializeToString()))
    # The pieces the Coax tests pin by hand.
    vi = helper.make_tensor_value_info("x", F, [1, 4]).SerializeToString()
    t = numpy_helper.from_array(np.array([1, -2], dtype=np.float32), "fc.bias").SerializeToString()
    print("value", vi.hex(), "tensor", t.hex())
    print(f"wrote {out.relative_to(ROOT)}: {len(data)} bytes")


def example():
    text = (ROOT / "psionic/fixtures/onnx-write.out").read_text()
    files = {}
    for name, body in re.findall(r"^(\w+)\.onnx, in hex:\n((?:  [0-9a-f ]+\n)+)", text, re.M):
        files[name] = bytes.fromhex(body.replace(" ", "").replace("\n", ""))
    assert set(files) == {"deadline", "latency"}, files.keys()
    rows = [[float(v) for v in r.split()] for r in re.findall(r"^  row [0-9]+: features ([-0-9. e]+?) -> deadline", text, re.M)]
    printed = {k: [] for k in files}
    for kind, values in re.findall(r"^  row [0-9]+: .*?-> (deadline|latency) ([-0-9. e]+)$", text, re.M):
        printed[kind].append([float(v) for v in values.split()])
    for name, data in files.items():
        m = onnx.load_from_string(data)
        onnx.checker.check_model(m, full_check=True)
        inits = {t.name: numpy_helper.to_array(t) for t in m.graph.initializer}
        cols = inits[f"{name}.bias"].shape[0]
        nodes = [gemm(name, ["features", f"{name}.weight", f"{name}.bias"], "probs" if cols == 2 else "seconds")]
        if cols == 2:
            nodes = [gemm(name, ["features", f"{name}.weight", f"{name}.bias"], "t1"),
                     helper.make_node("Softmax", ["t1"], ["probs"], name="/Softmax", axis=1)]
        rebuilt = build("features", inits[f"{name}.weight"].shape[0], "probs" if cols == 2 else "seconds", [1, cols], nodes,
                        [numpy_helper.from_array(inits[f"{name}.weight"], f"{name}.weight"),
                         numpy_helper.from_array(inits[f"{name}.bias"], f"{name}.bias")])
        assert rebuilt.SerializeToString() == data, name
        ev = ReferenceEvaluator(m)
        assert len(rows) == len(printed[name]) > 0
        for x, want in zip(rows, printed[name]):
            got = ev.run(None, {"features": np.array([x], dtype=np.float32)})[0][0]
            assert max(abs(float(a) - b) for a, b in zip(got, want)) <= 1e-6 * max(1.0, max(abs(b) for b in want)), (name, got, want)
        print(f"{name}.onnx: {len(data)} bytes pass onnx.checker, equal onnx's own serialization, "
              f"and {len(rows)} sample rows agree with ReferenceEvaluator")


if __name__ == "__main__":
    example() if "--example" in sys.argv else fixture()
