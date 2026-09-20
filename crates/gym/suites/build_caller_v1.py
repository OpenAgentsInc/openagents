#!/usr/bin/env python3
"""Builds a caller's labelled data into a gym suite and its question set.

The decision API's smallest unit of work is not a call — it is a record a
caller can verify. This builder takes the caller's own labelled data and
turns it into the same pinned, three-partition suite the repository's own
suites are, so `gym eval` against it produces rows no one can quietly
rewrite. The caller's labels stay the caller's: `label_source` names them,
`label_rule` records how the labels were produced, and the suite's
`provenance` carries the caller's own licence statement.

Input
-----

A JSONL file, one decision per line:

    {"family": "routing", "kind": "choice", "state": "...", "truth": "billing",
     "question": {"type": "choice", "instructions": "...", "criteria": {...}}}

Required per record: `family`, `kind`, `state`, `truth`, and `question` —
the question exactly as the door should read it, `type` matching `kind`.
Optional: `id` (else `<family>/<NNN>` in input order), `group` for
paraphrases of one scenario that must share a partition, and `label_rule`
to override `--label-rule` per record. Unknown fields are refused: a pinned
suite is no place for a typo.

Truths are checked against the item's own option set: a `choice` truth is a
key of its criteria, a `noul` truth normalizes to `yes`/`no` (the JSON
`true`/`false` and the strings `"true"`/`"false"` are accepted), and a
`score` truth is an index into its ordered criteria list, integer or string.
A float anywhere in `state` fails the build — the digest must byte-match the
Rust canonicalizer, whose float spelling differs from Python's.

Question keying follows `QuestionSet::ask`, which reads an item id before a
family: a family whose items share one identical question object is keyed
once by family name, and a family whose items carry different option sets is
keyed per item id. Both keyings may coexist in one set.

Partitions are drawn per family — 40 % calibration, 40 % development, 20 %
locked — under a fixed seed, with a `group` held whole in one partition
because two spellings of a scenario are one item of evidence. A family too
small to fill all three partitions fails the build rather than leaving the
locked slice empty.

Usage
-----

    python3 crates/gym/suites/build_caller_v1.py \
        --input acme.jsonl --name caller-acme-v1 --label-source acme \
        --label-rule "labelled by the caller's support leads" \
        --source "Acme support exports, September 2026" \
        --licence "Acme retains the labels; OpenAgents may score doors on them"

Writes `crates/gym/suites/<name>.json` and `crates/gym/questions/<name>.json`
beside this script's own checkout. Rebuilding from the same input produces
byte-identical files; the suite's digest is the evidence a later suite is or
is not the same measurement.
"""

import argparse
import datetime
import hashlib
import json
import random
import sys
from collections import OrderedDict
from pathlib import Path

SCHEMA = "openagents.gym.suite.v1"
QUESTION_SCHEMA = "openagents.gym.question_set.v1"
GATE = "probability-v2"
SEED = 9464

#: The fields a caller's record may carry. Anything else is refused rather
#: than silently digested, because a pinned suite is evidence.
FIELDS = {"id", "family", "kind", "state", "truth", "question", "group", "label_rule"}

#: The share of each family's items each partition takes, in fill order:
#: the smallest first, so the remainder lands in development rather than
#: leaving the locked partition empty. Locked is spent once through the
#: ledger; calibration fits maps; development is read freely.
SPLIT = (("locked", 0.2), ("calibration", 0.4), ("development", 0.4))


def fail(message):
    sys.exit(f"build_caller_v1: {message}")


def truth_of(record):
    """The label a knowledgeable answer lands on, as our item's `truth`."""
    kind = record["kind"]
    truth = record["truth"]
    criteria = record["question"]["criteria"]
    where = record.get("id") or record["family"]
    if kind == "noul":
        normalized = {True: "yes", False: "no", "true": "yes", "false": "no"}.get(
            truth, truth
        )
        if normalized not in ("yes", "no"):
            fail(f"{where}: a noul's truth is yes or no, got {truth!r}")
        return normalized
    if kind == "choice":
        if not isinstance(truth, str) or truth not in criteria:
            fail(f"{where}: truth {truth!r} is not in its own option set")
        return truth
    if kind == "score":
        if isinstance(truth, bool):
            fail(f"{where}: a score's truth is a level index, got {truth!r}")
        try:
            index = int(truth)
        except (TypeError, ValueError):
            fail(f"{where}: a score's truth is a level index, got {truth!r}")
        if str(truth) != str(index):
            fail(f"{where}: a score's truth is a level index, got {truth!r}")
        if not 0 <= index < len(criteria):
            fail(f"{where}: truth {truth!r} is outside {len(criteria)} levels")
        return str(index)
    fail(f"{where}: unknown kind {kind!r}")


