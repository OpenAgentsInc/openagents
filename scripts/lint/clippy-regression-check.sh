#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASELINE_FILE="${1:-$ROOT_DIR/scripts/lint/clippy-baseline.toml}"

if ! "$ROOT_DIR/scripts/lint/ownership-boundary-check.sh"; then
    printf 'Ownership boundary gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/workspace-dependency-drift-check.sh"; then
    printf 'Workspace dependency drift gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/skills/validate_registry.sh"; then
    printf 'Skills registry validation gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/clippy-debt-allowlist-check.sh"; then
    printf 'Clippy debt allowlist validation failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/touched-clippy-gate.sh"; then
    printf 'Touched-file clippy gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/strict-production-hardening-check.sh"; then
    printf 'Strict production hardening gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/clippy-warning-budget-check.sh"; then
    printf 'Clippy warning budget gate failed.\n' >&2
    exit 1
fi

if [[ ! -f "$BASELINE_FILE" ]]; then
    printf 'Missing baseline file: %s\n' "$BASELINE_FILE" >&2
    printf 'Run scripts/lint/clippy-baseline.sh first.\n' >&2
    exit 1
fi

# shellcheck disable=SC1090
source "$BASELINE_FILE"

run_lane() {
    local lane="$1"
    shift

    local tmp
    tmp="$(mktemp)"
    printf 'Running clippy lane: %s\n' "$lane" >&2

    if ! (cd "$ROOT_DIR" && "$@" >"$tmp" 2>&1); then
        cat "$tmp" >&2
        rm -f "$tmp"
        printf 'Lane failed: %s\n' "$lane" >&2
        exit 1
    fi

    local count
    count="$(rg -c '^warning:' "$tmp" 2>/dev/null || true)"
    if [[ -z "$count" ]]; then
        count=0
    fi

    rm -f "$tmp"
    printf '%s' "$count"
}

current_lib="$(run_lane lib cargo clippy --workspace --lib -- -W clippy::all)"
current_tests="$(run_lane tests cargo clippy --workspace --tests -- -W clippy::all -A clippy::unwrap_used -A clippy::expect_used -A clippy::panic)"
current_examples="$(run_lane examples cargo clippy -p wgpui --examples --features desktop -- -W clippy::all)"

status=0

check_lane() {
    local lane="$1"
    local baseline="$2"
    local current="$3"

    if (( current > baseline )); then
        printf 'FAIL %s: baseline=%s current=%s (net-new warnings=%s)\n' \
            "$lane" "$baseline" "$current" "$(( current - baseline ))" >&2
        status=1
    else
        printf 'PASS %s: baseline=%s current=%s\n' "$lane" "$baseline" "$current"
    fi
}

check_lane lib "${LIB_WARNINGS:-0}" "$current_lib"
check_lane tests "${TEST_WARNINGS:-0}" "$current_tests"
check_lane examples "${EXAMPLE_WARNINGS:-0}" "$current_examples"

exit "$status"
