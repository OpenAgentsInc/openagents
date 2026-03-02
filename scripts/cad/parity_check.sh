#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

LANE_LABELS=(
    "baseline-manifests"
    "fixture-corpus-pipeline"
    "kernel-adapter-v2"
    "ci-artifact-manifest"
    "risk-register-workflow"
    "baseline-dashboard"
    "parity-fixture-tests"
    "rustfmt-check"
)

usage() {
    cat <<USAGE
Usage:
  scripts/cad/parity_check.sh
  scripts/cad/parity_check.sh --list
  scripts/cad/parity_check.sh --skip-tests

Options:
  --list        Print orchestration lane IDs and exit
  --skip-tests  Run artifact/check scripts but skip cargo parity test lane
USAGE
}

LIST_ONLY=0
SKIP_TESTS=0
for arg in "$@"; do
    case "$arg" in
        --list)
            LIST_ONLY=1
            ;;
        --skip-tests)
            SKIP_TESTS=1
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n\n' "$arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if (( LIST_ONLY == 1 )); then
    for lane in "${LANE_LABELS[@]}"; do
        printf '%s\n' "$lane"
    done
    exit 0
fi

run_lane() {
    local lane="$1"
    shift

    local tmp
    tmp="$(mktemp)"
    printf 'Parity lane: %s\n' "$lane" >&2
    if ! (cd "$ROOT_DIR" && "$@" >"$tmp" 2>&1); then
        cat "$tmp" >&2
        rm -f "$tmp"
        printf 'Parity lane failed: %s\n' "$lane" >&2
        exit 1
    fi
    rm -f "$tmp"
    printf 'Parity lane pass: %s\n' "$lane" >&2
}

run_lane "baseline-manifests" \
    "$ROOT_DIR/scripts/cad/freeze-parity-baseline.sh" --check

run_lane "fixture-corpus-pipeline" \
    "$ROOT_DIR/scripts/cad/parity-fixture-corpus-ci.sh"

run_lane "kernel-adapter-v2" \
    "$ROOT_DIR/scripts/cad/parity-kernel-adapter-v2-ci.sh"

run_lane "ci-artifact-manifest" \
    "$ROOT_DIR/scripts/cad/parity-ci-artifacts-ci.sh"

run_lane "risk-register-workflow" \
    "$ROOT_DIR/scripts/cad/parity-risk-register-ci.sh"

run_lane "baseline-dashboard" \
    "$ROOT_DIR/scripts/cad/parity-dashboard-ci.sh"

if (( SKIP_TESTS == 0 )); then
    run_lane "parity-fixture-tests" \
        cargo test -p openagents-cad parity_ --quiet
fi

run_lane "rustfmt-check" \
    cargo fmt --all -- --check

printf 'CAD parity orchestration checks passed.\n'
