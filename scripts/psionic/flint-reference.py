#!/usr/bin/env python3
"""Independent eager PyTorch Flint diagnostic; never part of native serving.

Reads the original checkpoint and writes teacher-forced logits. Uses explicit
attention, dense expert selection, and no Candle or Psionic implementation.
"""
import argparse
import json
from pathlib import Path
import torch
import torch.nn.functional as F
from safetensors import safe_open

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("weights", type=Path)
p.add_argument("request", type=Path)
p.add_argument("output", type=Path)
args = p.parse_args()
torch.backends.cuda.matmul.allow_tf32 = False
c = json.loads((args.weights / "config.json").read_text())
index = json.loads((args.weights / "model.safetensors.index.json").read_text())["weight_map"]
w = {}
for shard in sorted(set(index.values())):
    with safe_open(args.weights / shard, framework="pt", device="cpu") as f:
        for name in f.keys():
            w[name] = f.get_tensor(name).cuda()
    print("loaded", shard, flush=True)
w["lm_head.weight"] = w["lm_head.weight"].float()


def norm(x, key):
    y = x.float()
    return (y * torch.rsqrt((y * y).mean(-1, keepdim=True) + c["rms_norm_eps"]) * w[key].float()).to(x.dtype)


def ffn(x, prefix):
    return F.linear(F.silu(F.linear(x, w[prefix + ".gate_proj.weight"])) * F.linear(x, w[prefix + ".up_proj.weight"]), w[prefix + ".down_proj.weight"])


def rotary(x, position):
    half = c["head_dim"] // 2
    freq = position / (c["rope_theta"] ** (torch.arange(half, device="cuda", dtype=torch.float32) / half))
    cos, sin = freq.cos().to(x.dtype), freq.sin().to(x.dtype)
    a, b = x.chunk(2, dim=-1)
    return torch.cat((a * cos - b * sin, a * sin + b * cos), -1)


@torch.inference_mode()
def run(tokens, output):
    cache = [None] * c["num_hidden_layers"]
    h, kv, d = c["num_attention_heads"], c["num_key_value_heads"], c["head_dim"]
    for position, token in enumerate(tokens):
        x = w["model.embed_tokens.weight"][token].unsqueeze(0)
        for layer in range(c["num_hidden_layers"]):
            prefix = f"model.layers.{layer}"
            n = norm(x, prefix + ".input_layernorm.weight")
            attn = prefix + ".self_attn"
            q = norm(F.linear(n, w[attn + ".q_proj.weight"]).view(h, d), attn + ".q_norm.weight")
            k = norm(F.linear(n, w[attn + ".k_proj.weight"]).view(kv, d), attn + ".k_norm.weight")
            if layer not in c["no_rope_layer_indices"]:
                q, k = rotary(q, position), rotary(k, position)
            k = k.unsqueeze(1)
            v = F.linear(n, w[attn + ".v_proj.weight"]).view(kv, 1, d)
            if cache[layer] is not None:
                k = torch.cat((cache[layer][0], k), dim=1)
                v = torch.cat((cache[layer][1], v), dim=1)
            if c["layer_types"][layer] == "sliding_attention":
                k, v = k[:, -c["sliding_window"]:], v[:, -c["sliding_window"]:]
            cache[layer] = k, v
            keys = k.repeat_interleave(h // kv, dim=0).float()
            vals = v.repeat_interleave(h // kv, dim=0).float()
            scores = torch.bmm(q.float().unsqueeze(1), keys.transpose(1, 2)) / d**.5
            scores = torch.cat((scores, w[attn + ".softmax_offset"].float().view(h, 1, 1)), dim=-1)
            probs = scores.softmax(-1)[..., :-1]
            out = torch.bmm(probs, vals).to(x.dtype).view(1, h*d)
            x = x + F.linear(out, w[attn + ".o_proj.weight"])
            n = norm(x, prefix + ".post_attention_layernorm.weight")
            mlp = prefix + ".mlp"
            if layer in c["mlp_only_layers"]:
                out = ffn(n, mlp)
            else:
                scores = F.linear(n, w[mlp + ".gate.weight"]).float().sigmoid().flatten()
                ids = scores.argsort(descending=True, stable=True)[:c["num_experts_per_tok"]]
                weights = scores[ids] / scores[ids].sum()
                out = torch.zeros_like(n, dtype=torch.float32)
                for expert, score in zip(ids.tolist(), weights):
                    out += ffn(n, mlp + f".experts.{expert}").float() * score
                out = out.to(x.dtype) + ffn(n, mlp + ".shared_expert")
            x = x + out
        logits = F.linear(norm(x, "model.norm.weight").float(), w["lm_head.weight"])
        output.write(json.dumps(dict(position=position, token=token, logits=logits.flatten().tolist())) + "\n")
        output.flush()
        print("position", position, "top1", logits.argmax().item(), flush=True)


with args.output.open("w") as output:
    run(json.loads(args.request.read_text())["token_ids"], output)
