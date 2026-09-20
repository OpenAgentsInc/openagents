#!/usr/bin/env python3
"""Builds `external-v1.json`, a suite nobody in this repository wrote.

Every other suite here was authored and labelled by the people who read its
scores, and the questions that judge the items co-evolved with the items.
A held-out split does not detect that, because every partition shares the
authorship. This suite takes its items and labels from two public sets that
`docs/kev/model-cards.md` names among kev's training and held-out sources,
so a door's score on it is the one number here that our authorship cannot
have fitted.

Where the items come from
-------------------------

`boolq/…`
    BoolQ (Clark et al., 2019), the `validation` split of the Hugging Face
    dataset `google/boolq`, licensed CC BY-SA 3.0. A passage and a yes/no
    question, answered by crowd annotators. Becomes a `noul` question: the
    passage and the question are the state, and the answer is the label.

`mnli/…`
    MultiNLI (Williams et al., 2018), the `validation_matched` split of
    `nyu-mll/multi_nli`. The dataset card lists its licences as CC BY-3.0,
    CC BY-SA 3.0, MIT, and other, by genre source; the annotations are
    OANC-derived and the card's terms apply. A premise and a hypothesis,
    labelled by crowd annotators as entailment, neutral, or contradiction.
    Becomes a `choice` question over those three.

Both are read from the Parquet files Hugging Face serves for the dataset
revision pinned below. The items are the first `PER_FAMILY` rows of a
seeded shuffle of each split, and each item records the row index it came
from, so the sample can be redrawn and checked against the source.

Neither set was written for this contract, so the question text is ours.
That is the one thing here we authored, and it is inline on every item so
the digest covers it. The labels are not ours, and `label_source` says so.

Usage
-----

    python3 crates/gym/suites/build_external_v1.py \\
        --cache ~/work9381/external > crates/gym/suites/external-v1.json

The Parquet files are fetched into `--cache` on first use. Reading them
needs `pyarrow`.
"""

import argparse
import hashlib
import json
import os
import random
import sys
import urllib.request

SCHEMA = "openagents.gym.suite.v1"
GATE = "probability-v2"
SEED = 9381

#: Items per family, and how they split. Development is what a door is
#: scored on; calibration fits a map; locked is spent once through the
#: ledger.
PER_FAMILY = 100
SPLIT = (("calibration", 40), ("development", 40), ("locked", 20))

SOURCES = {
    "boolq": {
        "dataset": "google/boolq",
        "revision": "35b264d03638db9f4ce671b711558bf7ff0f80d5",
        "split": "validation",
        "url": "https://huggingface.co/api/datasets/google/boolq/parquet/default/validation/0.parquet",
        "licence": "CC BY-SA 3.0",
        "citation": "Clark et al., BoolQ: Exploring the Surprising Difficulty of Natural Yes/No Questions, NAACL 2019",
    },
    "mnli": {
        "dataset": "nyu-mll/multi_nli",
        "revision": "da70db2af9d09693783c3320c4249840212ee221",
        "split": "validation_matched",
        "url": "https://huggingface.co/api/datasets/nyu-mll/multi_nli/parquet/default/validation_matched/0.parquet",
        "licence": "CC BY-3.0, CC BY-SA 3.0, MIT, and other, by genre; see the dataset card",
        "citation": "Williams, Nangia, and Bowman, A Broad-Coverage Challenge Corpus for Sentence Understanding through Inference, NAACL 2018",
    },
}

BOOLQ_QUESTION = {
    "type": "noul",
    "instructions": "Read the passage. Is the answer to the question yes?",
    "criteria": {
        "true": "The passage supports answering the question with yes",
        "false": "The passage supports answering the question with no",
    },
}

MNLI_LABELS = ["entailment", "neutral", "contradiction"]
MNLI_QUESTION = {
    "type": "choice",
    "instructions": "How does the hypothesis relate to the premise?",
    "criteria": {
        "entailment": "The premise makes the hypothesis true",
        "neutral": "The premise leaves the hypothesis undetermined",
        "contradiction": "The premise makes the hypothesis false",
    },
}

