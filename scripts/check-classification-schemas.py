#!/usr/bin/env python3
"""Validate published classification schemas and runtime response examples."""

import copy
import json
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit(
        "Install jsonschema==4.25.1 in an isolated Python environment to run this check."
    ) from error

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs" / "decision-models"


def read(path):
    return json.loads(path.read_text())


def main():
    validators = {}
    for name in ("request", "response"):
        schema = read(DOCS / "schemas" / f"classify-{name}-v1.json")
        Draft202012Validator.check_schema(schema)
        validators[name] = Draft202012Validator(schema)

    corpus = DOCS / "fixtures" / "classify-v1"
    requests = 0
    for case in read(corpus / "manifest.json")["cases"]:
        # Runtime-invalid requests can still satisfy the structural schema.
        if "code" not in case["expected"]:
            validators["request"].validate(read(corpus / case["file"]))
            requests += 1

    responses = sorted((corpus / "responses").glob("*.json"))
    if len(responses) < 7:
        raise SystemExit("The runtime response corpus is incomplete.")
    for path in responses:
        validators["response"].validate(read(path))

    original = read(corpus / "responses" / "single-label.json")
    invalid = []
    value = copy.deepcopy(original)
    value["results"][0]["units"][0]["raw"]["probabilities"]["a"] = 1.1
    invalid.append(("out-of-range probability", value))
    value = copy.deepcopy(original)
    value["results"][0]["units"][0]["selected"] = ["a"]
    invalid.append(("categorical selection with the wrong type", value))
    value = copy.deepcopy(original)
    value["usage"]["input_tokens_complete"] = False
    invalid.append(("partial usage presented as a complete total", value))
    value = copy.deepcopy(original)
    unit = value["results"][0]["units"][0]
    unit.update(outcome="unavailable", cause="fixture", selected=None)
    invalid.append(("unavailable answer retaining raw scores", value))
    value = copy.deepcopy(original)
    del value["outcomes"]["unattempted"]
    invalid.append(("missing outcome counter", value))
    for name, value in invalid:
        if validators["response"].is_valid(value):
            raise SystemExit(f"The response schema accepted {name}.")
    print(
        f"Validated {requests} requests, {len(responses)} responses, "
        f"and {len(invalid)} rejected response mutations."
    )


if __name__ == "__main__":
    main()
