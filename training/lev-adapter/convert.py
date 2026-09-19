#!/usr/bin/env python3
"""Converts the Lev suite into Apple's adapter training format.

The suite already carries everything training needs: a state, a typed
question, a labelled answer, and a fixed calibration/evaluation split. This
turns each item into the JSON Lines message shape the toolkit reads, with the
constrained enum expressed as a ``response_format`` on the user message so
the adapter trains against the same guided generation that serving uses.

Train and serve must share one renderer. This script reproduces the Rust
renderer in ``crates/lev/src/schema.rs``; ``check_parity.py`` proves the two
agree rather than trusting that they do.

    python3 convert.py --out data/

Writes ``train.jsonl`` from the calibration split and ``valid.jsonl`` from the
evaluation split. Training never sees an evaluation item, which is what makes
the improvement number afterwards mean anything.
"""

import argparse
import json
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
SUITE = REPO / "crates" / "lev" / "suites" / "support-v1.json"

BANDS = [
    "almost certainly not",
    "unlikely",
    "even odds",
    "likely",
    "almost certain",
]


def render(value, depth=0):
    """Mirrors `lev::render::render`."""
    pad = "  " * depth
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        out = []
        for item in value:
            if item is None:
                continue
            if isinstance(item, (dict, list)):
                out.append(f"{pad}- \n{render(item, depth + 1)}")
            else:
                out.append(f"{pad}- {render(item, depth)}")
        return "\n".join(out)
    if isinstance(value, dict):
        out = []
        for key, field in value.items():
            if field is None:
                continue
            if isinstance(field, (dict, list)):
                out.append(f"{pad}{key}:\n{render(field, depth + 1)}")
            else:
                out.append(f"{pad}{key}: {render(field, depth)}")
        return "\n".join(out)
    return str(value)


def options_and_legend(question):
    """Mirrors the option keys `lev::schema::compile` admits."""
    kind = question["type"]
    if kind == "noul":
        criteria = question.get("criteria") or {}
        return ["no", "yes"], [
            ("yes", render(criteria.get("true"))),
            ("no", render(criteria.get("false"))),
        ]
    if kind == "choice":
        criteria = question["criteria"]
        keys = list(criteria.keys())
        return keys, [(key, render(criteria[key])) for key in keys]
    if kind == "score":
        levels = question["criteria"]
        keys = [str(index) for index in range(len(levels))]
        return keys, [(str(i), render(level)) for i, level in enumerate(levels)]
    raise ValueError(f"unknown question type {kind!r}")


FALLBACK = {
    "noul": "Answer whether the statement holds for the state.",
    "choice": "Pick the one option that fits the state.",
    "score": "Pick the one level that fits the state. The levels are ordered.",
}


def instructions_text(question):
    """Mirrors `lev::schema::instructions_text`."""
    judgment = render(question.get("instructions")).strip()
    if not judgment:
        judgment = FALLBACK[question["type"]]
    _, legend = options_and_legend(question)
    lines = []
    for name, description in legend:
        if description:
            lines.append(f"- {name}: {description.replace(chr(10), ' ')}")
        else:
            lines.append(f"- {name}")
    body = "\n".join(lines)
    return (
        f"{judgment}\n\nAnswer with exactly one of the admitted options:\n{body}\n\n"
        "Judge only the state. Do not explain."
    )


def state_prompt(state):
    """Mirrors `lev::schema::state_prompt`.

    A plain label, deliberately. Fencing the state in delimiters is refused by
    Apple's guardrails; see docs/lev/architecture.md.
    """
    return f"STATE\n\n{render(state)}"


def band_for(correct, base_correct_rate):
    """The band an outcome-trained example should carry.

    Trained against outcomes, not opinions: an example the model should be
    sure about carries a high band, one it should hedge on carries a low one.
    The point is that the band means something afterwards, which it does not
    on the base model, where every item comes back `likely`.
    """
    if correct:
        return "almost certain" if base_correct_rate >= 0.9 else "likely"
    return "even odds" if base_correct_rate >= 0.5 else "unlikely"


def to_record(item, with_band, base_rates):
    options, _ = options_and_legend(item["question"])
    properties = {"choice": {"type": "string", "enum": options}}
    required = ["choice"]
    answer = {"choice": item["truth"]}
    if with_band:
        properties["certainty"] = {"type": "string", "enum": BANDS}
        required.append("certainty")
        rate = base_rates.get(item["id"], 1.0)
        answer["certainty"] = band_for(True, rate)
    return [
        {"role": "system", "content": instructions_text(item["question"])},
        {
            "role": "user",
            "content": state_prompt(item["state"]),
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "Decision",
                    "schema": {
                        "type": "object",
                        "properties": properties,
                        "required": required,
                        "additionalProperties": False,
                    },
                },
            },
        },
        {"role": "assistant", "content": json.dumps(answer)},
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", default=str(SUITE))
    parser.add_argument("--out", default="data")
    parser.add_argument(
        "--band",
        action="store_true",
        help="train the certainty band as well as the choice",
    )
    parser.add_argument(
        "--base-rates",
        help="JSON map of item id to the base model's measured accuracy, for band labels",
    )
    args = parser.parse_args()

    suite = json.loads(pathlib.Path(args.suite).read_text())
    base_rates = {}
    if args.base_rates:
        base_rates = json.loads(pathlib.Path(args.base_rates).read_text())

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    counts = {}
    for split, name in (("calibration", "train"), ("evaluation", "valid")):
        items = [item for item in suite["items"] if item["split"] == split]
        path = out / f"{name}.jsonl"
        with path.open("w") as handle:
            for item in items:
                handle.write(json.dumps(to_record(item, args.band, base_rates)) + "\n")
        counts[name] = len(items)
        print(f"wrote {path} with {len(items)} records", file=sys.stderr)

    meta = {
        "suite": suite["name"],
        "suite_digest": suite["digest"],
        "counts": counts,
        "band": args.band,
    }
    (out / "conversion.json").write_text(json.dumps(meta, indent=1) + "\n")
    print(json.dumps(meta, indent=1))


if __name__ == "__main__":
    main()
