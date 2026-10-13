"""Builds the int8 artifact for the Clef TPU worker with GPTQ (#11502).

Per layer and projection: the input Hessian H = X^T X over calibration
activations (the encoder fixture records the e2e gate does not use, rotated
by the block Hadamard of CLEF_TPU_INT8_ROT when set), then GPTQ rounding of
the (rotated) weight to per-output-channel int8: rows are quantized in
order and each row's rounding error is pushed onto the rows not yet
quantized through H^-1, which leaves far less output error than rounding to
nearest. Activations stay per-token dynamic int8 at run time.

  CLEF_TPU_INT8_ROT=4096 python gptq.py <clef-flash dir> <fixtures/clef> <out dir> [--records 40] [--group 4]

Writes <out dir>/layer<i>.npz with <name>.q (int8 [K, N]) and <name>.s
(f32 [N]) for every int8 projection, and manifest.json. Load with
CLEF_TPU_INT8_ARTIFACT=<out dir>.
"""
import argparse
import json
import sys
import time
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu import model as M  # noqa: E402
from clef_tpu.runner import Runner  # noqa: E402
from calibrate import calibration_records, chunks  # noqa: E402

# projections and the activation each reads
INPUT_OF = {"in_qkv": "h", "in_z": "h", "qkv": "h", "gate_up": "h2", "down": "act", "out": "mix", "o": "mix"}


@jax.jit
def _hess(x):
    x = x.astype(jnp.float32)
    return jnp.dot(x.T, x, precision=jax.lax.Precision.HIGHEST)


@jax.jit
def _out_err(W, Q, s, H):
    """E[|x (W - Q s)|^2] = sum((H E) * E) for E = W - Q s."""
    E = W - Q * s
    return jnp.sum(jnp.dot(H, E, precision=jax.lax.Precision.HIGHEST) * E)


@jax.jit
def inv_factor(H, damp=0.01):
    """Upper U with (H + damp mean(diag H) I)^-1 = U^T U (dead inputs get H_jj = 1)."""
    K = H.shape[0]
    dead = jnp.diag(H) == 0
    H = jnp.where(jnp.diag(dead), 1.0, H)
    H = H + damp * jnp.mean(jnp.diag(H)) * jnp.eye(K, dtype=H.dtype)
    L = jnp.linalg.cholesky(H)
    Hinv = jax.scipy.linalg.cho_solve((L, True), jnp.eye(K, dtype=H.dtype))
    return jnp.linalg.cholesky(Hinv).T


def _inv_factor_host(H, damp=0.01):
    H = H.astype(np.float64)
    d = np.diag(H).copy()
    dead = d == 0
    H[dead, dead] = 1.0
    H += damp * np.mean(np.diag(H)) * np.eye(H.shape[0])
    Hinv = np.linalg.inv(H)
    Hinv = (Hinv + Hinv.T) / 2
    return np.linalg.cholesky(Hinv).T


