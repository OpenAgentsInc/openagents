"""Chunked gated delta rule (Qwen3.5 linear attention) for TPU.

Intra-chunk work (the UT transform, decays, intra-chunk attention) is
parallel over chunks and heads and is all matmuls; (I + L)^-1 for the
strictly lower L is a unit lower-triangular solve (the matmul-only
nilpotent product loses precision on real keys; see `prep`).
The sequential part over chunks is either a `lax.scan` ("jax") or a Pallas
kernel that keeps the [dk, dv] state in VMEM ("pallas").

Semantics follow transformers' `torch_chunk_gated_delta_rule` with
`use_qk_l2norm_in_kernel=True`.
"""

import functools
import os

import jax
import jax.numpy as jnp
import numpy as np
from jax.experimental import pallas as pl
from jax.experimental.pallas import tpu as pltpu

_HP = jax.lax.Precision.HIGHEST
# precision of the intra-chunk einsums (keys/queries products, the UT solve)
# (CLEF_TPU_DELTA_PRECISION: HIGHEST | HIGH | DEFAULT). HIGH (bf16x3) here with the
# inverse at HIGHEST keeps the e2e answers at 104/104; the inverse at HIGH does not
# (102/104, max |dp| 0.03) and at DEFAULT overflows.
PREP = getattr(jax.lax.Precision, os.environ.get("CLEF_TPU_DELTA_PRECISION", "HIGH"))
# the Newton-Schulz inverse alone (CLEF_TPU_NS_PRECISION)
NS = getattr(jax.lax.Precision, os.environ.get("CLEF_TPU_NS_PRECISION", "HIGHEST"))
# its steps before the last (CLEF_TPU_NS_EARLY_PRECISION; HIGH gave NaN answers)
NS_EARLY = getattr(jax.lax.Precision, os.environ.get("CLEF_TPU_NS_EARLY_PRECISION", "HIGHEST"))


def l2n(x):
    return x * jax.lax.rsqrt(jnp.sum(x * x, -1, keepdims=True) + 1e-6)


def inv_unit_lower(L, C):
    eye = jnp.eye(C, dtype=L.dtype)
    n = -L
    acc = eye + n
    p = n
    j = 2
    while j < C:
        p = jnp.matmul(p, p, precision=_HP)
        acc = jnp.matmul(acc, eye + p, precision=_HP)
        j *= 2
    return acc


def ns_inverse(A, C):
    """(I + L)^-1 by Newton-Schulz, S <- S (2I - A S): exact after log2 C steps for
    nilpotent L and self-correcting (unlike the bare product of powers), as in
    tpu-inference's GDN triangle solver."""
    eye = jnp.eye(C, dtype=A.dtype)
    S = 2 * eye - A
    steps = int(np.log2(C))
    for i in range(steps):
        # every step at HIGHEST: dropping a step, or running the early ones at
        # HIGH, gives NaN answers on real keys (the inverse's entries are large)
        p = NS if i == steps - 1 else NS_EARLY
        S = jnp.matmul(S, 2 * eye - jnp.matmul(A, S, precision=p), precision=p)
    return S


# how (I + L) X = R is solved: "block" (Newton-Schulz inverses of BLOCK-wide
# diagonal blocks, then block forward substitution: stable and matmul-only),
# "tri" (XLA's triangular solve: stable, slower), or "ns" (the whole inverse by
# Newton-Schulz: fastest, but on near-parallel keys the 64-wide inverse has
# entries near 2^62 and the delta state diverges over a long prompt)
SOLVE = os.environ.get("CLEF_TPU_SOLVE", "block")
BLOCK = int(os.environ.get("CLEF_TPU_SOLVE_BLOCK", "16"))


def block_solve(A, R, C, b):
    """(I + L) X = R for unit lower-triangular A = I + L, without forming
    the full inverse: invert each b-wide diagonal block, then substitute
    block by block (C/b sequential matmul steps)."""
    n = C // b
    X = []
    for i in range(n):
        Ri = R[..., i * b:(i + 1) * b, :]
        for j in range(i):
            Ri = Ri - jnp.matmul(A[..., i * b:(i + 1) * b, j * b:(j + 1) * b], X[j], precision=_HP)
        Tii = ns_inverse(A[..., i * b:(i + 1) * b, i * b:(i + 1) * b], b)
        X.append(jnp.matmul(Tii, Ri, precision=_HP))
    return jnp.concatenate(X, -2)
