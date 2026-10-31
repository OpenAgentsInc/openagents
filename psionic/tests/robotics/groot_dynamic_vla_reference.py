#!/usr/bin/env python3
"""Firm reason2: original VLM/head MPS oracle for a genuine prepared observation.

No NVIDIA checkout executes, no learned CPU fallback, no invented scene input.
The caller supplies already-prepared pixels/IDs/state and explicit initial noise.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import time
import numpy as np
import torch
import diffusers
import transformers
from transformers import Qwen3VLConfig

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("weights", type=Path)
    p.add_argument("cosmos", type=Path)
    p.add_argument("prepared_npz", type=Path)
    p.add_argument("provenance", type=Path)
    p.add_argument("new_output", type=Path)
    args = p.parse_args()
    assert torch.__version__.split("+")[0] == "2.9.0" and diffusers.__version__ == "0.35.1" and transformers.__version__ == "4.57.3"
    assert torch.backends.mps.is_available()
    assert os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK", "0") == "0" and os.environ.get("PYTORCH_MPS_FAST_MATH", "0") == "0"
    source = Path(__file__).with_name("groot_vla_reference.py")
    spec = importlib.util.spec_from_file_location("independent_groot_reference", source)
    reference = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reference)
    provenance = json.loads(args.provenance.read_text())
    with np.load(args.prepared_npz, allow_pickle=False) as data:
        pixels, ids, state, noise = [data[k].copy() for k in ("pixels", "ids", "state", "noise")]
    tokens = len(ids)
    assert 137 <= tokens <= 512 and pixels.shape == (512, 1536)
    assert state.shape == (1, 1, 132) and noise.shape == (1, 40, 132)
    assert ids.dtype.kind in "iu" and np.all((ids >= 0) & (ids < 151936)) and np.count_nonzero(ids == 151655) == 128
    assert all(a.dtype == np.float32 and np.isfinite(a).all() for a in (pixels, state, noise))
    capture = reference.Capture(args.new_output)
    shutil.copyfile(args.prepared_npz, args.new_output / "inputs.npz")
    config_path = args.cosmos / "config.json"
    assert sha(config_path) == "bec4b3d446efa05807365c9e1cec03ac590836879d02f3a6da879971154bdd3b"
    config = Qwen3VLConfig.from_dict(json.loads(config_path.read_text()))
    config.text_config.num_hidden_layers = 16
    config._attn_implementation = config.text_config._attn_implementation = config.vision_config._attn_implementation = "sdpa"
    started = time.monotonic()
    print(f"genuine prepared observation: {tokens}tokens; loading original tensors onMPS", flush=True)
    model, weights = reference.load_weights(args.weights, config)
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
    with torch.inference_mode():
        token_ids = torch.from_numpy(ids.astype(np.int64)[None]).to("mps")
        result = model(input_ids=token_ids, attention_mask=torch.ones_like(token_ids),
                       pixel_values=torch.from_numpy(pixels).to("mps"),
                       image_grid_thw=torch.tensor([[1,16,16],[1,16,16]], device="mps"),
                       output_hidden_states=True, use_cache=False)
        features = capture("backbone", result.last_hidden_state)
        del model, result
        actions = reference.ActionHead(weights, capture)(features, torch.from_numpy(state).to("mps"), torch.from_numpy(noise).to("mps"), token_ids)
        capture("normalized-actions", actions)
    torch.mps.synchronize()
    finished = time.monotonic()
    record = {"schema":"psionic-groot-prepared-vla-reference-v2", "model_revision":reference.REVISION,
              "cosmos_revision":reference.COSMOS, "category":2, "device":"mps", "dtype":"float32",
              "torch":torch.__version__, "transformers":transformers.__version__, "diffusers":diffusers.__version__, "numpy":np.__version__,
              "source_checkout_executed":False, "cpu_model_fallback":False, "coax_full_policy_compared":False,
              "simulated_task_success_measured":False, "whole_shard_hash_rechecked_here":False,
              "request_tokens":tokens, "prepared_observation_sha256":sha(args.prepared_npz),
              "observation_provenance_sha256":sha(args.provenance), "observation_provenance":provenance,
              "recipe":{"tokens":tokens,"image_grid_thw":[[1,16,16],[1,16,16]],"unpadded":True},
              "load_seconds":loaded-started,"execution_and_capture_seconds":finished-loaded,
              "source_sha256":{source.name:sha(source),Path(__file__).name:sha(Path(__file__))},"outputs":capture.records}
    (args.new_output / "record.json").write_text(json.dumps(record,indent=2,sort_keys=True)+"\n")
    print(f"original prepared-observation GPU reference: {len(capture.records)}boundaries,{tokens}tokens,{finished-loaded:.6f}s; Coax proof separate",flush=True)

if __name__ == "__main__":
    main()
