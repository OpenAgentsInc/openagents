#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE_DIR="$ROOT_DIR/crates/arc/fixtures/policy/public_eval_hygiene"

VALID_FIXTURES=(
  "$FIXTURE_DIR/valid_public_eval_non_regression.json"
)

INVALID_FIXTURES=(
  "$FIXTURE_DIR/invalid_public_eval_optimization.json"
  "$FIXTURE_DIR/invalid_public_eval_training_feed.json"
)

validate_manifest() {
  local manifest="$1"

  jq -e '
    .schema_version == 1 and
    (.artifact_id | type == "string" and length > 0) and
    (.benchmark_family == "arc") and
    (.evaluation_visibility | IN("public_eval", "internal_holdout", "synthetic_regression")) and
    (.artifact_labels | type == "array") and
    (.artifact_labels | all(type == "string")) and
    (.per_task_manual_tuning | type == "boolean") and
    (.feeds_training | type == "boolean") and
    (.synthetic_derivation | IN("none", "from_internal_holdout", "from_synthetic_regression", "from_public_eval"))
  ' "$manifest" >/dev/null

  jq -e '
    if .evaluation_visibility == "public_eval" then
      (.artifact_labels | index("public-eval")) != null and
      (.artifact_labels | index("non-regression")) != null and
      (.artifact_labels | index("non-optimization")) != null and
      (.artifact_labels | index("optimization")) == null and
      .per_task_manual_tuning == false and
      .feeds_training == false and
      .synthetic_derivation == "none"
    else
      .synthetic_derivation != "from_public_eval"
    end
  ' "$manifest" >/dev/null
}

expect_valid() {
  local manifest="$1"
  validate_manifest "$manifest"
}

expect_invalid() {
  local manifest="$1"
  if validate_manifest "$manifest"; then
    echo "expected invalid manifest to fail validation: $manifest" >&2
    return 1
  fi
}

for manifest in "${VALID_FIXTURES[@]}"; do
  expect_valid "$manifest"
done

for manifest in "${INVALID_FIXTURES[@]}"; do
  expect_invalid "$manifest"
done

for manifest in "$@"; do
  expect_valid "$manifest"
done

echo "ARC public-eval hygiene check passed."
