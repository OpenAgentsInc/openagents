#!/usr/bin/env python3
"""Builds `program-selection-v1.json`, the suite that scores which program
a request asks for.

`crates/coder/src/turn.rs` asks one question before every turn: which
program does this request ask this machine to run, from the ones this host
would run, or **none**. A wrong `none` costs a retry. A wrong program runs
a program nobody asked for, which for `delegate-fan-out` means subprocesses.
The two errors are not the same size, and an accuracy figure treats them as
though they were, so this suite exists to have them counted apart.

Where the states come from
--------------------------

Two places, kept apart in the item ids and never pooled silently.

`turn/…`
    Real turns. They are the `action` family of `coder-turns-v1`, whose
    builder harvested them from the local coding-agent session record for
    this repository under the filters `AGENTS.md` requires. Reusing that
    file rather than re-harvesting keeps one copy of those states in the
    repository and inherits the filtering that put them there. Each item
    keeps the id and partition that suite gave it, its locked partition
    included: inheriting the split is one fewer arbitrary choice than
    rolling a new one, and a partition somebody picked is not a holdout.

`authored/…`
    Written, not harvested. Real traffic holds almost no program requests —
    one in thirty-two here — so nothing harvested could say whether the
    question recognises a program request when it sees one. These say that
    and nothing about prevalence.

Where the labels come from
--------------------------

One place: `program-selection-v1-judgments.json`, written before any door
was asked and committed so it can be argued with. Every label is a reading,
so every item carries `label_source: author` and the rule that produced it.

What the suite does not carry
-----------------------------

The question text. It lives in `crates/gym/questions/program-selection-v1.json`,
which is generated from `questions/program.json` and the programs this host
admits, and pinned to them by a test in `crates/coder`. A suite that held a
copy of the wording would score the copy.

Usage
-----

    python3 crates/gym/suites/build_program_selection_v1.py \\
        > crates/gym/suites/program-selection-v1.json
"""

import argparse
import hashlib
import json
import os
import sys

SCHEMA = "openagents.gym.suite.v1"
GATE = "probability-v1"
QUESTIONS = "program-selection-v1"

#: The family every item is asked under, which is the question's own id.
FAMILY = "program"

#: The rule behind every label in this suite, carried per item because the
#: digest covers items and a rule kept anywhere else can be rewritten
#: without the suite noticing.
RULE = (
    "author: the request is labelled with a program when it asks this machine "
    "to run that program's work — `delegate-fan-out` for several pieces of "
    "work handed to other sessions at once, `answer-question` for one question "
    "about the repository handed to an executor — and `none` otherwise"
)

#: Where the harvested turns are read from, and the family that holds a
#: turn's own request.
SOURCE_SUITE = "coder-turns-v1.json"
SOURCE_FAMILY = "action"


def turns(path, labels):
    """The real turns, as selection items."""
    with open(path, encoding="utf-8") as handle:
        source = json.load(handle)
    items = []
    for item in source["items"]:
        if item["family"] != SOURCE_FAMILY:
            continue
        ident = "turn/{}".format(item["id"].split("/", 1)[1])
        if ident not in labels:
            raise SystemExit("{} has no label".format(ident))
        items.append(
            {
                "id": ident,
                "family": FAMILY,
                "kind": "choice",
                "state": {"request": item["state"]["task"]},
                "truth": labels[ident],
                "partition": item["partition"],
                "label_source": "author",
                "label_rule": RULE,
            }
        )
    return items


def authored(written):
    """The written requests, as selection items.

    They sit in the development partition: nothing is fitted on this suite,
    and an item written to be recognised is the last one a map should ever
    be tuned on.
    """
    return [
        {
            "id": item["id"],
            "family": FAMILY,
            "kind": "choice",
            "state": {"request": item["request"]},
            "truth": item["truth"],
            "partition": "development",
            "label_source": "author",
            "label_rule": RULE,
        }
        for item in written
    ]


def build(source, judgments):
    items = turns(source, judgments["labels"]) + authored(judgments["authored"])
    suite = {
        "schema": SCHEMA,
        "name": "program-selection-v1",
        "created": "2026-09-19",
        "description": (
            "Requests, asked which program they want, in the state shape "
            "coder::runtime::select sends: one field, the operator's sentence. "
            "`turn/…` items are real turns, reused from coder-turns-v1's "
            "`action` family, partitions and all; `authored/…` items "
            "are written, because real traffic holds almost no program "
            "requests and nothing harvested would say whether the question "
            "recognises one. Every label is a reading and says so. The two "
            "sources are never pooled without saying which is which, and the "
            "false-positive and false-negative rates are reported apart: a "
            "missed program costs a retry and a spurious one runs a program "
            "nobody asked for."
        ),
        "tier": "scored",
        "gate": GATE,
        "questions": QUESTIONS,
        "sampling": {
            "turns": len([item for item in items if item["id"].startswith("turn/")]),
            "authored": len(judgments["authored"]),
            "locked": len([item for item in items if item["partition"] == "locked"]),
        },
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite["digest"] = hashlib.sha256(blob.encode()).hexdigest()
    return suite


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        default=os.path.join(here, SOURCE_SUITE),
        help="the suite the real turns are read from",
    )
    parser.add_argument(
        "--judgments",
        default=os.path.join(here, "program-selection-v1-judgments.json"),
        help="the author's labels and the written requests",
    )
    args = parser.parse_args()

    with open(args.judgments, encoding="utf-8") as handle:
        judgments = json.load(handle)
    json.dump(build(args.source, judgments), sys.stdout, indent=1, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
