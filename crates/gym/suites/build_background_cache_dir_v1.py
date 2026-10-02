#!/usr/bin/env python3
"""Builds `background-cache-dir-v1.json` and its question set.

The background rules' unknown-folder judgment (docs/background, phase 3,
#10158) shows Jev one folder no cleanup class covers and asks two things:
a Noul, "Is this directory output a program will regenerate, or a download
cache, holding nothing a person made?", read against the setting
`background.cache_dir` (0.9), and a Choice of kind. This suite is the
labeled set the setting is calibrated on: folders a person would recognize
as build output, package caches, and application caches, beside user data,
source, and folders nobody can call from what is shown.

Each row of `crates/background/fixtures/cache-dir-v1.json` becomes two
items, `cache/<id>` (Noul, truth yes or no) and `kind/<id>` (Choice). The
state is the text `background::judged::Unknown::state` builds, under
`directory`; `crates/background/src/judged_suite.rs` rebuilds it from the
fixture with the production code and fails when they drift, and checks the
question set is the production question word for word.

Run from the repository root:

    python3 crates/gym/suites/build_background_cache_dir_v1.py
"""

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "crates/background/fixtures/cache-dir-v1.json"
SUITE = ROOT / "crates/gym/suites/background-cache-dir-v1.json"
QUESTIONS = ROOT / "crates/gym/questions/background-cache-dir-v1.json"

GB = 1_000_000_000

QUESTION = (
    "Is this directory output a program will regenerate, or a download "
    "cache, holding nothing a person made?"
)
KINDS = [
    ("build_output", "Output a build tool writes and rebuilds (compiled objects, bundles)."),
    ("package_cache", "Downloaded packages or dependencies a package manager fetches again."),
    ("app_cache", "A program's cache it refills on its own (thumbnails, downloads, indexes)."),
    ("user_data", "Things a person made or collected: documents, media, datasets, settings, keys."),
    ("source", "Source code or a project someone works on."),
    ("unknown", "Cannot tell from what is shown."),
]


def size(n):
    if n >= GB:
        return f"{(n + GB // 2) // GB} GB"
    return f"{(n + 500_000) // 1_000_000} MB"


def state(row):
    top = ", ".join(f"{name} ({size(bytes_)})" for name, bytes_ in row["top"]) or "none"
    markers = ", ".join(row["markers"]) or "none"
    return (
        f"Path: {row['path']}\n"
        f"Size: {size(row['bytes'])}\n"
        f"Last changed: {row['age_days']} days ago\n"
        f"Largest entries: {top}\n"
        f"Markers: {markers}\n"
        f"Processes using it now: {row['users']}"
    )


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def main():
    rows = json.loads(FIXTURE.read_text())["rows"]
    items = []
    for row in rows:
        directory = {"directory": state(row)}
        rule = row["why"]
        items.append(
            {
                "id": f"cache/{row['id']}",
                "family": "cache",
                "kind": "noul",
                "state": directory,
                "truth": "yes" if row["cache"] else "no",
                "partition": row["partition"],
                "label_rule": rule,
            }
        )
        items.append(
            {
                "id": f"kind/{row['id']}",
                "family": "kind",
                "kind": "choice",
                "state": directory,
                "truth": row["kind"],
                "partition": row["partition"],
                "label_rule": rule,
            }
        )
    digest = hashlib.sha256(canonical(items).encode()).hexdigest()
    suite = {
        "schema": "openagents.gym.suite.v1",
        "name": "background-cache-dir-v1",
        "description": (
            "Folders no background cleanup class covers, in the state the unknown-folder "
            "judgment shows Jev (#10158): build output, package and application caches, user "
            "data, source, and folders nobody can call, each labeled whether it holds only "
            "what a program regenerates and what kind it is. Calibrates background.cache_dir."
        ),
        "created": "2026-10-02",
        "tier": "smoke",
        "gate": "decision-v1",
        "questions": "background-cache-dir-v1",
        "digest": digest,
        "items": items,
    }
    SUITE.write_text(json.dumps(suite, indent=2, ensure_ascii=False) + "\n")
    questions = {
        "$comment": [
            "The production questions of crates/background/src/judged.rs (QUESTION and ALL_KINDS), word for word; judged_suite.rs checks they match."
        ],
        "schema": "openagents.gym.question_set.v1",
        "id": "background-cache-dir-v1",
        "suite": "background-cache-dir-v1",
        "questions": {
            "cache": {
                "type": "noul",
                "instructions": QUESTION,
                "decision": {"threshold": 0.9},
            },
            "kind": {
                "type": "choice",
                "instructions": {"question": "What kind of folder is this?"},
                "criteria": {kind: {"what": what} for kind, what in KINDS},
            },
        },
    }
    QUESTIONS.write_text(json.dumps(questions, indent=2, ensure_ascii=False) + "\n")
    print(f"{len(items)} items, digest {digest}")


if __name__ == "__main__":
    main()