BOOLQ_RULE = (
    "crowd: BoolQ's annotators read the passage and answered the question yes or "
    "no; the label is their answer, unchanged"
)
MNLI_RULE = (
    "crowd: MultiNLI's annotators labelled the pair entailment, neutral, or "
    "contradiction; the label is the gold label, unchanged"
)


def fetch(cache, family):
    source = SOURCES[family]
    path = os.path.join(cache, f"{family}-{source['split']}.parquet")
    if not os.path.exists(path):
        os.makedirs(cache, exist_ok=True)
        urllib.request.urlretrieve(source["url"], path)
    import pyarrow.parquet as pq

    return pq.read_table(path).to_pylist()


def partitions():
    order = []
    for name, count in SPLIT:
        order.extend([name] * count)
    return order


def sample(rows):
    indices = list(range(len(rows)))
    random.Random(SEED).shuffle(indices)
    return indices[:PER_FAMILY]


def boolq_items(rows):
    items = []
    for position, (index, partition) in enumerate(zip(sample(rows), partitions())):
        row = rows[index]
        items.append(
            {
                "id": f"boolq/{position:03d}",
                "family": "boolq",
                "kind": "noul",
                "state": {"passage": row["passage"], "question": row["question"]},
                "question": BOOLQ_QUESTION,
                "truth": "yes" if row["answer"] else "no",
                "partition": partition,
                "label_source": "crowd",
                "label_rule": BOOLQ_RULE,
                "source_row": index,
            }
        )
    return items


def mnli_items(rows):
    items = []
    for position, (index, partition) in enumerate(zip(sample(rows), partitions())):
        row = rows[index]
        items.append(
            {
                "id": f"mnli/{position:03d}",
                "family": "mnli",
                "kind": "choice",
                "state": {"premise": row["premise"], "hypothesis": row["hypothesis"]},
                "question": MNLI_QUESTION,
                "truth": MNLI_LABELS[row["label"]],
                "partition": partition,
                "label_source": "crowd",
                "label_rule": MNLI_RULE,
                "source_row": index,
            }
        )
    return items


def build(cache):
    items = boolq_items(fetch(cache, "boolq")) + mnli_items(fetch(cache, "mnli"))
    # `source_row` is not a field the reader keeps, so it is not in the
    # digest; hash what the reader hashes.
    digested = [{k: v for k, v in item.items() if k != "source_row"} for item in items]
    blob = json.dumps(digested, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return {
        "schema": SCHEMA,
        "name": "external-v1",
        "created": "2026-09-20",
        "description": (
            "Two hundred items nobody in this repository wrote or labelled: 100 BoolQ "
            "validation items as a noul family and 100 MultiNLI validation_matched items "
            "as a choice family, drawn with a seeded shuffle from the Hugging Face Parquet "
            "files at the revisions named in `provenance`. The question text is ours and "
            "is the only authored part; the labels are the sets' own crowd labels, "
            "unchanged. A door's score here is the one number in this repository our "
            "own authorship cannot have fitted. Built by build_external_v1.py."
        ),
        "tier": "scored",
        "gate": GATE,
        "provenance": {
            family: {
                "dataset": source["dataset"],
                "revision": source["revision"],
                "split": source["split"],
                "licence": source["licence"],
                "citation": source["citation"],
                "sampled": f"first {PER_FAMILY} indices of random.Random({SEED}).shuffle over the split; each item's `source_row` is its index",
            }
            for family, source in SOURCES.items()
        },
        "digest": hashlib.sha256(blob.encode()).hexdigest(),
        "items": items,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--cache", required=True, help="where the Parquet files are kept")
    args = parser.parse_args()
    json.dump(build(os.path.expanduser(args.cache)), sys.stdout, indent=1, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