# the delta rule's own chunk inside a prefill chunk (CLEF_TPU_SUBCHUNK). 128 overflows on
# real decays (NaN answers); 64 is the reference chunk.
SUBCHUNK = int(os.environ.get("CLEF_TPU_SUBCHUNK", "64"))
# scan steps XLA unrolls (CLEF_TPU_SCAN_UNROLL)
UNROLL = int(os.environ.get("CLEF_TPU_SCAN_UNROLL", "8"))


def prep(q, k, v, g, beta, C):
    """q,k [H,T,dk] v [H,T,dv] g,beta [H,T] (T % C == 0)."""
    H, T, dk = q.shape
    N = T // C
    q = l2n(q) * (dk ** -0.5)
    k = l2n(k)
    q, k, v = (x.reshape(H, N, C, -1) for x in (q, k, v))
    g = g.reshape(H, N, C)
    beta = beta.reshape(H, N, C)
    # cumulative log decay as a matmul with a triangle of ones (cumsum lowers to a slow reduce_window)
    gc = jnp.matmul(g, jnp.triu(jnp.ones((C, C), g.dtype)), precision=_HP)
    tril = jnp.tril(jnp.ones((C, C), bool))
    decay = jnp.where(tril, jnp.exp(jnp.where(tril, gc[..., :, None] - gc[..., None, :], 0.0)), 0.0)
    kb = k * beta[..., None]
    vb = v * beta[..., None]
    strict = jnp.tril(jnp.ones((C, C), bool), -1)
    Lm = jnp.where(strict, jnp.einsum("hnik,hnjk->hnij", kb, k, precision=PREP) * decay, 0.0)
    # (I + L) X = [vb | kb e^gc]: a unit lower-triangular solve. The nilpotent
    # product (`inv_unit_lower`) is matmul-only but its powers of L lose all
    # precision (and overflow) on real keys with decays near 1.
    rhs = jnp.concatenate([vb, kb * jnp.exp(gc)[..., None]], -1)
    A_ut = Lm + jnp.eye(C, dtype=Lm.dtype)
    if SOLVE == "block":
        sol = block_solve(A_ut, rhs, C, min(BLOCK, C))
    elif SOLVE == "ns":
        sol = jnp.matmul(ns_inverse(A_ut, C), rhs, precision=PREP)
    else:
        sol = jax.lax.linalg.triangular_solve(A_ut, rhs, left_side=True, lower=True, unit_diagonal=True)
    u, w = sol[..., :vb.shape[-1]], sol[..., vb.shape[-1]:]
    A = jnp.where(tril, jnp.einsum("hnik,hnjk->hnij", q, k, precision=PREP) * decay, 0.0)
    qd = q * jnp.exp(gc)[..., None]
    glast = gc[..., -1]
    kt = k * jnp.exp(glast[..., None] - gc)[..., None]
    return qd, A, u, w, kt, jnp.exp(glast)


def scan_jax(qd, A, u, w, kt, dl, S0):
    H, N, C, dk = qd.shape
    dv = u.shape[-1]

    def one(S, xs):
        qd_i, A_i, u_i, w_i, kt_i, dl_i = xs
        vn = u_i - jnp.matmul(w_i, S, precision=_HP)
        o = jnp.matmul(qd_i, S, precision=_HP) + jnp.matmul(A_i, vn, precision=_HP)
        S = S * dl_i[:, None, None] + jnp.matmul(jnp.swapaxes(kt_i, -1, -2), vn, precision=_HP)
        return S, o

    xs = tuple(jnp.moveaxis(x, 1, 0) for x in (qd, A, u, w, kt, dl))
    S, o = jax.lax.scan(one, S0, xs, unroll=min(N, UNROLL))
    return jnp.moveaxis(o, 0, 1).reshape(H, N * C, dv), S


