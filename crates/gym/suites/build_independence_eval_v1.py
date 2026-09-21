#!/usr/bin/env python3
"""Builds `independence-eval-v1.json`, the v1-versus-v2 independence panel.

openagents#9508 reworded the fan-out's independence gate:
`openagents.independence.v1` says "the six tasks", which is the golden's
count and not the work list's, and `openagents.independence.v2` asks about
"the listed tasks" and adds the dependence half of the rule. This suite is
the labeled comparison the binding flip is decided on: twelve task lists in
the shape `crates/coder/src/runtime.rs::plan_state` hands the `decide`
step, at 0, 1, 2, 6, and 12 tasks, drawn from the fixture shapes
`crates/coder-project/src/semantics.rs` pins.

# The design

Every item is one task list, and the family's question is the gate's:
can these tasks run at once. The shapes are the semantics harness's four:

- disjoint: nothing the tasks touch overlaps.
- colliding: two or more tasks write the same file, with the `collisions`
  field a real lookup would have found carried in the state.
- mixed: readers against writers with one real collision inside mostly
  disjoint work.
- ambiguous: two tasks that cannot declare what they write, so a
  collision cannot be ruled out.

The counts are the ones the binding cares about: six is what v1's wording
names, zero and one are the degenerate lists, two is the smallest pair,
and twelve is the `max_results` bound `delegate-fan-out` declares.

# The labels

Every label is the author's, read off the state by the rule the item's
`label_rule` states: `yes` when no two tasks can collide and nothing is
ordered, `no` when a pair collides, a pair is ordered, or a footprint
cannot be established. The ambiguous item's `no` is the gate's safe
answer, not a claim the tasks collide.

    python3 build_independence_eval_v1.py > independence-eval-v1.json
"""

import hashlib
import json
import sys

GATE = "decision-v1"
QUESTIONS = "independence-gate-v1"

REQUEST = "Fan out one session per task."


def plan(tasks):
    return {
        "plan": {
            "program": "delegate-fan-out",
            "request": REQUEST,
            "executor": "devin",
            "tasks": len(tasks),
        },
        "tasks": tasks,
    }


def reader(n, path, prompt=None):
    return {
        "id": f"t{n}",
        "prompt": prompt or f"Read {path} and report what it says.",
        "reads": path,
        "writes": False,
    }


def writer(n, paths, prompt, reads=None):
    return {
        "id": f"t{n}",
        "prompt": prompt,
        "reads": reads,
        "writes": True,
        "touches": paths,
    }


def collision(path, work):
    return {"path": path, "work": work}


def item(id, partition, tasks, truth, rule, collisions=None):
    state = plan(tasks)
    if collisions:
        state["collisions"] = collisions
    return {
        "id": id,
        "family": "independence",
        "kind": "noul",
        "state": state,
        "truth": truth,
        "partition": partition,
        "label_source": "author",
        "label_rule": rule,
    }


