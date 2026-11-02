# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 58's independent checks of Minitron's whole model and long prompts.

Recomputes Llama-3.1-Minitron-4B-Width-Base in float64 from the original BF16
bytes with minitron_reference.py's own RMSNorm, llama3 frequencies,
rotate-half pairs and causal softmax (written from the published
architecture, not from Coax), spreading the matrix-vector products over
worker processes with chapter 81's ministral_run_oracle.py. Tooling only; it
never imports or runs anything from the download.

    python3 -I psionic/tests/minitron_run_oracle.py run MODEL_DIRECTORY RUN_DIRECTORY [JOBS]
        all 32 layers over the run's tokens (the prompt and every emitted
        token but the last), the final norm and the untied head at every
        position from the prompt's last on. RUN_DIRECTORY holds tokens.txt and
        what psionic/tests/minitron_run.c run wrote: emitted.txt, layers.f32
        and logits.f32. Every layer and the last logits must lie within the
        frozen tolerance, and the reference's own greedy choice at every
        position must be the token the Coax run emitted.
    python3 -I psionic/tests/minitron_run_oracle.py long MODEL_DIRECTORY OUT_DIRECTORY [JOBS]
        the slices (embedding gather, layer 0, lm_head rows 0..4095) that
        minitron_run.c slices wrote for a prompt longer than 64 tokens.