def _kernel(qd_ref, a_ref, u_ref, w_ref, kt_ref, dl_ref, s0_ref, o_ref, s_out_ref, s_ref):
    n = pl.program_id(1)

    @pl.when(n == 0)
    def _():
        s_ref[...] = s0_ref[0]

    S = s_ref[...]
    dot = lambda a, b: jnp.dot(a, b, preferred_element_type=jnp.float32, precision=_HP)
    vn = u_ref[0, 0] - dot(w_ref[0, 0], S)
    o_ref[0, 0] = dot(qd_ref[0, 0], S) + dot(a_ref[0, 0], vn)
    S = S * dl_ref[0, 0][0:1, :] + jax.lax.dot_general(
        kt_ref[0, 0], vn, (((0,), (0,)), ((), ())), preferred_element_type=jnp.float32, precision=_HP)
    s_ref[...] = S
    s_out_ref[0] = S


def scan_pallas(qd, A, u, w, kt, dl, S0):
    H, N, C, dk = qd.shape
    dv = u.shape[-1]
    dlb = jnp.broadcast_to(dl[:, :, None, None], (H, N, 8, dv))
    blk = lambda d: pl.BlockSpec((1, 1, C, d), lambda h, n: (h, n, 0, 0))
    o, S = pl.pallas_call(
        _kernel,
        grid=(H, N),
        in_specs=[blk(dk), blk(C), blk(dv), blk(dk), blk(dk),
                  pl.BlockSpec((1, 1, 8, dv), lambda h, n: (h, n, 0, 0)),
                  pl.BlockSpec((1, dk, dv), lambda h, n: (h, 0, 0))],
        out_specs=[blk(dv), pl.BlockSpec((1, dk, dv), lambda h, n: (h, 0, 0))],
        out_shape=[jax.ShapeDtypeStruct((H, N, C, dv), jnp.float32),
                   jax.ShapeDtypeStruct((H, dk, dv), jnp.float32)],
        scratch_shapes=[pltpu.VMEM((dk, dv), jnp.float32)],
        compiler_params=pltpu.CompilerParams(dimension_semantics=("parallel", "arbitrary")),
    )(qd, A, u, w, kt, dlb, S0)
    return o.reshape(H, N * C, dv), S


def chunked(q, k, v, g, beta, S0, *, C=64, scan="jax"):
    C = SUBCHUNK or C
    parts = prep(q, k, v, g, beta, C)
    return (scan_pallas if scan == "pallas" else scan_jax)(*parts, S0)


def step_ref(q, k, v, g, beta, S0):
    """Token-step reference (transformers' recurrent rule)."""
    dk = q.shape[-1]
    q = l2n(q) * (dk ** -0.5)
    k = l2n(k)

    def one(S, xs):
        qt, kt, vt, gt, bt = xs
        S = S * jnp.exp(gt)[:, None, None]
        kv = jnp.einsum("hk,hkv->hv", kt, S, precision=_HP)
        d = (vt - kv) * bt[:, None]
        S = S + kt[:, :, None] * d[:, None, :]
        return S, jnp.einsum("hk,hkv->hv", qt, S, precision=_HP)

    xs = (q.transpose(1, 0, 2), k.transpose(1, 0, 2), v.transpose(1, 0, 2), g.T, beta.T)
    S, o = jax.lax.scan(one, S0, xs)
    return o.transpose(1, 0, 2), S


# ------------------------------------------------------------------ fused kernel
#
# One Pallas kernel per prefill chunk does the whole gated delta rule: the
# l2 norms, the cumulative decay, the UT transform, the intra-chunk
# attention and the state update, one delta sub-chunk per grid step, all
# heads at once (batched matmuls: on a v6e a single small matmul is ~1 us of
# latency, 32 independent ones cost little more), with the [Hv, dk, dv]
# state in f32 in VMEM across the steps.
#
# The UT transform needs (I + L)^-1 for the strictly lower L. It is formed
# explicitly by forward substitution on the VPU (column elimination,
# C - 1 steps over all heads), which is as stable as a triangular solve and
# needs no matmul chain.

FUSED_PREC = getattr(jax.lax.Precision, os.environ.get("CLEF_TPU_FUSED_PRECISION", "HIGHEST"))
# rows per forward-substitution block in the fused kernel (CLEF_TPU_FS_BLOCK)
FS_BLOCK = int(os.environ.get("CLEF_TPU_FS_BLOCK", "16"))