def build():
    six_readers = [reader(n, f"docs/part-{n}.md") for n in range(1, 7)]
    six_writers = [
        writer(n, [f"docs/part-{n}.md"], f"Write the summary to docs/part-{n}.md.")
        for n in range(1, 7)
    ]
    six_appenders = [
        writer(n, ["docs/log.md"], f"Append a summary line to docs/log.md.")
        for n in range(1, 7)
    ]
    twelve_readers = [reader(n, f"docs/page-{n:02d}.md") for n in range(1, 13)]

    items = [
        item(
            "independence/n0-empty",
            "development",
            [],
            "no",
            "The list names no tasks, so there is nothing to run at once; the "
            "admitting answer is no.",
        ),
        item(
            "independence/n1-single",
            "development",
            [reader(1, "docs/overview.md")],
            "yes",
            "A list of one task has no pair that can collide.",
        ),
        item(
            "independence/n2-disjoint-read",
            "calibration",
            [reader(1, "src/a.rs"), reader(2, "src/b.rs")],
            "yes",
            "Both tasks only read, and they read different files.",
        ),
        item(
            "independence/n2-shared-write",
            "calibration",
            [
                writer(1, ["docs/log.md"], "Append a summary line to docs/log.md."),
                writer(2, ["docs/log.md"], "Append a result line to docs/log.md."),
            ],
            "no",
            "Both tasks write the same file.",
            [collision("docs/log.md", ["t1", "t2"])],
        ),
        item(
            "independence/n2-ordered",
            "development",
            [
                reader(1, "src/a.rs"),
                {
                    "id": "t2",
                    "prompt": "Read src/b.rs and report what it says.",
                    "reads": "src/b.rs",
                    "writes": False,
                    "after": ["t1"],
                },
            ],
            "no",
            "t2 is declared after t1, so the pair cannot run at the same time "
            "whatever their footprints say.",
        ),
        item(
            "independence/n6-disjoint-read",
            "calibration",
            six_readers,
            "yes",
            "Six read-only tasks on six different files.",
        ),
        item(
            "independence/n6-shared-append",
            "development",
            six_appenders,
            "no",
            "Every task writes the same file.",
            [collision("docs/log.md", [f"t{n}" for n in range(1, 7)])],
        ),
        item(
            "independence/n6-mixed",
            "calibration",
            [
                writer(
                    1,
                    ["src/a.rs", "docs/a-summary.md"],
                    "Summarize src/a.rs into docs/a-summary.md.",
                    reads="src/a.rs",
                ),
                reader(2, "docs/a-summary.md", "Check docs/a-summary.md for accuracy."),
                reader(3, "src/c.rs"),
                reader(4, "src/d.rs"),
                reader(5, "src/e.rs"),
                reader(6, "src/f.rs"),
            ],
            "no",
            "t2 reads the file t1 writes, so that pair collides even though the "
            "other tasks are disjoint.",
            [collision("docs/a-summary.md", ["t1", "t2"])],
        ),
        item(
            "independence/n6-undeclared",
            "development",
            [
                {
                    "id": "t1",
                    "prompt": "Update the project's configuration wherever it lives.",
                    "reads": None,
                    "writes": True,
                },
                {
                    "id": "t2",
                    "prompt": "Fix whichever test is failing.",
                    "reads": None,
                    "writes": True,
                },
                reader(3, "src/c.rs"),
                reader(4, "src/d.rs"),
                reader(5, "src/e.rs"),
                reader(6, "src/f.rs"),
            ],
            "no",
            "Two tasks cannot say what they write, so a collision cannot be "
            "ruled out; the gate's safe answer is no.",
        ),
        item(
            "independence/n6-disjoint-write",
            "calibration",
            six_writers,
            "yes",
            "Six tasks, each writing a different file.",
        ),
        item(
            "independence/n12-disjoint-read",
            "calibration",
            twelve_readers,
            "yes",
            "Twelve read-only tasks on twelve different files.",
        ),
        item(
            "independence/n12-one-collision",
            "locked",
            twelve_readers[:10]
            + [
                writer(
                    11,
                    ["src/changes.rs", "CHANGELOG.md"],
                    "Draft the CHANGELOG.md entry for src/changes.rs.",
                    reads="src/changes.rs",
                ),
                writer(
                    12,
                    ["docs/style.md", "CHANGELOG.md"],
                    "Rewrite CHANGELOG.md to the style in docs/style.md.",
                    reads="docs/style.md",
                ),
            ],
            "no",
            "Two of the twelve tasks write the same file, so the list is not "
            "independent.",
            [collision("CHANGELOG.md", ["t11", "t12"])],
        ),
    ]

    suite = {
        "schema": "openagents.gym.suite.v1",
        "name": "independence-eval-v1",
        "created": "2026-09-21",
        "description": (
            "Twelve task lists in the shape the fan-out's decide step sends, at "
            "zero, one, two, six, and twelve tasks, drawn from the colliding, "
            "disjoint, ambiguous, and mixed shapes of the semantics harness. The "
            "panel decides whether openagents.independence.v2, which names no "
            "count and adds the dependence half of the rule, is at least as good "
            "as openagents.independence.v1, whose wording says six tasks "
            "(openagents#9508). Labels are the author's, read off the state by "
            "the rule each item carries."
        ),
        "tier": "smoke",
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