def check_record(record, line):
    """The contract a caller's record must keep, checked before it is digested."""
    where = record.get("id") or f"line {line}"
    unknown = set(record) - FIELDS
    if unknown:
        fail(f"{where}: unknown fields {sorted(unknown)}; fix the export rather than guess")
    for field in ("family", "kind", "state", "truth", "question"):
        if field not in record:
            fail(f"{where}: missing {field}")
    if not isinstance(record["family"], str) or not record["family"]:
        fail(f"{where}: family must be a nonempty string")
    question = record["question"]
    if not isinstance(question, dict):
        fail(f"{where}: question must be an object")
    if question.get("type") != record["kind"]:
        fail(
            f"{where}: kind {record['kind']!r} does not match "
            f"question.type {question.get('type')!r}"
        )
    if not isinstance(question.get("instructions"), str):
        fail(f"{where}: question.instructions must be a string")
    criteria = question.get("criteria")
    if record["kind"] == "score":
        if not isinstance(criteria, list) or len(criteria) < 2:
            fail(f"{where}: a score's criteria is an ordered list of at least two levels")
    elif not isinstance(criteria, dict) or not criteria:
        fail(f"{where}: criteria must be a nonempty object")


def read_records(path):
    records = []
    for line, text in enumerate(path.read_text().splitlines(), start=1):
        if not text.strip():
            continue
        record = json.loads(text)
        check_record(record, line)
        records.append(record)
    if not records:
        fail(f"{path} holds no records")
    return records


def to_item(record, label_source, label_rule):
    item = {
        "id": record.get("id"),
        "family": record["family"],
        "kind": record["kind"],
        "state": record["state"],
        "truth": truth_of(record),
        "label_source": label_source,
        "label_rule": record.get("label_rule") or label_rule,
        # The question stays on the item until the set is keyed, then leaves
        # it: the suite carries items, the set carries text.
        "question": record["question"],
        # Filled in by `partition()`. Written before it so the digested dict
        # lists every field the reader hashes.
        "partition": None,
    }
    # `group` records that two items are paraphrases of one scenario. Kept
    # undigested, the way `external-v1` keeps `source_row`: it groups the
    # item's evidence rather than describing the item.
    if record.get("group"):
        item["group"] = record["group"]
    return item


def assign_ids(items):
    """Caller ids win; the rest are `<family>/<NNN>` in input order."""
    families = {item["family"] for item in items}
    counts = {}
    for item in items:
        if item["id"] is None:
            count = counts.get(item["family"], 0) + 1
            counts[item["family"]] = count
            item["id"] = f"{item['family']}/{count:03d}"
        elif item["id"] in families:
            fail(
                f"{item['id']}: an item id must not be a family name — "
                "a question keyed by that name would answer it"
            )
    ids = [item["id"] for item in items]
    if len(ids) != len(set(ids)):
        fail("duplicate item ids")


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
        targets = {name: max(1, round(total * share)) for name, share in SPLIT[:-1]}
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
                fail(
                    f"{family}: the {name} partition came out empty; "
                    "a family needs enough distinct items — or groups — to fill all three"
                )


def key_questions(items):
    """Key each family's question by family name when the items share it,
    else per item id — the order `QuestionSet::ask` reads.

    Returns the keyed questions and the family's keying per family name.
    """
    questions = OrderedDict()
    keying = OrderedDict()
    families = OrderedDict()
    for item in items:
        families.setdefault(item["family"], []).append(item)
    for family, members in families.items():
        first = members[0]["question"]
        if all(item["question"] == first for item in members):
            questions[family] = first
            keying[family] = "family"
        else:
            for item in members:
                questions[item["id"]] = item["question"]
            keying[family] = "item"
    return questions, keying


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
        fail(
            f"{where}: a float would make the digest's spelling ambiguous; "
            "write it as a string"
        )
    if isinstance(value, list):
        for entry in value:
            no_floats(entry, where)
    elif isinstance(value, dict):
        for key, entry in value.items():
            no_floats(key, where)
            no_floats(entry, where)