def gptq(W, U, block=128):
    """W [K, N] f32 (input rows), U from `inv_factor`. Returns (q int8 [K, N], scale [N])."""
    K, N = W.shape
    scale = jnp.maximum(jnp.max(jnp.abs(W), 0), 1e-12) / 127.0
    hp = jax.lax.Precision.HIGHEST

    def blk(b, carry):
        W, Q = carry
        s0 = b * block
        W1 = jax.lax.dynamic_slice_in_dim(W, s0, block, 0)
        U1 = jax.lax.dynamic_slice(U, (s0, s0), (block, block))

        def row(i, c):
            W1, Q1, E1 = c
            w = W1[i]
            q = jnp.clip(jnp.round(w / scale), -127, 127)
            err = (w - q * scale) / U1[i, i]
            W1 = W1 - jnp.where((jnp.arange(block) > i)[:, None], U1[i][:, None] * err[None, :], 0.0)
            return W1, Q1.at[i].set(q), E1.at[i].set(err)

        W1, Q1, E1 = jax.lax.fori_loop(0, block, row, (W1, jnp.zeros_like(W1), jnp.zeros_like(W1)))
        # the block's errors onto every later row
        Ub = jax.lax.dynamic_slice_in_dim(U, s0, block, 0)               # [block, K]
        later = (jnp.arange(K) >= s0 + block)[:, None]
        W = W - jnp.where(later, jnp.dot(Ub.T, E1, precision=hp), 0.0)
        Q = jax.lax.dynamic_update_slice_in_dim(Q, Q1, s0, 0)
        return W, Q

    _, Q = jax.lax.fori_loop(0, K // block, blk, (W, jnp.zeros_like(W)))
    return Q.astype(jnp.int8), scale


gptq = jax.jit(gptq, static_argnames=("block",))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("fixtures")
    ap.add_argument("out")
    ap.add_argument("--records", type=int, default=40)
    ap.add_argument("--group", type=int, default=4, help="layers whose Hessians one calibration pass collects")
    ap.add_argument("--calib", default=None, help="SmoothQuant map (calibrate.py); its scales are applied before GPTQ")
    a = ap.parse_args()
    rot = M.ROT
    stats = dict(np.load(a.calib)) if a.calib else {}
    r = Runner(a.model)
    cfg = r.cfg
    # records up to 2048 tokens: one K/V cache bucket, so two compiled chunk programs
    recs = [x for x in calibration_records(Path(a.fixtures)) if len(x["input_ids"]) <= 2048][:a.records]
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    names = [n for n in M.os.environ.get("CLEF_TPU_INT8", "gate_up,down,in_qkv,in_z,qkv,o,out").split(",") if n]
    t0 = time.time()
    report = {}
    for g0 in range(0, cfg.layers, a.group):
        layers = range(g0, min(g0 + a.group, cfg.layers))
        H = {}
        ntok = 0
        for rec in recs:
            T, C, Tp, cache, ids_dev = chunks(r, rec["input_ids"])
            ntok += T
            state = M.init_state(cfg, cache)
            for c0 in range(0, Tp, C):
                x = jnp.asarray(r.embed_dev[jax.lax.dynamic_slice_in_dim(ids_dev, c0, C)], jnp.float32)
                state, _, xs = M.prefill_chunk(cfg, r.params, state, x, jnp.int32(c0), jnp.int32(T), calib="inputs")
                for i in layers:
                    for ik in ("h", "h2", "act", "mix"):
                        src = next((f"{i}.{nm}" for nm in INPUT_OF if INPUT_OF[nm] == ik and f"{i}.{nm}" in xs), None)
                        if src is None:
                            continue
                        xin = M.rotate(xs[src], rot) if rot else xs[src]
                        hk = f"{i}.{ik}"
                        H[hk] = _hess(xin) if hk not in H else H[hk] + _hess(xin)
                del xs
        for i in layers:
            arrs = {}
            for nm in names:
                if nm not in r.params["layers"][i]:
                    continue
                W = jnp.asarray(r.params["layers"][i][nm], jnp.float32)
                Hn = H[f"{i}.{INPUT_OF[nm]}"] / ntok
                if rot:
                    W = M.rotate(W.T, rot).T
                elif f"{i}.{nm}" in stats:
                    # x W = (x / s)(s W): the Hessian of x / s is H / (s s^T)
                    key = f"alpha.{i}.{nm}"
                    sm = M.smooth_scale(np.asarray(W), stats[f"{i}.{nm}"], float(stats[key]) if key in stats else 0.5)
                    W = W * jnp.asarray(sm)[:, None]
                    smd = jnp.asarray(sm)
                    Hn = Hn / (smd[:, None] * smd[None, :])
                    arrs[f"{nm}.inv"] = (1.0 / sm).astype(np.float32)
                # output channels are independent in GPTQ: slice them to bound device memory
                # (on the host for the 12288-wide down input: XLA's Cholesky there needs ~33 GB)
                U = inv_factor(Hn) if Hn.shape[0] <= 8192 else jnp.asarray(_inv_factor_host(np.asarray(Hn)), jnp.float32)
                qs = [gptq(W[:, n0:n0 + 2048], U) for n0 in range(0, W.shape[1], 2048)]
                del U
                q = jnp.concatenate([x[0] for x in qs], 1)
                s = jnp.concatenate([x[1] for x in qs], 0)
                rtn = jnp.clip(jnp.round(W / s), -127, 127)
                x_err = lambda Q: float(_out_err(W, Q, s, Hn))
                report[f"{i}.{nm}"] = {"gptq": x_err(q.astype(jnp.float32)), "rtn": x_err(rtn)}
                arrs[f"{nm}.q"] = np.asarray(q)
                arrs[f"{nm}.s"] = np.asarray(s)
            np.savez(out / f"layer{i}.npz", **arrs)
            print(json.dumps({"layer": i, "seconds": round(time.time() - t0),
                              **{k: round(v["gptq"] / max(v["rtn"], 1e-30), 3) for k, v in report.items()
                                 if k.startswith(f"{i}.")}}), flush=True)
        del H
    (out / "manifest.json").write_text(json.dumps({"rot": rot, "calib": a.calib, "names": names, "records": len(recs),
                                                   "tokens_per_pass": ntok, "report": report}, indent=1))


if __name__ == "__main__":
    main()
