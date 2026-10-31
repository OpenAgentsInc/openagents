#!/usr/bin/env python3
"""Write the small nanochat checkpoint fixture with torch.save (tooling only).

Run once, in an isolated CPU-only venv, with python -I; it never imports
nanochat. It reads the weights psionic/tests/nanochat_oracle.py --weights
checkpoint prints (values already rounded to the dtypes nanochat stores: F32
master weights, BF16 embeddings), checks every BF16 bit pattern torch makes
against the oracle's, and saves what base_train's save_checkpoint saves at
step 0: model_000000.pt (the state dict, in GPT.state_dict() order) and
meta_000000.json (step, model_config and the run settings).

  python3 -I psionic/tools/export/make_nanochat_checkpoint.py WEIGHTS.json OUT_DIR
"""
import collections
import json
import os
import sys

import torch


def main():
    doc = json.load(open(sys.argv[1]))
    out = sys.argv[2]
    state = collections.OrderedDict()
    for t in doc["tensors"]:
        x = torch.tensor(t["values"], dtype=torch.float32).reshape(t["shape"])
        if t["dtype"] == "BF16":
            x = x.to(torch.bfloat16)
            bits = x.view(torch.int16).reshape(-1).tolist()
            assert [b & 0xFFFF for b in bits] == t["bits"], t["name"]
        else:
            assert t["dtype"] == "F32"
            assert x.reshape(-1).tolist() == t["values"], t["name"]
        state[t["name"]] = x
    os.makedirs(out, exist_ok=True)
    torch.save(state, os.path.join(out, "model_000000.pt"))
    meta = {"step": 0, "val_bpb": None, "model_config": doc["model_config"],
            "user_config": {"depth": doc["model_config"]["n_layer"], "max_seq_len": doc["model_config"]["sequence_len"],
                            "window_pattern": doc["model_config"]["window_pattern"]},
            "device_batch_size": 1, "max_seq_len": doc["model_config"]["sequence_len"], "total_batch_size": doc["model_config"]["sequence_len"]}
    with open(os.path.join(out, "meta_000000.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2)
    print(f"torch {torch.__version__}: {len(state)} tensors saved")


if __name__ == "__main__":
    main()
