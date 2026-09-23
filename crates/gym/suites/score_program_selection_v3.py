"""Report current program-selection errors without dropping unanswered items.

Run `gym compare` on the store first to verify its receipt chain. This report
requires one suite, question identity, and model execution identity per input.
It never reads a locked result or silently picks a later duplicate trial.
"""

import argparse
import collections
import hashlib
import json
from pathlib import Path


def report(suite, rows, question_digest):
    items = [item for item in suite["items"] if item["partition"] != "locked"]
    by_id = {item["id"]: item for item in items}
    answers = {}
    identities = set()
    question_digests = set()
    for row in rows:
        if row["suite_digest"] != suite["digest"]:
            raise ValueError("The row and suite digests differ")
        if row["question_digest"] != question_digest:
            raise ValueError("The row does not use the frozen question text")
        ident = row["item_id"]
        if ident not in by_id or ident in answers:
            raise ValueError(f"Unexpected, locked, or duplicate item: {ident}")
        answers[ident] = row
        identities.add(json.dumps(row["door_identity"], sort_keys=True))
        question_digests.add(row["question_digest"])
    if len(identities) != 1 or len(question_digests) != 1:
        raise ValueError("Expected exactly one model execution and question identity")
    result = {
        "suite_digest": suite["digest"],
        "question_digest": next(iter(question_digests)),
        "door_identity": json.loads(next(iter(identities))),
        "locked_scored": 0, "groups": {}, "errors": [],
    }
    for name, selected in [
        ("real", [item for item in items if item["id"].startswith("turn/")]),
        ("authored", [item for item in items if not item["id"].startswith("turn/")]),
        ("all", items),
    ]:
        counts = collections.Counter()
        errors = []
        squared_errors = []
        for item in selected:
            counts["expected"] += 1
            counts["negative" if item["truth"] == "none" else "positive"] += 1
            row = answers.get(item["id"])
            if row is None:
                counts["harness_missing"] += 1
                continue
            if not row["answered"]:
                counts["refused"] += 1
                continue
            counts["answered"] += 1
            chosen = row.get("selected")
            if chosen is None:
                raise ValueError("A new result must record its selected answer")
            correct = chosen == item["truth"]
            if row["correct"] != correct:
                raise ValueError("Stored correctness disagrees with the pinned label")
            counts["correct"] += correct
            confidence = row["raw_top"]
            squared_errors.append((confidence - int(correct)) ** 2)
            if not correct:
                kind = "spurious" if item["truth"] == "none" else "missed" if chosen == "none" else "wrong_program"
                counts[kind] += 1
                counts["confident_errors"] += confidence >= 0.9
                errors.append({"item": item["id"], "truth": item["truth"], "selected": chosen, "confidence": confidence, "kind": kind})
        group = {key: counts[key] for key in ["expected", "answered", "refused", "harness_missing", "correct", "negative", "positive", "spurious", "missed", "wrong_program", "confident_errors"]}
        group["accuracy_answered"] = counts["correct"] / counts["answered"] if counts["answered"] else None
        group["top_probability_brier"] = sum(squared_errors) / len(squared_errors) if squared_errors else None
        group["constants"] = {option: sum(item["truth"] == option for item in selected) for option in ["none", "answer-question", "burn-down", "delegate-fan-out", "review-runs"]}
        result["groups"][name] = group
        if name == "all":
            result["errors"] = errors
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results")
    parser.add_argument("--suite", default=str(Path(__file__).with_name("program-selection-v3.json")))
    args = parser.parse_args()
    suite = json.loads(Path(args.suite).read_text())
    digest = hashlib.sha256(json.dumps(suite["items"], sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
    if digest != suite["digest"]:
        raise ValueError("Suite digest mismatch")
    rows = [json.loads(line) for line in Path(args.results).read_text().splitlines() if line.strip()]
    question_path = Path(__file__).resolve().parent.parent / "questions" / "program-selection-v3.json"
    questions = json.loads(question_path.read_text())["questions"]
    question_digest = hashlib.sha256(json.dumps(questions, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
    print(json.dumps(report(suite, rows, question_digest), indent=2))


if __name__ == "__main__":
    main()
