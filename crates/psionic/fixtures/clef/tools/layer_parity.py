"""Per-layer hidden-state parity for the Clef backbone (#11195).

Every dump directory holds `l_out-N.f32` (layer N's residual rows,
`tokens x 4096` f32, before the output norm) and `result_norm.f32` (the
final normalized rows). Three writers share the layout:

- llama.cpp: `lldump.cpp` (this directory) through libllama's `cb_eval`;
- Psionic: the `clef::tests::dump_layer_rows` test (CPU or CUDA lane);
- the Hugging Face reference: `layer_parity.py hf` below.

  layer_parity.py hf <clef-flash dir> <dtype f32|bf16> <tokens.txt> <out dir>
      runs the reference backbone with output_hidden_states.
  layer_parity.py compare <dir A> <dir B> [<dir C> ...]
      per-layer row cosine (mean, min) and normalized RMSE of every dump
      against the first one, as JSON.
"""

import json
import os
import sys
from pathlib import Path

import numpy as np


def load(directory, name, tokens=None):
    data = np.fromfile(Path(directory) / name, dtype=np.float32)
    return data.reshape(-1, 4096)


def stats(a, b):
    a64, b64 = a.astype(np.float64), b.astype(np.float64)
    cos = (a64 * b64).sum(-1) / (np.linalg.norm(a64, axis=-1) * np.linalg.norm(b64, axis=-1))
    rmse = np.sqrt(((a64 - b64) ** 2).mean(-1)) / np.sqrt((a64 ** 2).mean(-1))
    return {
        "mean_cos": round(float(cos.mean()), 6),
        "min_cos": round(float(cos.min()), 6),
        "mean_nrmse": round(float(rmse.mean()), 5),
        "max_nrmse": round(float(rmse.max()), 5),
    }


def names(directory):
    layers = sorted(
        (int(p.stem.split("-")[1]) for p in Path(directory).glob("l_out-*.f32"))
    )
    return [f"l_out-{n}.f32" for n in layers] + ["result_norm.f32"]


if sys.argv[1] == "hf":
    sys.path.insert(0, sys.argv[2])
    import torch
    import joint_schema_model as reference

    torch.set_num_threads(int(os.environ.get("REF_THREADS", "16")))
    dtype = {"f32": torch.float32, "bf16": torch.bfloat16}[sys.argv[3]]
    model, _ = reference.load_release_model(sys.argv[2], device="cpu", dtype=dtype)
    ids = [int(v) for v in Path(sys.argv[4]).read_text().split()]
    out = Path(sys.argv[5])
    out.mkdir(parents=True, exist_ok=True)
    base = model.language_model.model
    text = base.language_model if hasattr(base, "language_model") else base
    tensor = torch.tensor([ids])
    with torch.inference_mode():
        result = text(
            input_ids=tensor,
            attention_mask=torch.ones_like(tensor),
            use_cache=False,
            output_hidden_states=True,
            return_dict=True,
        )
    hidden = result.hidden_states  # [embeddings, layer 0 out, ..., final (normed)]
    layers = len(hidden) - 1
    for n in range(layers - 1):
        hidden[n + 1][0].float().numpy().tofile(out / f"l_out-{n}.f32")
    result.last_hidden_state[0].float().numpy().tofile(out / "result_norm.f32")
    print(json.dumps({"tokens": len(ids), "layers_written": layers - 1}))
elif sys.argv[1] == "compare":
    base = sys.argv[2]
    report = {}
    for other in sys.argv[3:]:
        rows = {}
        for name in names(base):
            if not (Path(other) / name).exists():
                continue
            rows[name.removesuffix(".f32")] = stats(load(base, name), load(other, name))
        report[other] = rows
    print(json.dumps({"base": base, "against": report}, indent=1))
