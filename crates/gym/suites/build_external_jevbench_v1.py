#!/usr/bin/env python3
"""Builds `external-jevbench-v1.json` and its question set, from JevBench.

Every other suite here was authored and labelled by the people who read its
scores. `external-v1` took its labels from BoolQ and MultiNLI so that one
number could not have been fitted by our own authorship, but its items were
not written for this contract — the question text is ours. JevBench's public
items were written for `noul`/`choice`/`score` by someone else, frozen and
hashed before any system ran, and licensed MIT per item. A door's score on
them is an external measurement of the contract itself.

Where the items come from
-------------------------

`datasets/public/easy.jsonl`, `original.jsonl`, and `hard.jsonl` in
`fstandhartinger/jevbench`, at the commit pinned below. Each file's SHA-256
is checked against the value JevBench's own `datasets/manifest.json`
records, and the checkout's `HEAD` is checked against the pinned commit;
the builder refuses anything else.

231 of JevBench's 534 decisions are public. The other 303 — 24 standard
held-out, 24 easy held-out, 146 judge, and 109 hard — are not in the
upstream repository at all, so this suite can never produce a JevBench
Score. It produces public-item accuracy under our own gates.

How a record becomes an item
----------------------------

`id`, `state`, and `question` carry over verbatim — the question text is
part of what was measured upstream, criteria order included. `expected`
becomes `truth` (for `score`, the index into `labels`), `question.type`
becomes `kind`, and `provenance.label_basis` becomes `label_rule`.
`label_source` is `jevbench`: the labels are JevBench's authors', which for
the hard tier means a frontier model wrote them and a second frontier model
reviewed them. The family becomes `<tier>/<family>`, matching JevBench's own
tier-by-topic reporting.

Every item carries its own option set, so the question text cannot live one
per family the way `support-v2-three-way`'s does. It lives in the
`external-jevbench-v1` question set instead, keyed by item id.

Partitions are drawn per family — calibration 40 %, development 40 %,
locked 20 % — under a fixed seed. JevBench's paraphrase `group` keeps its
items in one partition, because two paraphrases of one scenario are not two
items of evidence. The `group` id is kept on the item, undigested, the way
`external-v1` keeps `source_row`.

Usage
-----

    python3 crates/gym/suites/build_external_jevbench_v1.py --repo ~/work/jevbench

Writes `crates/gym/suites/external-jevbench-v1.json` and
`crates/gym/questions/external-jevbench-v1.json` beside this script's own
checkout.
"""

import argparse
import hashlib
import json
import random
import subprocess
import sys
from collections import OrderedDict
from pathlib import Path

SCHEMA = "openagents.gym.suite.v1"
QUESTION_SCHEMA = "openagents.gym.question_set.v1"
NAME = "external-jevbench-v1"
GATE = "probability-v2"
CREATED = "2026-09-20"
SEED = 9463

#: The upstream revision, and each public file's SHA-256 as recorded in
#: JevBench's own `datasets/manifest.json`. A checkout anywhere else is a
#: different dataset and the builder refuses it.
COMMIT = "484d414e8b58f1828f9f4c07e3ce6fe81ef3a1aa"
FILES = OrderedDict(
    [
        (
            "datasets/public/easy.jsonl",
            {
                "tier": "easy",
                "sha256": "231df3c2c8e88a1a8c137ebe85de96ba70fabd330849098ac7b3c52c70b7172b",
            },
        ),
        (
            "datasets/public/original.jsonl",
            {
                "tier": "standard",
                "sha256": "5c2414edb3006b8bfcb70fda433f0f9ca015759433849f8d3104328a1f7c4180",
            },
        ),
        (
            "datasets/public/hard.jsonl",
            {
                "tier": "hard",
                "sha256": "89e9e6becb33ed88c1de7d42dcc87531b2fb64cfaef4e1986faf7c37b3f80ebb",
            },
        ),
    ]
)

#: The share of each family's items each partition takes, in fill order:
#: the smallest first, so the remainder lands in development rather than
#: leaving the locked partition empty. Locked is spent once through the
#: ledger; calibration fits maps; development is read freely.
SPLIT = (("locked", 0.2), ("calibration", 0.4), ("development", 0.4))


def fail(message):
    sys.exit(f"build_external_jevbench_v1: {message}")


