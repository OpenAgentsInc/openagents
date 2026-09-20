#!/usr/bin/env python3
"""Builds `restatement-v1.json`, the panel behind openagents#9414.

The issue is one recorded answer: asked whether every task in a six-task
delegation plan is read-only, a door answered 0.17 about a state that says
the tasks are read-only. Four explanations were on the table, and they have
different fixes, so the panel separates them rather than illustrating one.

# The design

Every state names three facts and nothing else:

- `count`: whether the plan has six tasks.
- `distinct`: whether the tasks work on different files.
- `readonly`: whether any task writes.

The three vary independently, which gives eight cells. Each cell has three
states written in three different sentence shapes, so a result is a property
of the fact rather than of one phrasing. That is 24 states, and every family
is asked about all 24, so the families differ only in their question text.

The replicates are the partitions: one whole factorial in each. Every
partition therefore carries the same eight cells in the same balance, which a
random split of 24 states would not.

| Replicate | Partition | Shape |
| --- | --- | --- |
| 1 | development | Noun phrase, like the recorded call's own state |
| 2 | calibration | Two or three short sentences |
| 3 | locked | "The plan runs N tasks. ..." |

# The families

Eight question texts over those 24 states. Seven are restatements, whose
answer the state asserts; one is the inference the fan-out actually gates on.

| Family | Truth | Separates |
| --- | --- | --- |
| `count_six` | `count` | Restatement with no negation and no lexical polarity |
| `distinct_files` | `distinct` | The same, on a second fact |
| `count_and_files` | `count and distinct` | Conjunction on its own, both halves positive |
| `readonly_simple` | `readonly` | The term "read-only", alone |
| `readonly_conjunction` | `readonly` | The recorded text, word for word |
| `writes_none` | `readonly` | The same fact under an explicit negation |
| `writes_some` | `not readonly` | The same fact under the opposite polarity |
| `independent` | `distinct or readonly` | The inference `delegate-fan-out` gates on |

`writes_none` and `writes_some` have exactly complementary labels on every
state, which is what makes a sign error visible: a door that reads the fact
and inverts it scores near zero on one and near one on the other, where a
door with no signal scores near half on both.

# The labels

Every label is read off the state by a rule stated before the states were
written, and `label_rule` carries the rule on each item:

- `count` is true when the state says six tasks.
- `distinct` is true when the state says the tasks work on different files.
- `readonly` is true when the state says no task writes.
- Two tasks collide when they touch the same file and at least one writes,
  so `independent` is true unless the state says the tasks share a file and
  at least one of them writes.

`independent` is 18 true and 6 false, because only one of the four
`distinct`/`readonly` combinations collides. The six false items are the ones
a fan-out's safety rests on, and a run reports them apart from the rest.

    python3 build_restatement_v1.py > restatement-v1.json
"""

import hashlib
import json
import sys

GATE = "probability-v1"
QUESTIONS = "restatement-v1"

# (count, distinct, readonly) -> three states, one per replicate.
#
# Replicate 1 leads with the recorded call's own sentence, so the panel holds
# the state the issue is about rather than only states like it.
CELLS = {
    (True, True, True): [
        "Six read-only tasks, each reading a different file.",
        "Six tasks. Each one reads a different file and answers a question about it. No task writes anything.",
        "The plan runs six tasks. Every task opens a different file, and every task is read-only.",
    ],
    (True, True, False): [
        "Six tasks, each editing a different file and saving the change.",
        "Six tasks. Each one rewrites a different file to add a missing doc comment.",
        "The plan runs six tasks. Every task edits a different file, and every task writes.",
    ],
    (True, False, True): [
        "Six read-only tasks, all pointed at one file.",
        "Six tasks, all reading the same file and summarizing a different section of it. None of them write.",
        "The plan runs six tasks. Every task reads the same file, and no task changes it.",
    ],
    (True, False, False): [
        "Six tasks, each appending a line to one shared file.",
        "Six tasks, all editing the same file.",
        "The plan runs six tasks. Every task writes to the same file.",
    ],
    (False, True, True): [
        "Four read-only tasks, each reading a different file.",
        "Three tasks. Each one reads a different file and reports what it found. Nothing is written.",
        "The plan runs two tasks. Each reads a different file, and neither writes.",
    ],
    (False, True, False): [
        "Nine tasks, each editing a different file and saving the change.",
        "Three tasks. Each one rewrites a different file.",
        "The plan runs two tasks. Each edits a different file, and both write.",
    ],
    (False, False, True): [
        "Ten read-only tasks, all pointed at one file.",
        "Three tasks, all reading the same file. None of them write.",
        "The plan runs two tasks. Both read the same file, and neither changes it.",
    ],
    (False, False, False): [
        "Five tasks, each appending a line to one shared file.",
        "Three tasks, all editing the same file.",
        "The plan runs two tasks. Both write to the same file.",
    ],
}

PARTITIONS = ["development", "calibration", "locked"]

# family -> (truth of one state, the rule that label is read by)
FAMILIES = {
    "count_six": (
        lambda count, distinct, readonly: count,
        "True when the state says the plan has six tasks.",
    ),
    "distinct_files": (
        lambda count, distinct, readonly: distinct,
        "True when the state says the tasks work on different files.",
    ),
    "count_and_files": (
        lambda count, distinct, readonly: count and distinct,
        "True when the state says both six tasks and different files.",
    ),
    "readonly_simple": (
        lambda count, distinct, readonly: readonly,
        "True when the state says no task writes.",
    ),
    "readonly_conjunction": (
        lambda count, distinct, readonly: readonly,
        "True when the state says no task writes; the two halves of the question are one fact.",
    ),
    "writes_none": (
        lambda count, distinct, readonly: readonly,
        "True when the state says no task writes.",
    ),
    "writes_some": (
        lambda count, distinct, readonly: not readonly,
        "True when the state says at least one task writes.",
    ),
    "independent": (
        lambda count, distinct, readonly: distinct or readonly,
        "Two tasks collide when they touch the same file and at least one writes, so this is "
        "false only when the state says the tasks share a file and at least one writes.",
    ),
}


def build():
    items = []
    for family, (truth_of, rule) in FAMILIES.items():
        for cell, (facts, states) in enumerate(CELLS.items(), start=1):
            for replicate, state in enumerate(states, start=1):
                items.append(
                    {
                        "id": f"{family}/c{cell}r{replicate}",
                        "family": family,
                        "kind": "noul",
                        "state": {"plan": state},
                        "truth": "yes" if truth_of(*facts) else "no",
                        "partition": PARTITIONS[replicate - 1],
                        "label_source": "author",
                        "label_rule": rule,
                    }
                )

    suite = {
        "schema": "openagents.gym.suite.v1",
        "name": "restatement-v1",
        "created": "2026-09-19",
        "description": (
            "Twenty-four delegation plans, each stating three facts — six tasks or not, "
            "different files or not, writing or not — crossed with eight question texts "
            "about those same facts. Seven of the eight are restatements, whose answer the "
            "state asserts; the eighth is the independence inference delegate-fan-out gates "
            "on. The panel separates polarity, conjunction, and restatement in general as "
            "explanations for openagents#9414. Labels are read off the state by the rule "
            "each item carries."
        ),
        "tier": "scored",
        "gate": GATE,
        "questions": QUESTIONS,
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":")).encode()
    suite["digest"] = hashlib.sha256(blob).hexdigest()
    return suite


if __name__ == "__main__":
    json.dump(build(), sys.stdout, indent=1)
    sys.stdout.write("\n")
