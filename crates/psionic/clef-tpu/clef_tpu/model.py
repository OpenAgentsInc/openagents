"""Clef backbone (Qwen3.5 hybrid) and joint schema head in JAX, for Cloud TPU.

The backbone runs a chunked prefill: each call consumes one fixed-size chunk
of token embeddings and carries the DeltaNet state, the causal-conv tail and
the full-attention K/V cache, so XLA compiles one program per (chunk, cache
bucket) shape. The head runs once per request on padded question/option
buckets with masks.

Numerics: weights are the release BF16 tensors. Matmuls take bf16 (or f32
with `precision="f32"`) inputs and accumulate in f32; the residual stream,
norms, the delta rule and the head stay in f32.
"""

from __future__ import annotations

import functools
import os
import json
import math
from dataclasses import dataclass
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np

from . import delta


@dataclass(frozen=True)
class Config:
    hidden: int
    ffn: int
    layers: int
    layer_types: tuple[str, ...]
    heads: int
    kv_heads: int
    head_dim: int
    rotary_dim: int
    rope_theta: float
    lin_k_heads: int
    lin_v_heads: int
    lin_k_dim: int
    lin_v_dim: int
    conv_kernel: int
    eps: float
    vocab: int

    @staticmethod
    def load(path: Path) -> "Config":
        raw = json.loads((path / "config.json").read_text())
        t = raw.get("text_config", raw)
        rope = t.get("rope_parameters") or t.get("rope_scaling") or {}
        theta = rope.get("rope_theta", t.get("rope_theta", 1e7))
        partial = rope.get("partial_rotary_factor", t.get("partial_rotary_factor", 0.25))
        head_dim = t.get("head_dim") or t["hidden_size"] // t["num_attention_heads"]
        n = t["num_hidden_layers"]
        types = t.get("layer_types") or [
            "full_attention" if (i + 1) % t.get("full_attention_interval", 4) == 0 else "linear_attention"
            for i in range(n)
        ]
        return Config(
            hidden=t["hidden_size"], ffn=t["intermediate_size"], layers=n, layer_types=tuple(types),
            heads=t["num_attention_heads"], kv_heads=t["num_key_value_heads"], head_dim=head_dim,
            rotary_dim=int(head_dim * partial), rope_theta=float(theta),
            lin_k_heads=t["linear_num_key_heads"], lin_v_heads=t["linear_num_value_heads"],
            lin_k_dim=t["linear_key_head_dim"], lin_v_dim=t["linear_value_head_dim"],
            conv_kernel=t["linear_conv_kernel_dim"], eps=t["rms_norm_eps"], vocab=t["vocab_size"],
        )


# ------------------------------------------------------------------ weights

def _open_all(path: Path):
    from safetensors import safe_open

    index = json.loads((path / "model.safetensors.index.json").read_text())["weight_map"]
    files = {}
    for name, f in index.items():
        files.setdefault(f, []).append(name)
    for f, names in files.items():
        with safe_open(str(path / f), framework="numpy") as h:
            for name in names:
                yield name, h


def hadamard(n):
    """Normalized Sylvester Hadamard matrix (n a power of two): orthogonal and symmetric."""
    h = np.ones((1, 1), np.float32)
    while h.shape[0] < n:
        h = np.block([[h, h], [h, -h]])
    return h / np.sqrt(n)


# int8 projections rotate their input by a block Hadamard first (CLEF_TPU_INT8_ROT:
# block size, 0 = off; 4096 is applied as H64 x H64): an orthogonal rotation
# spreads activation outliers over the block, so per-token int8 loses less,
# and the weight takes H^T W offline, which leaves x W unchanged
ROT = int(os.environ.get("CLEF_TPU_INT8_ROT", "0"))


