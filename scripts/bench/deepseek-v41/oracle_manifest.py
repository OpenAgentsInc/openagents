#!/usr/bin/env python3
"""Index the oracle dumps and cut the small fixture set the repository keeps (#12371).

Reads `tensors-rank*.safetensors` and `run.json` from an `oracle_dump.py`
output directory and writes:

  --out MANIFEST.json   every tensor: name, file, shape, dtype, bytes, SHA-256
                        of its raw little-endian row-major bytes
  --fixtures DIR        the repository copy, for
                        crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/oracle/:
                          manifest.json.gz  the run (prompts, token ids, generated text,
                                         top-64 logits per step, residual hashes)
                                         and every tensor's entry
                          slices.json.gz    the first and last 64 values of every
                                         tensor, as hex of their raw bytes
                          full.bin.gz       tensors kept whole, at the offsets the
                                         manifest gives (`full`)

    oracle_manifest.py --dump DIR --out DIR/manifest.json [--fixtures FIXDIR]
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import sys
from pathlib import Path

from safetensors import safe_open

SLICE = 64
FULL_LIMIT = 64 * 1024  # whole tensors under 64 KB ...
FULL_PROMPT = "p1-chat-thinking.decode"  # ... from the short prompt; elsewhere only these:
FULL_SMALL = 4096
FULL_BUDGET = 700_000
BUDGET = 5_000_000
FIXTURE_PROMPTS = ("p1-chat-thinking", "p2-code", "p3-2k", "p4-20k")
BUCKET = "gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump"
# The original checkpoint and the TP8 files the reference loaded are separate identities:
# the oracle ran on the converted files; #12373's loader reads the original ones.
CHECKPOINT = {
    "original": {
        "repo": "deepseek-ai/DeepSeek-V4.1-Flash-1017",
        "revision": "1bae0e1088993b9b82de08f59de5be5a34e42170",
        "copy": "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1/",
        "manifest": "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1.manifest.json",
        "verified": "every file's SHA-256 (LFS) or git blob ID against the Hugging Face tree API",
    },
    "converted": {
        "command": "inference/convert.py --model-parallel 8 --expert-dtype fp4",
        "copy": "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1-tp8-fp4/",
        "manifest": "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1-tp8-fp4.manifest.json",
        "note": "the reference's own sharding: wo_a dequantized to BF16, Engram rows split by rank with padding",
    },
}


def compare_modes(raw: dict, entries: list[dict]) -> list[dict]:
    """Cached decode against replay and chunked prefill: for every tensor recorded in two modes
    at the same label and site, the largest absolute difference and, for logits, the argmax."""
    import torch

    dtypes = {e["name"]: e["dtype"] for e in entries}
    shapes = {e["name"]: e["shape"] for e in entries}

    def load(name):
        data, _ = raw[name]
        t = torch.frombuffer(bytearray(data), dtype=getattr(torch, dtypes[name]))
        return t.reshape(shapes[name]).float()

    out = []
    for name in sorted(raw):
        prompt, mode, label, site = name.split(".", 3)
        if mode != "decode" or label == "router_ids" or not dtypes[name].startswith(("bfloat", "float32")):
            continue
        for other in ("replay", "chunked"):
            twin = f"{prompt}.{other}.{label}.{site}"
            if twin not in raw or shapes[twin] != shapes[name]:
                continue
            a, b = load(name), load(twin)
            row = {"prompt": prompt, "label": label, "site": site, "against": other,
                   "max_abs_diff": float((a - b).abs().max()) if a.numel() else 0.0,
                   "max_abs": float(a.abs().max()) if a.numel() else 0.0}
            if site == "logits":
                row["argmax_equal"] = int(a.argmax()) == int(b.argmax())
            out.append(row)
    return out


def compact_case(case: dict) -> dict:
    """A prompt's run for the repository: residual hashes as [step][layer] lists per mode,
    and near ties as a count plus the first 32."""
    out = {k: v for k, v in case.items() if k not in ("prompt_text", "residual_sha256", "near_ties")}
    residual: dict[str, dict[int, dict[int, str]]] = {}
    for key, digest in case.get("residual_sha256", {}).items():
        _prompt, mode, step, _layers, layer, _out = key.split(".")
        residual.setdefault(mode, {}).setdefault(int(step.removeprefix("step")), {})[int(layer)] = digest
    out["residual_sha256"] = {
        mode: [[layers[i] for i in sorted(layers)] for _, layers in sorted(steps.items())]
        for mode, steps in residual.items()
    }
    ties = case.get("near_ties", [])
    out["near_tie_count"] = len(ties)
    out["near_ties"] = ties[:32]
    return out


def summarize(rows: list[dict]) -> list[dict]:
    """One line per prompt and mode pair: the logits rows and the five sites that differ most."""
    groups: dict[tuple, list] = {}
    for r in rows:
        groups.setdefault((r["prompt"], r["against"]), []).append(r)
    out = []
    for (prompt, other), rs in sorted(groups.items()):
        logits = [r for r in rs if r["site"] == "logits"]
        worst = sorted((r for r in rs if r["site"] != "logits"), key=lambda r: -r["max_abs_diff"])[:5]
        out.append({
            "prompt": prompt, "against": other, "compared": len(rs),
            "identical": sum(r["max_abs_diff"] == 0 for r in rs),
            "logits": [{k: r[k] for k in ("label", "max_abs_diff", "max_abs", "argmax_equal")} for r in logits],
            "largest": [{k: r[k] for k in ("label", "site", "max_abs_diff", "max_abs")} for r in worst],
        })
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dump", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--fixtures", type=Path)
    args = parser.parse_args()

    import torch

    run = json.loads((args.dump / "run.json").read_text())
    cases = [json.loads(p.read_text()) for p in sorted(args.dump.glob("*/run.json"))]
    entries, raw = [], {}
    for path in sorted(args.dump.glob("*/tensors-rank*.safetensors")):
        with safe_open(path, framework="pt") as f:
            for name in f.keys():
                t = f.get_tensor(name).contiguous()
                data = t.view(torch.uint8).numpy().tobytes() if t.numel() else b""
                entries.append(
                    {
                        "name": name,
                        "file": f"{path.parent.name}/{path.name}",
                        "shape": list(t.shape),
                        "dtype": str(t.dtype).removeprefix("torch."),
                        "element_bytes": t.element_size(),
                        "bytes": len(data),
                        "sha256": hashlib.sha256(data).hexdigest(),
                    }
                )
                raw[name] = (data, t.element_size())
    comparisons = compare_modes(raw, entries)
    names = [e["name"] for e in entries]
    assert len(names) == len(set(names)), "a tensor name appears in two rank files"
    entries.sort(key=lambda e: e["name"])
    manifest = {
        "schema": "openagents.deepseek_v41.oracle_manifest.v1",
        "revision": "1bae0e1088993b9b82de08f59de5be5a34e42170",
        "bucket": BUCKET,
        "tensors": entries,
        "mode_comparisons": comparisons,
    }
    args.out.write_text(json.dumps(manifest, indent=1) + "\n")
    print(f"{len(entries)} tensors, {sum(e['bytes'] for e in entries):,} bytes")

    if args.fixtures:
        args.fixtures.mkdir(parents=True, exist_ok=True)
        full, slices, offset = bytearray(), {}, 0
        fixture_entries = []
        for e in entries:
            prompt, mode = e["name"].split(".", 2)[:2]
            if prompt not in FIXTURE_PROMPTS or mode != "decode":
                continue  # the bucket manifest lists the rest
            data, size = raw[e["name"]]
            entry = {k: e[k] for k in ("name", "file", "shape", "dtype", "element_bytes", "bytes", "sha256")}
            keep_whole = e["bytes"] <= FULL_SMALL or (
                e["bytes"] <= FULL_LIMIT and e["name"].startswith(FULL_PROMPT + ".")
            )
            if keep_whole and len(full) + e["bytes"] <= FULL_BUDGET:
                entry["full"] = {"offset": offset, "bytes": e["bytes"]}
                full += data
                offset += e["bytes"]
            else:
                n = SLICE * size
                slices[e["name"]] = {"head": data[:n].hex(), "tail": data[-n:].hex()}
            fixture_entries.append(entry)
        prompts = [compact_case(c) for c in cases]
        fixture = {
            "schema": "openagents.deepseek_v41.oracle_fixtures.v1",
            "revision": manifest["revision"],
            "bucket": BUCKET,
            "bucket_manifest": f"{BUCKET}/manifest.json",
            "bucket_manifest_sha256": hashlib.sha256(args.out.read_bytes()).hexdigest(),
            "scope": "decode-mode tensors of the four main prompts; the bucket manifest lists every "
            "tensor of every prompt and mode, and each prompt's run.json in the bucket has every near tie",
            "labels": run["labels"],
            "modes": run["modes"],
            "precision": run["precision"],
            "tie_rules": run["tie_rules"],
            "environment": run["environment"],
            "checkpoint": CHECKPOINT,
            "prompts": prompts,
            "mode_comparisons": summarize(comparisons),
            "full_file": "full.bin",
            "full_sha256": hashlib.sha256(full).hexdigest(),
            "tensors": fixture_entries,
        }
        # one line per prompt and per tensor, so the file stays small and diffs stay readable
        tensors = fixture.pop("tensors")
        prompts = fixture.pop("prompts")
        head = json.dumps(fixture, indent=1)[:-2]
        body = ",\n".join(json.dumps(t, separators=(",", ":")) for t in tensors)
        plist = ",\n".join(json.dumps(q, separators=(",", ":")) for q in prompts)
        text = f'{head},\n "prompts": [\n{plist}\n],\n "tensors": [\n{body}\n]\n}}'
        # gzip keeps each file under the repository's 1 MB limit; mtime 0 keeps them reproducible
        def write_gz(name: str, data: bytes):
            (args.fixtures / f"{name}.gz").write_bytes(gzip.compress(data, 9, mtime=0))

        write_gz("manifest.json", (text + "\n").encode())
        write_gz("slices.json", (json.dumps(slices, separators=(",", ":")) + "\n").encode())
        write_gz("full.bin", bytes(full))
        total = sum(f.stat().st_size for f in args.fixtures.iterdir())
        print(f"fixtures: {total:,} bytes ({len(full):,} whole tensors, {len(slices)} sliced, "
              f"{len(fixture_entries)} listed)")
        if total > BUDGET * 1.1:
            print("fixtures are over budget", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
