#!/usr/bin/env python3
"""Independent layer-streamed CPU Transformers reference; tooling on CoderOS only.

A float32 copy of all of Mammoth needs about 108 GB, more than this host can
hold beside the checkpoint mapping. This script therefore builds one
Transformers `Qwen3_5DecoderLayer` at a time, loads that layer's original BF16
tensors converted to float32, runs the whole token sequence through it and
frees it before the next layer. Full-attention layers use eager attention and
the same additive two-block mask as the chapter 14 reference; linear layers
receive no mask, as in an unpadded Transformers forward. The embedding rows,
final norm and every vocabulary row of the head are also float32.

The original files must already be authenticated. Nothing here runs code from
the model repository: only `config.json` and safetensors data are read, and the
Hugging Face hub is kept offline. This script is not a runtime dependency.
"""
import os
os.environ['HF_HUB_OFFLINE'] = '1'
os.environ['TRANSFORMERS_OFFLINE'] = '1'
import importlib.util, json, resource, sys, time
from pathlib import Path
import torch
from safetensors import safe_open
from transformers import AutoConfig
import transformers.models.qwen3_5.modeling_qwen3_5 as module

SLOTS = [32, 33]
PREFIX = 'model.language_model.'


def score(logits):
    selected = logits[torch.tensor(SLOTS)]
    mass = (torch.logsumexp(selected, dim=0) - torch.logsumexp(logits, dim=0)).exp().item()
    p = torch.softmax(selected, dim=0)
    return {'logits': [float(x) for x in selected], 'probabilities': [float(x) for x in p], 'slot_mass': mass}


class Weights:
    def __init__(self, directory):
        index = directory / 'model.safetensors.index.json'
        if index.exists():
            names = json.loads(index.read_text())['weight_map']
        else:
            with safe_open(str(directory / 'model.safetensors'), 'pt') as f:
                names = {k: 'model.safetensors' for k in f.keys()}
        self.where = names
        self.files = {n: safe_open(str(directory / n), 'pt') for n in sorted(set(names.values()))}

    def get(self, name):
        tensor = self.files[self.where[name]].get_tensor(name)
        if tensor.dtype != torch.bfloat16:
            raise ValueError(f'unexpected dtype for {name}')
        return tensor.to(torch.float32)

    def layer(self, index):
        start = f'{PREFIX}layers.{index}.'
        return {k[len(start):]: self.get(k) for k in self.where if k.startswith(start)}


def main():
    if len(sys.argv) != 4:
        raise SystemExit('usage: mammoth_reference.py ORIGINAL_DIR TOKEN_IDS OUTPUT_JSON')
    began = time.monotonic()
    directory = Path(sys.argv[1])
    lines = [int(x) for x in Path(sys.argv[2]).read_text().split()]
    prefix, tokens = lines[0], lines[1:]
    if not 0 <= prefix < len(tokens):
        raise ValueError('prefix')
    if any(importlib.util.find_spec(name) for name in ('fla', 'causal_conv1d', 'kernels')):
        raise ValueError('expected the torch-only linear-attention path')
    torch.set_num_threads(int(os.environ.get('REFERENCE_THREADS', '8')))
    config = AutoConfig.from_pretrained(str(directory), local_files_only=True).text_config
    config._attn_implementation = 'eager'
    weights = Weights(directory)
    n = len(tokens)
    with torch.inference_mode():
        table = weights.files[weights.where[PREFIX + 'embed_tokens.weight']].get_tensor(PREFIX + 'embed_tokens.weight')
        hidden = table[torch.tensor(tokens)].to(torch.float32).unsqueeze(0)
        del table
        rotary = module.Qwen3_5TextRotaryEmbedding(config=config)
        positions = torch.arange(n).view(1, 1, n).expand(3, 1, n)
        embeddings = rotary(hidden, positions)
        mask = torch.zeros((1, 1, n, n), dtype=torch.float32)
        mask[:, :, 0:prefix, prefix:] = torch.finfo(torch.float32).min
        text_positions = torch.arange(n).view(1, n)
        layer_seconds = []
        for i in range(config.num_hidden_layers):
            t = time.monotonic()
            layer = module.Qwen3_5DecoderLayer(config, i).eval()
            missing = layer.load_state_dict(weights.layer(i), strict=True)
            if missing.missing_keys or missing.unexpected_keys:
                raise ValueError(f'layer {i} weights')
            full = config.layer_types[i] == 'full_attention'
            hidden = layer(hidden, position_embeddings=embeddings, attention_mask=mask if full else None,
                           position_ids=text_positions, past_key_values=None, use_cache=False)
            del layer
            layer_seconds.append(time.monotonic() - t)
            print(f'layer {i} {config.layer_types[i]} {layer_seconds[-1]:.1f}s', flush=True)
        last = hidden[0, -1]
        norm = module.Qwen3_5RMSNorm(config.hidden_size, eps=config.rms_norm_eps)
        norm.load_state_dict({'weight': weights.get(PREFIX + 'norm.weight')}, strict=True)
        normed = norm(last.view(1, -1))[0]
        head = weights.files[weights.where['lm_head.weight']].get_tensor('lm_head.weight')
        logits = torch.empty(head.shape[0], dtype=torch.float32)
        for row in range(0, head.shape[0], 32768):
            logits[row:row + 32768] = head[row:row + 32768].to(torch.float32) @ normed
        del head
        decision = score(logits)
        decision['hidden_first4'] = [float(x) for x in last[:4]]
    import transformers
    result = {
        'torch': torch.__version__, 'transformers': transformers.__version__,
        'profile': 'float32 eager CPU, one decoder layer resident at a time',
        'decision': decision, 'prefix': prefix, 'tokens': n, 'layers': config.num_hidden_layers,
        'seconds': round(time.monotonic() - began, 1),
        'peak_rss_kib': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
    }
    Path(sys.argv[3]).write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2), flush=True)


if __name__ == '__main__':
    main()
