"""Independent oracle for chapter 161a, standard library only.

Trains chapter 70's affine models with chapter 70's own oracle, folds their
scaling into binary32 weights, encodes each ONNX file with its own protobuf
writer, runs the files in emulated binary32, and rebuilds the example's whole
output, which must equal psionic/fixtures/onnx-write.out line for line.
"""
import importlib.util
import math
import pathlib
import struct

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location("tabular_oracle", HERE / "tabular_oracle.py")
tab = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tab)


def f32(v):
    return struct.unpack("<f", struct.pack("<f", v))[0]


def r4(v):
    return math.floor(v * 10000 + 0.5) / 10000


def up9(v):
    return -math.floor(-v * 1000000000) / 1000000000


def coax_sigmoid(z):
    return math.exp((z - abs(z)) / 2) / (1 + math.exp(-abs(z)))


# Protobuf, written the way onnx's serializer writes it.
def var(v):
    v &= (1 << 64) - 1
    out = bytearray()
    while v >= 128:
        out.append(v & 127 | 128)
        v >>= 7
    out.append(v)
    return bytes(out)


def field(number, payload):
    if isinstance(payload, int):
        return var(number * 8) + var(payload)
    if isinstance(payload, str):
        payload = payload.encode()
    return var(number * 8 + 2) + var(len(payload)) + payload


def value_info(name, dims):
    shape = b"".join(field(1, field(1, d)) for d in dims)
    return field(1, name) + field(2, field(1, field(1, 1) + field(2, shape)))


def tensor(name, dims, values):
    return b"".join(field(1, d) for d in dims) + field(2, 1) + field(8, name) + field(9, struct.pack(f"<{len(values)}f", *values))


def model(stem, weight, bias, cols, output):
    gemm_out = "t1" if cols == 2 else output
    nodes = field(1, field(1, "features") + field(1, stem + ".weight") + field(1, stem + ".bias") + field(2, gemm_out)
                  + field(3, f"/{stem}/Gemm") + field(4, "Gemm"))
    if cols == 2:
        nodes += field(1, field(1, "t1") + field(2, output) + field(3, "/Softmax") + field(4, "Softmax")
                       + field(5, field(1, "axis") + field(3, 1) + field(20, 2)))
    graph = (nodes + field(2, "psionic") + field(5, tensor(stem + ".weight", [4, cols], weight))
             + field(5, tensor(stem + ".bias", [cols], bias)) + field(11, value_info("features", [1, 4]))
             + field(12, value_info(output, [1, cols])))
    return field(1, 8) + field(2, "psionic") + field(7, graph) + field(8, field(1, "") + field(2, 17))


def fold(p, mean, sd, logistic):
    w = [p[j] / sd[j] for j in range(4)]
    b = math.fsum([p[4]] + [-(p[j] * mean[j] / sd[j]) for j in range(4)])
    if logistic:
        return [v for j in range(4) for v in (0.0, f32(w[j]))], [0.0, f32(b)]
    return [f32(v) for v in w], [f32(b)]


def run(weight, bias, cols, x):
    """chapter 161's CPU run: binary32 sums in a fixed order, Softmax in binary64."""
    row = [f32(v) for v in x]
    z = []
    for j in range(cols):
        total = 0.0
        for q in range(4):
            total = f32(total + f32(row[q] * weight[q * cols + j]))
        z.append(f32(total + bias[j]))
    if cols == 1:
        return z
    top = max(z)
    total = 0.0
    for v in z:
        total += math.exp(v - top)
    return [f32(math.exp(v - top) / total) for v in z]


def hexdump(name, data):
    lines = [f"{name}.onnx, in hex:"]
    for i in range(0, len(data), 32):
        chunk = data[i:i + 32]
        lines.append("  " + " ".join(chunk[k:k + 4].hex() for k in range(0, len(chunk), 4)))
    return lines


def main():
    x, y, lat, groups = tab.simulate(70, 300)
    train, test = tab.rows_of(groups, 0), tab.rows_of(groups, 2)
    mean, sd = tab.fit_scale(x, train)
    xs = tab.apply_scale(x, mean, sd)
    pg = tab.train(xs, y, train, 4, 0, True, 40, 16, 0.5, 0.0, 1, 2)
    pl = tab.train(xs, lat, train, 4, 0, False, 40, 16, 0.05, 0.0, 1, 2)
    zg, zl = tab.outputs(pg, xs, 4, 0), tab.outputs(pl, xs, 4, 0)
    out = [f"chapter 70's deadline requests: {len(train)} training rows, {len(test)} test rows, 4 raw features",
           f"trained in binary64: logistic test log loss {r4(tab.log_loss(zg, y, test))}, "
           f"line test squared error {r4(tab.mse(zl, lat, test))} s^2"]
    wg, bg = fold(pg, mean, sd, True)
    wl, bl = fold(pl, mean, sd, False)
    deadline = model("deadline", wg, bg, 2, "probs")
    latency = model("latency", wl, bl, 1, "seconds")
    out.append("deadline.onnx: Gemm, then Softmax over [0, z]")
    out.append(f"  {len(deadline)} bytes, 2 nodes; deadline.weight, deadline.bias read back by chapter 161 bit for bit")
    worst, agree = 0.0, 0
    for i in test:
        p = run(wg, bg, 2, x[i])[1]
        want = coax_sigmoid(zg[i])
        worst = max(worst, abs(p - want))
        agree += (p >= 0.5) == (want >= 0.5)
    out.append(f"  {len(test)} test rows: P(on time) within {up9(worst)} of the binary64 model; "
               f"decisions at 0.5 agree on {agree} of {len(test)}")
    out.append("latency.onnx: Gemm alone")
    out.append(f"  {len(latency)} bytes, 1 node; latency.weight, latency.bias read back by chapter 161 bit for bit")
    worst = max(abs(run(wl, bl, 1, x[i])[0] - zl[i]) for i in test)
    out.append(f"  {len(test)} test rows: seconds within {up9(worst)} of the binary64 model")
    out.append("chapter 70's MLP: op, tanh is outside the subset")
    out.append("sample test rows:")
    for k in range(4):
        i = test[120 * k]
        features = " ".join(repr(v) for v in x[i])
        o = run(wg, bg, 2, x[i])
        out.append(f"  row {i}: features {features} -> deadline {o[0]!r} {o[1]!r}")
        out.append(f"  row {i}: features {features} -> latency {run(wl, bl, 1, x[i])[0]!r}")
    out += hexdump("deadline", deadline) + hexdump("latency", latency)
    fixture = (ROOT / "psionic/fixtures/onnx-write.out").read_text().splitlines()
    for n, (a, b) in enumerate(zip(out, fixture)):
        assert a == b, f"line {n + 1}:\n  oracle  {a}\n  fixture {b}"
    assert len(out) == len(fixture), (len(out), len(fixture))
    print(f"onnx-write oracle: both files encoded independently, {len(test)} test rows run in emulated binary32; "
          f"the example's {len(out)} lines are rebuilt line for line")


if __name__ == "__main__":
    main()
