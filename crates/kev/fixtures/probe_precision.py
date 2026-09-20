"""Export pinned fixture probabilities through upstream's merged load path.

Run with PYTHONPATH pointing to the immutable upstream checkout and with
HF_HUB_OFFLINE=1 after verifying the cached base against its artifact lock.
The reference environment supplies torch, transformers, and peft.
"""

import argparse
import json
from pathlib import Path
import resource
import sys
import subprocess
import time

import torch
from kev.evaluate import load


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", required=True)
    parser.add_argument("--fixtures", required=True)
    parser.add_argument("--dtype", choices=["f32", "bf16"], required=True)
    parser.add_argument("--device", choices=["cpu", "mps"], required=True)
    parser.add_argument("--upstream-revision", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    checkout = Path(sys.modules["kev"].__file__).resolve().parent.parent
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=checkout, text=True
    ).strip()
    if revision != args.upstream_revision:
        parser.error(f"upstream checkout is {revision}, expected {args.upstream_revision}")
    torch.set_num_threads(4)

    def synchronize():
        if args.device == "mps":
            torch.mps.synchronize()

    dtype = {"f32": torch.float32, "bf16": torch.bfloat16}[args.dtype]
    start = time.perf_counter()
    tokenizer, model = load(args.run, args.device, dtype=dtype, merge=True, attn="eager")
    synchronize()
    result = {
        "upstream_revision": args.upstream_revision,
        "dtype": args.dtype,
        "device": args.device,
        "torch": torch.__version__,
        "load_ms": (time.perf_counter() - start) * 1000,
        "head_dtype": str(next(model.head.parameters()).dtype),
        "records": [],
    }
    for path in sorted((Path(args.fixtures) / "encodings").glob("*.json")):
        record = json.loads(path.read_text())["record"]
        encoding = model.encode(tokenizer, record, max_state=8192, max_branch=8192)
        start = time.perf_counter()
        probabilities = [probability.tolist() for probability in model.probs(encoding)]
        synchronize()
        result["records"].append({
            "name": path.stem,
            "tokens": len(encoding["ids"]),
            "probs": probabilities,
            "elapsed_ms": (time.perf_counter() - start) * 1000,
        })
        print(path.stem, flush=True)
    # macOS reports bytes; Linux reports KiB.
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    result["peak_rss_bytes"] = rss if sys.platform == "darwin" else rss * 1024
    Path(args.out).write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
