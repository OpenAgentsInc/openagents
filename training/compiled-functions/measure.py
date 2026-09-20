"""Compile the `routing` spec on ProgramAsWeights and score it on our suite.

One run compiles a spec on the hosted PAW service, loads the compiled program
through the SDK, asks it every held-out `routing` item, and writes a JSON
record with each item's raw output beside the label. Nothing here fits
anything: the labels are read only to score, and the four example pairs the
`with-examples` spec carries come from the calibration partitions of both
suites, so no scored item appears in any spec.

    .venv/bin/python measure.py --spec prose --out runs/prose.json
    .venv/bin/python measure.py --spec with-examples --out runs/with-examples.json
    .venv/bin/python measure.py --spec base-prompt --out runs/base-prompt.json
    .venv/bin/python measure.py --spec base-prompt-examples --out runs/base-prompt-examples.json

`--spec base-prompt` skips the compiler and prompts the interpreter's own
base model with the same prose, so the record holds what the hypernetwork
added over the base it writes into. `--spec base-prompt-examples` prompts
it with the `with-examples` spec instead, the same four pairs as few-shot
examples.

A compiled function returns a bare string, so accuracy is the only metric on
the panel that can be computed. The record says so instead of inventing a
probability.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
SUITES = HERE / "../../crates/gym/suites"
SUPPORT_V2 = SUITES / "support-v2.json"
THREE_WAY = SUITES / "support-v2-three-way.json"

COMPILER = "paw-4b-qwen3-0.6b"
EXAMPLES = 4

# How a base-prompt run appends the message to the spec. `input-output`
# mirrors the example pairs in the `with-examples` spec; `message-team`
# names the field the way the base model's chat template expects a question.
BASE_FORMS = {
    "input-output": "\n\nInput: {state}\nOutput:",
    "message-team": "\n\nMessage: {state}\nTeam:",
}


def load_items() -> dict:
    """Reads both suites and returns the routing items by role.

    `scored` is the union of the `support-v2` evaluation split and the
    `support-v2-three-way` development partition. `pool` holds the items
    that are in the calibration partition of both suites; the example pairs
    come from the front of that pool. The `locked` partition is never read.
    """
    two = json.loads(SUPPORT_V2.read_text())
    three = json.loads(THREE_WAY.read_text())
    routing2 = [i for i in two["items"] if i["family"] == "routing"]
    routing3 = {i["id"]: i for i in three["items"] if i["family"] == "routing"}
    for item in routing2:
        other = routing3[item["id"]]
        if other["state"] != item["state"] or other["truth"] != item["truth"]:
            raise SystemExit(f"{item['id']} differs between the two suites")
    questions = {json.dumps(i["question"], sort_keys=True) for i in routing2}
    if len(questions) != 1:
        raise SystemExit("routing items do not share one question")
    evaluation = [i for i in routing2 if i["split"] == "evaluation"]
    development = [
        i for i in routing2 if routing3[i["id"]]["partition"] == "development"
    ]
    pool = [
        i
        for i in routing2
        if i["split"] == "calibration"
        and routing3[i["id"]]["partition"] == "calibration"
    ]
    scored_ids = {i["id"] for i in evaluation} | {i["id"] for i in development}
    scored = [i for i in routing2 if i["id"] in scored_ids]
    return {
        "question": routing2[0]["question"],
        "scored": scored,
        "evaluation_ids": sorted(i["id"] for i in evaluation),
        "development_ids": sorted(i["id"] for i in development),
        "pool": pool,
        "digests": {
            "support-v2": two["digest"],
            "support-v2-three-way": three["digest"],
        },
    }


def pick_examples(pool: list[dict], count: int) -> list[dict]:
    """Takes the first item of each label in pool order, then fills in order."""
    chosen: list[dict] = []
    seen: set[str] = set()
    for item in pool:
        if item["truth"] not in seen:
            chosen.append(item)
            seen.add(item["truth"])
    for item in pool:
        if len(chosen) >= count:
            break
        if item not in chosen:
            chosen.append(item)
    return chosen[:count]


def prose(question: dict) -> str:
    options = list(question["criteria"])
    lines = [question["instructions"], ""]
    for name, meaning in question["criteria"].items():
        lines.append(f"- {name}: {meaning}")
    lines += ["", f"Return ONLY one of: {', '.join(options)}."]
    return "\n".join(lines)


def build_spec(kind: str, data: dict) -> tuple[str, list[str]]:
    text = prose(data["question"])
    if kind in ("prose", "base-prompt"):
        return text, []
    if kind in ("with-examples", "base-prompt-examples"):
        examples = pick_examples(data["pool"], EXAMPLES)
        parts = [text, ""]
        for item in examples:
            parts += [f"Input: {item['state']}", f"Output: {item['truth']}", ""]
        return "\n".join(parts).rstrip() + "\n", [i["id"] for i in examples]
    raise SystemExit(f"unknown spec kind {kind}")


def parse_output(raw: str, options: list[str]) -> str | None:
    """Maps a bare string to an option, or None when it is not one.

    Only the first word counts, after markdown emphasis and punctuation are
    removed, so a reply that explains itself is read by its opening label.
    """
    words = [w.strip("*`\"'.,:;!()[]") for w in raw.strip().lower().split()]
    if not words or words[0] not in options:
        return None
    return words[0]


def wilson(correct: int, total: int) -> tuple[float, float]:
    if total == 0:
        return (0.0, 0.0)
    z = 1.959963984540054
    p = correct / total
    denominator = 1 + z * z / total
    centre = (p + z * z / (2 * total)) / denominator
    half = z * math.sqrt(p * (1 - p) / total + z * z / (4 * total * total))
    half /= denominator
    return (max(0.0, centre - half), min(1.0, centre + half))


def summarize(rows: list[dict], ids: list[str] | None = None) -> dict:
    subset = rows if ids is None else [r for r in rows if r["id"] in set(ids)]
    total = len(subset)
    correct = sum(1 for r in subset if r["correct"])
    unparsed = sum(1 for r in subset if r["answer"] is None)
    accuracy = correct / total if total else 0.0
    low, high = wilson(correct, total)
    return {
        "items": total,
        "correct": correct,
        "unparsed": unparsed,
        "accuracy": round(accuracy, 4),
        "se": round(math.sqrt(accuracy * (1 - accuracy) / total), 4) if total else 0.0,
        "wilson_95": [round(low, 4), round(high, 4)],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", choices=["prose", "with-examples", "base-prompt", "base-prompt-examples"], required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--compiler", default=COMPILER)
    parser.add_argument("--programs", type=Path, default=HERE / "runs/programs.json")
    parser.add_argument("--gpu-layers", type=int, default=0)
    parser.add_argument(
        "--base-form",
        choices=sorted(BASE_FORMS),
        default="message-team",
        help="how a base-prompt run appends the message to the spec",
    )
    args = parser.parse_args()

    import programasweights as paw

    data = load_items()
    spec, example_ids = build_spec(args.spec, data)
    spec_digest = hashlib.sha256(spec.encode()).hexdigest()
    options = list(data["question"]["criteria"])

    programs = json.loads(args.programs.read_text()) if args.programs.exists() else {}
    program_record = None
    compile_seconds = None
    if args.spec.startswith("base-prompt"):
        fn = paw.function(None, interpreter="Qwen/Qwen3-0.6B", n_gpu_layers=args.gpu_layers)

        form = BASE_FORMS[args.base_form]

        def ask(state: str) -> str:
            return fn(spec + form.format(state=state), max_tokens=8)

    else:
        key = f"{args.compiler}:{spec_digest}"
        if key in programs:
            program_record = programs[key]
        else:
            started = time.monotonic()
            # The free tier compiles anonymously only as a public program.
            # The spec holds nothing that is not already in the suite file.
            program = paw.compile(spec, compiler=args.compiler, public=True)
            compile_seconds = round(time.monotonic() - started, 2)
            if program.status != "ready":
                raise SystemExit(f"compile failed: {program.error}")
            program_record = {
                "id": program.id,
                "compiler": args.compiler,
                "compiler_snapshot": program.compiler_snapshot,
                "compile_seconds": compile_seconds,
                "compiled_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                "spec_kind": args.spec,
                "spec_sha256": spec_digest,
            }
            programs[key] = program_record
            args.programs.parent.mkdir(parents=True, exist_ok=True)
            args.programs.write_text(json.dumps(programs, indent=2, sort_keys=True) + "\n")
        fn = paw.function(program_record["id"], n_gpu_layers=args.gpu_layers)

        def ask(state: str) -> str:
            return fn(state, max_tokens=8)

    rows = []
    started = time.monotonic()
    for item in data["scored"]:
        t0 = time.monotonic()
        raw = ask(item["state"])
        latency_ms = round((time.monotonic() - t0) * 1000, 1)
        answer = parse_output(raw, options)
        rows.append(
            {
                "id": item["id"],
                "truth": item["truth"],
                "raw": raw,
                "answer": answer,
                "correct": answer == item["truth"],
                "latency_ms": latency_ms,
            }
        )
        print(f"{item['id']} {item['truth']:<10} {raw!r}", file=sys.stderr)
    inference_seconds = round(time.monotonic() - started, 1)

    record = {
        "schema": "openagents.training.compiled_functions.v1",
        "recorded_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "spec_kind": args.spec,
        "spec": spec,
        "spec_sha256": spec_digest,
        "example_ids": example_ids,
        "base_form": args.base_form if args.spec.startswith("base-prompt") else None,
        "program": program_record,
        "interpreter": "Qwen/Qwen3-0.6B",
        "sdk": {"programasweights": paw.__version__},
        "machine": {
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "gpu_layers": args.gpu_layers,
            "cpus": os.cpu_count(),
        },
        "suites": data["digests"],
        "scored": {
            "support-v2 evaluation": summarize(rows, data["evaluation_ids"]),
            "support-v2-three-way development": summarize(rows, data["development_ids"]),
            "union": summarize(rows),
        },
        "panel": "accuracy only; a compiled function returns a bare string and no distribution, so ECE, Brier, and log loss are not defined",
        "inference_seconds": inference_seconds,
        "items": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record["scored"], indent=2))


if __name__ == "__main__":
    main()
