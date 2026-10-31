#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent oracle for chapters 226 and 227: gadget results in binary64 and
binary32, and seeded gate graphs with their integer values.

Written from the chapters' descriptions, not translated from the Coax units.
Tooling only: `--header` prints a C header for psionic/tests/gate.c.
"""
import random
import struct
import sys

VALUE_MAX = 2 ** 26
OPS = ["input", "add", "sub", "scale", "mul", "relu", "step", "eq0", "select", "constant"]
GADGETS = ["mul", "relu", "step", "eq0", "select", "copy"]


def f32(x):
    return struct.unpack("<f", struct.pack("<f", float(x)))[0]


def mul_single(a, b):
    """a*b through the two-unit product with every operation rounded to binary32:
    out = (0 + 1*(a*ReLU(b))) + (-1)*(a*ReLU(-b)), each step rounded."""
    h0 = f32(f32(a) * max(f32(b), 0.0))
    h1 = f32(f32(a) * max(f32(-b), 0.0))
    return f32(f32(0.0 + h0) - h1)


def gadget_cases():
    rng = random.Random(167)
    cases = []
    edges = [0, 1, -1, 4095, 4096, 4097, 8191, 2 ** 20, VALUE_MAX - 1, VALUE_MAX, -VALUE_MAX]
    for a in edges:
        for b in edges:
            cases.append(("mul", a, b, 0, False, float(a * b)))
            cases.append(("mul", a, b, 0, True, mul_single(a, b)))
    for _ in range(200):
        g = rng.choice(GADGETS)
        a, b, c = (rng.randint(-1000, 1000) for _ in range(3))
        if g == "select":
            a = rng.randint(0, 1)
        want = {"mul": a * b, "relu": max(a, 0), "step": 1 if a >= 1 else 0, "eq0": 1 if a == 0 else 0,
                "select": b if a == 1 else c, "copy": a}[g]
        cases.append((g, a, b, c, False, float(want)))
    return cases


def evaluate(nodes, inputs):
    """('ok', values) or ('range', node index)."""
    v = []
    for i, (op, a, b, c, k) in enumerate(nodes):
        if op == "input":
            x = inputs[k]
        elif op == "add":
            x = v[a] + v[b]
        elif op == "sub":
            x = v[a] - v[b]
        elif op == "scale":
            x = v[a] * k
        elif op == "mul":
            x = v[a] * v[b]
        elif op == "constant":
            x = k
        elif op == "relu":
            x = max(v[a], 0)
        elif op == "step":
            x = 1 if v[a] >= 1 else 0
        elif op == "eq0":
            x = 1 if v[a] == 0 else 0
        else:
            x = v[a] if v[c] == 1 else v[b]
        if abs(x) > VALUE_MAX:
            return "range", i
        v.append(x)
    return "ok", v


def first_long_form(nodes, limit=8):
    """The first node whose form needs more than `limit` distinct terms, or None.

    A form maps base nodes (inputs, constants and gates, each its own base) to
    coefficients; like terms merge and cancelled terms vanish. Gadgets build
    forms of their own: step and equality add a constant term to their operand,
    and select forms a - b. A sum that needs more than `limit` distinct terms
    while it is being built is refused, even if later terms would cancel."""
    forms = []

    def plus(a, b):
        r = dict(a)
        for key, coef in b.items():
            if key in r:
                r[key] += coef
            elif len(r) == limit:
                return None
            else:
                r[key] = coef
        return {key: coef for key, coef in r.items() if coef != 0}

    for i, (op, a, b, c, k) in enumerate(nodes):
        if op in ("add", "sub"):
            f = plus(forms[a], forms[b] if op == "add" else {key: -v for key, v in forms[b].items()})
        elif op == "scale":
            f = {key: v * k for key, v in forms[a].items()}
        elif op == "constant":
            f = {i: k}
        else:
            f = {i: 1}
        if f is None:
            return i
        if op in ("step", "eq0") and len(forms[a]) >= limit:
            return i
        if op == "select" and plus(forms[a], {key: -v for key, v in forms[b].items()}) is None:
            return i
        forms.append(f)
    return None


def random_graph(rng):
    n_inputs = rng.randint(1, 4)
    nodes = [("input", 0, 0, 0, k) for k in range(n_inputs)]
    bits = []
    for _ in range(rng.randint(1, 18)):
        i = len(nodes)
        op = rng.choice(["add", "sub", "scale", "mul", "relu", "step", "eq0", "select", "select", "constant"])
        if op == "select" and not bits:
            op = "step"
        a, b = rng.randrange(i), rng.randrange(i)
        c = rng.choice(bits) if op == "select" else 0
        k = rng.choice([-3, -1, 2, 5]) if op == "scale" else rng.randint(-9, 9) if op == "constant" else 0
        nodes.append((op, a, b, c, k))
        if op in ("step", "eq0"):
            bits.append(i)
    outputs = sorted(rng.sample(range(len(nodes)), min(len(nodes), rng.randint(1, 3))))
    return nodes, outputs


def long_form_graphs():
    """Graphs that reach the term limit on purpose: a nine-term sum, a step on a
    full eight-term form, and a select whose a - b needs nine terms."""
    def summed(n):
        nodes = [("input", 0, 0, 0, k) for k in range(n)] + [("add", 0, 1, 0, 0)]
        for i in range(2, n):
            nodes.append(("add", len(nodes) - 1, i, 0, 0))
        return nodes  # the last node sums x0..x(n-1)
    nine = summed(9)  # node 16
    eight = summed(8)  # node 14
    step = eight + [("step", 14, 0, 0, 0)]
    select = eight + [("relu", 0, 0, 0, 0), ("step", 0, 0, 0, 0), ("select", 14, 15, 16, 0)]
    return [(nine, [16]), (step, [15]), (select, [17])]


def graph_cases():
    rng = random.Random(168)
    cases = []
    for nodes, outputs in long_form_graphs():
        n_inputs = sum(1 for n in nodes if n[0] == "input")
        inputs = [rng.randint(-9, 9) for _ in range(n_inputs)]
        cases.append((nodes, outputs, inputs, "terms", [first_long_form(nodes)]))
    while len(cases) < 303:
        nodes, outputs = random_graph(rng)
        n_inputs = sum(1 for n in nodes if n[0] == "input")
        scale = rng.choice([3, 50, 3000, 2 ** 13])
        inputs = [rng.randint(-scale, scale) for _ in range(n_inputs)]
        status, result = evaluate(nodes, inputs)
        long = first_long_form(nodes)
        if status != "ok":
            cases.append((nodes, outputs, inputs, "range", [result]))
        elif long is not None:
            cases.append((nodes, outputs, inputs, "terms", [long]))
        else:
            cases.append((nodes, outputs, inputs, "ok", [result[o] for o in outputs]))
    return cases


def nxt(x):
    return (x * 1103515245 + 12345) % 2147483648


def harness_graph(seed, g):
    """Chapter 228's generator, from its description: one to three inputs, then
    gates and linear nodes over earlier nodes, at most 16 nodes in all."""
    x = nxt((seed % 65536) * 65537 + g % 65536)
    x = nxt(x)
    n_inputs = 1 + x % 3
    nodes = [("input", 0, 0, 0, i) for i in range(n_inputs)]
    x = nxt(x)
    total = n_inputs + 4 + x % (16 - n_inputs - 3)
    bits = []
    while len(nodes) < total:
        count = len(nodes)
        x = nxt(x)
        pick = x % 12
        x = nxt(x)
        a = x % count
        x = nxt(x)
        b = x % count
        n = ("relu", a, 0, 0, 0)
        if pick < 2:
            n = ("mul", a, b, 0, 0)
        elif pick < 4:
            n = ("step", a, 0, 0, 0)
        elif pick < 5:
            n = ("eq0", a, 0, 0, 0)
        elif pick < 7 and bits:
            x = nxt(x)
            n = ("select", a, b, bits[x % len(bits)], 0)
        elif pick < 8:
            n = ("sub", a, b, 0, 0)
        elif pick < 9:
            n = ("constant", 0, 0, 0, x % 7 - 3)
        nodes.append(n)
        if n[0] in ("step", "eq0"):
            bits.append(count)
    return nodes


def harness_case(seed, g):
    nodes = harness_graph(seed, g)
    x = nxt(seed * 7919 + g)
    values = []
    for n in nodes:
        if n[0] == "input":
            x = nxt(x)
            values.append(x % 201 - 100)
    x = nxt(x)
    return nodes, values, [x % (len(nodes) - 1), len(nodes) - 1]


def harness_skipped(seed, count):
    """Cases the harness cannot compare: a value out of range, or a form too long."""
    skipped = 0
    for g in range(count):
        nodes, values, _ = harness_case(seed, g)
        if evaluate(nodes, values)[0] != "ok" or first_long_form(nodes) is not None:
            skipped += 1
    return skipped


HARNESS_PINS = [(1, 0), (1, 1), (1, 7), (2, 0), (3, 694), (7, 0), (7, 1), (8, 0), (20, 999), (65535, 3),
                (65536, 0), (65537, 2), (1, 65536), (2, 65538)]


def harness_coax():
    lines = ["// Generated by psionic/tests/gate_oracle.py --harness; chapter 228's cases from its description.",
             "HV_SKIPPED_SEED1_40 :: %d;" % harness_skipped(1, 40),
             "HV_SKIPPED_SEED11_40 :: %d;" % harness_skipped(11, 40)]
    for seed, g in HARNESS_PINS:
        nodes, values, outputs = harness_case(seed, g)
        lines.append('#test "independent harness case seed %d index %d" {' % (seed, g))
        # Start from nodes that are not inputs, so a skipped write cannot pass as one.
        lines.append("    nodes:[16]GgNode;for i:0..<16 {nodes[i]=GgNode.{op=GgOp.relu,a=5,k=9};}")
        lines.append("    c:=ch_case(%d,%d,nodes[..]);" % (seed, g))
        lines.append("    #assert c.count==%d && c.inputs==%d && c.outputs[0]==%d && c.outputs[1]==%d;" % (
            len(nodes), len(values), outputs[0], outputs[1]))
        for i, v in enumerate(values):
            lines.append("    #assert c.values[%d]==%d;" % (i, v))
        for i, (op, a, b, c, k) in enumerate(nodes):
            lines.append("    #assert nodes[%d].op==GgOp.%s && nodes[%d].a==%d && nodes[%d].b==%d && nodes[%d].c==%d && nodes[%d].k==%d;" % (
                i, op, i, a, i, b, i, c, i, k))
        lines.append("}")
    return "\n".join(lines) + "\n"


def harness_header():
    lines = ["/* Generated by psionic/tests/gate_oracle.py --harness-header; do not edit. */",
             "#include <stdint.h>",
             "typedef struct { int seed, graph, count, inputs; int op[16], a[16], b[16], c[16]; int64_t k[16];",
             "  int64_t values[3]; int outputs[2]; } harness_case;",
             "static const harness_case harness_cases[] = {"]
    for seed in range(1, 4):
        for g in range(300):
            nodes, values, outputs = harness_case(seed, g)
            col = lambda j: ",".join(str(n[j]) for n in nodes)
            lines.append("  {%d, %d, %d, %d, {%s}, {%s}, {%s}, {%s}, {%s}, {%s}, {%d, %d}}," % (
                seed, g, len(nodes), len(values), ",".join(str(OPS.index(n[0])) for n in nodes),
                col(1), col(2), col(3), ",".join("%dLL" % n[4] for n in nodes),
                ",".join("%dLL" % v for v in values), outputs[0], outputs[1]))
    lines.append("};")
    lines.append("static const int harness_skipped[21] = {0%s};" % "".join(
        ", %d" % harness_skipped(seed, 1000) for seed in range(1, 21)))
    return "\n".join(lines)


def header():
    lines = ["/* Generated by psionic/tests/gate_oracle.py; do not edit. */",
             "#include <stdint.h>",
             "typedef struct { int gadget; int64_t a, b, c; int single; double want; } gadget_case;",
             "static const gadget_case gadget_cases[] = {"]
    for g, a, b, c, single, want in gadget_cases():
        lines.append("  {%d, %dLL, %dLL, %dLL, %d, %r}," % (GADGETS.index(g), a, b, c, int(single), want))
    lines.append("};")
    lines.append("typedef struct { int n; int op[24]; int a[24]; int b[24]; int c[24]; int64_t k[24];")
    lines.append("  int outputs; int out[3]; int inputs; int64_t in[9]; int ok; int64_t want[3]; } graph_case;")
    lines.append("static const graph_case graph_cases[] = {")
    for nodes, outputs, inputs, status, want in graph_cases():
        col = lambda j: ",".join(str(n[j]) for n in nodes)
        lines.append("  {%d, {%s}, {%s}, {%s}, {%s}, {%s}, %d, {%s}, %d, {%s}, %d, {%s}}," % (
            len(nodes), ",".join(str(OPS.index(n[0])) for n in nodes), col(1), col(2), col(3),
            ",".join("%dLL" % n[4] for n in nodes), len(outputs), ",".join(map(str, outputs)),
            len(inputs), ",".join("%dLL" % x for x in inputs), {"range": 0, "ok": 1, "terms": 2}[status],
            ",".join("%dLL" % x for x in want)))
    lines.append("};")
    return "\n".join(lines)


def main():
    if sys.argv[1:] == ["--header"]:
        print(header())
        return
    if sys.argv[1:] == ["--harness"]:
        sys.stdout.write(harness_coax())
        return
    if sys.argv[1:] == ["--harness-header"]:
        print(harness_header())
        return
    graphs = graph_cases()
    count = lambda name: sum(1 for g in graphs if g[3] == name)
    print("gate oracle: %d gadget cases; %d graphs (%d compile, %d out of range, %d over the term limit)" % (
        len(gadget_cases()), len(graphs), count("ok"), count("range"), count("terms")))


if __name__ == "__main__":
    main()
