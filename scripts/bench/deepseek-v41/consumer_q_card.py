#!/usr/bin/env python3
"""Score teacher-forced captures against the frozen consumer-q evaluation.

    consumer_q_card.py [--final --artifact-manifest PATH
      --evaluation-sha256 SHA --checker-sha256 SHA] ORACLE_DIR OUT
      CASE=CAPTURE_DIR,LABEL ...

Final cards require every frozen case under both `exact` and `consumer-q`.
Pin the evaluation and this checker from committed source before capture.
Without --final, the output is an explicitly partial diagnostic card.
The first emitted logits predict the first generated token: they match
`decode.last.logits`, followed by `decode.g1.logits`, `decode.g2.logits`,
and so on. This is descriptive quality measurement, not G1 qualification.
"""
import argparse
import hashlib
import json
import math
import re
import struct
from pathlib import Path

import numpy as np


REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
REPO = "deepseek-ai/DeepSeek-V4.1-Flash-1017"


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def checked_hash(path, expected):
    actual = sha256(path)
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected) or actual != expected:
        raise ValueError(f"{path}: SHA-256 differs from the frozen input")
    return actual


def inside(root, relative):
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"{relative}: file is outside the oracle directory")
    return path


def safetensor(path, name, expected):
    with open(path, "rb") as stream:
        n = struct.unpack("<Q", stream.read(8))[0]
        header = json.loads(stream.read(n))
        meta = header[name]
        start, end = meta["data_offsets"]
        if (meta["dtype"] != "F32" or expected["dtype"] != "float32"
                or meta["shape"] != expected["shape"] or len(meta["shape"]) != 1
                or start < 0 or end - start != expected["bytes"]
                or expected["bytes"] != 4 * meta["shape"][0]):
            raise ValueError(f"{path}: invalid logits metadata for {name}")
        stream.seek(8 + n + start)
        data = stream.read(end - start)
    if len(data) != end - start or hashlib.sha256(data).hexdigest() != expected["sha256"]:
        raise ValueError(f"{path}: oracle tensor {name} differs from the frozen input")
    return np.frombuffer(data, dtype="<f4").astype(np.float64)


def log_softmax(x):
    if x.ndim != 1 or not x.size or not np.isfinite(x).all():
        raise ValueError("Logits must be a nonempty finite vector")
    m = np.max(x)
    return x - m - math.log(np.sum(np.exp(x - m)))


def captured(folder):
    """Read contiguous answer steps, after explicitly recorded prefill steps."""
    folder = Path(folder)
    steps = sorted((int(p.name.split("-")[1].split(".")[0]), p)
                   for p in folder.glob("step-*.logits.f32"))
    if not steps:
        raise ValueError(f"{folder}: no captured logits")
    first = steps[0][0]
    if [step for step, _ in steps] != list(range(first, first + len(steps))):
        raise ValueError(f"{folder}: captured answer steps have gaps")
    for step in range(first + len(steps)):
        path = folder / f"step-{step}.json"
        metadata = json.loads(path.read_text()) if path.is_file() else {}
        if metadata.get("emit") is not (step >= first):
            raise ValueError(f"{folder}: missing or incorrect emit record for step {step}")
    expected = {step for step, _ in steps}
    for path in folder.glob("step-*.json"):
        if json.loads(path.read_text()).get("emit") is True:
            step = int(path.stem.split("-")[1])
            if step not in expected:
                raise ValueError(f"{folder}: missing logits for emitted step {step}")
    if any(not p.stat().st_size or p.stat().st_size % 4 for _, p in steps):
        raise ValueError(f"{folder}: captured logits are not complete float32 vectors")
    return [np.fromfile(p, dtype="<f4").astype(np.float64) for _, p in steps]


def validate_request(folder, oracle, positions):
    request = json.loads((Path(folder) / "request.json").read_text())
    if request["prompt_ids"] != oracle["prompt_token_ids"]:
        raise ValueError(f"{folder}: prompt tokens differ from the oracle")
    if oracle.get("thinking_mode") is not None and request.get("mode") != oracle["thinking_mode"]:
        raise ValueError(f"{folder}: prompt mode differs from the oracle")
    maximum = request.get("max_tokens")
    if type(maximum) is not int or not 0 < positions <= maximum:
        raise ValueError(f"{folder}: captured positions exceed request.max_tokens")
    forced = request.get("force_ids")
    if (not isinstance(forced, list) or len(forced) < positions
            or forced[:positions] != oracle["generated_ids"][:positions]):
        raise ValueError(f"{folder}: forced history differs from the oracle")
    return request


def validate_positions(folder, prompt_tokens, count):
    """Check where each emitted vector sits in the teacher-forced sequence."""
    records = sorted((int(p.stem.split("-")[1]), json.loads(p.read_text()))
                     for p in Path(folder).glob("step-*.json"))
    emitted = [record for _, record in records if record.get("emit") is True]
    if len(emitted) != count:
        raise ValueError(f"{folder}: emitted position count differs")
    for k, record in enumerate(emitted):
        if record.get("position", -1) + record.get("tokens", 0) - 1 != prompt_tokens - 1 + k:
            raise ValueError(f"{folder}: emitted logits are at the wrong token position")