def rotate(x, r):
    """x [..., K] -> x (I_{K/r} (x) H_r), the block Hadamard rotation of the last axis."""
    K = x.shape[-1]
    lead = x.shape[:-1]
    hp = jax.lax.Precision.HIGH
    if r == 4096:
        h = jnp.asarray(hadamard(64))
        y = x.reshape(lead + (K // 4096, 64, 64))
        y = jnp.einsum("...ij,jk->...ik", y, h, precision=hp)
        y = jnp.einsum("ki,...ij->...kj", h, y, precision=hp)
        return y.reshape(lead + (K,))
    h = jnp.asarray(hadamard(r))
    return jnp.einsum("...ij,jk->...ik", x.reshape(lead + (K // r, r)), h, precision=hp).reshape(lead + (K,))


def smooth_scale(w, act_max, alpha):
    """SmoothQuant per-input-channel scale s_j = act_max_j^alpha / w_max_j^(1-alpha) of a [in, out] weight."""
    a = np.maximum(np.asarray(act_max, np.float32), 1e-5)
    wm = np.maximum(np.abs(np.asarray(w, np.float32)).max(1), 1e-5)
    return np.clip(a ** alpha / wm ** (1.0 - alpha), 1e-4, 1e4)


def quantize_int8(w, act_max=None, alpha=0.5, rot=0):
    """Per-output-channel symmetric int8 of a [in, out] weight: (q int8, scale f32 [out]).

    With `act_max` (the calibrated per-input-channel |x| maximum of the
    activation this weight multiplies), SmoothQuant migration: each input
    channel j is scaled by s_j = act_max_j^alpha / w_max_j^(1-alpha), the
    weight row by s_j and the activation by 1/s_j (`inv`, applied before the
    activation is quantized), so activation outliers move into the weights,
    which are quantized per output channel."""
    w = np.asarray(w, np.float32)
    out = {}
    if rot:
        # x W = (x H)(H^T W), H symmetric: rotate the weight's rows the same way
        w = np.asarray(rotate(jnp.asarray(w.T), rot)).T
        act_max = None
    if act_max is not None:
        sm = smooth_scale(w, act_max, alpha)
        w = w * sm[:, None]
        out["inv"] = jnp.asarray(1.0 / sm, jnp.float32)
    scale = np.maximum(np.abs(w).max(0), 1e-12) / 127.0
    q = np.clip(np.rint(w / scale), -127, 127).astype(np.int8)
    out.update(q=jnp.asarray(q), s=jnp.asarray(scale, jnp.float32))
    if os.environ.get("CLEF_TPU_INT8_DIAG") == "a8w16":
        out["wf"] = jnp.asarray(w, jnp.bfloat16)
    return out


def load_backbone(path: Path, cfg: Config, quant: str | None = None, calib: str | None = None):
    """Returns (device params, host embed table, host lm_head table).
    `quant="int8"` stores the layer projections as per-channel int8 (our own
    int8 artifact, built from the BF16 release at load). `calib` is a
    calibration map (`calibrate.py`: per layer and projection, the input
    channels' |x| maxima); with it the int8 projections are SmoothQuant-scaled
    (CLEF_TPU_SMOOTH_ALPHA, default 0.5)."""
    import ml_dtypes  # noqa: F401  (bf16 numpy dtype for safetensors)
    from safetensors import safe_open

    index = json.loads((path / "model.safetensors.index.json").read_text())["weight_map"]
    prefix = None
    for name in index:
        if name.endswith("layers.0.input_layernorm.weight") and "visual" not in name:
            prefix = name[: -len("layers.0.input_layernorm.weight")]
    assert prefix is not None, "no text layers found"

    def get(name):
        f = index[name]
        with safe_open(str(path / f), framework="numpy") as h:
            return h.get_tensor(name)

    bf = lambda a: jnp.asarray(a, jnp.bfloat16)
    # which projections go int8 (CLEF_TPU_INT8: comma-separated names; the
    # ones fed by a normed hidden row have the fewest activation outliers)
    int8_names = set(os.environ.get("CLEF_TPU_INT8", "gate_up,in_qkv,in_z,qkv").split(","))
    # layers kept in bf16 whatever the projection (CLEF_TPU_INT8_SKIP_LAYERS: comma-separated indexes)
    skip_layers = {int(x) for x in os.environ.get("CLEF_TPU_INT8_SKIP_LAYERS", "").split(",") if x}
    alpha = float(os.environ.get("CLEF_TPU_SMOOTH_ALPHA", "0.5"))
    stats = dict(np.load(calib)) if calib else {}
    # a prebuilt int8 artifact (gptq.py): per layer, <name>.q int8 [K, N] and <name>.s [N]
    art = os.environ.get("CLEF_TPU_INT8_ARTIFACT")
    if art and quant == "int8":
        man = json.loads((Path(art) / "manifest.json").read_text())
        assert man["rot"] == ROT, f"artifact built with CLEF_TPU_INT8_ROT={man['rot']}, running with {ROT}"
    cur = {"layer": 0}
    def dev(a, name=None):
        i = cur["layer"]
        if quant == "int8" and art and name in int8_names and i not in skip_layers:
            if "art" not in cur or cur["art"][0] != i:
                cur["art"] = (i, dict(np.load(Path(art) / f"layer{i}.npz")))
            z = cur["art"][1]
            d = {"q": jnp.asarray(z[name + ".q"]), "s": jnp.asarray(z[name + ".s"], jnp.float32)}
            if name + ".inv" in z:
                d["inv"] = jnp.asarray(z[name + ".inv"], jnp.float32)
            return d
        if quant == "int8" and name in int8_names and i not in skip_layers:
            al = float(stats[f"alpha.{i}.{name}"]) if f"alpha.{i}.{name}" in stats else alpha
            return quantize_int8(a, stats.get(f"{i}.{name}"), al, ROT)
        return bf(a)
    devf = lambda a: jnp.asarray(np.asarray(a, np.float32))
    layers = []
    for i in range(cfg.layers):
        cur["layer"] = i
        p = f"{prefix}layers.{i}."
        L = {
            "in_norm": devf(get(p + "input_layernorm.weight")),
            "post_norm": devf(get(p + "post_attention_layernorm.weight")),
            # gate|up fused [d, 2F] (one wide matmul runs faster than two), down [F, d]
            "gate_up": dev(np.concatenate([get(p + "mlp.gate_proj.weight"), get(p + "mlp.up_proj.weight")], 0).T, "gate_up"),
            "down": dev(get(p + "mlp.down_proj.weight").T, "down"),
        }
        if cfg.layer_types[i] == "linear_attention":
            a = p + "linear_attn."
            L.update({
                "in_qkv": dev(get(a + "in_proj_qkv.weight").T, "in_qkv"),
                "in_z": dev(get(a + "in_proj_z.weight").T, "in_z"),
                "in_ba": dev(np.concatenate([get(a + "in_proj_b.weight"), get(a + "in_proj_a.weight")], 0).T, "in_ba"),
                "conv": devf(get(a + "conv1d.weight")[:, 0, :]),      # [conv_dim, K]
                "dt_bias": devf(get(a + "dt_bias")),
                "A_log": devf(get(a + "A_log")),
                "gnorm": devf(get(a + "norm.weight")),
                "out": dev(get(a + "out_proj.weight").T, "out"),
            })
        else:
            a = p + "self_attn."
            L.update({
                "qkv": dev(np.concatenate([get(a + "q_proj.weight"), get(a + "k_proj.weight"),
                                           get(a + "v_proj.weight")], 0).T, "qkv"),
                "q_norm": devf(get(a + "q_norm.weight")),
                "k_norm": devf(get(a + "k_norm.weight")),
                "o": dev(get(a + "o_proj.weight").T, "o"),
            })
        layers.append(L)
    params = {"layers": layers, "norm": devf(get(prefix + "norm.weight"))}
    embed = get(prefix + "embed_tokens.weight")              # host, bf16
    lm_name = "lm_head.weight" if "lm_head.weight" in index else prefix + "embed_tokens.weight"
    lm_head = get(lm_name)
    return params, embed, lm_head


# ------------------------------------------------------------------ backbone

def rms(x, w, eps):
    return x * jax.lax.rsqrt(jnp.mean(x * x, -1, keepdims=True) + eps) * (1.0 + w)


MM_BARRIER = os.environ.get("CLEF_TPU_MM_BARRIER", "0") == "1"
# int8 error diagnostics (CLEF_TPU_INT8_DIAG): "w8a16" quantizes only the
# weights, "a8w16" only the activations
INT8_DIAG = os.environ.get("CLEF_TPU_INT8_DIAG", "")


def mm(x, w, precision, tap=None, name=None):
    if tap is not None:
        tap(name, x)
    if isinstance(w, dict):
        # w8a8: activations quantized per row on the fly, int8 x int8 -> int32 on the MXU
        if "inv" in w:
            x = x * w["inv"][None, :]
        if ROT:
            x = rotate(x, ROT)
        if INT8_DIAG == "w8a16":
            # diagnostic: int8 weights, bf16 activations
            return jnp.dot(x.astype(jnp.bfloat16), w["q"].astype(jnp.bfloat16),
                           preferred_element_type=jnp.float32) * w["s"][None, :]
        ax = jnp.maximum(jnp.max(jnp.abs(x), -1, keepdims=True), 1e-12) / 127.0
        xq = jnp.clip(jnp.round(x / ax), -127, 127).astype(jnp.int8)
        if INT8_DIAG == "a8w16":
            # diagnostic: int8 activations, the weights dequantized back (per-channel int8 grid kept out)
            return jnp.dot((xq.astype(jnp.float32) * ax).astype(jnp.bfloat16), w["wf"],
                           preferred_element_type=jnp.float32)
        if MM_BARRIER:
            xq, ax = jax.lax.optimization_barrier((xq, ax))
        y = jnp.dot(xq, w["q"], preferred_element_type=jnp.int32)
        return y.astype(jnp.float32) * ax * w["s"][None, :]
    if precision == "f32":
        return jnp.dot(x, w.astype(jnp.float32), precision=jax.lax.Precision.HIGHEST)
    xb = x.astype(jnp.bfloat16)
    if MM_BARRIER:
        # materialize the bf16 operand: XLA otherwise fuses the norm (or the
        # gating) producing it into the matmul, which recomputes it per tile
        xb = jax.lax.optimization_barrier(xb)
    return jnp.dot(xb, w, preferred_element_type=jnp.float32)


def rope_tables(cfg: Config, positions):
    half = cfg.rotary_dim // 2
    inv = 1.0 / (cfg.rope_theta ** (np.arange(0, cfg.rotary_dim, 2, dtype=np.float64) / cfg.rotary_dim))
    f = positions.astype(jnp.float32)[:, None] * jnp.asarray(inv, jnp.float32)[None, :]
    f = jnp.concatenate([f, f], -1)
    return jnp.cos(f), jnp.sin(f)


def apply_rope(x, cos, sin, rd):
    xr, xp = x[..., :rd], x[..., rd:]
    h = rd // 2
    rot = jnp.concatenate([-xr[..., h:], xr[..., :h]], -1)
    return jnp.concatenate([xr * cos[:, None, :] + rot * sin[:, None, :], xp], -1)


def init_state(cfg: Config, cache_len: int, batch: int = 1):
    st = []
    conv_dim = 2 * cfg.lin_k_heads * cfg.lin_k_dim + cfg.lin_v_heads * cfg.lin_v_dim
    for t in cfg.layer_types:
        if t == "linear_attention":
            st.append({
                "conv": jnp.zeros((cfg.conv_kernel - 1, conv_dim), jnp.float32),
                "S": jnp.zeros((cfg.lin_v_heads, cfg.lin_k_dim, cfg.lin_v_dim), jnp.float32),
            })
        else:
            st.append({
                "k": jnp.zeros((cache_len, cfg.kv_heads, cfg.head_dim), jnp.bfloat16),
                "v": jnp.zeros((cache_len, cfg.kv_heads, cfg.head_dim), jnp.bfloat16),
            })
    return st


def deltanet(cfg, L, st, x, precision, scan, real=None, tap=None):
    """`real` [C, 1] marks the prompt's rows; pad rows leave the delta state
    and the conv tail untouched, so a state snapshot at a prefix boundary is
    the same as stopping there."""
    C = x.shape[0]
    Hk, Hv, dk, dv = cfg.lin_k_heads, cfg.lin_v_heads, cfg.lin_k_dim, cfg.lin_v_dim
    kd, vd = Hk * dk, Hv * dv
    qkv = mm(x, L["in_qkv"], precision, tap, "in_qkv")
    z = mm(x, L["in_z"], precision, tap, "in_z")
    ba = mm(x, L["in_ba"], precision, tap, "in_ba")
    b, a = ba[:, :Hv], ba[:, Hv:]
    K = cfg.conv_kernel
    full = jnp.concatenate([st["conv"], qkv], 0)                    # [K-1+C, conv_dim]
    w = L["conv"]                                                   # [conv_dim, K]
    conv = sum(full[j: j + C] * w[:, j][None, :] for j in range(K))
    conv = jax.nn.silu(conv)
    if real is None:
        new_conv = full[C:]
    else:
        n_real = jnp.sum(real.astype(jnp.int32))
        new_conv = jax.lax.dynamic_slice_in_dim(full, n_real, K - 1, 0)
    beta = jax.nn.sigmoid(b)                                        # [C, Hv]
    g = -jnp.exp(L["A_log"]) * jax.nn.softplus(a + L["dt_bias"])   # [C, Hv]
    if real is not None:
        beta = jnp.where(real, beta, 0.0)
        g = jnp.where(real, g, 0.0)
    if scan == "fused":
        # one Pallas kernel reads q | k | v from the conv output and writes o as [C, Hv dv]
        o, S = delta.fused(conv, g, beta, st["S"], Hk=Hk, Hv=Hv, dk=dk, dv=dv)
        o = o.reshape(C, Hv, dv)
    else:
        q, k, v = jnp.split(conv, [kd, 2 * kd], -1)
        q = q.reshape(C, Hk, dk)
        k = k.reshape(C, Hk, dk)
        v = v.reshape(C, Hv, dv)
        rep = Hv // Hk
        q = jnp.repeat(q, rep, axis=1)
        k = jnp.repeat(k, rep, axis=1)
        o, S = delta.chunked(q.transpose(1, 0, 2), k.transpose(1, 0, 2), v.transpose(1, 0, 2),
                             g.T, beta.T, st["S"], scan=scan)
        o = o.transpose(1, 0, 2)                                    # [C, Hv, dv]
    z = z.reshape(C, Hv, dv)
    o = o * jax.lax.rsqrt(jnp.mean(o * o, -1, keepdims=True) + cfg.eps) * L["gnorm"]
    o = o * jax.nn.silu(z)
    out = mm(o.reshape(C, vd), L["out"], precision, tap, "out")
    return out, {"conv": new_conv, "S": S}


# exact row max before the softmax pass (CLEF_TPU_ATTN_TWO_PASS=1): chunk-invariant probabilities
ATTN_TWO_PASS = os.environ.get("CLEF_TPU_ATTN_TWO_PASS", "0") == "1"
# keys per online-softmax step; the cache buckets are multiples of it
KEY_BLOCK = int(__import__("os").environ.get("CLEF_TPU_KEY_BLOCK", "2048"))


def causal_attention(qh, kc, vc, pos0, precision):
    """Queries at pos0..pos0+C against the cache's keys 0..pos0+C, in key
    blocks of KEY_BLOCK with an online softmax: the cost follows the real prefix,
    not the cache bucket. qh [C, Hkv, rep, D]; returns [C, Hkv, rep, D] f32."""
    C, Hkv, rep, D = qh.shape
    scale = 1.0 / math.sqrt(D)
    qpos = pos0 + jnp.arange(C)
    if precision == "f32":
        qx = qh * scale
        dot = lambda a, b, eq: jnp.einsum(eq, a, b.astype(jnp.float32), precision=jax.lax.Precision.HIGHEST)
    else:
        qx = (qh * scale).astype(jnp.bfloat16)
        dot = lambda a, b, eq: jnp.einsum(eq, a.astype(jnp.bfloat16), b.astype(jnp.bfloat16),
                                          preferred_element_type=jnp.float32)
    KB = min(KEY_BLOCK, kc.shape[0]) if kc.shape[0] % KEY_BLOCK == 0 else C
    n_blocks = (pos0 + C + KB - 1) // KB

    def body(j, carry):
        m, l, acc = carry
        start = j * KB
        kb = jax.lax.dynamic_slice_in_dim(kc, start, KB, 0)
        vb = jax.lax.dynamic_slice_in_dim(vc, start, KB, 0)
        s = dot(qx, kb, "cgrd,tgd->grct")
        kpos = start + jnp.arange(KB)
        s = jnp.where((kpos[None, :] <= qpos[:, None])[None, None], s, -jnp.inf)
        m_new = jnp.maximum(m, s.max(-1))
        p = jnp.exp(s - m_new[..., None])
        corr = jnp.exp(m - m_new)
        l = l * corr + p.sum(-1)
        acc = acc * corr[..., None] + dot(p, vb, "grct,tgd->grcd")
        return m_new, l, acc

    m0 = jnp.full((Hkv, rep, C), -jnp.inf, jnp.float32)
    l0 = jnp.zeros((Hkv, rep, C), jnp.float32)
    a0 = jnp.zeros((Hkv, rep, C, D), jnp.float32)
    if ATTN_TWO_PASS:
        # the row max over every key first, then one pass with it fixed: the
        # probabilities (rounded to bf16 for P V) then do not depend on how
        # the keys were blocked, i.e. on the prefill chunk
        def mx(j, m):
            start = j * KB
            kb = jax.lax.dynamic_slice_in_dim(kc, start, KB, 0)
            s = dot(qx, kb, "cgrd,tgd->grct")
            kpos = start + jnp.arange(KB)
            return jnp.maximum(m, jnp.where((kpos[None, :] <= qpos[:, None])[None, None], s, -jnp.inf).max(-1))

        m = jax.lax.fori_loop(0, n_blocks, mx, m0)

        def body2(j, carry):
            l, acc = carry
            start = j * KB
            kb = jax.lax.dynamic_slice_in_dim(kc, start, KB, 0)
            vb = jax.lax.dynamic_slice_in_dim(vc, start, KB, 0)
            s = dot(qx, kb, "cgrd,tgd->grct")
            kpos = start + jnp.arange(KB)
            p = jnp.where((kpos[None, :] <= qpos[:, None])[None, None], jnp.exp(s - m[..., None]), 0.0)
            return l + p.sum(-1), acc + dot(p, vb, "grct,tgd->grcd")

        l, acc = jax.lax.fori_loop(0, n_blocks, body2, (l0, a0))
    else:
        _, l, acc = jax.lax.fori_loop(0, n_blocks, body, (m0, l0, a0))
    return (acc / l[..., None]).transpose(2, 0, 1, 3)


def attention(cfg, L, st, x, pos0, cos, sin, precision, tap=None):
    C = x.shape[0]
    H, Hkv, D = cfg.heads, cfg.kv_heads, cfg.head_dim
    proj = mm(x, L["qkv"], precision, tap, "qkv")
    qg, k, v = jnp.split(proj, [H * D * 2, H * D * 2 + Hkv * D], -1)
    qg = qg.reshape(C, H, 2 * D)
    q, gate = qg[..., :D], qg[..., D:].reshape(C, H * D)
    k = k.reshape(C, Hkv, D)
    v = v.reshape(C, Hkv, D)
    q = rms(q, L["q_norm"], cfg.eps)
    k = rms(k, L["k_norm"], cfg.eps)
    q = apply_rope(q, cos, sin, cfg.rotary_dim)
    k = apply_rope(k, cos, sin, cfg.rotary_dim)
    kc = jax.lax.dynamic_update_slice(st["k"], k.astype(st["k"].dtype), (pos0, 0, 0))
    vc = jax.lax.dynamic_update_slice(st["v"], v.astype(st["v"].dtype), (pos0, 0, 0))
    rep = H // Hkv
    qh = q.reshape(C, Hkv, rep, D)
    o = causal_attention(qh, kc, vc, pos0, precision)
    o = o.reshape(C, H * D) * jax.nn.sigmoid(gate)
    return mm(o, L["o"], precision, tap, "o"), {"k": kc, "v": vc}


@functools.partial(jax.jit, static_argnames=("cfg", "precision", "scan", "calib"), donate_argnums=(2,))
def prefill_chunk(cfg: Config, params, state, x, pos0, n_valid, *, precision="bf16", scan="fused", calib=False):
    """x [C, d] f32 embeddings at positions pos0..pos0+C; rows at or past
    `n_valid` are padding. Returns (state, final-normed rows [C, d]); with
    `calib`, also {"<layer>.<projection>": max |input| per input channel}
    over the real rows, for int8 calibration.

    Pad rows are held at zero after every layer: causal attention gives them
    zero weight in real rows, but a pad row that overflowed would still turn
    0 * inf into NaN in the probability-times-value product."""
    C = x.shape[0]
    real = ((pos0 + jnp.arange(C)) < n_valid)[:, None]
    x = jnp.where(real, x, 0.0)
    cos, sin = rope_tables(cfg, pos0 + jnp.arange(C))
    new_state = []
    stats = {}
    for i, L in enumerate(params["layers"]):
        tap = None
        if calib == "inputs":
            def tap(name, v, i=i):
                stats[f"{i}.{name}"] = jnp.where(real, v, 0.0)
        elif calib:
            def tap(name, v, i=i):
                stats[f"{i}.{name}"] = jnp.max(jnp.abs(jnp.where(real, v, 0.0)), 0)
        h = rms(x, L["in_norm"], cfg.eps)
        if cfg.layer_types[i] == "linear_attention":
            y, s = deltanet(cfg, L, state[i], h, precision, scan, real, tap)
        else:
            y, s = attention(cfg, L, state[i], h, pos0, cos, sin, precision, tap)
        new_state.append(s)
        x = jnp.where(real, x + y, 0.0)
        h = rms(x, L["post_norm"], cfg.eps)
        gu = mm(h, L["gate_up"], precision, tap, "gate_up")
        x = x + mm(jax.nn.silu(gu[:, :cfg.ffn]) * gu[:, cfg.ffn:], L["down"], precision, tap, "down")
        x = jnp.where(real, x, 0.0)
    if calib:
        return new_state, rms(x, params["norm"], cfg.eps), stats
    return new_state, rms(x, params["norm"], cfg.eps)


@functools.partial(jax.jit, static_argnames=("cfg", "precision", "scan"), donate_argnums=(2,))
def prefill_chunk_batch(cfg: Config, params, state, x, pos0, n_valid, *, precision="bf16", scan="jax"):
    """`prefill_chunk` over a batch of independent requests: state, x [B, C, d],
    pos0 and n_valid [B] carry a leading batch axis; the weights are shared, so
    every projection is one [B*C, d] matmul."""
    f = lambda s, xx, p, n: prefill_chunk.__wrapped__(cfg, params, s, xx, p, n, precision=precision, scan=scan)
    return jax.vmap(f)(state, x, pos0, n_valid)


# ------------------------------------------------------------------ head

def load_head(path: Path):
    from safetensors.numpy import load_file
    import ml_dtypes  # noqa: F401

    hc = json.loads((path / "joint_head_config.json").read_text())
    raw = load_file(str(path / "joint_head.safetensors"))
    return hc, {k: jnp.asarray(np.asarray(v, np.float32)) for k, v in raw.items()}


def _ln(x, w, b, eps=1e-5):
    m = jnp.mean(x, -1, keepdims=True)
    v = jnp.mean((x - m) ** 2, -1, keepdims=True)
    return (x - m) * jax.lax.rsqrt(v + eps) * w + b


def _gelu(x):
    return jax.nn.gelu(x, approximate=False)


_HP = jax.lax.Precision.HIGHEST
# the head's two wide products over every token row (the memory projection and
# the span means; CLEF_TPU_HEAD_PRECISION). HIGH is bf16x3, ~f32 accuracy at half the passes
_HEAD_WIDE = getattr(jax.lax.Precision, os.environ.get("CLEF_TPU_HEAD_PRECISION", "HIGHEST"))


def _lin(x, W, b=None):
    y = jnp.dot(x, W.T, precision=_HP)
    return y if b is None else y + b


def _mha(W, pre, q, kv, key_mask, heads):
    d = q.shape[-1]
    wi, bi = W[pre + "in_proj_weight"], W[pre + "in_proj_bias"]
    Q = _lin(q, wi[:d], bi[:d])
    K = _lin(kv, wi[d:2 * d], bi[d:2 * d])
    V = _lin(kv, wi[2 * d:], bi[2 * d:])
    hd = d // heads
    Q = Q.reshape(-1, heads, hd)
    K = K.reshape(-1, heads, hd)
    V = V.reshape(-1, heads, hd)
    s = jnp.einsum("qhd,khd->hqk", Q, K, precision=_HP) / math.sqrt(hd)
    s = jnp.where(key_mask[None, None, :], s, -jnp.inf)
    p = jax.nn.softmax(s, -1)
    o = jnp.einsum("hqk,khd->qhd", p, V, precision=_HP).reshape(-1, d)
    return _lin(o, W[pre + "out_proj.weight"], W[pre + "out_proj.bias"])


@functools.partial(jax.jit, static_argnames=("heads", "routing", "layers"))
def head_logits(W, hidden, mem_mask, span_w, q_of_opt, opt_mask, q_mask, q_type, lexical,
                *, heads, routing, layers):
    """hidden [T, d] final-normed backbone rows (pads masked by mem_mask);
    span_w [Q+N, T] span-mean weights (questions first); q_of_opt [N] question index of each option;
    lexical [N, d] mean lm_head rows. Returns logits [N]."""
    Qn = q_mask.shape[0]
    Hh = _ln(hidden, W["hidden_norm.weight"], W["hidden_norm.bias"])
    M = jnp.dot(Hh, W["memory_projection.weight"].T, precision=_HEAD_WIDE)
    last = jnp.max(jnp.where(mem_mask, jnp.arange(hidden.shape[0]), -1))
    g = Hh[last]
    means = jnp.dot(span_w, Hh, precision=_HEAD_WIDE)
    qv, cv = means[:Qn], means[Qn:]
    R = (_lin(cv, W["option_context_projection.weight"]) + _lin(lexical, W["option_lexical_projection.weight"])
         + _lin(qv, W["option_question_projection.weight"])[q_of_opt])
    for i in range(routing):
        p = f"evidence_layers.{i}."
        mn = _ln(M, W[p + "memory_norm.weight"], W[p + "memory_norm.bias"])
        R = R + _mha(W, p + "attention.", _ln(R, W[p + "query_norm.weight"], W[p + "query_norm.bias"]), mn, mem_mask, heads)
        f = _ln(R, W[p + "feedforward_norm.weight"], W[p + "feedforward_norm.bias"])
        R = R + _lin(_gelu(_lin(f, W[p + "feedforward.0.weight"], W[p + "feedforward.0.bias"])),
                     W[p + "feedforward.3.weight"], W[p + "feedforward.3.bias"])
    base = _lin(qv, W["question_projection.weight"])
    width = R.shape[-1]
    member = (q_of_opt[None, :] == jnp.arange(Qn)[:, None]) & opt_mask[None, :]       # [Q, N]
    sc = jnp.einsum("qw,nw->qn", base, R, precision=_HP) / math.sqrt(width)
    sc = jnp.where(member, sc, -jnp.inf)
    rw = jax.nn.softmax(sc, -1)
    rw = jnp.where(member, rw, 0.0)
    summ = jnp.dot(rw, R, precision=_HP)
    F = (base + _ln(summ, W["option_summary_norm.weight"], W["option_summary_norm.bias"])
         + _lin(g, W["global_projection.weight"])[None] + W["type_embedding.weight"][q_type])
    for i in range(layers):
        p = f"layers.{i}."
        F = F + _mha(W, p + "self_attn.", _ln(F, W[p + "norm1.weight"], W[p + "norm1.bias"]),
                     _ln(F, W[p + "norm1.weight"], W[p + "norm1.bias"]), q_mask, heads)
        F = F + _mha(W, p + "multihead_attn.", _ln(F, W[p + "norm2.weight"], W[p + "norm2.bias"]), M, mem_mask, heads)
        f = _ln(F, W[p + "norm3.weight"], W[p + "norm3.bias"])
        F = F + _lin(_gelu(_lin(f, W[p + "linear1.weight"], W[p + "linear1.bias"])), W[p + "linear2.weight"], W[p + "linear2.bias"])
    F = _ln(F, W["field_norm.weight"], W["field_norm.bias"])
    anchor = qv + g[None]
    anchor = anchor / jnp.maximum(jnp.linalg.norm(anchor, axis=-1, keepdims=True), 1e-12)
    lex = lexical / jnp.maximum(jnp.linalg.norm(lexical, axis=-1, keepdims=True), 1e-12)
    prior = jnp.exp(jnp.minimum(W["prior_logit_scale"], math.log(100.0))) * jnp.sum(lex * anchor[q_of_opt], -1)
    O = _ln(R, W["option_norm.weight"], W["option_norm.bias"])
    Fo = F[q_of_opt]
    cos = jnp.sum(Fo * O, -1) / jnp.maximum(jnp.linalg.norm(Fo, axis=-1) * jnp.linalg.norm(O, axis=-1), 1e-8)
    feats = jnp.concatenate([Fo, O, Fo * O, jnp.abs(Fo - O)], -1)
    res = _lin(_gelu(_lin(feats, W["residual_scorer.0.weight"], W["residual_scorer.0.bias"])),
               W["residual_scorer.3.weight"], W["residual_scorer.3.bias"])[:, 0]
    joint = jnp.exp(jnp.minimum(W["joint_logit_scale"], math.log(100.0))) * cos + res
    return prior + jax.nn.sigmoid(W["residual_gate"]) * joint
