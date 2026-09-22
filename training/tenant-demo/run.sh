#!/bin/sh
# Rehearse the full tenant-training contract against a scratch registry:
# corpus check (one accepted, two refused), headroom, recipe freeze, a
# deterministic artifact run, the trial ledger, the candidate seal and
# inspection, and the retention delete — without spending the locked
# partition and without paid compute.
#
#   sh training/tenant-demo/run.sh /tmp/demo-registry target/debug/tenant-train
#
# Every step prints what it did; refused steps are expected and their
# refusals are the point.

set -eu

REGISTRY=${1:-/tmp/tenant-train-demo}
TRAIN=${2:-target/debug/tenant-train}
HERE=$(dirname "$0")

rm -rf "$REGISTRY"
mkdir -p "$REGISTRY"

step() { printf '\n=== %s ===\n' "$1"; }

step "check: the eligible corpus registers"
"$TRAIN" check --registry "$REGISTRY" --corpus "$HERE/corpus-eligible.json"

step "check: the leaked corpus is refused (exact duplicate across partitions, group spill)"
if "$TRAIN" check --registry "$REGISTRY" --corpus "$HERE/corpus-leaked.json"; then
    echo "UNEXPECTED: leaked corpus registered" >&2; exit 1
fi

step "check: the undocumented corpus is refused (no license)"
if "$TRAIN" check --registry "$REGISTRY" --corpus "$HERE/corpus-undocumented.json"; then
    echo "UNEXPECTED: undocumented corpus registered" >&2; exit 1
fi

step "headroom: the baseline's development failures are caused"
"$TRAIN" headroom --registry "$REGISTRY" \
    --corpus atlas-ledger-intents-v1 --scores "$HERE/baseline-scores.json"

step "freeze: the recipe seals"
"$TRAIN" freeze --registry "$REGISTRY" --recipe "$HERE/recipe.json"
RECIPE_DIGEST=$("$TRAIN" list --registry "$REGISTRY" | awk '$2 == "atlas-ledger-lora-v1" {print $3}')

step "train: seed 11 produces deterministic artifacts"
python3 "$HERE/make_artifacts.py" --seed 11 --out "$REGISTRY/artifacts" | tee "$REGISTRY/artifacts.out"
ADAPTER_SIG=$(awk '$1 == "adapter" {print $2}' "$REGISTRY/artifacts.out")
HEAD_SIG=$(awk '$1 == "head" {print $2}' "$REGISTRY/artifacts.out")
TOK_SIG=$(awk '$1 == "tokenizer" {print $2}' "$REGISTRY/artifacts.out")

step "trial: the kept run enters the ledger"
python3 - "$REGISTRY/trial-1.json" <<EOF
import json, sys
trial = {
    "v": "openagents.tenant_training.trial.v1",
    "recipe_digest": "$RECIPE_DIGEST",
    "seed": 11,
    "params": {"rank": 8, "alpha": 16, "epochs": 3, "learning_rate": "2e-4"},
    "metrics": {"accuracy": 0.833, "brier": 0.142},
    "artifacts": {"adapter": "$ADAPTER_SIG", "head": "$HEAD_SIG", "tokenizer": "$TOK_SIG"},
    "outcome": "kept",
    "recorded_at": "2026-09-24T01:00:00Z"
}
open(sys.argv[1], "w").write(json.dumps(trial, indent=2))
EOF
"$TRAIN" trial --registry "$REGISTRY" --record "$REGISTRY/trial-1.json"

step "trial: a seed outside the policy is refused"
python3 - "$REGISTRY/trial-bad.json" <<EOF
import json, sys
trial = {
    "v": "openagents.tenant_training.trial.v1",
    "recipe_digest": "$RECIPE_DIGEST",
    "seed": 99,
    "params": {"rank": 8},
    "metrics": {},
    "artifacts": {},
    "outcome": "failed",
    "reason": "demonstrating the seed policy",
    "recorded_at": "2026-09-24T01:01:00Z"
}
open(sys.argv[1], "w").write(json.dumps(trial, indent=2))
EOF
if "$TRAIN" trial --registry "$REGISTRY" --record "$REGISTRY/trial-bad.json"; then
    echo "UNEXPECTED: foreign-seed trial recorded" >&2; exit 1
fi

step "seal: the candidate binds every identity"
CORPUS_DIGEST=$(python3 -c "import json; print(json.load(open('$REGISTRY/training/corpora/atlas-ledger-intents-v1.json'))['digest'])")
python3 - "$REGISTRY/candidate.json" <<EOF
import json, sys
candidate = {
    "v": "openagents.tenant_training.candidate.v1",
    "workspace": "ws_demo_atlas",
    "name": "atlas-ledger-v1",
    "created": "2026-09-24T01:05:00Z",
    "identities": {
        "code": {"tool": "training/tenant-demo/make_artifacts.py", "version": "demo"},
        "recipe_digest": "$RECIPE_DIGEST",
        "corpus_digest": "$CORPUS_DIGEST",
        "base_model": {"id": "kev-3b-base", "signature": "sha256:0000000000000000000000000000000000000000000000000000000000000001"},
        "adapter": "$ADAPTER_SIG",
        "head": "$HEAD_SIG",
        "tokenizer": "$TOK_SIG"
    },
    "evidence": {
        "trial": 1,
        "metrics": {"accuracy": 0.833, "brier": 0.142},
        "confirmation": "locked partition, unspent"
    },
    "retention": {"days": 90, "access": "owner", "artifacts": "digests-only"}
}
open(sys.argv[1], "w").write(json.dumps(candidate, indent=2))
EOF
"$TRAIN" seal --registry "$REGISTRY" --candidate "$REGISTRY/candidate.json"

step "inspect: the candidate is readable, not served"
"$TRAIN" inspect --registry "$REGISTRY" --candidate atlas-ledger-v1 | head -30

step "delete: retention tombstones the corpus"
"$TRAIN" delete --registry "$REGISTRY" \
    --corpus atlas-ledger-intents-v1 --reason "demo retention window"

step "list: what the store holds"
"$TRAIN" list --registry "$REGISTRY"