def check_repo(repo):
    head = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
    )
    if head.returncode != 0:
        fail(f"{repo} is not a git checkout: {head.stderr.strip()}")
    if head.stdout.strip() != COMMIT:
        fail(
            f"{repo} is at {head.stdout.strip()}, not the pinned {COMMIT}; "
            "the items and labels are content-pinned, so a different revision is a different suite"
        )


def read_records(repo):
    records = []
    for relative, pinned in FILES.items():
        path = repo / relative
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != pinned["sha256"]:
            fail(
                f"{relative} hashes to {digest}, not the pinned {pinned['sha256']}; "
                "the manifest hash is the evidence the items are what upstream froze"
            )
        for line in path.read_text().splitlines():
            record = json.loads(line)
            records.append((pinned["tier"], record))
    return records


def truth_of(record):
    """The label a knowledgeable answer lands on, as our item's `truth`."""
    labels = record["labels"]
    expected = record["expected"]
    kind = record["question"]["type"]
    if kind == "score":
        if not isinstance(expected, int) or isinstance(expected, bool):
            fail(f"{record['id']}: a score's expected is a level index, got {expected!r}")
        if not 0 <= expected < len(labels):
            fail(f"{record['id']}: expected {expected} is outside {len(labels)} levels")
        return str(labels[expected])
    if expected not in labels:
        fail(f"{record['id']}: expected {expected!r} is not in its own label set")
    return str(expected)


def check_question(record):
    """The contract JevBench's own `validate()` keeps, checked again here."""
    question = record["question"]
    labels = record["labels"]
    criteria = question.get("criteria")
    kind = question["type"]
    if kind == "choice":
        if not isinstance(criteria, dict) or set(criteria) != set(labels):
            fail(f"{record['id']}: a choice's criteria must be its label set")
    elif kind == "noul":
        if set(labels) != {"yes", "no"}:
            fail(f"{record['id']}: a noul's labels are not yes/no: {labels}")
    elif kind == "score":
        if not isinstance(criteria, list) or len(criteria) != len(labels):
            fail(f"{record['id']}: a score's criteria must list every level")
    else:
        fail(f"{record['id']}: unknown question type {kind!r}")
    licence = record.get("provenance", {}).get("license")
    if licence != "MIT":
        fail(f"{record['id']}: provenance.license is {licence!r}, not MIT")


def to_item(tier, record):
    item = {
        "id": record["id"],
        "family": f"{tier}/{record['family']}",
        "kind": record["question"]["type"],
        "state": record["state"],
        "truth": truth_of(record),
        "label_source": "jevbench",
        "label_rule": record["provenance"]["label_basis"],
        # Filled in by `partition()`. Written before it so the digested dict
        # lists every field the reader hashes.
        "partition": None,
    }
    # `group` is where JevBench records that two items are paraphrases of one
    # scenario. Kept undigested, the way `external-v1` keeps `source_row`:
    # it locates the item in the upstream dataset rather than describing it.
    if record.get("group"):
        item["group"] = record["group"]
    return item


def partition(items):
    """Assign partitions per family, keeping a paraphrase group together."""
    families = OrderedDict()
    for item in items:
        families.setdefault(item["family"], []).append(item)
    for family, members in families.items():
        groups = OrderedDict()
        for item in members:
            groups.setdefault(item.get("group") or item["id"], []).append(item)
        order = list(groups.values())
        random.Random(SEED).shuffle(order)
        total = len(members)
        targets = {
            name: max(1, round(total * share)) for name, share in SPLIT[:-1]
        }
        targets["development"] = total - sum(targets.values())
        counts = {name: 0 for name, _ in SPLIT}
        for group in order:
            for name, _ in SPLIT:
                if counts[name] < targets[name]:
                    counts[name] += len(group)
                    for item in group:
                        item["partition"] = name
                    break
        for name, _ in SPLIT:
            if counts[name] == 0:
                fail(f"{family}: the {name} partition came out empty")


def digested(item):
    """The fields `gym::suite::Item` hashes, and no others."""
    keys = (
        "id",
        "family",
        "kind",
        "state",
        "truth",
        "partition",
        "label_source",
        "label_rule",
    )
    return {key: item[key] for key in keys}