def build(args):
    records = read_records(args.input)
    items = [to_item(record, args.label_source, args.label_rule) for record in records]
    assign_ids(items)
    partition(items)
    questions, keying = key_questions(items)
    for item in items:
        no_floats(item["state"], item["id"])
        del item["question"]
    blob = json.dumps(
        [digested(item) for item in items],
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    digest = hashlib.sha256(blob.encode()).hexdigest()
    keyed = {
        "family": sum(1 for key in keying.values() if key == "family"),
        "item": sum(1 for key in keying.values() if key == "item"),
    }
    agreement = {}
    for pair in args.agreement or []:
        if "=" not in pair:
            raise SystemExit(f"--agreement takes FAMILY=CEILING, got {pair!r}")
        family, ceiling = pair.split("=", 1)
        if family not in keying:
            raise SystemExit(f"--agreement names {family!r}, which is not a family")
        agreement[family] = ceiling
    suite = {
        "schema": SCHEMA,
        "name": args.name,
        "created": args.created,
        "description": args.description
        or (
            f"A caller suite: {len(items)} decisions across "
            f"{len(keying)} families, labelled by {args.label_source} under "
            f"the rule each item's `label_rule` records, from {args.source}. "
            "Partitions are drawn per family at 40/40/20 under a fixed seed, "
            "and a paraphrase group stays in one partition because two "
            "spellings of a scenario are one item of evidence. The question "
            "text lives in the question set of the same name — keyed by "
            "family where a family shares one wording, by item id where "
            "items carry their own option sets. Built by build_caller_v1.py."
        ),
        "tier": "scored",
        "gate": args.gate,
        "questions": args.name,
        "provenance": {
            "caller": args.label_source,
            "source": args.source,
            "licence": args.licence,
            "input": {
                "file": args.input.name,
                "sha256": hashlib.sha256(args.input.read_bytes()).hexdigest(),
            },
            "partition_rule": (
                "per family, paraphrase groups whole, "
                f"random.Random({SEED}).shuffle, "
                "40% calibration / 40% development / 20% locked"
            ),
            # The agreement ceiling a family's labels rest on, when the
            # caller states one. `gym report` prints it beside the family's
            # scores so a number is never read without the bar above it.
            "agreement": agreement,
        },
        "digest": digest,
        "items": items,
    }
    question_set = {
        "$comment": (
            f"The question text of `{args.name}`, supplied by the caller "
            "with the items. Keys are family names where a family shares one "
            "question, item ids where items carry their own option sets; "
            "`QuestionSet::ask` reads an item id before it reads a family."
        ),
        "schema": QUESTION_SCHEMA,
        "id": args.name,
        "suite": args.name,
        "questions": questions,
    }
    return suite, question_set, keyed


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--input", required=True, type=Path, help="the caller's JSONL")
    parser.add_argument("--name", required=True, help="the suite's name, e.g. caller-acme-v1")
    parser.add_argument(
        "--label-source", required=True, help="who the labels are, e.g. acme"
    )
    parser.add_argument(
        "--label-rule",
        required=True,
        help="how the labels were produced, in one sentence",
    )
    parser.add_argument("--source", required=True, help="where the data came from")
    parser.add_argument(
        "--licence",
        required=True,
        help="the caller's licence statement for measurement use",
    )
    parser.add_argument(
        "--created",
        default=datetime.date.today().isoformat(),
        help="the suite's creation date",
    )
    parser.add_argument(
        "--description",
        default=None,
        help="a description to use instead of the generated one",
    )
    parser.add_argument("--gate", default=GATE, help="the gate id the suite names")
    parser.add_argument(
        "--agreement",
        action="append",
        default=None,
        metavar="FAMILY=CEILING",
        help=(
            "the agreement ceiling a family's labels rest on, e.g. "
            "routing=0.91; repeatable, and the family must exist"
        ),
    )
    root = Path(__file__).resolve().parents[3]
    parser.add_argument(
        "--suite-out", default=None, type=Path, help="where the suite is written"
    )
    parser.add_argument(
        "--questions-out", default=None, type=Path, help="where the question set is written"
    )
    args = parser.parse_args()
    args.suite_out = args.suite_out or root / "crates/gym/suites" / f"{args.name}.json"
    args.questions_out = (
        args.questions_out or root / "crates/gym/questions" / f"{args.name}.json"
    )
    suite, question_set, keyed = build(args)
    for path, document in (
        (args.suite_out, suite),
        (args.questions_out, question_set),
    ):
        with open(path, "w") as out:
            json.dump(document, out, indent=1, ensure_ascii=False)
            out.write("\n")
        print(f"wrote {path}")
    print(f"digest {suite['digest']}")
    print(
        f"{len(suite['items'])} items, {keyed['family']} shared and "
        f"{keyed['item']} per-item question families"
    )


if __name__ == "__main__":
    main()
