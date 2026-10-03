#!/usr/bin/env python3
"""Compare one four-question Jev batch with four sequential requests."""
import argparse
import hashlib
import json
from pathlib import Path

import gateway


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = args.request.read_bytes()
    value = gateway.loads(raw)
    questions = value["questions"]
    if len(questions) != 4:
        raise ValueError("Supply exactly four independent questions")
    gateway.validate_questions(questions)
    args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
    result = {"request_sha256": hashlib.sha256(raw).hexdigest(),
              "design": "batch then sequential; sequential then batch", "blocks": []}
    for number, order in enumerate((("batch", "sequential"), ("sequential", "batch")), 1):
        block = {"number": number, "order": order, "conditions": {}}
        result["blocks"].append(block)
        for condition in order:
            groups = [questions] if condition == "batch" else [{k: q} for k, q in questions.items()]
            receipts, answers = [], {}
            for index, group in enumerate(groups):
                receipt, response = gateway.call(value["state"], group,
                    args.output / f"{number}-{condition}-{index + 1}")
                receipts.append(receipt)
                if response is not None:
                    answers.update(response["answers"])
                if receipt["cost_status"] != "gateway_reported":
                    raise RuntimeError("Accounting unavailable; retain artifacts and stop")
            block["conditions"][condition] = {
                "receipts": receipts, "answers": answers,
                "cost_usd": sum(r["cost_usd"] for r in receipts),
                "api_wall_s": sum(r["wall_s"] for r in receipts)}
        block["exact_answer_agreement"] = {
            k: block["conditions"]["batch"]["answers"].get(k) ==
               block["conditions"]["sequential"]["answers"].get(k) for k in questions}
        (args.output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({"block": number, "conditions": {
            k: {f: v[f] for f in ("cost_usd", "api_wall_s")} for k, v in block["conditions"].items()},
            "exact_answer_agreement": block["exact_answer_agreement"]}), flush=True)


if __name__ == "__main__":
    main()
