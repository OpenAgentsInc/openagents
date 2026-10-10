"""Cloudflare's reference Clef (HF weights, torch, CPU) as a comparator.

  ref_systemone.py answers <clef-flash dir> <dtype f32|bf16> <requests.jsonl> <out.jsonl>
      runs the reference `systemone` on each request (unrounded
      probabilities), in the e2e.py result shape.
  ref_systemone.py hidden <clef-flash dir> <dtype> <dump dir>
      compares Psionic's dumped final hidden rows (dump_head_parity_inputs)
      with the reference backbone's last_hidden_state on the same prompt.
"""

import json
import sys
import time

sys.path.insert(0, sys.argv[2])
import torch  # noqa: E402
import joint_schema_model as reference  # noqa: E402

torch.set_num_threads(int(__import__("os").environ.get("REF_THREADS", "16")))
mode, path, dtype_name = sys.argv[1], sys.argv[2], sys.argv[3]
dtype = {"f32": torch.float32, "bf16": torch.bfloat16}[dtype_name]
model, processor = reference.load_release_model(path, device="cpu", dtype=dtype)

if mode == "answers":
    # Unrounded probabilities: keep the reference answer shape.
    reference.round = lambda value, digits=None: value  # noqa: A001
    with open(sys.argv[5], "w", encoding="utf-8") as sink:
        for index, line in enumerate(open(sys.argv[4], encoding="utf-8")):
            began = time.time()
            try:
                body = reference.systemone(model, processor, json.loads(line))
                status = 200
            except Exception as error:  # noqa: BLE001
                body, status = {"error": {"message": str(error)}}, 400
            sink.write(json.dumps({"index": index, "status": status, "seconds": round(time.time() - began, 3), "body": body}, ensure_ascii=False) + "\n")
            sink.flush()
            print(index, status, round(time.time() - began, 1), file=sys.stderr, flush=True)
elif mode == "hidden":
    from pathlib import Path

    dump_dir = Path(sys.argv[4])
    dump = json.loads((dump_dir / "dump.json").read_text())
    ids = torch.tensor([dump["input_ids"]])
    ours = torch.frombuffer(bytearray((dump_dir / "hidden.f32").read_bytes()), dtype=torch.float32).reshape(len(dump["input_ids"]), -1)
    base = model.language_model.model
    text = base.language_model if hasattr(base, "language_model") else base
    with torch.inference_mode():
        theirs = text(input_ids=ids, attention_mask=torch.ones_like(ids), use_cache=False, return_dict=True).last_hidden_state[0].float()
    cos = torch.nn.functional.cosine_similarity(ours, theirs, dim=-1)
    rmse = ((ours - theirs).pow(2).mean(-1).sqrt() / theirs.pow(2).mean(-1).sqrt())
    print(json.dumps({
        "tokens": len(dump["input_ids"]),
        "dtype": dtype_name,
        "min_cosine": float(cos.min()),
        "mean_cosine": float(cos.mean()),
        "max_normalized_rmse": float(rmse.max()),
        "mean_normalized_rmse": float(rmse.mean()),
        "last_row_cosine": float(cos[-1]),
    }, indent=1))
