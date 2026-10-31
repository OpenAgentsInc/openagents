#!/usr/bin/env python3
"""Builds chapter 218's ONNX fixtures with the onnx package (pinned 1.17.0).

Not part of the CPU check: it needs `onnx==1.17.0` and numpy, so it runs in a
separate environment. It writes psionic/fixtures/onnx/*.onnx, the byte arrays
in psionic/fixtures/onnx/fixtures.coax, and, with `--reference`, checks each
admitted model with onnx.checker and runs onnx.reference.ReferenceEvaluator
on the example's inputs, printing the outputs the reader must match.

Weights are multiples of 1/64 in [-0.5, 0.5] from a SplitMix64 stream, so
every value is exact in binary32. Names follow a PyTorch export: anonymous
initializers ("onnx::Conv_5") and node names that carry the module path
("/conv/Conv").
"""
import hashlib
import math
import pathlib
import struct
import sys

import onnx
from onnx import TensorProto, helper

assert onnx.__version__ == "1.17.0", onnx.__version__
ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "psionic/fixtures/onnx"
MASK = (1 << 64) - 1


def uniform(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def weights(seed, n):
    return [round((2 * uniform(seed, i) - 1) * 32) / 64 for i in range(n)]


def tensor(name, dims, seed, raw):
    values = weights(seed, math.prod(dims))
    if raw:
        return helper.make_tensor(name, TensorProto.FLOAT, dims, struct.pack(f"<{len(values)}f", *values), raw=True)
    return helper.make_tensor(name, TensorProto.FLOAT, dims, values)


def model(nodes, inputs, outputs, inits, opset=17, ir=8, **graph_extra):
    g = helper.make_graph(nodes, "main", inputs, outputs, inits, **graph_extra)
    m = helper.make_model(g, opset_imports=[helper.make_opsetid("", opset)], producer_name="psionic-ch161", ir_version=ir)
    return m


def embedding(**over):
    ids = helper.make_tensor_value_info("input_ids", TensorProto.INT64, [1, 8])
    out = helper.make_tensor_value_info("sentence", TensorProto.FLOAT, [1, 8])
    nodes = [
        helper.make_node("Gather", ["onnx::Gather_0", "input_ids"], ["/embed/Gather_output_0"], name="/embed/Gather", axis=0),
        helper.make_node("ReduceMean", ["/embed/Gather_output_0"], ["sentence"], name="/ReduceMean", axes=[1], keepdims=0),
    ]
    inits = [tensor("onnx::Gather_0", [64, 8], 161, raw=False)]
    return model(nodes, [ids], [out], inits, **over)


def classifier_parts():
    x = helper.make_tensor_value_info("pixels", TensorProto.FLOAT, [1, 1, 6, 6])
    y = helper.make_tensor_value_info("probs", TensorProto.FLOAT, [1, 3])
    nodes = [
        helper.make_node("Conv", ["pixels", "onnx::Conv_5", "onnx::Conv_6"], ["/conv/Conv_output_0"], name="/conv/Conv",
                         kernel_shape=[3, 3], strides=[1, 1], pads=[0, 0, 0, 0], dilations=[1, 1], group=1),
        helper.make_node("Relu", ["/conv/Conv_output_0"], ["/relu/Relu_output_0"], name="/relu/Relu"),
        helper.make_node("Flatten", ["/relu/Relu_output_0"], ["/Flatten_output_0"], name="/Flatten", axis=1),
        helper.make_node("Gemm", ["/Flatten_output_0", "onnx::Gemm_7", "onnx::Gemm_8"], ["/fc/Gemm_output_0"], name="/fc/Gemm",
                         alpha=1.0, beta=1.0, transB=1),
        helper.make_node("Softmax", ["/fc/Gemm_output_0"], ["probs"], name="/Softmax", axis=-1),
    ]
    inits = [tensor("onnx::Conv_5", [2, 1, 3, 3], 162, raw=True), tensor("onnx::Conv_6", [2], 163, raw=True),
             tensor("onnx::Gemm_7", [3, 32], 164, raw=True), tensor("onnx::Gemm_8", [3], 165, raw=True)]
    return nodes, [x], [y], inits


def classifier(**over):
    nodes, ins, outs, inits = classifier_parts()
    return model(nodes, ins, outs, inits, **over)


def variant(change):
    nodes, ins, outs, inits = classifier_parts()
    extra = change(nodes, ins, outs, inits) or {}
    return model(nodes, ins, outs, inits, **extra)


def set_attr(node, name, value):
    for a in node.attribute:
        if a.name == name:
            node.attribute.remove(a)
            break
    node.attribute.append(helper.make_attribute(name, value))


def refusals():
    out = {}
    m = classifier()
    m.opset_import[0].version = 18
    out["opset"] = m
    m = classifier()
    m.ir_version = 3
    out["ir"] = m
    m = classifier()
    m.opset_import.append(helper.make_opsetid("com.microsoft", 1))
    out["domain"] = m

    def op(n, *_):
        n[1].op_type = "LeakyRelu"
    out["op"] = variant(op)

    def trans_a(n, *_):
        set_attr(n[3], "transA", 1)
    out["trans_a"] = variant(trans_a)

    def alpha(n, *_):
        set_attr(n[3], "alpha", 0.5)
    out["alpha"] = variant(alpha)

    def pads(n, *_):
        set_attr(n[0], "pads", [1, 1, 1, 1])
    out["pads"] = variant(pads)

    def unknown_attr(n, *_):
        n[1].attribute.append(helper.make_attribute("hint", 1))
    out["unknown_attr"] = variant(unknown_attr)

    def double(n, i, o, inits):
        inits[1].CopyFrom(helper.make_tensor("onnx::Conv_6", TensorProto.DOUBLE, [2], [0.25, -0.5]))
    out["double"] = variant(double)

    def external(n, i, o, inits):
        t = inits[1]
        t.ClearField("raw_data")
        t.data_location = TensorProto.EXTERNAL
        e = t.external_data.add()
        e.key, e.value = "location", "weights.bin"
    out["external"] = variant(external)

    def short(n, i, o, inits):
        inits[2].raw_data = inits[2].raw_data[:-4]
    out["short"] = variant(short)

    def symbolic(n, ins, o, inits):
        ins[0].type.tensor_type.shape.dim[0].ClearField("dim_value")
        ins[0].type.tensor_type.shape.dim[0].dim_param = "batch"
    out["symbolic"] = variant(symbolic)

    def order(n, *_):
        n[1], n[2] = n[2], n[1]
    out["order"] = variant(order)

    def unused(n, i, o, inits):
        inits.append(tensor("onnx::Gemm_9", [3], 166, raw=True))
    out["unused"] = variant(unused)

    def shared(n, *_):
        n[3].input[2] = "onnx::Conv_6"
    out["shared"] = variant(shared)

    def role(n, ins, o, inits):
        inits.append(tensor("onnx::Relu_9", [1, 2, 4, 4], 167, raw=True))
        n[1].input[0] = "onnx::Relu_9"
    out["role"] = variant(role)

    def wrong_out(n, i, outs, inits):
        outs[0].type.tensor_type.shape.dim[1].dim_value = 4
    out["wrong_output"] = variant(wrong_out)

    def duplicate(n, *_):
        n[2].output[0] = "/conv/Conv_output_0"
    out["duplicate"] = variant(duplicate)

    def sparse(n, i, o, inits):
        values = helper.make_tensor("s", TensorProto.FLOAT, [1], [1.0])
        indices = helper.make_tensor("s_i", TensorProto.INT64, [1], [0])
        return {"sparse_initializer": [helper.make_sparse_tensor(values, indices, [2])]}
    out["sparse"] = variant(sparse)
    m = classifier()
    m.functions.append(helper.make_function("local", "f", ["x"], ["y"], [helper.make_node("Relu", ["x"], ["y"])],
                                            [helper.make_opsetid("", 17)]))
    out["function"] = m
    return out


def coax_array(name, data):
    return f"{name} :: [{len(data)}] u8.[{','.join(str(b) for b in data)}];\n"


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    models = {"embedding": embedding(), "classifier": classifier()}
    models.update({"refuse_" + k: v for k, v in refusals().items()})
    text = ["// Generated by psionic/tests/onnx_fixtures.py with onnx 1.17.0; do not edit.\n",
            "// Each array is byte for byte the file of the same name in this directory.\n"]
    for name, m in models.items():
        data = m.SerializeToString()
        (OUT / f"{name}.onnx").write_bytes(data)
        text.append(coax_array("OX_" + name.upper(), data))
        text.append(f'OX_{name.upper()}_SHA :: "{hashlib.sha256(data).hexdigest()}";\n')
    text.append("// The case table: built and checked by onnx 1.17.0, statuses from the chapter's rules.\n")
    text.append(case_table())
    (OUT / "fixtures.coax").write_text("".join(text))
    if "--reference" in sys.argv:
        reference(models)


def reference(models):
    import numpy as np
    from onnx.reference import ReferenceEvaluator
    for name in ("embedding", "classifier"):
        onnx.checker.check_model(models[name], full_check=True)
    ids = np.array([[3, 17, 17, 42, 0, 63, 8, 21]], dtype=np.int64)
    got = ReferenceEvaluator(models["embedding"]).run(None, {"input_ids": ids})[0]
    print("embedding", " ".join(repr(float(v)) for v in got.reshape(-1)))
    for k in range(2):
        pixels = np.array([round((2 * uniform(170 + k, i) - 1) * 32) / 32 for i in range(36)], dtype=np.float32).reshape(1, 1, 6, 6)
        got = ReferenceEvaluator(models["classifier"]).run(None, {"pixels": pixels})[0]
        print("classifier", k, " ".join(repr(float(v)) for v in got.reshape(-1)))



# The case table: small models, each with the status the chapter's rules give
# it (written here by hand, from the rules) and, for an admitted one, an input
# and the outputs onnx's ReferenceEvaluator computes.
F, I64, D = TensorProto.FLOAT, TensorProto.INT64, TensorProto.DOUBLE


def vi(name, shape, elem=F):
    return helper.make_tensor_value_info(name, elem, shape)


def tiny(nodes=None, inputs=None, outputs=None, inits=None, opsets=(("", 17),), ir=8, **graph_extra):
    nodes = nodes if nodes is not None else [helper.make_node("Gemm", ["x", "W", "b"], ["y"], name="/fc/Gemm", transB=1)]
    inputs = inputs if inputs is not None else [vi("x", [1, 4])]
    outputs = outputs if outputs is not None else [vi("y", [1, 2])]
    inits = inits if inits is not None else [tensor("W", [2, 4], 201, True), tensor("b", [2], 202, False)]
    g = helper.make_graph(nodes, "tiny", inputs, outputs, inits, **graph_extra)
    m = helper.make_model(g, opset_imports=[helper.make_opsetid(d, v) for d, v in opsets])
    m.ir_version = ir
    return m


def gemm(*ins, name="/fc/Gemm", **attrs):
    return helper.make_node("Gemm", list(ins), ["y"], name=name, **attrs)


def relu_chain(n):
    nodes = [helper.make_node("Relu", ["x" if k == 0 else f"r{k}"], [f"r{k + 1}" if k < n - 1 else "y"], name=f"/r{k}/Relu") for k in range(n)]
    return tiny(nodes, [vi("x", [1, 2])], [vi("y", [1, 2])], [])


def accepted():
    """(name, model, feed shape, integer feed)"""
    cases = []
    cases.append(("gemm_plain", tiny([helper.make_node("Gemm", ["x", "W"], ["h"], name=""), helper.make_node("Relu", ["h"], ["y"], name="")],
                                     outputs=[vi("y", [1, 3])], inits=[tensor("W", [4, 3], 203, True)]), [1, 4], False))
    cases.append(("reduce_keep", tiny([helper.make_node("ReduceMean", ["x"], ["m"], name="/m/ReduceMean", axes=[-1], keepdims=1),
                                       helper.make_node("Flatten", ["m"], ["f"], name="/Flatten", axis=0),
                                       helper.make_node("Gemm", ["f", "W"], ["g"], name="/fc/Gemm"),
                                       helper.make_node("Softmax", ["g"], ["y"], name="/Softmax", axis=1)],
                                      [vi("x", [2, 3, 4])], [vi("y", [1, 2])], [tensor("W", [6, 2], 204, True)]), [2, 3, 4], False))
    cases.append(("reduce_first", tiny([helper.make_node("ReduceMean", ["x"], ["y"], name="", axes=[0])],
                                       [vi("x", [3, 2])], [vi("y", [1, 2])], []), [3, 2], False))
    cases.append(("conv_batch", tiny([helper.make_node("Conv", ["x", "W"], ["c"], name="/c/Conv"),
                                      helper.make_node("Relu", ["c"], ["r"], name="/Relu"),
                                      helper.make_node("Flatten", ["r"], ["f"], name="/Flatten", axis=-1),
                                      helper.make_node("Softmax", ["f"], ["y"], name="/Softmax")],
                                     [vi("x", [2, 3, 4, 3])], [vi("y", [24, 2])], [tensor("W", [4, 3, 2, 2], 205, True)]), [2, 3, 4, 3], False))
    cases.append(("conv_rect", tiny([helper.make_node("Conv", ["x", "W", "b"], ["y"], name="/c/Conv", kernel_shape=[3, 2])],
                                    [vi("x", [1, 1, 3, 5])], [vi("y", [1, 1, 1, 4])], [tensor("W", [1, 1, 3, 2], 214, True), tensor("b", [1], 215, True)]), [1, 1, 3, 5], False))
    cases.append(("conv_full_width", tiny([helper.make_node("Conv", ["x", "W"], ["y"], name="/c/Conv")],
                                          [vi("x", [2, 1, 4, 3])], [vi("y", [2, 2, 3, 1])], [tensor("W", [2, 1, 2, 3], 216, True)]), [2, 1, 4, 3], False))
    cases.append(("flatten_default", tiny([helper.make_node("Flatten", ["x"], ["y"], name="/Flatten")], [vi("x", [1, 2, 2, 2])], [vi("y", [1, 8])], []), [1, 2, 2, 2], False))
    cases.append(("softmax_rows", tiny([helper.make_node("Softmax", ["x"], ["y"], name="/Softmax", axis=1)], [vi("x", [2, 3])], [vi("y", [2, 3])], []), [2, 3], False))
    cases.append(("ir_7", tiny(ir=7), [1, 4], False))
    cases.append(("ir_10", tiny(ir=10), [1, 4], False))
    cases.append(("gather_rows", tiny([helper.make_node("Gather", ["E", "ids"], ["e"], name="/embed/Gather"),
                                       helper.make_node("ReduceMean", ["e"], ["m"], name="/ReduceMean", axes=[1], keepdims=0),
                                       helper.make_node("Gemm", ["m", "W", "b"], ["y"], name="/head/Gemm")],
                                      [vi("ids", [2, 3], I64)], [vi("y", [2, 2])],
                                      [tensor("E", [10, 4], 206, False), tensor("W", [4, 2], 207, True), tensor("b", [2], 208, True)]), [2, 3], True))
    # An older export: weights also listed as graph inputs, doc strings, value_info,
    # metadata, an explicit default data location and the "ai.onnx" domain.
    m = tiny(doc_string="graph doc", value_info=[vi("y", [1, 2])], opsets=(("ai.onnx", 17),))
    m.graph.input.extend([vi("W", [2, 4]), vi("b", [2])])
    m.doc_string, m.producer_version, m.domain, m.model_version = "model doc", "1", "org.example", 3
    helper.set_model_props(m, {"k": "v"})
    m.graph.node[0].doc_string = "node doc"
    m.graph.node[0].attribute[0].doc_string = "attribute doc"
    m.graph.initializer[0].doc_string = "tensor doc"
    m.graph.initializer[0].data_location = TensorProto.DEFAULT
    m.graph.input[0].doc_string = "input doc"
    m.graph.node[0].metadata_props.add(key="k", value="v")
    m.graph.metadata_props.add(key="k", value="v")
    cases.append(("old_export", m, [1, 4], False))
    cases.append(("sixteen_nodes", relu_chain(16), [1, 2], False))
    return cases


def refused():
    """(name, model, status, node)"""
    c = []

    def add(name, m, status, node=-1):
        c.append((name, m, status, node))

    m = tiny(); m.ClearField("ir_version"); add("ir_missing", m, "ir")
    add("ir_11", tiny(ir=11), "ir")
    add("ir_6", tiny(ir=6), "ir")
    add("no_opset", tiny(opsets=()), "opset")
    add("two_opsets", tiny(opsets=(("", 17), ("ai.onnx", 17))), "opset")
    m = tiny(); m.ClearField("graph"); add("no_graph", m, "field")
    m = tiny(); m.training_info.add(); add("training", m, "field")
    m = tiny(); m.graph.quantization_annotation.add(); add("quantization", m, "field")
    add("two_inputs", tiny(inputs=[vi("x", [1, 4]), vi("z", [1, 4])]), "input")
    add("no_input", tiny(inputs=[]), "input")
    add("input_double", tiny(inputs=[vi("x", [1, 4], D)]), "dtype")
    m = tiny(); m.graph.input[0].type.CopyFrom(helper.make_sequence_type_proto(helper.make_tensor_type_proto(F, [1]))); add("input_sequence", m, "input")
    m = tiny(); m.graph.input[0].type.tensor_type.ClearField("shape"); add("input_untyped", m, "shape")
    add("input_rank5", tiny(inputs=[vi("x", [1, 1, 1, 1, 4])]), "capacity")
    add("input_zero", tiny(inputs=[vi("x", [0, 4])]), "shape")
    add("input_large", tiny(inputs=[vi("x", [1, 5000])]), "shape")
    add("input_many", tiny(inputs=[vi("x", [2, 4096])]), "capacity")
    m = tiny(); m.graph.input[0].name = ""; add("input_unnamed", m, "name")
    m = tiny(); m.graph.input[0].type.tensor_type.shape.dim.add(); add("input_empty_dim", m, "shape")
    add("init_zero_dim", tiny(inits=[helper.make_tensor("W", F, [2, 0], []), tensor("b", [2], 202, False)]), "shape")
    m = tiny(); m.graph.initializer[0].dims[0] = -2; add("init_negative_dim", m, "shape")
    add("init_rank5", tiny(inits=[tensor("W", [1, 1, 1, 2, 4], 209, True), tensor("b", [2], 202, False)]), "shape")
    add("init_large", tiny(inits=[tensor("W", [2, 4], 201, True), tensor("b", [2], 202, False), tensor("Z", [4097], 210, True)]), "shape")
    m = tiny(); m.graph.initializer[1].name = ""; add("init_unnamed", m, "name")
    add("init_twice", tiny(inits=[tensor("W", [2, 4], 201, True), tensor("W", [2], 202, False)]), "name")
    m = tiny(); m.graph.initializer[1].int64_data.append(3); add("init_int64_data", m, "data")
    m = tiny(); m.graph.initializer[1].raw_data = struct.pack("<2f", 1, 2); add("init_both", m, "data")
    m = tiny(); m.graph.initializer[1].ClearField("float_data"); add("init_empty", m, "data")
    m = tiny(); m.graph.initializer[1].float_data.append(1.0); add("init_extra", m, "data")
    add("init_many", tiny(inits=[tensor("W", [2, 4], 201, True), tensor("b", [2], 202, False)] + [tensor(f"u{k}", [1], 300 + k, True) for k in range(31)]), "capacity")
    add("node_domain", tiny([helper.make_node("Gemm", ["x", "W", "b"], ["y"], name="/fc/Gemm", domain="com.example", transB=1)]), "opset", 0)
    add("node_four_inputs", tiny([gemm("x", "W", "b", "b", transB=1)]), "op", 0)
    add("node_two_outputs", tiny([helper.make_node("Gemm", ["x", "W", "b"], ["y", "z"], name="/fc/Gemm", transB=1)]), "op", 0)
    add("relu_two_inputs", tiny([helper.make_node("Relu", ["x", "x"], ["y"])], outputs=[vi("y", [1, 4])], inits=[]), "op", 0)
    add("gemm_one_input", tiny([gemm("x")], inits=[]), "op", 0)
    add("gather_one_input", tiny([helper.make_node("Gather", ["W"], ["y"])], inits=[tensor("W", [2, 4], 201, True)]), "op", 0)
    m = tiny(); m.graph.node[0].overload = "v2"; add("node_overload", m, "field", 0)
    add("node_undefined", tiny([gemm("q", "W", "b", transB=1)]), "order", 0)
    add("node_unnamed_output", tiny([helper.make_node("Gemm", ["x", "W", "b"], [""], transB=1)]), "name", 0)
    add("node_output_is_input", tiny([helper.make_node("Gemm", ["x", "W", "b"], ["x"], transB=1)]), "name", 0)
    add("relu_of_ids", tiny([helper.make_node("Relu", ["x"], ["y"])], [vi("x", [1, 4], I64)], [vi("y", [1, 4])], []), "dtype", 0)
    add("gather_float_ids", tiny([helper.make_node("Gather", ["W", "x"], ["y"])], outputs=[vi("y", [1, 4, 4])], inits=[tensor("W", [2, 4], 201, True)]), "dtype", 0)
    add("gather_computed", tiny([helper.make_node("Relu", ["x"], ["r"]), helper.make_node("Gather", ["r", "i"], ["y"])],
                                [vi("x", [2, 4]), ], [vi("y", [1, 4])], []), "role", 1)
    add("gemm_computed_weight", tiny([helper.make_node("Relu", ["x"], ["r"]), gemm("x", "r")], inits=[]), "role", 1)
    add("gemm_weight_input", tiny([gemm("W", "W2")], inits=[tensor("W", [1, 4], 201, True), tensor("W2", [4, 2], 211, True)]), "role", 0)
    add("gather_axis", tiny([helper.make_node("Gather", ["W", "x"], ["y"], axis=1)], [vi("x", [1, 4], I64)], [vi("y", [1, 4, 4])], [tensor("W", [2, 4], 201, True)]), "attribute", 0)

    def red(**a):
        return tiny([helper.make_node("ReduceMean", ["x"], ["y"], **a)], [vi("x", [3, 2])], [vi("y", [1, 2])], [])
    add("reduce_two_axes", red(axes=[0, 1]), "attribute", 0)
    add("reduce_no_axes", red(), "attribute", 0)
    add("reduce_axis_2", red(axes=[2]), "attribute", 0)
    add("reduce_axis_minus3", red(axes=[-3]), "attribute", 0)
    add("reduce_keepdims_2", red(axes=[0], keepdims=2), "attribute", 0)
    add("reduce_five_axes", red(axes=[0, 0, 0, 0, 0]), "attribute", 0)

    def conv(x=(1, 1, 4, 4), w=(2, 1, 3, 3), b=None, out=(1, 2, 2, 2), **a):
        ins = ["x", "W"] + (["b"] if b else [])
        inits = [tensor("W", list(w), 212, True)] + ([tensor("b", list(b), 213, True)] if b else [])
        return tiny([helper.make_node("Conv", ins, ["y"], **a)], [vi("x", list(x))], [vi("y", list(out))], inits)
    add("conv_ok_bias", conv(b=(2,)), "ok")
    add("conv_strides", conv(strides=[2, 2]), "attribute", 0)
    add("conv_strides_one", conv(strides=[1]), "attribute", 0)
    add("conv_dilations", conv(dilations=[1, 2]), "attribute", 0)
    add("conv_pads_three", conv(pads=[0, 0, 0]), "attribute", 0)
    add("conv_group", conv(group=2), "attribute", 0)
    add("conv_tall", conv(x=(1, 1, 2, 5), w=(1, 1, 3, 1), out=(1, 1, 1, 5)), "shape", 0)
    m = conv(); m.graph.node[0].attribute.extend([helper.make_attribute("group", 1) for _ in range(9)]); add("conv_nine_attributes", m, "attribute", 0)
    add("conv_kernel_wrong", conv(kernel_shape=[3, 2]), "attribute", 0)
    add("conv_kernel_wrong_w", conv(kernel_shape=[2, 3]), "attribute", 0)
    add("conv_kernel_three", conv(kernel_shape=[3, 3, 1]), "attribute", 0)
    add("conv_auto_pad", conv(auto_pad="VALID"), "attribute", 0)
    add("conv_rank3", conv(x=(1, 4, 4)), "shape", 0)
    add("conv_weight_rank3", conv(w=(2, 3, 3)), "shape", 0)
    add("conv_weight_rank3_channels", conv(w=(2, 1, 3)), "shape", 0)
    add("conv_channels", conv(x=(1, 2, 4, 4)), "shape", 0)
    add("conv_tall_kernel", conv(x=(1, 1, 2, 4)), "shape", 0)
    add("conv_wide_kernel", conv(x=(1, 1, 4, 2)), "shape", 0)
    add("conv_bias_length", conv(b=(3,)), "shape", 0)
    add("conv_bias_rank2", conv(b=(1, 2)), "shape", 0)
    add("gemm_beta", tiny([gemm("x", "W", "b", transB=1, beta=2.0)]), "attribute", 0)
    add("gemm_trans_b_2", tiny([gemm("x", "W", "b", transB=2)]), "attribute", 0)
    add("gemm_alpha_int", tiny([gemm("x", "W", "b", transB=1, alpha=1)]), "attribute", 0)
    add("gemm_trans_a_0", tiny([gemm("x", "W", "b", transB=1, transA=0)]), "ok")
    add("gemm_a_rank3", tiny(inputs=[vi("x", [1, 4, 1])]), "shape", 0)
    add("gemm_k", tiny(inputs=[vi("x", [1, 3])]), "shape", 0)
    add("gemm_k_untransposed", tiny([gemm("x", "W", "b")]), "shape", 0)
    add("gemm_weight_rank1", tiny(inits=[tensor("W", [4], 201, True), tensor("b", [2], 202, False)]), "shape", 0)
    add("gemm_bias_length", tiny(inits=[tensor("W", [2, 4], 201, True), tensor("b", [3], 202, False)]), "shape", 0)
    add("gemm_bias_rank2", tiny(inits=[tensor("W", [2, 4], 201, True), tensor("b", [1, 2], 202, False)]), "shape", 0)

    def one(op, shape, out, **a):
        return tiny([helper.make_node(op, ["x"], ["y"], **a)], [vi("x", shape)], [vi("y", out)], [])
    add("softmax_axis_0", one("Softmax", [2, 3], [2, 3], axis=0), "attribute", 0)
    add("softmax_axis_last", one("Softmax", [2, 3], [2, 3], axis=1), "ok")
    add("flatten_axis_3", one("Flatten", [2, 3], [6, 1], axis=3), "attribute", 0)
    add("flatten_axis_2", one("Flatten", [2, 3], [6, 1], axis=2), "ok")
    add("flatten_axis_minus3", one("Flatten", [2, 3], [1, 6], axis=-3), "attribute", 0)
    add("flatten_axis_minus2", one("Flatten", [2, 3], [1, 6], axis=-2), "ok")
    add("relu_attribute", one("Relu", [2, 3], [2, 3], alpha=1.0), "attribute", 0)
    add("relu_tensor_attribute", tiny([helper.make_node("Relu", ["x"], ["y"], value=helper.make_tensor("t", F, [1], [1.0]))],
                                      [vi("x", [2])], [vi("y", [2])], []), "attribute", 0)
    add("relu_string_attribute", one("Relu", [2, 3], [2, 3], mode="fast"), "attribute", 0)
    add("gather_data_rank3", tiny([helper.make_node("Gather", ["W", "x"], ["y"])], [vi("x", [1], I64)], [vi("y", [1, 2, 4])], [tensor("W", [2, 2, 4], 201, True)]), "shape", 0)
    add("gather_ids_rank3", tiny([helper.make_node("Gather", ["W", "x"], ["y"])], [vi("x", [1, 1, 1], I64)], [vi("y", [1, 1, 1, 4])], [tensor("W", [2, 4], 201, True)]), "shape", 0)
    add("seventeen_nodes", relu_chain(17), "capacity", 16)
    add("node_output_large", tiny([helper.make_node("Gather", ["W", "x"], ["y"])], [vi("x", [1, 2048], I64)], [vi("y", [1, 2048, 4])], [tensor("W", [2, 4], 201, True)]), "capacity", 0)
    add("unknown_op_two_inputs", tiny([helper.make_node("Add", ["x", "W"], ["y"])], outputs=[vi("y", [1, 4])], inits=[tensor("W", [1, 4], 201, True)]), "op", 0)
    unused = [tensor(f"u{k}", [1], 300 + k, True) for k in range(31)]
    add("values_full_at_node", tiny([helper.make_node("Relu", ["x"], ["y"])], outputs=[vi("y", [1, 4])], inits=list(unused)), "capacity", 0)
    add("values_full_at_input", tiny([helper.make_node("Relu", ["x"], ["y"])], outputs=[vi("y", [1, 4])], inits=unused + [tensor("u31", [1], 331, True)]), "capacity")
    add("output_named_first_weight", tiny([helper.make_node("Gemm", ["x", "b2", "b"], ["W"], transB=1)], inits=[tensor("W", [2, 4], 201, True), tensor("b2", [2, 4], 216, True), tensor("b", [2], 202, False)]), "name", 0)
    add("two_inputs_no_weights", tiny([helper.make_node("Relu", ["x"], ["y"])], [vi("x", [1, 4]), vi("z", [1, 4])], [vi("y", [1, 4])], []), "input")
    add("unused_first", tiny(inits=[tensor("U", [1], 217, True), tensor("W", [2, 4], 201, True), tensor("b", [2], 202, False)]), "unused")
    add("opset_other_17", tiny(opsets=(("com.example", 17),)), "opset")
    add("two_outputs", tiny(outputs=[vi("y", [1, 2]), vi("y", [1, 2])]), "output")
    add("output_is_input", tiny(outputs=[vi("x", [1, 4])]), "output")
    add("output_is_weight", tiny(outputs=[vi("b", [2])]), "output")
    add("output_int64", tiny(outputs=[vi("y", [1, 2], I64)]), "output")
    add("output_unknown", tiny(outputs=[vi("q", [1, 2])]), "output")
    add("output_rank", tiny(outputs=[vi("y", [2])]), "output")
    add("no_output", tiny(outputs=[]), "output")
    add("output_untyped", tiny(outputs=[helper.make_empty_tensor_value_info("y")]), "shape")
    return c


def case_table(arena_note=""):
    import numpy as np
    from onnx.reference import ReferenceEvaluator
    blob, feeds, outs, rows, names = bytearray(), [], [], [], []
    for name, m, shape, integer in accepted():
        data = m.SerializeToString()
        n = math.prod(shape)
        if integer:
            feed = [int(uniform(400 + len(rows), i) * 10) for i in range(n)]
            arr = np.array(feed, dtype=np.int64).reshape(shape)
        else:
            feed = [round((2 * uniform(400 + len(rows), i) - 1) * 32) / 32 for i in range(n)]
            arr = np.array(feed, dtype=np.float32).reshape(shape)
        plain = onnx.ModelProto()
        plain.CopyFrom(m)
        for o in plain.opset_import:
            o.domain = ""   # the evaluator knows the default domain only by its empty name
        got = ReferenceEvaluator(plain).run(None, {m.graph.input[0].name: arr})[0].reshape(-1)
        rows.append(f'    OxCase.{{at={len(blob)},size={len(data)},status=OxStatus.ok,node=-1,feed_at={len(feeds)},feed={n},out_at={len(outs)},out={len(got)}}}, // {name}')
        names.append(name)
        blob += data
        feeds += [float(v) for v in feed]
        outs += [float(v) for v in got]
    for name, m, status, node in refused():
        data = m.SerializeToString()
        rows.append(f'    OxCase.{{at={len(blob)},size={len(data)},status=OxStatus.{status},node={node}}}, // {name}')
        names.append(name)
        blob += data
    text = [f"OX_CASE_BYTES :: [{len(blob)}] u8.[{','.join(str(b) for b in blob)}];\n",
            f"OX_CASE_FEED :: [{len(feeds)}] f64.[{','.join(repr(v) for v in feeds)}];\n",
            f"OX_CASE_OUT :: [{len(outs)}] f64.[{','.join(repr(v) for v in outs)}];\n",
            f"OX_CASES :: [{len(rows)}] OxCase.[\n" + "\n".join(rows) + "\n];\n"]
    text += [f"OX_CASE_{n.upper()} :: {k};\n" for k, n in enumerate(names)]
    return "".join(text)


if __name__ == "__main__":
    main()
