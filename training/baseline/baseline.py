#!/usr/bin/env python3
"""Frozen sentence embeddings plus multinomial logistic regression, measured.

This is a measurement, not a door. It answers one question: on the suite this
repository scores its decision models against, does the cheapest supervised
method beat them? If it does not, the door proposed in issue #9377 does not
need building, and that is the result.

What it does, per Choice question family:

1. Embeds each item's state once with a frozen sentence encoder. The encoder
   is never fine-tuned, which is the whole point of the baseline.
2. Picks the L2 strength by stratified cross-validation *inside the fitting
   partition*, so the scoring partition is never read during fitting.
3. Refits on the whole fitting partition and predicts a distribution over the
   family's option set for each item in the scoring partition.
4. Scores the result on the same panel as every other door, through
   `panel.py`, a port of `crates/lev/src/calibrate.rs`.

What it refuses, and records as a refusal rather than a failure:

- **Noul**, which asks for the probability that a statement holds. A
  classifier fitted on yes and no labels returns the frequency of a class in
  a training set, and calling that the probability of the statement satisfies
  the type while changing the meaning.
- **Score**, which asks for a weighted position on an *ordered* rubric.
  Multinomial logistic regression treats its classes as unordered labels.
  Nothing in it keeps level 1 between level 0 and level 2, so a weighted mean
  over its output is arithmetic on a scale the model does not have.

Both refusals are typed, counted separately from harness failures, and
reported next to the numbers rather than under them.

    python3 baseline.py \
        --suite ../../crates/gym/suites/support-v2-three-way.json \
        --encoder sentence-transformers/all-MiniLM-L6-v2 \
        --revision 1110a243fdf4706b3f48f1d95db1a4f5529b4d41 \
        --out runs/three-way-minilm.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import sys
from pathlib import Path

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import StratifiedKFold
from sklearn.metrics import log_loss

from panel import Observation, accuracy_interval, binomial_standard_error, score

# The L2 strengths the fit chooses between. Selection happens inside the
# fitting partition; the scoring partition never votes. The grid runs four
# decades past where the choice settles, because a value selected at the edge
# of a grid was chosen by the grid rather than by the data.
C_GRID = [0.01, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 10_000.0]

# Typed refusals. A door that satisfies the type and violates the meaning is
# worse than no door.
REFUSALS = {
    "noul": {
        "code": "unsupported_primitive",
        "reason": (
            "A Noul is the probability that a statement holds. A classifier fitted on "
            "yes and no labels returns the frequency of a label in its training set, "
            "which is a different quantity that happens to have the same type."
        ),
    },
    "score": {
        "code": "unsupported_primitive",
        "reason": (
            "A Score is a weighted position on an ordered rubric. Multinomial logistic "
            "regression treats its classes as unordered labels, so nothing keeps level 1 "
            "between level 0 and level 2 and a weighted mean reports a position on a "
            "scale the model does not have."
        ),
    },
}


def load_suite(path: Path) -> tuple[dict, str, str, str]:
    """Reads a suite and works out which partition fits and which one scores.

    `support-v2-three-way.json` carries `partition`, with a `development`
    split that chooses between fitted things and a `locked` split that is
    read once through the Gym's ledger. This harness never reads `locked`.
    `support-v2.json` carries `split`, with `calibration` and `evaluation`.
    """
    suite = json.loads(path.read_text())
    items = suite["items"]
    if "partition" in items[0]:
        key = "partition"
        scoring = "development"
    elif "split" in items[0]:
        key = "split"
        scoring = "evaluation"
    else:
        raise SystemExit(f"{path} has neither a partition nor a split field")
    return suite, key, "calibration", scoring


def option_set(item: dict) -> list[str]:
    criteria = item["question"]["criteria"]
    if isinstance(criteria, dict):
        return list(criteria.keys())
    return [str(index) for index in range(len(criteria))]


class FrozenEncoder:
    """A sentence encoder used exactly as shipped, with no fitting at all."""

    def __init__(self, name: str, revision: str):
        from sentence_transformers import SentenceTransformer

        self.name = name
        self.revision = revision
        self.model = SentenceTransformer(name, revision=revision, device="cpu")
        self.model.eval()

    def features(self, fit_states: list[str], score_states: list[str]) -> tuple[np.ndarray, np.ndarray]:
        return self._encode(fit_states), self._encode(score_states)

    def _encode(self, states: list[str]) -> np.ndarray:
        return np.asarray(
            self.model.encode(states, normalize_embeddings=True, batch_size=32, show_progress_bar=False),
            dtype=np.float64,
        )

    def describe(self) -> dict:
        dimensions = (
            self.model.get_embedding_dimension()
            if hasattr(self.model, "get_embedding_dimension")
            else self.model.get_sentence_embedding_dimension()
        )
        return {
            "name": self.name,
            "revision": self.revision,
            "dimensions": dimensions,
            "parameters": sum(p.numel() for p in self.model.parameters()),
            "frozen": True,
            "input": "the item state only; the question text is constant within a family",
            "normalized": True,
        }


class WordCounts:
    """The floor below the floor: character and word n-grams, no model at all.

    A frozen encoder is cheap, but it is still a hundred million parameters
    someone trained. If a TF-IDF vectorizer fitted on the same forty items
    lands in the same place, the encoder is not earning its download either,
    and that is worth knowing before anything is built on top of one.
    """

    name = "tfidf"
    revision = "scikit-learn TfidfVectorizer, word 1-2 grams"

    def features(self, fit_states: list[str], score_states: list[str]) -> tuple[np.ndarray, np.ndarray]:
        from sklearn.feature_extraction.text import TfidfVectorizer

        vectorizer = TfidfVectorizer(ngram_range=(1, 2), sublinear_tf=True, min_df=1)
        # Fitted on the fitting partition only. A vocabulary that has seen the
        # scored items is a scored item leak wearing a preprocessing step's name.
        fit = vectorizer.fit_transform(fit_states).toarray().astype(np.float64)
        score_matrix = vectorizer.transform(score_states).toarray().astype(np.float64)
        self._dimensions = fit.shape[1]
        return fit, score_matrix

    def describe(self) -> dict:
        return {
            "name": self.name,
            "revision": self.revision,
            "dimensions": getattr(self, "_dimensions", None),
            "parameters": 0,
            "frozen": True,
            "input": "the item state only",
            "normalized": True,
        }


def pick_strength(x: np.ndarray, y: np.ndarray, classes: list[str], seed: int) -> dict:
    """Cross-validates the L2 strength inside the fitting set, under two rules.

    Selecting on the scoring partition is the failure this repository has
    already caught once, in the builder comment for the three-way suite. The
    folds are stratified so every fold carries every option.

    Both rules are reported because on this many items they disagree, and the
    disagreement is not small: `argmin` takes the lowest cross-validated loss,
    and `one-se` takes the strongest regularization whose loss is within one
    standard error of it, which is the usual answer to a flat loss surface.
    Neither is wrong. Picking one after seeing which flattered the result
    would be.
    """
    smallest = min(int((y == label).sum()) for label in np.unique(y))
    folds = max(2, min(5, smallest))
    splitter = StratifiedKFold(n_splits=folds, shuffle=True, random_state=seed)
    # `predict_proba` returns columns in `classes_` order, which scikit-learn
    # sorts, so the label list handed to `log_loss` has to be sorted too.
    ordered = sorted(classes)
    table, spread = {}, {}
    for strength in C_GRID:
        losses = []
        for fit_index, held_index in splitter.split(x, y):
            model = LogisticRegression(C=strength, max_iter=5000, random_state=0)
            model.fit(x[fit_index], y[fit_index])
            losses.append(log_loss(y[held_index], model.predict_proba(x[held_index]), labels=ordered))
        table[strength] = float(np.mean(losses))
        spread[strength] = float(np.std(losses, ddof=1) / np.sqrt(len(losses))) if len(losses) > 1 else 0.0

    argmin = min(table, key=table.get)
    threshold = table[argmin] + spread[argmin]
    one_se = min(c for c in C_GRID if table[c] <= threshold)
    return {
        "folds": folds,
        "log_loss_by_strength": table,
        "standard_error_by_strength": spread,
        "strength_by_rule": {"argmin": argmin, "one-se": one_se},
        "at_grid_edge": {
            "argmin": argmin in (C_GRID[0], C_GRID[-1]),
            "one-se": one_se in (C_GRID[0], C_GRID[-1]),
        },
    }


def serve_family(
    encoder,
    family: str,
    fit_items: list[dict],
    score_items: list[dict],
    seed: int,
) -> dict:
    """Fits on the fitting items and predicts a distribution for each scored item."""
    classes = option_set(fit_items[0])
    for item in fit_items + score_items:
        if option_set(item) != classes:
            return {
                "family": family,
                "served": False,
                "code": "option_set_drift",
                "reason": (
                    f"{item['id']} carries a different option set from the one the family was "
                    "fitted on. A fitted head is pinned to the labels it saw; it cannot answer a "
                    "caller-supplied option set."
                ),
                "items": len(score_items),
            }

    x_fit, x_score = encoder.features(
        [item["state"] for item in fit_items], [item["state"] for item in score_items]
    )
    y_fit = np.asarray([item["truth"] for item in fit_items])
    y_score = np.asarray([item["truth"] for item in score_items])

    selection = pick_strength(x_fit, y_fit, classes, seed)

    by_rule = {}
    for rule, strength in selection["strength_by_rule"].items():
        model = LogisticRegression(C=strength, max_iter=5000, random_state=0)
        model.fit(x_fit, y_fit)
        probabilities = model.predict_proba(x_score)
        order = list(model.classes_)
        answers, observations = [], []
        for item, row, truth in zip(score_items, probabilities, y_score):
            distribution = {label: float(p) for label, p in zip(order, row)}
            winner = max(distribution, key=distribution.get)
            correct = winner == truth
            answers.append(
                {
                    "id": item["id"],
                    "choice": winner,
                    "truth": str(truth),
                    "correct": correct,
                    "confidence": distribution[winner],
                    "probabilities": distribution,
                }
            )
            observations.append(Observation(raw=distribution[winner], correct=correct))
        panel = score(observations)
        low, high = accuracy_interval(observations)
        by_rule[rule] = {
            "strength": strength,
            "panel": panel.as_dict(),
            "accuracy_standard_error": binomial_standard_error(panel.accuracy, panel.items),
            "accuracy_interval": [low, high],
            "answers": answers,
        }

    return {
        "family": family,
        "served": True,
        "kind": "choice",
        "options": classes,
        "fitted_on": len(fit_items),
        "scored_on": len(score_items),
        # What always answering the fitting set's most common label would score.
        # A head that does not clear this has learned the label frequencies.
        "majority_class_floor": float(
            (y_score == max(set(y_fit.tolist()), key=y_fit.tolist().count)).mean()
        ),
        "selection": selection,
        "by_rule": by_rule,
    }


def stability(encoder, family_items: dict, seed_count: int) -> dict:
    """How much the answer moves when only the fold seed changes.

    The fit itself is deterministic. The one free draw is which items land in
    which cross-validation fold, which is what picks the L2 strength. This
    reports the spread that draw is worth, so a comparison can be read against
    it the way the seed-variance record asks.
    """
    if seed_count < 1:
        return {"measured": False, "reason": "no seeds requested"}
    moved = {}
    for rule in ("argmin", "one-se"):
        accuracies, strengths = [], []
        for seed in range(seed_count):
            served = serve_family(encoder, *family_items, seed=seed)
            if not served["served"]:
                return {"measured": False}
            accuracies.append(served["by_rule"][rule]["panel"]["accuracy"])
            strengths.append(served["by_rule"][rule]["strength"])
        moved[rule] = {
            "accuracies": accuracies,
            "strengths": strengths,
            "mean": float(np.mean(accuracies)),
            "standard_deviation": float(np.std(accuracies, ddof=0)),
            "range": [float(min(accuracies)), float(max(accuracies))],
        }
    return {"measured": True, "seeds": seed_count, "by_rule": moved}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", required=True, type=Path)
    parser.add_argument("--encoder", required=True, help="a sentence-transformers name, or `tfidf`")
    parser.add_argument("--revision", default="", help="the encoder commit this run is pinned to")
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--seed", type=int, default=0, help="the fold seed the headline run uses")
    parser.add_argument("--stability-seeds", type=int, default=8)
    args = parser.parse_args()

    suite, key, fit_partition, score_partition = load_suite(args.suite)
    items = suite["items"]
    families = sorted({item["family"] for item in items})

    if args.encoder == "tfidf":
        encoder = WordCounts()
    else:
        if not args.revision:
            raise SystemExit("--revision is required: an unpinned encoder is an unreproducible run")
        encoder = FrozenEncoder(args.encoder, args.revision)

    served, refused = [], []
    for family in families:
        family_items = [item for item in items if item["family"] == family]
        kind = family_items[0]["kind"]
        fit_items = [item for item in family_items if item[key] == fit_partition]
        score_items = [item for item in family_items if item[key] == score_partition]
        if kind != "choice":
            refusal = dict(REFUSALS[kind])
            refusal.update(
                {
                    "family": family,
                    "kind": kind,
                    "served": False,
                    "items_refused": len(score_items),
                    "items_in_family": len(family_items),
                }
            )
            refused.append(refusal)
            continue
        served.append(serve_family(encoder, family, fit_items, score_items, args.seed))

    pooled = {}
    for rule in ("argmin", "one-se"):
        observations = [
            Observation(raw=answer["confidence"], correct=answer["correct"])
            for family in served
            if family["served"]
            for answer in family["by_rule"][rule]["answers"]
        ]
        panel = score(observations)
        low, high = accuracy_interval(observations)
        pooled[rule] = {
            "panel": panel.as_dict(),
            "accuracy_standard_error": binomial_standard_error(panel.accuracy, panel.items),
            "accuracy_interval": [low, high],
        }

    spread = {}
    for family in served:
        if not family["served"]:
            continue
        name = family["family"]
        family_items = [item for item in items if item["family"] == name]
        spread[name] = stability(
            encoder,
            (
                name,
                [item for item in family_items if item[key] == fit_partition],
                [item for item in family_items if item[key] == score_partition],
            ),
            args.stability_seeds,
        )

    record = {
        "schema": "openagents.baseline.measurement.v1",
        "suite": {
            "path": str(args.suite),
            "name": suite.get("name", args.suite.stem),
            "digest": suite.get("digest"),
            "items": len(items),
            "partition_field": key,
            "fitted_on": fit_partition,
            "scored_on": score_partition,
        },
        "encoder": encoder.describe(),
        "head": {
            "model": "multinomial logistic regression, scikit-learn LogisticRegression",
            "penalty": "l2",
            "strength_grid": C_GRID,
            "strength_selected_on": f"{fit_partition}, stratified cross-validation",
            "strength_rules": ["argmin", "one-se"],
            "fold_seed": args.seed,
        },
        "machine": {"platform": platform.platform(), "python": sys.version.split()[0]},
        "served": served,
        "refused": refused,
        "pooled": pooled,
        "fold_seed_spread": spread,
    }
    body = json.dumps(record, indent=2, sort_keys=False)
    record["digest"] = hashlib.sha256(body.encode()).hexdigest()

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(record, indent=2))

    described = record["encoder"]
    print(f"suite {record['suite']['name']}, fitted on {fit_partition}, scored on {score_partition}")
    print(
        f"encoder {described['name']}@{described['revision'][:7] or 'none'}, "
        f"{described['parameters']:,} parameters, {described['dimensions']} dimensions\n"
    )
    print("| Family | Rule | Strength | Accuracy | ECE | Brier | NLL | Confident errors | Fitted on | Scored on |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for family in served:
        if not family["served"]:
            continue
        for rule, result in family["by_rule"].items():
            p = result["panel"]
            print(
                f"| `{family['family']}` | {rule} | {result['strength']:g} | {p['accuracy']:.3f} | "
                f"{p['ece']:.3f} | {p['brier']:.3f} | {p['nll']:.3f} | {p['confident_errors']} | "
                f"{family['fitted_on']} | {p['items']} |"
            )
    for rule, result in pooled.items():
        p = result["panel"]
        low, high = result["accuracy_interval"]
        print(
            f"| **all Choice** | {rule} | | {p['accuracy']:.3f} | {p['ece']:.3f} | {p['brier']:.3f} | "
            f"{p['nll']:.3f} | {p['confident_errors']} | | {p['items']} |"
        )
    print()
    for rule, result in pooled.items():
        p = result["panel"]
        low, high = result["accuracy_interval"]
        print(
            f"{rule}: accuracy {p['accuracy']:.3f}, standard error "
            f"{result['accuracy_standard_error']:.3f}, bootstrap 95% [{low:.3f}, {high:.3f}]"
        )
    for name, moved in spread.items():
        if not moved.get("measured"):
            continue
        for rule, by in moved["by_rule"].items():
            print(
                f"fold seed spread, `{name}`, {rule}: mean {by['mean']:.3f}, "
                f"sd {by['standard_deviation']:.4f}, range {by['range'][0]:.3f} to {by['range'][1]:.3f}"
            )
    print()
    for refusal in refused:
        print(f"refused `{refusal['family']}` ({refusal['kind']}), {refusal['items_refused']} items: {refusal['code']}")
        print(f"  {refusal['reason']}")
    if args.out:
        print(f"\nwrote {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