def no_floats(value, where):
    """The digest must byte-match the Rust canonicalizer, whose float
    spelling differs from Python's. No floats, no ambiguity to check."""
    if isinstance(value, bool) or value is None or isinstance(value, (str, int)):
        return
    if isinstance(value, float):
        fail(f"{where}: a float would make the digest's spelling ambiguous")
    if isinstance(value, list):
        for entry in value:
            no_floats(entry, where)
    elif isinstance(value, dict):
        for key, entry in value.items():
            no_floats(key, where)
            no_floats(entry, where)


def build(repo):
    check_repo(repo)
    records = read_records(repo)
    items, questions = [], OrderedDict()
    for tier, record in records:
        check_question(record)
        item = to_item(tier, record)
        items.append(item)
        questions[item["id"]] = record["question"]
    partition(items)
    for item in items:
        no_floats(item["state"], item["id"])
    blob = json.dumps(
        [digested(item) for item in items],
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    suite = {
        "schema": SCHEMA,
        "name": NAME,
        "created": CREATED,
        "description": (
            "The 231 public decisions of JevBench v1.2, an independent benchmark "
            "for this contract, from fstandhartinger/jevbench at the commit pinned "
            "in `provenance`. The items, the labels, and the question text are all "
            "JevBench's — nothing here was authored in this repository, which is "
            "the point: like `external-v1`, a door's score on this suite cannot "
            "have been fitted by our own authorship, and unlike `external-v1` the "
            "items were written for noul/choice/score natively rather than adapted "
            "to it. JevBench's 303 held-out decisions are not here and cannot be, "
            "so this suite reports public-item accuracy under our gates, never a "
            "JevBench Score. Families are `<tier>/<family>` after upstream's own "
            "tier-by-topic reporting; partitions are drawn per family at "
            "40/40/20 under a fixed seed, and a paraphrase group stays in one "
            "partition because two spellings of a scenario are one item of "
            "evidence. The question text lives in the question set of the same "
            "name, keyed by item because every item carries its own option set. "
            "Built by build_external_jevbench_v1.py."
        ),
        "tier": "scored",
        "gate": GATE,
        "questions": NAME,
        "provenance": {
            "upstream": "https://github.com/fstandhartinger/jevbench",
            "commit": COMMIT,
            "files": {
                relative: {"tier": pinned["tier"], "sha256": pinned["sha256"]}
                for relative, pinned in FILES.items()
            },
            "licence": (
                "MIT per item (each record's `provenance.license`); the harness is "
                "MIT and the 72 original decisions are upstream's own"
            ),
            "held_out": (
                "303 of JevBench's 534 decisions are not public: 24 standard "
                "held-out, 24 easy held-out, 146 judge whose text upstream does "
                "not redistribute, and 109 hard. Only whole-split hashes and "
                "aggregate results exist for them."
            ),
            "partition_rule": (
                "per family, paraphrase groups whole, "
                f"random.Random({SEED}).shuffle, "
                "40% calibration / 40% development / 20% locked"
            ),
        },
        "digest": hashlib.sha256(blob.encode()).hexdigest(),
        "items": items,
    }
    question_set = {
        "$comment": (
            "The question text of `external-jevbench-v1`, verbatim from JevBench "
            "at the commit the suite's provenance pins. The keys are item ids, "
            "not families: every item carries its own option set, so there is no "
            "per-family wording to share. `QuestionSet::ask` reads an item id "
            "before it reads a family."
        ),
        "schema": QUESTION_SCHEMA,
        "id": NAME,
        "suite": NAME,
        "questions": questions,
    }
    return suite, question_set


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument(
        "--repo", required=True, help="a checkout of fstandhartinger/jevbench"
    )
    root = Path(__file__).resolve().parents[3]
    parser.add_argument(
        "--suite-out",
        default=root / "crates/gym/suites" / f"{NAME}.json",
        type=Path,
        help="where the suite is written",
    )
    parser.add_argument(
        "--questions-out",
        default=root / "crates/gym/questions" / f"{NAME}.json",
        type=Path,
        help="where the question set is written",
    )
    args = parser.parse_args()
    suite, question_set = build(Path(args.repo).expanduser())
    for path, document in (
        (args.suite_out, suite),
        (args.questions_out, question_set),
    ):
        with open(path, "w") as out:
            json.dump(document, out, indent=1, ensure_ascii=False)
            out.write("\n")
        print(f"wrote {path}")
    print(f"digest {suite['digest']}")


if __name__ == "__main__":
    main()
