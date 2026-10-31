#!/usr/bin/env python3
"""Opt-in original GR00T GPU reference; never a Psionic runtime.

Firm reason: 2, independent numerical oracle. Uses installed Transformers for
the Qwen backbone and independently composed Torch operators for the action
head. No NVIDIA checkout is imported or executed. All learned arithmetic must
run on MPS; CPU model fallback is forbidden. Output is a private replay capsule,
not evidence that the Coax full policy has passed comparison.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import time

import cv2
import numpy as np
from safetensors import safe_open
import torch
import torch.nn.functional as F
import transformers
from tokenizers import Tokenizer
from transformers import Qwen3VLConfig, Qwen3VLModel
import diffusers
from diffusers.models.embeddings import get_timestep_embedding

REVISION = "2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21"
COSMOS = "9ce19a195e423419c349abfc86fd07178b230561"
SIZES = {"model-00001-of-00002.safetensors": 4990519232,
         "model-00002-of-00002.safetensors": 1919980184}


def require(condition, text):
    if not condition:
        raise SystemExit("GR00T VLA reference: " + text)


class Capture:
    def __init__(self, root):
        root.mkdir(parents=True, exist_ok=False)
        self.root, self.records = root, []

    def __call__(self, name, value):
        require(value.device.type == "mps", "non-GPU learned result: " + name)
        a = value.detach().float().cpu().numpy().astype("<f4", copy=False)
        require(np.isfinite(a).all(), "nonfinite result: " + name)
        raw = a.tobytes()
        (self.root / (name + ".f32")).write_bytes(raw)
        self.records.append({"name": name, "shape": list(a.shape),
                             "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        return value


def input_recipe(cosmos):
    # Owned, deliberately structured images: neither recorded demonstrations
    # nor a simulated camera. The two cameras remain separate still images.
    yy, xx = np.indices((256, 256), dtype=np.uint16)
    images = [np.stack(((xx + 3 * yy) % 256, (7 * xx + yy) % 256,
                        (xx ^ yy) % 256), axis=-1).astype(np.uint8),
              np.stack(((255 - xx) % 256, (2 * yy) % 256,
                        (5 * xx + 11 * yy) % 256), axis=-1).astype(np.uint8)]
    patches = []
    for image in images:
        # Pinned LIBERO wrapper: flip both spatial axes, retain RGB channels.
        image = image[::-1, ::-1].copy()
        # GR00T eval crop_fraction=.95 with target256 and INTER_AREA resize.
        image = cv2.resize(image[6:249, 6:249], (256, 256), interpolation=cv2.INTER_AREA)
        x = (image.astype(np.float32) - np.float32(127.5)) / np.float32(127.5)
        x = x.transpose(2, 0, 1)
        x = np.stack((x, x), axis=1)  # C, temporal2, H, W
        x = x.reshape(3, 2, 8, 2, 16, 8, 2, 16)
        # block_h,block_w,intra_h,intra_w,C,T,patch_h,patch_w
        patches.append(x.transpose(2, 5, 3, 6, 0, 1, 4, 7).reshape(256, 1536))
    language = re.sub(r"[^\w\s]", "", "Open the drawer!".lower())
    rendered = ("<|im_start|>user\n" +
                ("<|vision_start|>" + "<|image_pad|>" * 64 + "<|vision_end|>") * 2 +
                language + "<|im_end|>\n")
    tokenizer = Tokenizer.from_file(str(cosmos / "tokenizer.json"))
    ids = tokenizer.encode(rendered, add_special_tokens=False).ids
    require(len(ids) == 140 and ids[4:68] == [151655] * 64 and
            ids[70:134] == [151655] * 64, "owned request token contract")
    state = np.zeros((1, 1, 132), dtype=np.float32)
    state[0, 0, :8] = [-.75, -.5, -.25, 0, .25, .5, .75, 1]
    noise = np.random.Generator(np.random.PCG64(113)).standard_normal((1, 40, 132)).astype(np.float32)
    return np.concatenate(patches), ids, state, noise, rendered


def load_weights(root, config):
    index_path = Path(__file__).resolve().parents[2] / "fixtures/robotics/groot-libero/model.safetensors.index.json"
    index = json.loads(index_path.read_text())["weight_map"]
    backbone, action = {}, {}
    for shard, size in SIZES.items():
        path = root / shard
        require(path.stat().st_size == size, "wrong shard extent: " + shard)
        with safe_open(path, framework="pt", device="cpu") as source:
            for name in source.keys():
                require(index[name] == shard, "index/shard disagreement")
                if name == "backbone.model.lm_head.weight":
                    continue  # Serialized readout is unused by action conditioning.
                if name.startswith("backbone.model.model."):
                    backbone[name.removeprefix("backbone.model.model.")] = source.get_tensor(name).to("mps", torch.float32)
                else:
                    require(name.startswith("action_head."), "unknown learned tensor")
                    key = name.removeprefix("action_head.")
                    if key.endswith((".W", ".b")):
                        shape = source.get_slice(name).get_shape()
                        require(shape[0] == 32, "wrong embodiment axis")
                        value = source.get_slice(name)[2]  # category2, original input-major
                    else:
                        value = source.get_tensor(name)
                    action[key] = value.to("mps", torch.float32)
    require(len(backbone) == 493 and len(action) == 537, "complete active tensor inventory")
    with torch.device("meta"):
        model = Qwen3VLModel(config)
    model.load_state_dict(backbone, strict=True, assign=True)
    # These analytic buffers are intentionally non-persistent in Transformers.
    with torch.device("mps"):
        rotary = model.visual.rotary_pos_emb
        model.visual.rotary_pos_emb = type(rotary)(32)
        rotary = model.language_model.rotary_emb
        model.language_model.rotary_emb = type(rotary)(config=config.text_config, device="mps")
    model.eval().requires_grad_(False)
    return model, action


class ActionHead:
    def __init__(self, weights, capture):
        self.w, self.capture = weights, capture

    def linear(self, x, key):
        return F.linear(x, self.w[key + ".weight"], self.w[key + ".bias"])

    def category(self, x, key):
        return x @ self.w[key + ".W"] + self.w[key + ".b"]

    def norm(self, x, key=None, eps=1e-5):
        gamma = None if key is None else self.w[key + ".weight"]
        beta = None if key is None else self.w[key + ".bias"]
        return F.layer_norm(x, (x.shape[-1],), gamma, beta, eps)

    def block(self, x, key, heads, context=None, mask=None, time_embedding=None):
        if time_embedding is None:
            normalized = self.norm(x, key + ".norm1")
        else:
            scale, shift = self.linear(F.silu(time_embedding), key + ".norm1.linear").chunk(2, dim=-1)
            normalized = self.norm(x) * (1 + scale[:, None]) + shift[:, None]
        context = normalized if context is None else context
        q = self.linear(normalized, key + ".attn1.to_q")
        k = self.linear(context, key + ".attn1.to_k")
        v = self.linear(context, key + ".attn1.to_v")
        q = q.reshape(1, q.shape[1], heads, -1).transpose(1, 2)
        k = k.reshape(1, k.shape[1], heads, -1).transpose(1, 2)
        v = v.reshape(1, v.shape[1], heads, -1).transpose(1, 2)
        attention = F.scaled_dot_product_attention(q, k, v, attn_mask=mask, dropout_p=0)
        attention = attention.transpose(1, 2).reshape(1, x.shape[1], -1)
        x = x + self.linear(attention, key + ".attn1.to_out.0")
        normalized = self.norm(x, key + ".norm3" if time_embedding is None else None)
        ff = F.gelu(self.linear(normalized, key + ".ff.net.0.proj"), approximate="tanh")
        return x + self.linear(ff, key + ".ff.net.2")

    def time(self, bucket, width, action):
        if not action:
            return get_timestep_embedding(torch.tensor([bucket], device="mps"), width,
                                          flip_sin_to_cos=True, downscale_freq_shift=1)
        half = width // 2
        coefficient = (torch.log(torch.tensor(10000.0, dtype=torch.float32)) / half).item()
        frequencies = torch.exp(-torch.arange(half, device="mps", dtype=torch.float32) *
                                coefficient)
        phases = frequencies * bucket
        return torch.cat((phases.sin(), phases.cos()))[None]

    def __call__(self, features, state, noise, ids):
        features = self.norm(features, "vlln")
        for layer in range(4):
            features = self.block(features, f"vl_self_attention.transformer_blocks.{layer}", 32)
        self.capture("vl-conditioning", features)
        state = self.category(F.relu(self.category(state, "state_encoder.layer1")), "state_encoder.layer2")
        self.capture("state-features", state)
        image_mask = ids == 151655
        actions = noise
        for step, bucket in enumerate((0, 250, 500, 750)):
            a = self.category(actions, "action_encoder.W1")
            tau = self.time(bucket, 1536, True)[:, None].expand(1, 40, 1536)
            a = self.category(torch.cat((a, tau), dim=-1), "action_encoder.W2")
            a = self.category(a * torch.sigmoid(a), "action_encoder.W3")
            a = a + self.w["position_embedding.weight"][:40][None]
            x = torch.cat((state, a), dim=1)
            t = self.linear(F.silu(self.linear(self.time(bucket, 256, False),
                            "model.timestep_encoder.timestep_embedder.linear_1")),
                            "model.timestep_encoder.timestep_embedder.linear_2")
            for layer in range(32):
                if layer % 2:
                    context, mask = None, None
                else:
                    context = features
                    visible = ~image_mask if layer % 4 == 0 else image_mask
                    mask = visible[:, None, None, :]
                x = self.block(x, f"model.transformer_blocks.{layer}", 32, context, mask, t)
                if layer in (0, 1, 2, 3, 31):
                    self.capture(f"flow{step}-block{layer}", x)
            shift, scale = self.linear(F.silu(t), "model.proj_out_1").chunk(2, dim=-1)
            x = self.norm(x, eps=1e-6) * (1 + scale[:, None]) + shift[:, None]
            x = self.linear(x, "model.proj_out_2")
            velocity = self.category(F.relu(self.category(x, "action_decoder.layer1")), "action_decoder.layer2")[:, -40:]
            self.capture(f"flow{step}-velocity", velocity)
            actions = actions + .25 * velocity
            self.capture(f"flow{step}-actions", actions)
        return actions


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("weights", type=Path)
    p.add_argument("cosmos", type=Path)
    p.add_argument("output", type=Path)
    args = p.parse_args()
    require(torch.__version__.split("+")[0] == "2.9.0" and transformers.__version__ == "4.57.3" and
            diffusers.__version__ == "0.35.1", "reference versions")
    require(torch.backends.mps.is_available(), "MPS required; no CPU model fallback")
    require(os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK", "0") == "0", "MPS fallback enabled")
    require(os.environ.get("PYTORCH_MPS_FAST_MATH", "0") == "0", "MPS fast math enabled")
    capture = Capture(args.output)
    config = Qwen3VLConfig.from_dict(json.loads((args.cosmos / "config.json").read_text()))
    config.text_config.num_hidden_layers = 16
    config._attn_implementation = "sdpa"
    config.text_config._attn_implementation = "sdpa"
    config.vision_config._attn_implementation = "sdpa"
    pixels, ids, state, noise, rendered = input_recipe(args.cosmos)
    np.savez(args.output / "inputs.npz", pixels=pixels, ids=np.array(ids, dtype=np.int64), state=state, noise=noise)
    print("owned request ready; loading original active tensors onto MPS", flush=True)
    started = time.monotonic()
    model, weights = load_weights(args.weights, config)
    def save_output(name):
        def hook(module, arguments, output):
            capture(name, output)
        return hook

    def save_input(name):
        def hook(module, arguments):
            capture(name, arguments[0])
        return hook

    model.visual.patch_embed.register_forward_hook(save_output("vision-patch-projection"))
    model.visual.blocks[0].register_forward_pre_hook(save_input("vision-preblocks"))
    for layer, block in enumerate(model.visual.blocks):
        block.register_forward_hook(save_output(f"vision-block{layer}"))
    model.visual.merger.register_forward_pre_hook(save_input("vision-merger-input"))
    model.visual.merger.register_forward_hook(save_output("vision-merged"))
    for layer, merger in enumerate(model.visual.deepstack_merger_list):
        merger.register_forward_pre_hook(save_input(f"vision-deepstack{layer}-input"))
        merger.register_forward_hook(save_output(f"vision-deepstack{layer}"))
    torch.mps.synchronize()
    loaded = time.monotonic()
    print("original tensors loaded; executing vision24/text16/conditioning4/DiT32x4", flush=True)
    with torch.inference_mode():
        token_ids = torch.tensor([ids], device="mps")
        result = model(input_ids=token_ids, attention_mask=torch.ones_like(token_ids),
                       pixel_values=torch.from_numpy(pixels).to("mps"),
                       image_grid_thw=torch.tensor([[1, 16, 16], [1, 16, 16]], device="mps"),
                       output_hidden_states=True, use_cache=False)
        features = capture("backbone", result.last_hidden_state)
        del model, result
        actions = ActionHead(weights, capture)(features, torch.from_numpy(state).to("mps"),
                                                torch.from_numpy(noise).to("mps"), token_ids)
        capture("normalized-actions", actions)
    torch.mps.synchronize()
    finished = time.monotonic()
    record = {"schema": "psionic-groot-vla-reference-v1", "model_revision": REVISION,
              "cosmos_revision": COSMOS, "category": 2, "device": "mps", "dtype": "float32",
              "torch": torch.__version__, "transformers": transformers.__version__,
              "diffusers": diffusers.__version__,
              "numpy": np.__version__, "opencv": cv2.__version__,
              "source_checkout_executed": False, "cpu_model_fallback": False,
              "coax_full_policy_compared": False, "simulated_task_success_measured": False,
              "whole_shard_hash_rechecked_here": False,
              "input_scope": "Owned patterned images, authored normalized state, explicit PCG64 Gaussian noise; no captured robot episode or RNG parity claim.",
              "recipe": {"rendered_utf8_sha256": hashlib.sha256(rendered.encode()).hexdigest(),
                         "tokens": len(ids), "image_grid_thw": [[1, 16, 16], [1, 16, 16]]},
              "load_seconds": loaded - started, "execution_and_capture_seconds": finished - loaded,
              "outputs": capture.records}
    (args.output / "record.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    print(f"original VLA GPU reference captured {len(capture.records)} boundaries in {finished - loaded:.3f}s; Coax full-policy comparison remains separate")


if __name__ == "__main__":
    main()
