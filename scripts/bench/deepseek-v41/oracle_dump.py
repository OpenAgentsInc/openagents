#!/usr/bin/env python3
"""Dump golden activations from the pinned DeepSeek-V4.1-Flash reference (V0 oracle, #12371).

Retained acceptance tooling. It loads the checkpoint's own `inference/model.py`
through `torchrun`, as `generate.py` does, attaches forward hooks and
per-instance method wrappers, runs four fixed prompts with greedy decoding,
and writes what the hooks saw. It changes no computation: every capture is a
copy taken after the reference produced the value.

One value is not reachable from outside: the indexer's masked scores. The
tool loads a copy of `model.py` with one added line that keeps them on the
module (`PATCH` below), and writes the diff next to the dumps.

    torchrun --nproc-per-node 8 oracle_dump.py --ckpt TP8_DIR --reference HF_DIR --out OUT
    torchrun --nproc-per-node 1 oracle_dump.py --reference HF_DIR --out OUT --self-test

`--self-test` builds the small random model from `ModelArgs` defaults (as
`python model.py` does) and runs the same hooks on random tokens, to check
the plumbing on a GPU before the real weights are loaded.

Positions. A prompt of L tokens runs one prefill and 32 decode steps. The
prefill output is generated token 1 (g1); decode step k takes g_k at
position L + k - 1 and outputs g_{k+1}. Labels: `first` (prompt position 0),
`last` (position L - 1, whose logits pick g1), and `g1`, `g2`, `g32` (the
decode steps whose input is that generated token). The text is g1..g32.
"""

from __future__ import annotations

import argparse
import difflib
import hashlib
import importlib.metadata
import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path

N_GEN = 32

HC_LAYERS = [0, 1, 2, 14, 20, 21, 39]
# one layer per compress_ratios mode (0: 0-1, 2: 2-19, 1: 20-39), each KV source
# with the layer after it that reuses its cache (2/3, 14/15, 20/21), and the last
ATTN_LAYERS = [0, 1, 2, 3, 8, 9, 14, 15, 20, 21, 24, 25, 39]
COMPRESSOR_LAYERS = [2, 14, 20]
INDEXER_LAYERS = [2, 8, 14, 20, 24, 28, 32, 36]  # every index source
GATE_LAYERS = [0, 2, 14, 20, 39]
EXPERT_LAYERS = [2, 20]
TOPK_LOGITS = 64
GATE_TIE_EPS = 1e-4  # biased gate scores are O(1) f32
INDEX_TIE_EPS = 1e-4
COMPLETE_CAP = 2048