def _bdot(a, b, ca, cb, prec):
    return jax.lax.dot_general(a, b, (((ca,), (cb,)), ((0,), (0,))), preferred_element_type=jnp.float32,
                               precision=prec)


def _rep(x, rep):
    """[Hk, ...] -> [Hk * rep, ...], each key head repeated for its value heads."""
    if rep == 1:
        return x
    return jnp.broadcast_to(x[:, None], (x.shape[0], rep) + x.shape[1:]).reshape((x.shape[0] * rep,) + x.shape[1:])


# the fused kernel reads q, k, v straight from the conv output [T, (heads x d)]
# and writes o as [T, Hv dv], with the head split done in VMEM
# (CLEF_TPU_FUSED_2D=1), instead of head-major copies made by XLA around it
FUSED_2D = os.environ.get("CLEF_TPU_FUSED_2D", "1") == "1"


def _heads(ref, d):
    if FUSED_2D:
        x = ref[...]                                                 # [C, H d]
        return pltpu.einshape("t(hd)->htd", x, d=d)
    return ref[...]


def _tokens_last(ref):
    if FUSED_2D:
        return ref[...].T                                            # [C, H] -> [H, C]
    return ref[0]


def _put_heads(ref, o):
    if FUSED_2D:
        ref[...] = pltpu.einshape("htd->t(hd)", o)
    else:
        ref[...] = o


def _prep(q_ref, k_ref, v_ref, g_ref, b_ref, x_ref, *, C, rep, dk, prec):
    """Everything of one sub-chunk that does not need the state:
    (u, w, q e^gc, the masked intra-chunk A, k e^(gc_last - gc), e^gc_last)."""
    f32 = jnp.float32
    q = _heads(q_ref, dk)                                            # [Hk, C, dk]
    k = _heads(k_ref, dk)
    q = q * jax.lax.rsqrt(jnp.sum(q * q, -1, keepdims=True) + 1e-6) * (dk ** -0.5)
    k = k * jax.lax.rsqrt(jnp.sum(k * k, -1, keepdims=True) + 1e-6)
    KK = _rep(_bdot(k, k, 2, 2, prec), rep)                          # [Hv, C, C]
    QK = _rep(_bdot(q, k, 2, 2, prec), rep)
    q = _rep(q, rep)
    k = _rep(k, rep)
    g = _tokens_last(g_ref)                                          # [Hv, C]
    beta = _tokens_last(b_ref)
    r = jax.lax.broadcasted_iota(jnp.int32, (C, C), 0)
    c = jax.lax.broadcasted_iota(jnp.int32, (C, C), 1)
    gc = jax.lax.dot_general(g, (r <= c).astype(f32), (((1,), (0,)), ((), ())),
                             preferred_element_type=f32, precision=jax.lax.Precision.HIGHEST)   # [Hv, C] cumsum
    gcol = gc[:, :, None]                                            # [Hv, C, 1]
    bcol = beta[:, :, None]
    tril = (r >= c)[None]
    strict = (r > c)[None]
    D = jnp.exp(jnp.where(tril, gcol - gc[:, None, :], -jnp.inf))   # [Hv, C, C] decay
    Lm = jnp.where(strict, bcol * KK * D, 0.0)
    # X = (I + Lm)^-1 by forward substitution, in FS_BLOCK-row blocks: a
    # block's rows first take the earlier blocks' part in one matmul, then
    # eliminate within the block (row j is final before it is used). Rows
    # above j never change at step j; each update starts at j's 8-row tile.
    eye = (r == c).astype(f32)
    for b0 in range(0, C, FS_BLOCK):
        b1 = b0 + FS_BLOCK
        if b0 == 0:
            x_ref[:, :b1, :] = jnp.broadcast_to(eye[None, :b1], (Lm.shape[0], b1, C))
        else:
            x_ref[:, b0:b1, :] = eye[None, b0:b1] - _bdot(Lm[:, b0:b1, :b0], x_ref[:, :b0, :], 2, 1, prec)
        for j in range(b0, b1 - 1):
            r0 = (j + 1) // 8 * 8
            x_ref[:, r0:b1, :] = x_ref[:, r0:b1, :] - Lm[:, r0:b1, j:j + 1] * x_ref[:, j:j + 1, :]
    X = x_ref[...]
    egc = jnp.exp(gcol)
    UW = _bdot(X, jnp.concatenate([_heads(v_ref, dk) * bcol, k * (bcol * egc)], -1), 2, 1, prec)   # [Hv, C, dv + dk]
    dv = UW.shape[-1] - dk
    gl = gc[:, C - 1:C][:, :, None]                                  # [Hv, 1, 1]
    return (UW[..., :dv], UW[..., dv:], q * egc, jnp.where(tril, QK * D, 0.0),
            k * jnp.exp(gl - gcol), jnp.exp(gl))