def parse_specs(specs):
    result = []
    for spec in specs:
        case, rest = spec.split("=", 1)
        folder, _, label = rest.rpartition(",") if "," in rest else (rest, "", "run")
        if not case or case != Path(case).name or case in (".", ".."):
            raise ValueError(f"{case}: invalid case name")
        result.append((case, Path(folder), label or "run"))
    pairs = [(case, label) for case, _, label in result]
    if len(set(pairs)) != len(pairs):
        raise ValueError("Each case and label must appear only once")
    return result


def frozen_inputs(args, specs):
    """Verify independently supplied pins before reading measured logits."""
    evaluation_hash = checked_hash(args.evaluation, args.evaluation_sha256)
    checker_hash = checked_hash(Path(__file__), args.checker_sha256)
    evaluation = json.loads(args.evaluation.read_text())
    if evaluation["schema"] != "openagents.deepseek_v41.consumer_q_evaluation.v1":
        raise ValueError("Unsupported frozen evaluation")
    expected = {case["id"]: case["positions"] for case in evaluation["cases"]}
    if {(case, label) for case, _, label in specs} != {
            (case, label) for case in expected for label in ("exact", "consumer-q")}:
        raise ValueError("Final cards require every frozen case under exact and consumer-q")
    manifest_hash = checked_hash(args.oracle_dir / "manifest.json", evaluation["oracle_manifest_sha256"])
    corpus_hash = checked_hash(args.oracle_dir / "holdout-corpus.json", evaluation["oracle_corpus_sha256"])
    run_pins = evaluation["oracle_run_sha256"]
    if set(run_pins) != set(expected):
        raise ValueError("Frozen evaluation must pin every oracle run record")
    run_hashes = {case: checked_hash(inside(args.oracle_dir, f"{case}/run.json"), run_pins[case])
                  for case in expected}
    artifact = json.loads(args.artifact_manifest.read_text())
    if (artifact.get("schema") != "openagents.deepseek_v41.consumer_q.v1"
            or artifact.get("profile") != "consumer-q" or artifact.get("lossy") is not True
            or artifact.get("qualified") is not False
            or artifact.get("source") != {"repo": REPO, "revision": REVISION}):
        raise ValueError("Target artifact is not a consumer-q artifact of the frozen checkpoint")
    return evaluation, {
        "evaluation_sha256": evaluation_hash,
        "checker_sha256": checker_hash,
        "oracle_manifest_sha256": manifest_hash,
        "oracle_corpus_sha256": corpus_hash,
        "oracle_run_sha256": run_hashes,
        "artifact_manifest_sha256": sha256(args.artifact_manifest),
    }


def capture_hashes(folder, final, checkpoint):
    manifest = folder / "manifest.json"
    if not manifest.is_file():
        manifest = folder.parent / "manifest.json"
    if final and (not manifest.is_file() or json.loads(manifest.read_text()).get("index_sha256") != checkpoint):
        raise ValueError(f"{folder}: capture manifest differs from the frozen checkpoint")
    if final:
        for path in folder.glob("step-*.json"):
            if not path.with_suffix(".msg").is_file():
                raise ValueError(f"{folder}: missing captured stage input for {path.stem}")
    files = {p.name: sha256(p) for p in sorted(folder.iterdir()) if p.is_file()}
    if manifest.is_file() and manifest.parent != folder:
        files["../manifest.json"] = sha256(manifest)
    return files