"""
import math
import multiprocessing
import operator
import pathlib
import sys
import time
from array import array

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import minitron_reference as ref  # noqa: E402
import ministral_run_oracle as par  # noqa: E402

# Frozen before the first comparison: every layer's output and the last
# logits of the whole-model run may differ from float64 by at most 1e-3 of
# the slice's largest magnitude (Coax keeps f32 values between float64 sums,
# across 32 layers), as for chapter 81. The long-prompt slices keep the
# slices' own 1e-4 (ref.LAYER_TOLERANCE); the gather is exact.
TOLERANCE = 1e-3


def load_array(c, name, first=0, count=None):
    """A BF16 tensor (or rows of it) as a compact float32 array (exact)."""
    shard, start, t = c.where[name]
    assert t["dtype"] == "BF16"
    n = 1
    for v in t["shape"]:
        n *= v
    count = n - first if count is None else count
    with open(c.directory / shard, "rb") as f:
        f.seek(start + t["data_offsets"][0] + 2 * first)
        raw = f.read(2 * count)
    wide = bytearray(4 * count)
    wide[2::4] = raw[0::2]
    wide[3::4] = raw[1::2]
    return array("f", bytes(wide))


ROLES = {"wq": "self_attn.q_proj", "wk": "self_attn.k_proj", "wv": "self_attn.v_proj", "wo": "self_attn.o_proj",
         "w1": "mlp.gate_proj", "w2": "mlp.down_proj", "w3": "mlp.up_proj"}


def load_layer(c, l):
    p = f"model.layers.{l}."
    for k, role in ROLES.items():
        par.WEIGHTS[k] = load_array(c, p + role + ".weight")
    return {"attention_norm": load_array(c, p + "input_layernorm.weight").tolist(),
            "ffn_norm": load_array(c, p + "post_attention_layernorm.weight").tolist()}


def layer(pool, jobs, x_rows, w, freqs):
    D, H, KV, HD, HIDDEN = ref.DIM, ref.HEADS, ref.KV_HEADS, ref.HEAD_DIM, ref.HIDDEN
    T = len(x_rows)
    h = [ref.rms(x, w["attention_norm"]) for x in x_rows]
    qs = par.matvec_all(pool, jobs, "wq", H * HD, D, h)
    ks = par.matvec_all(pool, jobs, "wk", KV * HD, D, h)
    vs = par.matvec_all(pool, jobs, "wv", KV * HD, D, h)
    qs = [ref.rotate(qs[t], H, HD, t, freqs) for t in range(T)]
    ks = [ref.rotate(ks[t], KV, HD, t, freqs) for t in range(T)]
    group = H // KV
    attended = []
    for t in range(T):
        row = []
        for hh in range(H):
            g = hh // group
            q = qs[t][hh * HD:(hh + 1) * HD]
            scores = [math.fsum(map(operator.mul, q, ks[u][g * HD:(g + 1) * HD])) / math.sqrt(HD) for u in range(t + 1)]
            top = max(scores)
            e = [math.exp(s - top) for s in scores]
            z = math.fsum(e)
            for j in range(HD):
                row.append(math.fsum(e[u] / z * vs[u][g * HD + j] for u in range(t + 1)))
        attended.append(row)
    o = par.matvec_all(pool, jobs, "wo", D, H * HD, attended)
    x_rows = [[a + b for a, b in zip(x_rows[t], o[t])] for t in range(T)]
    h = [ref.rms(x, w["ffn_norm"]) for x in x_rows]
    gate = par.matvec_all(pool, jobs, "w1", HIDDEN, D, h)
    up = par.matvec_all(pool, jobs, "w3", HIDDEN, D, h)
    act = [[g / (1 + math.exp(-g)) * u for g, u in zip(gate[t], up[t])] for t in range(T)]
    m = par.matvec_all(pool, jobs, "w2", D, HIDDEN, act)
    return [[a + b for a, b in zip(x_rows[t], m[t])] for t in range(T)]


def compare(what, ref_values, got, tolerance):
    worst = max(abs(a - b) for a, b in zip(ref_values, got))
    scale = max(abs(v) for v in ref_values)
    print(f"{what}: max |difference| {worst:.3e}, largest |value| {scale:.3e}", flush=True)
    return worst <= tolerance * scale


def decode_text(directory, ids):
    vocab, _ = ref.load_tokenizer(directory)
    inverse = {v: k for k, v in ref.byte_unicode().items()}
    pieces = {i: bytes(inverse[ch] for ch in w) for w, i in vocab.items()}
    return b"".join(pieces[t] for t in ids).decode("utf-8", "replace")


def run(directory, out, jobs):
    prompt = [int(v) for v in (out / "tokens.txt").read_text().split()]
    emitted = [int(v) for v in (out / "emitted.txt").read_text().split()]
    tokens = prompt + emitted[:-1]
    T = len(tokens)
    got_layers = ref.read_f32(out / "layers.f32", ref.LAYERS * T * ref.DIM)
    c = ref.Checkpoint(directory)
    began = time.time()
    embed = load_array(c, "model.embed_tokens.weight")
    x = [embed[tok * ref.DIM:(tok + 1) * ref.DIM].tolist() for tok in tokens]
    del embed
    freqs = ref.rope_frequencies(ref.HEAD_DIM)
    ok = True
    ctx = multiprocessing.get_context("fork")
    for l in range(ref.LAYERS):
        w = load_layer(c, l)
        with ctx.Pool(jobs) as pool:
            x = layer(pool, jobs, x, w, freqs)
        flat = [v for row in x for v in row]
        ok = compare(f"layer {l:2d}: {T} positions", flat, got_layers[l * T * ref.DIM:(l + 1) * T * ref.DIM], TOLERANCE) and ok
    for k in ROLES:
        par.WEIGHTS.pop(k, None)
    norm = load_array(c, "model.norm.weight").tolist()
    par.WEIGHTS["head"] = load_array(c, "lm_head.weight")
    first = len(prompt) - 1
    finals = [ref.rms(x[t], norm) for t in range(first, T)]
    with ctx.Pool(jobs) as pool:
        logits = par.matvec_all(pool, jobs, "head", ref.VOCAB, ref.DIM, finals)
    ok = compare("last logits", logits[-1], ref.read_f32(out / "logits.f32", ref.VOCAB), TOLERANCE) and ok
    choices = [max(range(ref.VOCAB), key=lambda i: row[i]) for row in logits]
    same = choices == emitted
    print(f"reference greedy at positions {first}..{T - 1}: {choices} ({'the tokens the Coax run emitted' if same else 'DIFFERENT from ' + str(emitted)})")
    print(f"emitted text: {decode_text(directory, emitted)!r}")
    print(f"reference: {time.time() - began:.0f} s on {jobs} processes")
    return 0 if ok and same else 1


def long(directory, out, jobs):
    toks = [int(v) for v in (out / "tokens.txt").read_text().split()]
    T = len(toks)
    c = ref.Checkpoint(directory)
    began = time.time()
    embed = load_array(c, "model.embed_tokens.weight")
    x = [embed[tok * ref.DIM:(tok + 1) * ref.DIM].tolist() for tok in toks]
    del embed
    if ref.read_f32(out / "embed.f32", T * ref.DIM) != [v for row in x for v in row]:
        print("embedding slice differs", file=sys.stderr)
        return 1
    print(f"embed: {T} tokens, exact")
    ctx = multiprocessing.get_context("fork")
    w = load_layer(c, 0)
    with ctx.Pool(jobs) as pool:
        y = layer(pool, jobs, x, w, ref.rope_frequencies(ref.HEAD_DIM))
    got = ref.read_f32(out / "layer0.f32", T * ref.DIM)
    ok = compare(f"layer 0: {T} tokens (positions 0..{T - 1})", [v for row in y for v in row], got, ref.LAYER_TOLERANCE)
    n = ref.rms(got[(T - 1) * ref.DIM:], load_array(c, "model.norm.weight").tolist())
    head = load_array(c, "lm_head.weight", 0, ref.READOUT_ROWS * ref.DIM)
    want = [math.fsum(a * b for a, b in zip(head[r * ref.DIM:(r + 1) * ref.DIM], n)) for r in range(ref.READOUT_ROWS)]
    ok = compare(f"readout rows 0..{ref.READOUT_ROWS} of lm_head at position {T - 1}", want,
                 ref.read_f32(out / "readout.f32", ref.READOUT_ROWS), ref.LAYER_TOLERANCE) and ok
    print(f"reference: {time.time() - began:.0f} s on {jobs} processes")
    return 0 if ok else 1


def main():
    a = sys.argv[1:]
    if len(a) not in (3, 4) or a[0] not in ("run", "long"):
        print("usage: python3 -I psionic/tests/minitron_run_oracle.py run|long MODEL_DIRECTORY OUT_DIRECTORY [JOBS]", file=sys.stderr)
        return 2
    jobs = int(a[3]) if len(a) == 4 else 20
    directory, out = pathlib.Path(a[1]), pathlib.Path(a[2])
    return run(directory, out, jobs) if a[0] == "run" else long(directory, out, jobs)


if __name__ == "__main__":
    sys.exit(main())