PATCH_ANCHOR = "            compress_lens = end_pos // ratio\n"
PATCH = (
    PATCH_ANCHOR
    + "        self.dump_index_score = index_score  # oracle dump (#12371): masked scores, before candidate filtering\n"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def tensor_bytes(t) -> bytes:
    import torch

    t = t.detach().contiguous().cpu()
    return t.view(torch.uint8).numpy().tobytes() if t.numel() else b""


def prompts(reference: Path) -> list[dict]:
    """The four fixed cases. Sources come from the pinned revision, so the text is fixed."""
    engram = (reference / "inference" / "engram.py").read_text()
    # model.py line 127 names the image token in a comment; encoding/ refuses image special
    # tokens in text, so spell its full-width bar as ASCII
    model = (reference / "inference" / "model.py").read_text().replace("<｜deepseek_image｜>", "<|deepseek_image|>")
    convert = (reference / "inference" / "convert.py").read_text()
    return [
        {
            "id": "p1-chat-thinking",
            "thinking_mode": "thinking",
            "messages": [{"role": "user", "content": "Why is the sky blue? Answer in two sentences."}],
        },
        {
            "id": "p2-code",
            "thinking_mode": "chat",
            "messages": [
                {
                    "role": "user",
                    "content": "Write a Rust function `fn fib(n: u32) -> Option<u128>` that returns the n-th "
                    "Fibonacci number, using checked arithmetic so it returns None on overflow.",
                }
            ],
        },
        {
            "id": "p3-2k",
            "thinking_mode": "chat",
            "messages": [
                {
                    "role": "user",
                    "content": "Here is a Python file:\n\n```python\n"
                    + engram
                    + "```\n\nWhat shape does `NgramHashState.forward` return, and why is the look-back "
                    "blocked at image spans?",
                }
            ],
        },
        {
            "id": "p4-20k",
            "thinking_mode": "chat",
            "messages": [
                {
                    "role": "user",
                    "content": "Here are two Python files.\n\n`model.py`:\n\n```python\n"
                    + model
                    + "```\n\n`convert.py`:\n\n```python\n"
                    + convert
                    + "```\n\nIn `convert.py`, which tensors are split across ranks by rows of the Engram "
                    "table, and how does `ParallelEngramEmbedding` in `model.py` read them back?",
                }
            ],
        },
    ]


ENGRAM_TEXT = (
    "The the THE tHe. Hello hello HELLO  hello\t\thello. "
    "Café cafe CAFÉ caf\u00e9. one two three one two three one two three four. "
    "Ｆｕｌｌｗｉｄｔｈ fullwidth. ﬁne fine. "
)


def build_cases(reference: Path, generate, tokenizer, args) -> list[dict]:
    """The four main prompts, then short edge cases. Rendering goes through the checkpoint's
    own encoding/ (generate.prepare_case), so the token ids are what generate.py would feed."""
    cases = []
    for case in prompts(reference):
        prompt, tokens, _types, images = generate.prepare_case(
            {"messages": case["messages"]}, case["thinking_mode"], tokenizer, args
        )
        assert images is None or not any(images)
        cases.append({**case, "prompt": prompt, "tokens": list(tokens), "n_gen": N_GEN, "replay": True,
                      "chunked": case["id"] != "p4-20k", "complete": case["id"] in ("p1-chat-thinking", "p2-code")})
    # Sliding-window and compression edges: the 2k prompt cut to 127, 128, and 129 tokens.
    # Eight decode steps cross the 128-slot ring and alternate odd and even ratio-2 groups.
    base = next(c for c in cases if c["id"] == "p3-2k")
    for n in (127, 128, 129):
        cases.append({"id": f"e-window-{n}", "thinking_mode": None, "prompt": "", "tokens": base["tokens"][:n],
                      "n_gen": 8, "replay": True, "chunked": True, "complete": False})
    # Engram normalization and history: case, accents, whitespace, NFKC forms, repeated n-grams.
    messages = [{"role": "user", "content": "Repeat this text exactly: " + ENGRAM_TEXT}]
    prompt, tokens, _t, _i = generate.prepare_case({"messages": messages}, "chat", tokenizer, args)
    cases.append({"id": "e-engram", "thinking_mode": "chat", "messages": messages, "prompt": prompt,
                  "tokens": list(tokens), "n_gen": 8, "replay": True, "chunked": True, "complete": False})
    return cases


def holdout_cases(spec, reference: Path, generate, tokenizer, args) -> list[dict]:
    """G1 v2 holdout cases (#12405). A case has messages and a thinking mode, or
    names an earlier case (`prompt_of`) and keeps its first `cut` tokens. A message's
    content may be a list of strings and {"reference_file": PATH} parts, joined."""
    def text(part):
        if isinstance(part, str):
            return part
        return (reference / part["reference_file"]).read_text()

    cases = []
    for c in spec["cases"]:
        if "prompt_of" in c:
            base = next(x for x in cases if x["id"] == c["prompt_of"])
            cases.append({"id": c["id"], "thinking_mode": None, "prompt": "", "tokens": base["tokens"][: c["cut"]],
                          "n_gen": c["n_gen"], "replay": True, "chunked": False, "complete": False})
            continue
        messages = [{"role": m["role"], "content": "".join(text(p) for p in m["content"])
                     if isinstance(m["content"], list) else m["content"]} for m in c["messages"]]
        prompt, tokens, _t, images = generate.prepare_case({"messages": messages}, c["thinking_mode"], tokenizer, args)
        assert images is None or not any(images)
        cases.append({"id": c["id"], "thinking_mode": c["thinking_mode"], "messages": messages, "prompt": prompt,
                      "tokens": list(tokens), "n_gen": c["n_gen"], "replay": True, "chunked": False,
                      "complete": False})
    return cases


MODES = {
    "decode": "prefill the prompt in one forward, then one decode step per generated token (cached decode)",
    "replay": "one prefill over the fixed sequence (prompt plus the decode run's greedy tokens): "
    "teacher-forced, with the same captures and full logits from position L-1 on",
    "chunked": "prefill the first half of the prompt, then one token per step through the fixed sequence; "
    "the reference has no multi-token step after position 0, so this is its chunked prefill",
}

# What each boundary computes in, from the pinned inference/model.py and kernel.py.
PRECISION = {
    "default_dtype": "bfloat16 (torch.set_default_dtype in generate.main); activations between modules are BF16",
    "embed": "BF16 table lookup; TP8 ranks zero rows they don't own and all_reduce (exact: one nonzero term)",
    "rmsnorm": "x.float(); x * rsqrt(mean(x^2) + eps), eps = norm_eps = 1e-20; times the BF16 weight in F32; cast to the input dtype",
    "act_quant (FP8 activations)": "per row, blocks of 32 along K; amax = max(|x|, 1e-4) in F32; scale = 2^ceil(log2(amax / 448)) "
    "(ue8m0, power of two); q = clamp(x / scale, -448, 448) cast to E4M3 (round to nearest even); scale stored as E8M0",
    "fp8_gemm": "E4M3 x E4M3 tensor-core products accumulated in F32 per 32-wide K block, then scaled by "
    "act_scale[row, k] * weight_scale[n // 32, k] and added to a second F32 accumulator; output BF16",
    "fp4_gemm (routed experts)": "FP8 activations as above times E2M1 weights with one E8M0 scale per 32 along K; "
    "F32 accumulation; output BF16. Nibbles: the low nibble is element 2i",
    "row_parallel_linear": "partial BF16 outputs cast to F32, all_reduce in F32 over 8 ranks, cast back to BF16",
    "wo_a": "convert.py dequantizes it to BF16 (weight.float() * scale.float()); the attention output is "
    "quantized and dequantized in place (act_quant inplace, block 32) before an einsum with it",
    "attention": "q from wq_b; RoPE on the last 64 dims in F32 via complex multiply, written back to BF16; window KV "
    "quantized and dequantized to FP8 in place (block 32, ue8m0); sparse_attn TileLang kernel with F32 softmax and attn_sink",
    "compressor": "ratio > 1: wkv and wgate in F32 (weights promoted), softmax over the group in F32, then RMSNorm; "
    "ratio 1: BF16 projection then RMSNorm. Output before RoPE",
    "compressed_kv": "after RoPE, FP4 quantize and dequantize in place with groups of 16 and E4M3 scales",
    "indexer": "q and k FP4 quantize and dequantize in place (groups of 32, E8M0); scores = sum over heads of "
    "relu(q.k) * weights_proj(x) * index_head_dim^-0.5 * n_heads^-0.5, in BF16 einsum then all_reduce",
    "hc_mixes": "x.flatten(2).float(); rsqrt(mean(x^2) + 1e-20); F.linear with the F32 hc_fn; hc_split_sinkhorn "
    "TileLang kernel in F32 (20 iterations, eps 1e-6) gives pre, post, comb in F32",
    "hc_pre": "sum(pre_mix * x.float()) over the 4 copies in F32, cast to BF16",
    "hc_post": "post * x + sum(comb * residual) in F32, cast to BF16",
    "engram": "FP8 rows times E8M0 scales (blocks of 32) in F32, cast to BF16; wkv FP8 linear; the gate in F32: "
    "sigmoid(signed sqrt(max(|dot|, 1e-6))) with dot normalized per copy; output cast to the stream dtype",
    "gate": "F.linear(x.float(), weight.float()) in F32; scores = sqrt(softplus(.)); top-6 of scores + bias; weights = "
    "unbiased scores gathered, divided by (sum + 1e-20), times route_scale 1.5",
    "expert": "w1 and w3 outputs to F32; up clamped to [-10, 10], gate clamped above at 10; silu(gate) * up * "
    "combine weight in F32; cast to BF16 before w2",
    "moe_sum": "routed outputs added into an F32 buffer in expert-id order, all_reduce in F32, plus the shared expert, cast to BF16",
    "head": "F32 weight (BF16 in the checkpoint); logits = F.linear(x.float(), weight) in F32; all_gather over ranks",
}

TIE_RULES = {
    "gate": "experts = torch.topk(scores + bias, 6) on CUDA. Its order among exactly equal values is unspecified, "
    "so a Psionic match is required only when the 6th and 7th biased scores differ; selection_margins records "
    "the smallest gap per layer and near_ties every row within 1e-4. The order inside the six doesn't change "
    "which experts run, but the F32 sum adds them in expert-id order",
    "indexer": "topk(min(512, visible positions)) of the masked scores, re-sorted by position; same tie caveat; "
    "margins are the k-th against the (k+1)-th score",
    "candidates": "layer 20 keeps the 2,048 best 8-token blocks by max score, with the newest partly filled block "
    "pinned in; margins are the 2,048th against the 2,049th block score",
}


class Recorder:
    """Holds the step context and every capture. Rank 0 keeps replicated values; a rank keeps
    the routed-expert values for the experts it owns."""

    def __init__(self, rank: int):
        self.rank = rank
        self.prompt = ""
        self.mode = ""  # decode, replay, chunked, complete
        self.step = 0
        self.rows: list[tuple[str, int]] = []  # (label, row in this forward) to capture
        self.tracking = False  # residual hashes, router ids, and near ties for this forward
        self.full_logit_rows: list[tuple[str, int]] = []  # replay: logits at these rows
        self.tensors: dict[str, object] = {}
        self.hashes: dict[str, str] = {}
        self.router_ids: dict[str, list] = {}
        self.ties: list[dict] = []
        self.margins: dict[str, float] = {}
        self.gate_indices = None  # current layer's [n, topk] from the gate hook
        self.router_tops = 0  # extra scores past top-k kept per row (--router-tops)
        self.router_tops_scores: dict[str, list] = {}
        self.router_tops_ids: dict[str, list] = {}
        self.expert_targets: dict[int, list[tuple[str, int]]] = {}  # expert id -> [(label, row)]

    def begin(self, prompt: str, mode: str, step: int, rows, tracking: bool, full_logit_rows=()):
        self.prompt, self.mode, self.step = prompt, mode, step
        self.rows, self.tracking, self.full_logit_rows = list(rows), tracking, list(full_logit_rows)

    def key(self, label: str, site: str) -> str:
        return f"{self.prompt}.{self.mode}.{label}.{site}"

    def put(self, label: str, site: str, t, replicated: bool = True):
        if replicated and self.rank != 0:
            return
        self.tensors[self.key(label, site)] = t.detach().to("cpu", copy=True).contiguous()

    def put_rows(self, site: str, t, row_dim: int = 1, replicated: bool = True):
        """t has a batch dim of 1 and the sequence at `row_dim`; store each labeled row."""
        for label, row in self.rows:
            self.put(label, site, t.select(0, 0).select(row_dim - 1, row), replicated)

    def put_hash(self, site: str, t):
        if self.rank == 0 and self.tracking:
            self.hashes[f"{self.prompt}.{self.mode}.step{self.step:02d}.{site}"] = sha256(tensor_bytes(t))

    def margin(self, kind: str, layer: int, gaps, eps: float):
        """Record the smallest selection margin of this forward and every row closer than eps."""
        if self.rank != 0 or not self.tracking or gaps.numel() == 0:
            return
        import torch

        gaps = gaps.flatten().float()
        finite = torch.isfinite(gaps)
        if not finite.any():
            return
        g = torch.where(finite, gaps, torch.inf)
        low = float(g.min())
        k = f"{self.prompt}.{self.mode}.{kind}.layers.{layer}"
        self.margins[k] = min(self.margins.get(k, float("inf")), low)
        for row in (g < eps).nonzero().flatten().tolist()[:64]:
            self.ties.append(
                {"prompt": self.prompt, "mode": self.mode, "step": self.step, "kind": kind,
                 "layer": layer, "row": row, "margin": float(g[row])}
            )


def install_hooks(model, rec: Recorder, kernel, args_cfg, shared_attn):
    import torch
    import torch.nn.functional as F

    n_layers = len(model.layers)

    def keep(layers):
        return [i for i in layers if i < n_layers]

    # embeddings
    model.embed.register_forward_hook(lambda m, a, out: rec.put_rows("embed.out", out))

    # final norm (collapsed stream, before the head); a replay also takes the full logits of
    # its comparison rows through the reference's own head
    def norm_hook(m, a, out):
        rec.put_rows("norm.out", out)
        if rec.full_logit_rows:
            rows = torch.tensor([r for _, r in rec.full_logit_rows], device=out.device)
            logits = model.head(out[:, rows], full_logits=True)
            for j, (label, _) in enumerate(rec.full_logit_rows):
                rec.put(label, "logits", logits[0, j].float())

    model.norm.register_forward_hook(norm_hook)

    for i, block in enumerate(model.layers):
        # per-layer residual hash for every step (whole prompt at prefill)
        def block_hash(m, a, out, i=i):
            rec.put_hash(f"layers.{i}.out", out[0])

        block.register_forward_hook(block_hash)

        # router ids for every token at every layer
        def gate_all(m, a, out, i=i):
            weights, indices = out
            rec.gate_indices = indices
            if rec.rank == 0 and rec.tracking:
                k = f"{rec.prompt}.{rec.mode}.router_ids.layers.{i}"
                rec.router_ids.setdefault(k, []).append(indices.detach().to("cpu", torch.int32))
            if rec.tracking:
                # Selection margin: the 6th against the 7th biased score. torch.topk's order among
                # exact ties is unspecified, so a margin of 0 makes the expert set ambiguous.
                x = a[0]
                biased = F.softplus(F.linear(x.float(), m.weight.float()) / m.gate_temp).sqrt() + m.bias
                top = biased.topk(m.topk + 1, dim=-1).values
                rec.margin("gate", i, top[:, m.topk - 1] - top[:, m.topk], GATE_TIE_EPS)
                if rec.router_tops and rec.rank == 0:
                    # G1 v2 (#12405): every row's best scores and ids, so a near-tie
                    # substitution's oracle-score regret can be computed
                    tops = biased.topk(m.topk + rec.router_tops, dim=-1)
                    k = f"{rec.prompt}.{rec.mode}.router_tops.layers.{i}"
                    rec.router_tops_scores.setdefault(k, []).append(tops.values.detach().float().cpu())
                    rec.router_tops_ids.setdefault(k, []).append(tops.indices.detach().to("cpu", torch.int32))
            # each labeled row's top-1 expert is the one whose steps get recorded
            rec.expert_targets = {}
            if i in EXPERT_LAYERS:
                for label, row in rec.rows:
                    rec.expert_targets.setdefault(int(indices[row, 0]), []).append((label, row))

        block.ffn.gate.register_forward_hook(gate_all)

    for i in keep(HC_LAYERS):
        block = model.layers[i]
        p = f"layers.{i}.hc"
        state = {"mix": 0, "pre": 0, "post": 0}

        def pre_hook(m, a, p=p, state=state):
            state.update(mix=0, pre=0, post=0)
            x, _start, pre_mix = a[0], a[1], a[2]
            rec.put_rows(f"{p}.in", x)
            rec.put_rows(f"{p}.pre_mix_in", pre_mix)

        block.register_forward_pre_hook(pre_hook)

        def post_hook(m, a, out, p=p):
            rec.put_rows(f"{p}.out", out[0])
            rec.put_rows(f"{p}.pre_mix_out", out[1])

        block.register_forward_hook(post_hook)

        def wrap_mixes(fn, p=p, state=state):
            def wrapped(*a, **k):
                pre, post, comb = fn(*a, **k)
                part = ("attn", "ffn")[state["mix"]]
                state["mix"] += 1
                rec.put_rows(f"{p}.{part}.pre", pre)
                rec.put_rows(f"{p}.{part}.post", post)
                rec.put_rows(f"{p}.{part}.comb", comb)
                return pre, post, comb

            return wrapped

        def wrap_pre(fn, p=p, state=state):
            def wrapped(*a, **k):
                y = fn(*a, **k)
                # the third call is Transformer.forward collapsing the last layer's stream for the head
                part = ("attn", "ffn", "final")[state["pre"]]
                state["pre"] += 1
                rec.put_rows(f"{p}.{part}.collapsed", y)
                return y

            return wrapped

        def wrap_post(fn, p=p, state=state):
            def wrapped(*a, **k):
                y = fn(*a, **k)
                part = ("attn", "ffn")[state["post"]]
                state["post"] += 1
                rec.put_rows(f"{p}.{part}.expanded", y)
                return y

            return wrapped

        block.hc_mixes = wrap_mixes(block.hc_mixes)
        block.hc_pre = wrap_pre(block.hc_pre)
        block.hc_post = wrap_post(block.hc_post)

    # Engram: input stream, hash ids, dequantized rows, output stream
    for block in model.layers:
        eng = block.engram
        if eng is None:
            continue
        p = f"layers.{block.layer_id}.engram"

        def eng_pre(m, a, p=p):
            rec.put_rows(f"{p}.in", a[0])
            rec.put_rows(f"{p}.hash_ids", a[1])

        eng.register_forward_pre_hook(eng_pre)
        eng.register_forward_hook(lambda m, a, out, p=p: rec.put_rows(f"{p}.out", out))
        eng.embed.register_forward_hook(lambda m, a, out, p=p: rec.put_rows(f"{p}.rows", out))

    # attention input (after attn_norm) and output (after wo_b and its all_reduce)
    for i in keep(ATTN_LAYERS):
        attn = model.layers[i].attn
        p = f"layers.{i}.attn"
        attn.register_forward_pre_hook(lambda m, a, p=p: rec.put_rows(f"{p}.in", a[0]))
        attn.register_forward_hook(lambda m, a, out, p=p: rec.put_rows(f"{p}.out", out))

    # compressor latents (before RoPE): prefill keeps the first and last complete group;
    # a decode step keeps the latent when its group completes
    for i in keep(COMPRESSOR_LAYERS):
        comp = model.layers[i].attn.compressor
        if comp is None:
            continue
        p = f"layers.{i}.attn.compressor"

        def comp_hook(m, a, out, p=p):
            if out is None or rec.rank != 0:
                return
            if rec.step == 0:
                rec.put("prefill", f"{p}.group0", out[0, 0])
                rec.put("prefill", f"{p}.group{out.size(1) - 1}", out[0, -1])
            elif rec.rows:
                rec.put(rec.rows[0][0], f"{p}.latent", out[0, 0])

        comp.register_forward_hook(comp_hook)

    # indexer: selected compressed positions, masked scores (patched copy), candidates
    for idx_mod in [b.attn.indexer for b in model.layers if b.attn.indexer is not None]:
        layer = next(i for i, b in enumerate(model.layers) if b.attn.indexer is idx_mod)
        wanted = True  # every index source: each one's selection is reused by the layers after it

        def idx_hook(m, a, out, layer=layer, wanted=wanted):
            score = getattr(m, "dump_index_score", None)
            m.dump_index_score = None
            if not wanted:
                return
            p = f"layers.{layer}.attn.indexer"
            if score is not None and rec.tracking:
                k = out.size(-1)
                if score.size(-1) > k:
                    top = score.topk(k + 1, dim=-1).values
                    rec.margin("indexer", layer, top[..., k - 1] - top[..., k], INDEX_TIE_EPS)
                if m.is_candidate_source:
                    width = score.size(-1)
                    blocks = F.pad(score, (0, -width % m.candidate_block_size), value=-torch.inf)
                    blocks = blocks.unflatten(-1, (-1, m.candidate_block_size)).amax(dim=-1)
                    if blocks.size(-1) > m.candidate_topk_blocks:
                        top = blocks.topk(m.candidate_topk_blocks + 1, dim=-1).values
                        rec.margin(
                            "candidates", layer,
                            top[..., m.candidate_topk_blocks - 1] - top[..., m.candidate_topk_blocks],
                            INDEX_TIE_EPS,
                        )
            rec.put_rows(f"{p}.topk_idxs", out)
            if score is not None:
                rec.put_rows(f"{p}.score", score)
            if m.is_candidate_source and shared_attn.candidates is not None:
                cand = shared_attn.candidates
                for label, row in rec.rows:
                    mask = cand[0, row] if cand.dim() == 3 else cand.reshape(-1, cand.size(-1))[row]
                    blocks = mask[:: m.candidate_block_size].nonzero().flatten().to(torch.int32)
                    rec.put(label, f"{p}.candidate_blocks", blocks)

        idx_mod.register_forward_hook(idx_hook)

    # gate scores, ids and weights; MoE and shared-expert outputs; one routed expert per row
    for i in keep(GATE_LAYERS):
        moe = model.layers[i].ffn
        p = f"layers.{i}.ffn"

        def gate_hook(m, a, out, p=p):
            x = a[0]
            weights, indices = out
            # the reference's own scoring (Gate.forward), recomputed from its input
            scores = F.softplus(F.linear(x.float(), m.weight.float()) / m.gate_temp).sqrt()
            for label, row in rec.rows:
                rec.put(label, f"{p}.gate.in", x[row])
                rec.put(label, f"{p}.gate.scores", scores[row])
                rec.put(label, f"{p}.gate.biased_scores", scores[row] + m.bias)
                rec.put(label, f"{p}.gate.ids", indices[row].to(torch.int32))
                rec.put(label, f"{p}.gate.weights", weights[row])

        moe.gate.register_forward_hook(gate_hook)
        moe.register_forward_hook(lambda m, a, out, p=p: rec.put_rows(f"{p}.out", out))
        def shared_hook(m, a, out, p=p):
            for label, row in rec.rows:
                rec.put(label, f"{p}.shared.out", out[row])

        moe.shared_experts.register_forward_hook(shared_hook)

    for i in keep(EXPERT_LAYERS):
        moe = model.layers[i].ffn
        p = f"layers.{i}.ffn.expert"

        for e, expert in enumerate(moe.experts):
            if expert is None:
                continue
            picked: list[tuple[str, int]] = []  # (label, row within this expert's routed batch)

            def expert_pre(m, a, e=e, p=p, picked=picked):
                picked.clear()
                targets = rec.expert_targets.get(e)
                if not targets:
                    return
                routed = torch.where(rec.gate_indices == e)[0]
                for label, row in targets:
                    j = int((routed == row).nonzero()[0, 0])
                    picked.append((label, j))
                    x, w = a[0][j : j + 1], a[1][j]
                    rec.put(label, f"{p}.id", torch.tensor([e], dtype=torch.int32), replicated=False)
                    rec.put(label, f"{p}.in", x[0], replicated=False)
                    rec.put(label, f"{p}.combine_weight", w, replicated=False)
                    # linear() quantizes each row in blocks of 32 the same way, whatever the batch
                    xq, xs = kernel.act_quant(x, 32, "ue8m0", torch.float8_e8m0fnu)
                    rec.put(label, f"{p}.in_fp8", xq[0], replicated=False)
                    rec.put(label, f"{p}.in_fp8_scale", xs[0], replicated=False)

            def sub_out(name, p=p, picked=picked):
                def hook(m, a, out):
                    for label, j in picked:
                        rec.put(label, f"{p}.{name}_out", out[j], replicated=False)
                        if name == "w2":
                            h = a[0][j : j + 1]
                            rec.put(label, f"{p}.w2_in", h[0], replicated=False)
                            hq, hs = kernel.act_quant(h, 32, "ue8m0", torch.float8_e8m0fnu)
                            rec.put(label, f"{p}.w2_in_fp8", hq[0], replicated=False)
                            rec.put(label, f"{p}.w2_in_fp8_scale", hs[0], replicated=False)

                return hook

            expert.register_forward_pre_hook(expert_pre)
            for name in ("w1", "w3", "w2"):
                getattr(expert, name).register_forward_hook(sub_out(name))
            def expert_out(m, a, out, p=p, picked=picked):
                for label, j in picked:
                    rec.put(label, f"{p}.out", out[j], replicated=False)

            expert.register_forward_hook(expert_out)


def load_reference(reference: Path, out: Path, rank: int):
    """Import the pinned modules, with model.py from a copy that carries PATCH."""
    inference = reference / "inference"
    patched_dir = out / "patched"
    source = (inference / "model.py").read_text()
    assert source.count(PATCH_ANCHOR) == 1, "the patch anchor moved; is this the pinned model.py?"
    patched = source.replace(PATCH_ANCHOR, PATCH)
    if rank == 0:
        patched_dir.mkdir(parents=True, exist_ok=True)
        (patched_dir / "model.py").write_text(patched)
        diff = difflib.unified_diff(
            source.splitlines(True), patched.splitlines(True), "inference/model.py", "patched/model.py"
        )
        (out / "model.py.diff").write_text("".join(diff))
    if int(os.getenv("WORLD_SIZE", "1")) > 1:
        import torch.distributed as dist

        dist.barrier()
    sys.path[:0] = [str(patched_dir), str(inference), str(reference / "encoding")]
    import generate  # noqa: F401  (also puts encoding/ on the path, as the reference does)
    import kernel
    import model

    assert Path(model.__file__).resolve() == (patched_dir / "model.py").resolve()
    return model, kernel, generate


def install_attention_diagnostics(model, rec: Recorder, model_mod):
    """Copy the first attention stages before in-place RoPE and quantization.

    Rank-local head tensors retain their rank suffix. The reference's forward
    and kernels run unchanged; no diagnostic recomputation replaces an output.
    """
    active = {"layer": None}
    for i, block in enumerate(model.layers):
        attn = block.attn
        prefix = f"layers.{i}.attn"

        def enter(m, a, i=i):
            active["layer"] = i

        attn.register_forward_pre_hook(enter)
        attn.register_forward_hook(lambda m, a, out: active.update(layer=None))
        for member, site, local in [
            ("q_norm", "q.normalized", False),
            ("wq_b", "q.projected", True),
            ("kv_norm", "kv.normalized", False),
        ]:
            suffix = f".rank{rec.rank}" if local else ""
            getattr(attn, member).register_forward_hook(
                lambda m, a, out, site=f"{prefix}.{site}{suffix}", local=local:
                    rec.put_rows(site, out, replicated=not local)
            )
        attn.wo_b.register_forward_pre_hook(
            lambda m, a, prefix=prefix:
                rec.put_rows(f"{prefix}.output.low_rank.rank{rec.rank}", a[0], replicated=False)
        )

    sparse = model_mod.sparse_attn

    def observed_sparse(q, kv, sinks, indices, scale):
        prefix = f"layers.{active['layer']}.attn"
        rec.put_rows(f"{prefix}.q.rotated.rank{rec.rank}", q, replicated=False)
        # In prefill, window KV rows coincide with query rows. Decode has a ring.
        if rec.step == 0:
            rec.put_rows(f"{prefix}.kv.quantized", kv)
        out = sparse(q, kv, sinks, indices, scale)
        rec.put_rows(f"{prefix}.heads.rank{rec.rank}", out, replicated=False)
        return out

    model_mod.sparse_attn = observed_sparse
    quantize = model_mod.act_quant

    def observed_quantize(x, *args, **kwargs):
        layer = active["layer"]
        if layer is not None:
            prefix = f"layers.{layer}.attn"
            if x.dim() == 3 and x.size(-1) == model.layers[layer].attn.head_dim:
                rec.put_rows(f"{prefix}.kv.rotated", x)
            if x.dim() == 4:
                rec.put_rows(f"{prefix}.heads.unrotated.rank{rec.rank}", x, replicated=False)
        return quantize(x, *args, **kwargs)

    model_mod.act_quant = observed_quantize
    reduce = model_mod.dist.all_reduce

    def observed_reduce(tensor, *args, **kwargs):
        import torch

        layer = active["layer"]
        if layer is not None and tensor.dim() == 3:
            prefix = f"layers.{layer}.attn"
            if tensor.dtype == torch.bfloat16:
                rec.put_rows(f"{prefix}.indexer.rank_partial.rank{rec.rank}", tensor, replicated=False)
            elif tensor.size(-1) == model.layers[layer].attn.dim:
                rec.put_rows(f"{prefix}.output.rank_partial.rank{rec.rank}", tensor, replicated=False)
        return reduce(tensor, *args, **kwargs)

    model_mod.dist.all_reduce = observed_reduce


def environment(reference: Path) -> dict:
    import torch

    env = {
        "python": platform.python_version(),
        "torch": torch.__version__,
        "cuda_runtime": torch.version.cuda,
        "gpu": torch.cuda.get_device_name(0),
        "gpu_capability": list(torch.cuda.get_device_capability(0)),
        "gpu_count": torch.cuda.device_count(),
        "nccl_version": torch.cuda.nccl.version(),
        "collective_controls": {name: os.getenv(name) for name in
            ["NCCL_ALGO", "NCCL_PROTO", "NCCL_NVLS_ENABLE", "NCCL_DEBUG", "NCCL_DEBUG_SUBSYS"]},
        "packages": {},
    }
    for name in ["tilelang", "safetensors", "transformers", "tokenizers", "numpy", "sympy", "triton"]:
        try:
            env["packages"][name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            pass
    try:
        env["nvidia_smi"] = subprocess.run(
            ["nvidia-smi", "--query-gpu=name,driver_version,memory.total", "--format=csv,noheader"],
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout.strip().splitlines()
        env["nvcc"] = subprocess.run(["nvcc", "--version"], capture_output=True, text=True, timeout=30).stdout.strip().splitlines()[-1:]
    except (OSError, subprocess.TimeoutExpired):
        pass
    files = {}
    for sub in ["inference", "encoding"]:
        for f in sorted((reference / sub).rglob("*")):
            if f.is_file() and "__pycache__" not in f.parts:
                files[f.relative_to(reference).as_posix()] = sha256(f.read_bytes())
    env["reference_files"] = files
    tool = Path(__file__).resolve()
    env["dump_tool_sha256"] = sha256(tool.read_bytes())
    env["dump_tool_commit"] = os.getenv("DUMP_TOOL_COMMIT", "")
    return env


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ckpt", type=Path, help="convert.py output (model{rank}-mp{world}.safetensors)")
    parser.add_argument("--reference", type=Path, required=True, help="the checkpoint directory with inference/ and encoding/")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--config", type=Path, help="defaults to REFERENCE/inference/config.json")
    parser.add_argument("--max-seq-len", type=int, default=32768)
    parser.add_argument("--only", default="", help="comma-separated prompt ids to run")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--attention-diagnostics", action="store_true",
                        help="capture query, KV, and sparse output boundaries on every rank")
    parser.add_argument("--captures-only", action="store_true",
                        help="omit completion-to-EOS and the separate chunked mode")
    parser.add_argument("--extra-rows", default="",
                        help="diagnostic positions captured beside the standard labels, ID:A-B,C;ID2:D")
    parser.add_argument("--upload", default="", help="gs:// prefix to copy each finished prompt to")
    parser.add_argument("--cases-file", type=Path,
                        help="run these cases instead (G1 v2 holdout, #12405): a JSON list of id, "
                             "thinking_mode, and messages, or prompt_of and cut for a cut prompt")
    parser.add_argument("--router-tops", type=int, default=0,
                        help="also keep each row's best top-k + N biased router scores and ids (#12405)")
    opts = parser.parse_args()

    world_size = int(os.getenv("WORLD_SIZE", "1"))
    rank = int(os.getenv("RANK", "0"))
    local_rank = int(os.getenv("LOCAL_RANK", "0"))
    import torch
    import torch.distributed as dist

    if world_size > 1:
        dist.init_process_group("nccl")
    torch.cuda.set_device(local_rank)
    opts.out.mkdir(parents=True, exist_ok=True)
    model_mod, kernel, generate = load_reference(opts.reference, opts.out, rank)

    # the same process setup as generate.main
    torch.cuda.memory._set_allocator_settings("expandable_segments:True")
    torch.set_default_dtype(torch.bfloat16)
    torch.set_num_threads(8)
    torch.manual_seed(33377335)

    if opts.self_test:
        args = model_mod.ModelArgs(candidate_source_layer=3, candidate_topk_blocks=4, candidate_block_size=2)
        tokenizer = None
    else:
        from transformers import AutoTokenizer

        cfg = json.loads((opts.config or opts.reference / "inference" / "config.json").read_text())
        args = model_mod.ModelArgs(**cfg)
        tokenizer = AutoTokenizer.from_pretrained(opts.ckpt)
    args.temperature = 0  # greedy: sample() takes the argmax
    args.max_batch_size = 1
    args.max_seq_len = opts.max_seq_len

    t0 = time.time()
    with torch.device("cuda"):
        model = model_mod.Transformer(args, tokenizer)
    if not opts.self_test:
        from safetensors.torch import load_model

        load_model(model, str(opts.ckpt / f"model{rank}-mp{world_size}.safetensors"))
    torch.set_default_device("cuda")
    load_seconds = time.time() - t0

    rec = Recorder(rank)
    rec.router_tops = opts.router_tops
    install_hooks(model, rec, kernel, args, model_mod.shared_attn)
    if opts.attention_diagnostics:
        install_attention_diagnostics(model, rec, model_mod)

    env = environment(opts.reference) if rank == 0 else {}
    if rank == 0:
        header = {
            "schema": "openagents.deepseek_v41.oracle_run.v1",
            "self_test": opts.self_test,
            "world_size": world_size,
            "max_seq_len": args.max_seq_len,
            "load_seconds": round(load_seconds, 1),
            "labels": "first = prompt position 0; last = position L-1; gK = the forward whose input is generated token K, at position L+K-1",
            "modes": MODES,
            "precision": PRECISION,
            "tie_rules": TIE_RULES,
            "environment": env,
        }
        (opts.out / "run.json").write_text(json.dumps(header, indent=1) + "\n")
        upload(opts, opts.out / "run.json", "run.json")
        if (opts.out / "model.py.diff").exists():
            upload(opts, opts.out / "model.py.diff", "model.py.diff")

    if opts.self_test:
        g = torch.Generator().manual_seed(0)
        cases = [
            {"id": "s1", "tokens": torch.randint(0, args.vocab_size, (40,), generator=g, device="cpu").tolist(), "n_gen": 4,
             "replay": True, "chunked": True, "complete": False},
            {"id": "s2", "tokens": torch.randint(0, args.vocab_size, (300,), generator=g, device="cpu").tolist(), "n_gen": 4,
             "replay": True, "chunked": False, "complete": False},
        ]
    elif opts.cases_file:
        cases = holdout_cases(json.loads(opts.cases_file.read_text()), opts.reference, generate, tokenizer, args)
    else:
        cases = build_cases(opts.reference, generate, tokenizer, args)
    if opts.only:
        cases = [c for c in cases if c["id"] in opts.only.split(",")]
    if opts.captures_only:
        for case in cases:
            case.update(complete=False, chunked=False)
    for pid, rows in parse_extra_rows(opts.extra_rows).items():
        for case in cases:
            if case["id"] == pid:
                case["extra_rows"] = rows

    for case in cases:
        pid = case["id"]
        case_dir = opts.out / pid
        done = case_dir / "run.json"
        if done.exists() and all((case_dir / f"tensors-rank{r}.safetensors").exists() for r in range(world_size)):
            if rank == 0:
                print(f"{pid}: already dumped, skipping", flush=True)
            continue
        case_dir.mkdir(parents=True, exist_ok=True)
        result = run_case(model, rec, case, tokenizer, args, generate)
        save_case(opts, rec, case_dir, result, rank, world_size)

    if world_size > 1:
        dist.barrier()
        dist.destroy_process_group()
    return 0


def labels_for(L: int, n_gen: int, extra=()) -> list[tuple[str, int]]:
    """Labeled positions in the fixed sequence: (label, position). `extra` adds
    diagnostic positions: `pN` inside the prompt, `gK` from L on (the names the
    CPU qualifier already gives every generated position)."""
    out = [("first", 0), ("last", L - 1)]
    for k in sorted({1, 2, n_gen}):
        out.append((f"g{k}", L + k - 1))
    taken = {pos for _, pos in out}
    for pos in sorted(set(extra)):
        if pos in taken or not 0 <= pos < L + n_gen:
            continue
        out.append((f"p{pos}" if pos < L else f"g{pos - L + 1}", pos))
    return out


def parse_extra_rows(spec: str) -> dict[str, list[int]]:
    """`ID:A-B,C;ID2:D` -> {ID: [A..B, C], ID2: [D]}."""
    rows: dict[str, list[int]] = {}
    for part in filter(None, spec.split(";")):
        pid, _, ranges = part.partition(":")
        for r in filter(None, ranges.split(",")):
            lo, _, hi = r.partition("-")
            rows.setdefault(pid, []).extend(range(int(lo), int(hi or lo) + 1))
    return rows


def run_case(model, rec, case, tokenizer, args, generate):
    import torch

    pid, tokens, n_gen = case["id"], list(case["tokens"]), case["n_gen"]
    L = len(tokens)
    assert L + COMPLETE_CAP + 2 <= args.max_seq_len or not case.get("complete")
    assert L + n_gen + 1 <= args.max_seq_len
    labeled = labels_for(L, n_gen, case.get("extra_rows", ()))
    gen_label = {pos - L + 1: label for label, pos in labeled if pos >= L - 1 and label != "last"}
    result = {"id": pid, "thinking_mode": case.get("thinking_mode"), "prompt_tokens": L, "prompt_token_ids": tokens,
              "prompt_text_sha256": sha256(case.get("prompt", "").encode()), "n_gen": n_gen,
              "labels": dict(labeled)}

    def step_entry(step, position, inp_token, next_token, logits):
        top = logits.topk(TOPK_LOGITS)
        return {"step": step, "position": position, "input_token": inp_token, "argmax": next_token,
                "logits_sha256": sha256(tensor_bytes(logits)), "top_ids": top.indices.tolist(),
                "top_values": top.values.tolist()}

    # 1. decode: prefill the prompt, then greedy decode steps taking g1..g{n_gen}; on for
    #    `complete`, keep going without captures until EOS or COMPLETE_CAP tokens
    t1 = time.time()
    eos = tokenizer.eos_token_id if tokenizer is not None else -1
    generated, steps = [], []
    inp, start, step = torch.tensor([tokens], dtype=torch.long), 0, 0
    while True:
        capture = step <= n_gen
        if step == 0:
            rows = [(label, pos) for label, pos in labeled if pos < L]
        else:
            rows = [(gen_label[step], 0)] if step in gen_label else []
        rec.begin(pid, "decode", step, rows if capture else [], tracking=capture)
        next_token, logits, _ = model.forward(inp, start)
        logits = logits[0].float()
        token = int(next_token[0])
        if capture:
            steps.append(step_entry(step, start + inp.size(1) - 1, int(inp[0, -1]), token, logits))
            rec.put("last" if step == 0 else f"g{step}", "logits", logits)
        generated.append(token)
        start += inp.size(1)
        step += 1
        if step > n_gen and (not case.get("complete") or eos in generated or len(generated) >= COMPLETE_CAP):
            break
        inp = torch.tensor([[token]], dtype=torch.long)
    torch.cuda.synchronize()
    result["decode_seconds"] = round(time.time() - t1, 2)
    result["generated_ids"] = generated[:n_gen]
    result["extra_step_argmax"] = generated[n_gen]
    result["steps"] = steps
    if tokenizer is not None:
        text_ids = generated[:n_gen]
        cut = text_ids[: text_ids.index(eos)] if eos in text_ids else text_ids
        result["generated_text"] = tokenizer.decode(cut)
        result["generated_text_raw"] = tokenizer.decode(text_ids)
        result["prompt_text"] = case.get("prompt", "")
    if case.get("complete"):
        full = generated[: generated.index(eos) + 1] if eos in generated else generated[:COMPLETE_CAP]
        answer = {"ids": full, "finished": eos in full, "cap": COMPLETE_CAP, "tokens": len(full)}
        if tokenizer is not None:
            answer["text"] = tokenizer.decode(full)
            try:
                from encoding import parse_message_from_completion_text

                answer["parsed"] = parse_message_from_completion_text(
                    answer["text"], thinking_mode=case.get("thinking_mode") or "chat"
                )
            except Exception as error:  # the parser only accepts well-formed output
                answer["parse_error"] = repr(error)
        result["complete_answer"] = answer
    fixed = tokens + generated[:n_gen]  # the fixed sequence later runs are forced through

    # 2. replay: one prefill over the whole fixed sequence, same captures, full logits at
    #    every position from L-1 on
    if case.get("replay", True):
        t2 = time.time()
        logit_rows = [("last", L - 1)] + [(f"g{k}", L + k - 1) for k in range(1, n_gen + 1)]
        rec.begin(pid, "replay", 0, labeled, tracking=True, full_logit_rows=logit_rows)
        next_token, logits, _ = model.forward(torch.tensor([fixed], dtype=torch.long), 0)
        torch.cuda.synchronize()
        result["replay_seconds"] = round(time.time() - t2, 2)
        result["replay_last_argmax"] = int(next_token[0])

    # 3. chunked: prefill the first half, then one token at a time through the fixed sequence.
    #    The reference only prefills from position 0, so this is its chunked form.
    if case.get("chunked"):
        t3 = time.time()
        c = max(1, L // 2)
        result["chunked_prefill"] = c
        chunk_steps = []
        inp, start, step = torch.tensor([fixed[:c]], dtype=torch.long), 0, 0
        while start + inp.size(1) <= len(fixed):
            pos = start + inp.size(1) - 1
            rec.begin(pid, "chunked", step, [], tracking=False)
            next_token, logits, _ = model.forward(inp, start)
            if pos >= L - 1:
                logits = logits[0].float()
                label = "last" if pos == L - 1 else f"g{pos - L + 1}"
                rec.put(label, "logits", logits)
                chunk_steps.append({"position": pos, "argmax": int(next_token[0]),
                                    "logits_sha256": sha256(tensor_bytes(logits))})
            start += inp.size(1)
            step += 1
            if start >= len(fixed):
                break
            inp = torch.tensor([[fixed[start]]], dtype=torch.long)
        torch.cuda.synchronize()
        result["chunked_steps"] = chunk_steps
        result["chunked_seconds"] = round(time.time() - t3, 2)
    return result


def save_case(opts, rec, case_dir, result, rank, world_size):
    """Write this prompt's tensors (each rank) and run.json (rank 0), then upload them,
    so a lost spot machine never repeats a finished prompt."""
    import torch
    import torch.distributed as dist
    from safetensors.torch import save_file

    if rank == 0:
        for k, v in rec.router_ids.items():
            rec.tensors[k] = torch.cat(v)
        for k, v in rec.router_tops_scores.items():
            rec.tensors[k + ".scores"] = torch.cat(v)
        for k, v in rec.router_tops_ids.items():
            rec.tensors[k + ".ids"] = torch.cat(v)
        rec.router_tops_scores, rec.router_tops_ids = {}, {}
    save_file(dict(rec.tensors), str(case_dir / f"tensors-rank{rank}.safetensors"))
    n_tensors = len(rec.tensors)
    rec.tensors, rec.router_ids = {}, {}
    if world_size > 1:
        dist.barrier()
    if rank == 0:
        result["residual_sha256"] = rec.hashes
        result["selection_margins"] = rec.margins
        result["near_ties"] = rec.ties
        (case_dir / "run.json").write_text(json.dumps(result, indent=1) + "\n")
        rec.hashes, rec.margins, rec.ties = {}, {}, []
        text = result.get("generated_text", "")
        print(f"{result['id']}: {result['prompt_tokens']} prompt tokens, decode {result['decode_seconds']} s, "
              f"{n_tensors} tensors on rank 0: {text!r}", flush=True)
        upload(opts, case_dir, result["id"])
    if world_size > 1:
        dist.barrier()


def upload(opts, path: Path, dest: str):
    if not opts.upload:
        return
    target = f"{opts.upload.rstrip('/')}/{dest}"
    argv = ["gcloud", "storage", "rsync", "-r", str(path), target] if path.is_dir() else [
        "gcloud", "storage", "cp", str(path), target]
    proc = subprocess.run(argv, capture_output=True, text=True)
    if proc.returncode:
        print(f"upload of {path} failed: {proc.stderr[-500:]}", file=sys.stderr, flush=True)


if __name__ == "__main__":
    sys.exit(main())