def build_card(args):
    specs = parse_specs(args.specs)
    evaluation, pins = frozen_inputs(args, specs) if args.final else (None, {
        "checker_sha256": sha256(Path(__file__)),
        "oracle_manifest_sha256": sha256(args.oracle_dir / "manifest.json"),
    })
    manifest = json.loads((args.oracle_dir / "manifest.json").read_text())
    if args.final and manifest.get("revision") != REVISION:
        raise ValueError("Oracle checkpoint revision differs")
    tensors = {t["name"]: t for t in manifest["tensors"]}
    if len(tensors) != len(manifest["tensors"]):
        raise ValueError("Oracle manifest repeats tensor names")
    expected = {c["id"]: c["positions"] for c in evaluation["cases"]} if evaluation else {}
    corpus = json.loads((args.oracle_dir / "holdout-corpus.json").read_text()) if args.final else None
    corpus_cases = {c["id"]: c for c in corpus["cases"]} if corpus else {}
    pooled, rows, captures, oracle_files, used_tensors = {}, [], [], {}, {}
    for case, folder, label in specs:
        run_path = inside(args.oracle_dir, f"{case}/run.json")
        run = json.loads(run_path.read_text())
        generated = run["generated_ids"]
        logits = captured(folder)
        request = validate_request(folder, run, len(logits))
        validate_positions(folder, len(request["prompt_ids"]), len(logits))
        checkpoint = evaluation["checkpoint_index_sha256"] if args.final else None
        if args.final:
            count = expected[case]
            if (len(logits) != count or len(generated) != count or run.get("id") != case
                    or run.get("n_gen") != count or corpus_cases[case]["n_gen"] != count
                    or run.get("thinking_mode") != corpus_cases[case].get("thinking_mode")):
                raise ValueError(f"{case}: run or capture differs from the frozen case")
            artifact_hash = request.get("consumer_q_manifest_sha256")
            if (request.get("checkpoint_index_sha256") != checkpoint
                    or (label == "exact" and artifact_hash is not None)
                    or (label == "consumer-q" and artifact_hash != pins["artifact_manifest_sha256"])):
                raise ValueError(f"{case}/{label}: capture artifact differs from its declared label")
        captures.append({"case": case, "label": label, "files": capture_hashes(folder, args.final, checkpoint)})
        oracle_files[f"{case}/run.json"] = sha256(run_path)
        for k, q in enumerate(logits):
            name = f"{case}.decode.{'last' if k == 0 else f'g{k}'}.logits"
            if name not in tensors or k >= len(generated):
                raise ValueError(f"{case}: no matching oracle position for capture step {k}")
            meta = tensors[name]
            oracle_path = inside(args.oracle_dir, meta["file"])
            o = safetensor(oracle_path, name, meta)
            if args.final:
                step = run["steps"][k]
                if (step["logits_sha256"] != meta["sha256"] or step["argmax"] != generated[k]
                        or int(np.argmax(o)) != generated[k]
                        or step["position"] != len(run["prompt_token_ids"]) - 1 + k
                        or step["input_token"] != (run["prompt_token_ids"][-1] if k == 0 else generated[k - 1])):
                    raise ValueError(f"{case}: oracle run does not match its frozen tensor or token position")
            used_tensors[name] = meta["sha256"]
            if o.shape != q.shape:
                raise ValueError(f"{case}: oracle and capture vocabulary sizes differ")
            lo, lq = log_softmax(o), log_softmax(q)
            po, pq = np.exp(lo), np.exp(lq)
            ref = generated[k]
            if type(ref) is not int or not 0 <= ref < len(o):
                raise ValueError(f"{case}: oracle token is outside the vocabulary")
            row = {
                "label": label, "case": case, "position": k + 1, "oracle_tensor": name,
                "input_token": run["prompt_token_ids"][-1] if k == 0 else generated[k - 1],
                "reference_token": ref,
                "kl": float(np.sum(po * (lo - lq))),
                "tv": float(0.5 * np.sum(np.abs(po - pq))),
                "top1_agrees": int(np.argmax(o)) == int(np.argmax(q)),
                "ref_logprob_error": float(lq[ref] - lo[ref]),
                "runtime": request["runtime"],
                "checkpoint_index_sha256": request["checkpoint_index_sha256"],
                "consumer_q_manifest_sha256": request.get("consumer_q_manifest_sha256"),
            }
            rows.append(row)
            pooled.setdefault(label, []).append(row)
    summary = {}
    for label, rs in pooled.items():
        kls = np.array([r["kl"] for r in rs])
        summary[label] = {
            "positions": len(rs), "kl_mean": float(kls.mean()),
            "kl_p90": float(np.quantile(kls, 0.9)), "kl_max": float(kls.max()),
            "tv_mean": float(np.mean([r["tv"] for r in rs])),
            "top1_agreement": float(np.mean([r["top1_agrees"] for r in rs])),
            "ref_logprob_error_mean": float(np.mean([r["ref_logprob_error"] for r in rs])),
            "cases": sorted({r["case"] for r in rs}),
        }
    return {
        "schema": "openagents.deepseek_v41.consumer_q_card.v1", "issue": 12465,
        "lossy": True, "qualified": False, "final": args.final,
        "diagnostic": not args.final, "partial": not args.final,
        "note": "Teacher-forced logits at matching token positions. consumer-q is lossy and unqualified.",
        "inputs": pins, "captures": captures, "oracle_files": oracle_files,
        "oracle_tensors": used_tensors, "summary": summary, "rows": rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--final", action="store_true")
    parser.add_argument("--evaluation", type=Path, default=Path(__file__).with_name("consumer_q_evaluation.json"))
    parser.add_argument("--evaluation-sha256")
    parser.add_argument("--checker-sha256")
    parser.add_argument("--artifact-manifest", type=Path)
    parser.add_argument("oracle_dir", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("specs", nargs="+")
    args = parser.parse_args()
    if args.final and not all((args.evaluation_sha256, args.checker_sha256, args.artifact_manifest)):
        parser.error("--final requires --evaluation-sha256, --checker-sha256, and --artifact-manifest")
    card = build_card(args)
    with args.out.open("x" if args.final else "w") as output:
        output.write(json.dumps(card, indent=1, allow_nan=False) + "\n")
    print(json.dumps(card["summary"], indent=1))


if __name__ == "__main__":
    main()
