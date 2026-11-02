# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 81's independent check of the recorded whole-model run.

Recomputes Ministral 3 3B over the run's tokens (the prompt and every emitted
token but the last) in float64 from the original BF16 bytes: all 26 layers,
the final norm and the tied readout at every position from the prompt's last
on. It uses ministral_reference.py's own RMSNorm, pair rotation, YaRN
frequencies and causal softmax (written from the published architecture, not
from Coax) and spreads the matrix-vector products over worker processes.
Tooling only; it never imports or runs anything from the download.

    python3 -I psionic/tests/ministral_run_oracle.py MODEL_DIRECTORY RUN_DIRECTORY [JOBS]

RUN_DIRECTORY holds tokens.txt and what psionic/tests/ministral_run.c wrote:
emitted.txt, layers.f32 (each layer's output over every position) and
logits.f32 (the last position's logits). Each layer and the logits must lie
within the frozen tolerance, and the reference's own greedy choice at every
position must be the token the Coax run emitted.
"""
import math
import multiprocessing
import operator
import pathlib
import sys
import time
from array import array

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import ministral_reference as ref  # noqa: E402

# Frozen before the first comparison: every layer's output and the last
# logits may differ from float64 by at most 1e-3 of the slice's largest
# magnitude (Coax keeps f32 values between float64 sums, across 26 layers).
TOLERANCE = 1e-3

WEIGHTS = {}


def load_array(directory, start, h, name):
    """A whole BF16 tensor as a compact float32 array (exact)."""
    t = h[name]
    assert t["dtype"] == "BF16"
    a, b = t["data_offsets"]
    with open(pathlib.Path(directory) / "consolidated.safetensors", "rb") as f:
        f.seek(start + a)
        raw = f.read(b - a)
    wide = bytearray(2 * len(raw))
    wide[2::4] = raw[0::2]
    wide[3::4] = raw[1::2]
    return array("f", bytes(wide))


def rows_job(task):
    name, cols, first, last, xs = task
    w = WEIGHTS[name]
    out = []
    for r in range(first, last):
        row = w[r * cols:(r + 1) * cols].tolist()
        out.append([math.fsum(map(operator.mul, row, x)) for x in xs])
    return out


def matvec_all(pool, jobs, name, rows, cols, xs):
    """W x for every x in xs: one list of `rows` values per x."""
    parts = 4 * jobs
    bounds = [rows * i // parts for i in range(parts + 1)]
    tasks = [(name, cols, bounds[i], bounds[i + 1], xs) for i in range(parts) if bounds[i] < bounds[i + 1]]
    by_row = [v for chunk in pool.map(rows_job, tasks) for v in chunk]
    return [[by_row[r][t] for r in range(rows)] for t in range(len(xs))]


def layer(pool, jobs, x_rows, w, freqs):
    D, H, KV, HD, HIDDEN = ref.DIM, ref.HEADS, ref.KV_HEADS, ref.HEAD_DIM, ref.HIDDEN
    T = len(x_rows)
    h = [ref.rms(x, w["attention_norm"]) for x in x_rows]
    qs = matvec_all(pool, jobs, "wq", H * HD, D, h)
    ks = matvec_all(pool, jobs, "wk", KV * HD, D, h)
    vs = matvec_all(pool, jobs, "wv", KV * HD, D, h)
    qs = [[v * ref.query_scale(t) for v in ref.rotate(qs[t], H, HD, t, freqs)] for t in range(T)]
    ks = [ref.rotate(ks[t], KV, HD, t, freqs) for t in range(T)]
    group = H // KV
    attended = []
    for t in range(T):
        a = []
        for hh in range(H):
            g = hh // group
            q = qs[t][hh * HD:(hh + 1) * HD]
            scores = [math.fsum(map(operator.mul, q, ks[u][g * HD:(g + 1) * HD])) / math.sqrt(HD) for u in range(t + 1)]
            top = max(scores)
            e = [math.exp(s - top) for s in scores]
            z = math.fsum(e)
            for j in range(HD):
                a.append(math.fsum(e[u] / z * vs[u][g * HD + j] for u in range(t + 1)))
        attended.append(a)
    o = matvec_all(pool, jobs, "wo", D, H * HD, attended)
    x_rows = [[a + b for a, b in zip(x_rows[t], o[t])] for t in range(T)]
    h = [ref.rms(x, w["ffn_norm"]) for x in x_rows]
    gate = matvec_all(pool, jobs, "w1", HIDDEN, D, h)
    up = matvec_all(pool, jobs, "w3", HIDDEN, D, h)
    act = [[g / (1 + math.exp(-g)) * u for g, u in zip(gate[t], up[t])] for t in range(T)]
    m = matvec_all(pool, jobs, "w2", D, HIDDEN, act)
    return [[a + b for a, b in zip(x_rows[t], m[t])] for t in range(T)]


def compare(what, ref_values, got):
    worst = max(abs(a - b) for a, b in zip(ref_values, got))
    scale = max(abs(v) for v in ref_values)
    print(f"{what}: max |difference| {worst:.3e}, largest |value| {scale:.3e}", flush=True)
    return worst <= TOLERANCE * scale


def main():
    if len(sys.argv) not in (3, 4):
        print("usage: python3 -I psionic/tests/ministral_run_oracle.py MODEL_DIRECTORY RUN_DIRECTORY [JOBS]", file=sys.stderr)
        return 2
    directory, run = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    jobs = int(sys.argv[3]) if len(sys.argv) == 4 else 20
    prompt = [int(v) for v in (run / "tokens.txt").read_text().split()]
    emitted = [int(v) for v in (run / "emitted.txt").read_text().split()]
    tokens = prompt + emitted[:-1]
    T = len(tokens)
    got_layers = ref.read_f32(run / "layers.f32", ref.LAYERS * T * ref.DIM)
    start, h = ref.header(directory)
    began = time.time()
    WEIGHTS["embed"] = load_array(directory, start, h, "tok_embeddings.weight")
    x = [WEIGHTS["embed"][tok * ref.DIM:(tok + 1) * ref.DIM].tolist() for tok in tokens]
    freqs = ref.rope_frequencies(ref.HEAD_DIM)
    ok = True
    ctx = multiprocessing.get_context("fork")
    for l in range(ref.LAYERS):
        w = {}
        for k in ("wq", "wk", "wv", "wo"):
            WEIGHTS[k] = load_array(directory, start, h, f"layers.{l}.attention.{k}.weight")
        for k in ("w1", "w2", "w3"):
            WEIGHTS[k] = load_array(directory, start, h, f"layers.{l}.feed_forward.{k}.weight")
        w["attention_norm"] = load_array(directory, start, h, f"layers.{l}.attention_norm.weight").tolist()
        w["ffn_norm"] = load_array(directory, start, h, f"layers.{l}.ffn_norm.weight").tolist()
        with ctx.Pool(jobs) as pool:
            x = layer(pool, jobs, x, w, freqs)
        flat = [v for row in x for v in row]
        ok = compare(f"layer {l:2d}: {T} positions", flat, got_layers[l * T * ref.DIM:(l + 1) * T * ref.DIM]) and ok
    norm = load_array(directory, start, h, "norm.weight").tolist()
    first = len(prompt) - 1
    finals = [ref.rms(x[t], norm) for t in range(first, T)]
    with ctx.Pool(jobs) as pool:
        logits = matvec_all(pool, jobs, "embed", ref.VOCAB, ref.DIM, finals)
    ok = compare("last logits", logits[-1], ref.read_f32(run / "logits.f32", ref.VOCAB)) and ok
    choices = [max(range(ref.VOCAB), key=lambda i: row[i]) for row in logits]
    same = choices == emitted
    print(f"reference greedy at positions {first}..{T - 1}: {choices} ({'the tokens the Coax run emitted' if same else 'DIFFERENT from ' + str(emitted)})")
    ranks = ref.load_tekken(directory)
    pieces = {r + ref.SPECIALS: b for b, r in ranks.items()}
    text = b"".join(pieces.get(t, b"") for t in emitted).decode("utf-8", "replace")
    print(f"emitted text: {text!r}")
    print(f"reference: {time.time() - began:.0f} s on {jobs} processes")
    return 0 if ok and same else 1


if __name__ == "__main__":
    sys.exit(main())