# precision of the three state matmuls (CLEF_TPU_FUSED_STATE_PRECISION; default FUSED_PREC)
STATE_PREC = getattr(jax.lax.Precision, os.environ.get("CLEF_TPU_FUSED_STATE_PRECISION", "")
                     or os.environ.get("CLEF_TPU_FUSED_PRECISION", "HIGHEST"))


def _scan(S, u, w, qe, Aq, kt, dl, *, C, prec):
    """One sub-chunk against the state: (o, new state)."""
    prec = STATE_PREC
    WQ = _bdot(jnp.concatenate([w, qe], 1), S, 2, 1, prec)          # [Hv, 2C, dv]
    vn = u - WQ[:, :C]
    o = WQ[:, C:] + _bdot(Aq, vn, 2, 1, prec)
    return o, S * dl + _bdot(kt, vn, 1, 1, prec)


def _fused_kernel(q_ref, k_ref, v_ref, g_ref, b_ref, s0_ref, o_ref, s_out_ref, s_ref, x_ref, *, C, rep, dk, prec):
    n = pl.program_id(0)

    @pl.when(n == 0)
    def _():
        s_ref[...] = s0_ref[...]

    parts = _prep(q_ref, k_ref, v_ref, g_ref, b_ref, x_ref, C=C, rep=rep, dk=dk, prec=prec)
    o, S = _scan(s_ref[...], *parts, C=C, prec=prec)
    _put_heads(o_ref, o)
    s_ref[...] = S
    s_out_ref[...] = S


def _fused_kernel_pipe(q_ref, k_ref, v_ref, g_ref, b_ref, s0_ref, o_ref, s_out_ref, s_ref, x_ref, *slots,
                       C, N, rep, dk, prec):
    """Software-pipelined: grid step n prepares sub-chunk n (while it exists)
    and scans sub-chunk n - 1 (from step 1), in one basic block, so the
    prep's VPU work can overlap the scan's matmul chain. Each slot scratch
    holds two sub-chunks' parts: [n % 2] is written, [(n + 1) % 2] read."""
    n = pl.program_id(0)

    @pl.when(n == 0)
    def _():
        s_ref[...] = s0_ref[...]

    rd, wr = (n + 1) % 2, n % 2
    prev_parts = [a[rd] for a in slots]
    parts = _prep(q_ref, k_ref, v_ref, g_ref, b_ref, x_ref, C=C, rep=rep, dk=dk, prec=prec)
    S0 = s_ref[...]
    o, S = _scan(S0, *prev_parts, C=C, prec=prec)
    o_ref[...] = o
    S = jnp.where(n > 0, S, S0)
    s_ref[...] = S
    s_out_ref[...] = S
    for a, x in zip(slots, parts):
        a[wr] = x


PIPE = os.environ.get("CLEF_TPU_FUSED_PIPE", "0") == "1"


