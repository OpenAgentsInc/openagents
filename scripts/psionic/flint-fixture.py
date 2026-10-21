#!/usr/bin/env python3
"""Generate independent NumPy Flint transformer fixtures (no model download)."""
import json
from pathlib import Path
import numpy as np

c = dict(model_type="flint", hidden_size=4, intermediate_size=6,
         moe_intermediate_size=3, shared_expert_intermediate_size=5,
         num_hidden_layers=3, num_attention_heads=2, num_key_value_heads=1,
         head_dim=2, vocab_size=8, num_experts=3, num_experts_per_tok=2,
         mlp_only_layers=[0], no_rope_layer_indices=[0],
         layer_types=["full_attention", "sliding_attention", "sliding_attention"],
         sliding_window=2, max_position_embeddings=16, rms_norm_eps=1e-6,
         rope_theta=10000.0, router_score_function="sigmoid", norm_topk_prob=True,
         softmax_type="learnable", hidden_act="silu", attention_bias=False,
         tie_word_embeddings=False)
w = {}


def weight(name, shape, norm=False):
    offset = sum(name.encode())
    a = np.sin(np.arange(np.prod(shape), dtype=np.float32) + offset) * .17
    if norm:
        a += 1
    w[name] = a.reshape(shape).astype(np.float32)


weight("model.embed_tokens.weight", (8, 4))
weight("lm_head.weight", (8, 4))
weight("model.norm.weight", (4,), True)
for i in range(3):
    p = f"model.layers.{i}"
    for n in ["input_layernorm", "post_attention_layernorm"]:
        weight(f"{p}.{n}.weight", (4,), True)
    for n, shape in [("q_proj.weight", (4, 4)), ("k_proj.weight", (2, 4)),
                     ("v_proj.weight", (2, 4)), ("o_proj.weight", (4, 4)),
                     ("q_norm.weight", (2,)), ("k_norm.weight", (2,)),
                     ("softmax_offset", (2,))]:
        weight(f"{p}.self_attn.{n}", shape, "norm" in n)
    def mlp(prefix, f):
        for n, shape in [("gate_proj", (f, 4)), ("up_proj", (f, 4)), ("down_proj", (4, f))]:
            weight(f"{prefix}.{n}.weight", shape)
    if i == 0:
        mlp(f"{p}.mlp", 6)
    else:
        weight(f"{p}.mlp.gate.weight", (3, 4))
        mlp(f"{p}.mlp.shared_expert", 5)
        for e in range(3):
            mlp(f"{p}.mlp.experts.{e}", 3)


def norm(x, key):
    return x / np.sqrt(np.mean(x*x, axis=-1, keepdims=True) + 1e-6) * w[key]


def ffn(x, key):
    g = x @ w[key + ".gate_proj.weight"].T
    return (g / (1 + np.exp(-g)) * (x @ w[key + ".up_proj.weight"].T)) @ w[key + ".down_proj.weight"].T


cache = [[] for _ in range(3)]
logits = []
tokens = [1, 3, 2, 5, 4]
for pos, token in enumerate(tokens):
    x = w["model.embed_tokens.weight"][token].copy()
    for i in range(3):
        p = f"model.layers.{i}"
        n = norm(x, p + ".input_layernorm.weight")
        a = p + ".self_attn"
        q = norm((n @ w[a + ".q_proj.weight"].T).reshape(2, 2), a + ".q_norm.weight")
        k = norm((n @ w[a + ".k_proj.weight"].T).reshape(1, 2), a + ".k_norm.weight")
        if i:
            for z in [q, k]:
                first, second = z[:, 0].copy(), z[:, 1].copy()
                z[:, 0] = first * np.cos(pos) - second * np.sin(pos)
                z[:, 1] = first * np.sin(pos) + second * np.cos(pos)
        v = (n @ w[a + ".v_proj.weight"].T).reshape(1, 2)
        cache[i].append((k, v))
        if i:
            cache[i] = cache[i][-2:]
        keys = np.stack([t[0] for t in cache[i]], axis=1).repeat(2, axis=0)
        values = np.stack([t[1] for t in cache[i]], axis=1).repeat(2, axis=0)
        scores = np.einsum("hd,htd->ht", q, keys) / np.sqrt(2)
        scores = np.concatenate([scores, w[a + ".softmax_offset"][:, None]], axis=1)
        probs = np.exp(scores - scores.max(axis=1, keepdims=True))
        probs /= probs.sum(axis=1, keepdims=True)
        out = np.einsum("ht,htd->hd", probs[:, :-1], values).flatten()
        x += out @ w[a + ".o_proj.weight"].T
        n = norm(x, p + ".post_attention_layernorm.weight")
        if not i:
            out = ffn(n, p + ".mlp")
        else:
            scores = 1 / (1 + np.exp(-(n @ w[p + ".mlp.gate.weight"].T)))
            selected = np.argsort(-scores, kind="stable")[:2]
            weights = scores[selected] / scores[selected].sum()
            out = sum(float(s) * ffn(n, p + f".mlp.experts.{e}") for e, s in zip(selected, weights))
            out += ffn(n, p + ".mlp.shared_expert")
        x += out
    logits.append((norm(x, "model.norm.weight") @ w["lm_head.weight"].T).tolist())

dest = Path(__file__).resolve().parents[2] / "crates/psionic/crates/psionic-flint/testdata/tiny.json"
dest.parent.mkdir(parents=True, exist_ok=True)
dest.write_text(json.dumps(dict(config=c, weights={n: dict(shape=list(a.shape), values=a.flatten().tolist()) for n, a in w.items()}, tokens=tokens, logits=logits), indent=2) + "\n")
print(dest)
