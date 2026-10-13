"""Clef TPU runner: encoded record in, per-option logits out.

The input is what Psionic's Rust encoder produces (and what the reference
`encode_record` produces): token ids plus, per question, its type, question
span and option spans. Encoding, answer shaping and the HTTP edge live in the
Rust door; this worker only runs the model.
"""

from __future__ import annotations

import collections
import hashlib
import math
import time
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np

from . import model as M


@jax.jit
def _gather(table, ids):
    return table[ids].astype(jnp.float32)


def _bucket(n, least):
    """The next power of two at or above n (at least `least`): the head
    compiles once per (tokens, questions, options) bucket, so the buckets
    are few and a persistent compilation cache keeps them across restarts."""
    b = least
    while b < n:
        b *= 2
    return b


class Runner:
    def __init__(self, path: str, *, chunk=0, prefix_budget=0, precision="bf16", scan="jax", cache_buckets=(2048, 4096, 8192, 16384),
                 calib=None):
        self.path = Path(path)
        self.cfg = M.Config.load(self.path)
        t = time.time()
        self.params, self.embed, self.lm_head = M.load_backbone(
            self.path, self.cfg, quant="int8" if precision == "int8" else None, calib=calib)
        self.calib = calib
        # the embedding table on the device (2 GB bf16): a 16k prompt's rows
        # would otherwise cross from the host as 268 MB of f32
        self.embed_dev = jnp.asarray(self.embed, jnp.bfloat16)
        self.head_cfg, self.head = M.load_head(self.path)
        self.load_seconds = time.time() - t
        self.chunk = chunk
        # prefix snapshots on the device (bytes); 0 turns prefix reuse off
        self.prefix_budget = prefix_budget
        self._prefixes = collections.OrderedDict()
        self._prefix_bytes = {}
        self.precision = precision
        self.scan = scan
        self.cache_buckets = cache_buckets
        h = hashlib.sha256()
        h.update((self.path / "joint_head.safetensors").read_bytes())
        self.head_digest = "sha256:" + h.hexdigest()

    def identity(self):
        return {"backend": "tpu", "device": str(jax.devices()[0].device_kind), "precision": self.precision,
                "prefill_chunk": self.chunk or "auto (512 | 768)", "head_digest": self.head_digest, "scan": self.scan}

    @staticmethod
    def auto_chunk(T):
        """The prefill chunk for a T-token prompt (measured on a v6e-1, P2):
        768 or 512, whichever pads less, 768 on a tie; 512 above 8k tokens,
        where 768's extra attention blocks cost more than its padding saves."""
        if T > 8192:
            return 512
        pad = lambda c: -(-T // c) * c - T
        return 768 if pad(768) <= pad(512) else 512

    def _segment(self, state, ids_dev, start, end, C):
        """Prefills positions start..end (end exclusive) in chunks of C from
        `state`; returns (state, rows [ceil((end-start)/C)*C, d])."""
        rows = []
        for c0 in range(start, end, C):
            x = _gather(self.embed_dev, jax.lax.dynamic_slice_in_dim(ids_dev, c0, C))
            state, h = M.prefill_chunk(self.cfg, self.params, state, x, jnp.int32(c0), jnp.int32(end),
                                       precision=self.precision, scan=self.scan)
            rows.append(h)
        return state, (jnp.concatenate(rows, 0) if len(rows) > 1 else rows[0])

    def hidden_rows(self, ids, chunk=None, prefix_tokens=0):
        """Final-normed backbone rows [T_pad, d] (device); rows past len(ids)
        are padding. With `prefix_tokens` P (the door's exact prompt prefix:
        system text and state), the backbone state and rows at P are kept
        (LRU, `prefix_budget` bytes) and a later prompt with the same first
        P tokens restarts from them. Returns (rows, restored tokens)."""
        T = len(ids)
        C = chunk or self.chunk or self.auto_chunk(T)
        P = prefix_tokens if self.prefix_budget and 512 <= prefix_tokens < T else 0
        n_rest = -(-(T - P) // C)
        Tp = P + n_rest * C
        need = max(Tp, -(-P // C) * C)
        cache = next((b for b in self.cache_buckets if b >= need), -(-need // 2048) * 2048)
        ids_p = np.zeros(need + C, np.int64)
        ids_p[:T] = ids
        ids_dev = jnp.asarray(ids_p, jnp.int32)
        if not P:
            _, rows = self._segment(M.init_state(self.cfg, cache), ids_dev, 0, T, C)
            return rows, 0
        key = (hashlib.sha256(np.asarray(ids[:P], np.int64).tobytes()).digest(), C, self.precision)
        hit = self._prefixes.get(key)
        if hit is not None:
            self._prefixes.move_to_end(key)
            snap, prefix_rows = hit
            state = []
            for s0, st in zip(snap, M.init_state(self.cfg, cache)):
                if "k" in st:
                    st = {n: jax.lax.dynamic_update_slice(st[n], s0[n], (0, 0, 0)) for n in ("k", "v")}
                else:
                    # copies: prefill donates its state buffers
                    st = {n: jnp.array(v, copy=True) for n, v in s0.items()}
                state.append(st)
            restored = P
        else:
            state, rows = self._segment(M.init_state(self.cfg, cache), ids_dev, 0, P, C)
            prefix_rows = rows[:P]
            snap = [{n: (v[:P] if "k" in st else jnp.array(v, copy=True)) for n, v in st.items()} for st in state]
            size = sum(int(np.prod(v.shape)) * v.dtype.itemsize for st in snap for v in st.values())
            size += int(np.prod(prefix_rows.shape)) * 4
            self._prefixes[key] = (snap, prefix_rows)
            self._prefix_bytes[key] = size
            while sum(self._prefix_bytes.values()) > self.prefix_budget and len(self._prefixes) > 1:
                old, _ = self._prefixes.popitem(last=False)
                self._prefix_bytes.pop(old, None)
            restored = 0
        _, rest = self._segment(state, ids_dev, P, T, C)
        return jnp.concatenate([prefix_rows, rest], 0), restored

    HEAD_SHAPES = ((8, 32), (8, 64), (16, 64), (16, 128), (32, 128), (32, 256), (64, 256), (64, 512))

    def warm_head(self, max_tokens=16384):
        """Compiles the head for every token bucket and the common
        (questions, options) buckets, so a first request of a new shape does
        not wait ~30 s for XLA (the persistent cache keeps them across restarts)."""
        d = self.lm_head.shape[1]
        hc = self.head_cfg
        Tb = 512
        while Tb <= max_tokens:
            for Qb, Nb in self.HEAD_SHAPES:
                out = M.head_logits(self.head, jnp.zeros((Tb, d), jnp.float32), jnp.arange(Tb) < 3,
                                    jnp.zeros((Qb + Nb, Tb), jnp.float32), jnp.zeros(Nb, jnp.int32),
                                    jnp.arange(Nb) < 2, jnp.arange(Qb) < 1, jnp.zeros(Qb, jnp.int32),
                                    jnp.zeros((Nb, d), jnp.float32),
                                    heads=hc["heads"], routing=hc["routing_layers"], layers=hc["layers"])
                jax.block_until_ready(out)
            Tb *= 2

    def logits(self, ids, questions, chunk=None, prefix_tokens=0):
        """questions: [{"type": int, "question_span": [a,b], "option_spans": [[a,b],...]}]."""
        T = len(ids)
        H, self.last_restored = self.hidden_rows(ids, chunk, prefix_tokens)
        Tp = H.shape[0]
        Tb = _bucket(Tp, 512)
        if Tb != Tp:
            H = jnp.pad(H, ((0, Tb - Tp), (0, 0)))
        Qn = len(questions)
        N = sum(len(q["option_spans"]) for q in questions)
        Qb, Nb = _bucket(Qn, 8), _bucket(N, 32)
        span_w = np.zeros((Qb + Nb, Tb), np.float32)
        q_of_opt = np.zeros(Nb, np.int32)
        opt_mask = np.zeros(Nb, bool)
        q_mask = np.zeros(Qb, bool)
        q_type = np.zeros(Qb, np.int32)
        lexical = np.zeros((Nb, self.lm_head.shape[1]), np.float32)
        ids = np.asarray(ids)
        n = 0
        for qi, q in enumerate(questions):
            a, b = q["question_span"]
            span_w[qi, a:b] = 1.0 / (b - a)
            q_mask[qi] = True
            q_type[qi] = q["type"]
            for (a, b) in q["option_spans"]:
                span_w[Qb + n, a:b] = 1.0 / (b - a)
                q_of_opt[n] = qi
                opt_mask[n] = True
                lexical[n] = np.asarray(self.lm_head[ids[a:b]], np.float32).mean(0)
                n += 1
        mem_mask = np.arange(Tb) < T
        hc = self.head_cfg
        out = M.head_logits(self.head, H, jnp.asarray(mem_mask), jnp.asarray(span_w), jnp.asarray(q_of_opt),
                            jnp.asarray(opt_mask), jnp.asarray(q_mask), jnp.asarray(q_type), jnp.asarray(lexical),
                            heads=hc["heads"], routing=hc["routing_layers"], layers=hc["layers"])
        out = np.asarray(out)
        res, n = [], 0
        for q in questions:
            k = len(q["option_spans"])
            res.append(out[n:n + k].tolist())
            n += k
        return res


def softmax(xs):
    m = max(xs)
    e = [math.exp(x - m) for x in xs]
    s = sum(e)
    return [v / s for v in e]


def answer(question, option_ids, logits):
    """Reference `systemone_answer`, unrounded."""
    p = dict(zip(option_ids, softmax(logits)))
    if question["type"] == "noul":
        return {"type": "noul", "noul": p["true"]}
    if question["type"] == "choice":
        opts = [str(o) for o in question["criteria"]]
        choice = max(opts, key=p.__getitem__)
        return {"type": "choice", "choice": choice, "confidence": p[choice], "probabilities": {o: p[o] for o in opts}}
    levels = [str(i) for i in range(len(question["criteria"]))]
    return {"type": "score", "score": sum(i * p[l] for i, l in enumerate(levels)),
            "confidence": max(p[l] for l in levels), "legend": dict(zip(levels, question["criteria"])),
            "probabilities": {l: p[l] for l in levels}}
