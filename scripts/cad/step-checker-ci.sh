#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACT_DIR="${CAD_STEP_CHECKER_ARTIFACT_DIR:-$ROOT_DIR/artifacts/cad-step-checker}"
BACKEND="${CAD_STEP_CHECKER_BACKEND:-structural}"

mkdir -p "$ARTIFACT_DIR"
LOG_FILE="$ARTIFACT_DIR/step-checker.log"

printf 'Running CAD STEP checker fixtures (backend=%s)\n' "$BACKEND"

if ! (
    cd "$ROOT_DIR" && \
    CAD_STEP_CHECKER_BACKEND="$BACKEND" \
    CAD_STEP_CHECKER_ARTIFACT_DIR="$ARTIFACT_DIR" \
    cargo test -p openagents-cad step_checker_exports_baseline_and_variant_fixtures -- --nocapture >"$LOG_FILE" 2>&1
); then
    cat "$LOG_FILE" >&2
    printf 'CAD STEP checker failed. Artifacts directory: %s\n' "$ARTIFACT_DIR" >&2
    exit 1
fi

printf 'CAD STEP checker passed. Reports written to: %s\n' "$ARTIFACT_DIR"