def fused(conv, g, beta, S0, *, Hk, Hv, dk, dv, C=64):
    """conv [T, 2 Hk dk + Hv dv] (q | k | v after the causal conv and SiLU),
    g, beta [T, Hv] (pad rows: g = beta = 0), S0 [Hv, dk, dv] f32.
    Returns (o [T, Hv dv] f32, S [Hv, dk, dv])."""
    T = conv.shape[0]
    C = SUBCHUNK or C
    N = T // C
    kd = Hk * dk
    if FUSED_2D and not PIPE:
        kern = functools.partial(_fused_kernel, C=C, rep=Hv // Hk, dk=dk, prec=FUSED_PREC)
        o, S = pl.pallas_call(
            kern,
            grid=(N,),
            in_specs=[pl.BlockSpec((C, kd), lambda n: (n, 0)),
                      pl.BlockSpec((C, kd), lambda n: (n, 1)),
                      pl.BlockSpec((C, Hv * dv), lambda n: (n, 2 * kd // (Hv * dv))),
                      pl.BlockSpec((C, Hv), lambda n: (n, 0)),
                      pl.BlockSpec((C, Hv), lambda n: (n, 0)),
                      pl.BlockSpec((Hv, dk, dv), lambda n: (0, 0, 0))],
            out_specs=[pl.BlockSpec((C, Hv * dv), lambda n: (n, 0)),
                       pl.BlockSpec((Hv, dk, dv), lambda n: (0, 0, 0))],
            out_shape=[jax.ShapeDtypeStruct((T, Hv * dv), jnp.float32),
                       jax.ShapeDtypeStruct((Hv, dk, dv), jnp.float32)],
            scratch_shapes=[pltpu.VMEM((Hv, dk, dv), jnp.float32), pltpu.VMEM((Hv, C, C), jnp.float32)],
            compiler_params=pltpu.CompilerParams(dimension_semantics=("arbitrary",), vmem_limit_bytes=100 << 20),
        )(conv, conv, conv, g, beta, S0)
        return o, S
    q = conv[:, :kd].reshape(T, Hk, dk).transpose(1, 0, 2)
    k = conv[:, kd:2 * kd].reshape(T, Hk, dk).transpose(1, 0, 2)
    v = conv[:, 2 * kd:].reshape(T, Hv, dv).transpose(1, 0, 2)
    gT = g.reshape(N, C, Hv).transpose(0, 2, 1)                      # [N, Hv, C]
    bT = beta.reshape(N, C, Hv).transpose(0, 2, 1)
    rep = Hv // Hk
    scratch = [pltpu.VMEM((Hv, dk, dv), jnp.float32), pltpu.VMEM((Hv, C, C), jnp.float32)]
    if PIPE:
        kern = functools.partial(_fused_kernel_pipe, C=C, N=N, rep=rep, dk=dk, prec=FUSED_PREC)
        scratch += [pltpu.VMEM((2, Hv, C, dv), jnp.float32), pltpu.VMEM((2, Hv, C, dk), jnp.float32),
                    pltpu.VMEM((2, Hv, C, dk), jnp.float32), pltpu.VMEM((2, Hv, C, C), jnp.float32),
                    pltpu.VMEM((2, Hv, C, dk), jnp.float32), pltpu.VMEM((2, Hv, 1, 1), jnp.float32)]
        steps, ix, ox = N + 1, (lambda n: jnp.minimum(n, N - 1)), (lambda n: jnp.maximum(n - 1, 0))
    else:
        kern = functools.partial(_fused_kernel, C=C, rep=rep, dk=dk, prec=FUSED_PREC)
        steps, ix, ox = N, (lambda n: n), (lambda n: n)
    o, S = pl.pallas_call(
        kern,
        grid=(steps,),
        in_specs=[pl.BlockSpec((Hk, C, dk), lambda n: (0, ix(n), 0)),
                  pl.BlockSpec((Hk, C, dk), lambda n: (0, ix(n), 0)),
                  pl.BlockSpec((Hv, C, dv), lambda n: (0, ix(n), 0)),
                  pl.BlockSpec((1, Hv, C), lambda n: (ix(n), 0, 0)),
                  pl.BlockSpec((1, Hv, C), lambda n: (ix(n), 0, 0)),
                  pl.BlockSpec((Hv, dk, dv), lambda n: (0, 0, 0))],
        out_specs=[pl.BlockSpec((Hv, C, dv), lambda n: (0, ox(n), 0)),
                   pl.BlockSpec((Hv, dk, dv), lambda n: (0, 0, 0))],
        out_shape=[jax.ShapeDtypeStruct((Hv, T, dv), jnp.float32),
                   jax.ShapeDtypeStruct((Hv, dk, dv), jnp.float32)],
        scratch_shapes=scratch,
        compiler_params=pltpu.CompilerParams(dimension_semantics=("arbitrary",), vmem_limit_bytes=100 << 20),
    )(q, k, v, gT, bT, S0)
    return o.transpose(1, 0, 2).reshape(T, Hv * dv), S
