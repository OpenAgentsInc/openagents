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

if ! "$ROOT_DIR/scripts/lint/allow-attribute-expiry-check.sh"; then
    printf 'Allow-attribute expiry guard failed.\n' >&2
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

if ! "$ROOT_DIR/scripts/lint/codex-protocol-parity-gate.sh"; then
    printf 'Codex protocol parity gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/autopilot-earnings-epic-test-gate.sh"; then
    printf 'Autopilot earnings epic test matrix gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/clippy-warning-budget-check.sh"; then
    printf 'Clippy warning budget gate failed.\n' >&2
    exit 1
fi

if ! "$ROOT_DIR/scripts/lint/module-size-budget-check.sh"; then
    printf 'Module-size budget gate failed.\n' >&2
    exit 1
fi

if [[ ! -f "$BASELINE_FILE" ]]; then
    printf 'Missing baseline file: %s\n' "$BASELINE_FILE" >&2
    printf 'Run scripts/lint/clippy-baseline.sh first.\n' >&2
    exit 1
fi

LOG_ROOT="${CLIPPY_REGRESSION_LOG_DIR:-${TMPDIR:-/tmp}/openagents-clippy-regression}"
mkdir -p "$LOG_ROOT"

# shellcheck disable=SC1090
source "$BASELINE_FILE"

run_lane() {
    local lane="$1"
    shift

    local lane_log="$LOG_ROOT/${lane}.log"
    printf 'Running clippy lane: %s\n' "$lane" >&2

    if ! (cd "$ROOT_DIR" && "$@" >"$lane_log" 2>&1); then
        cat "$lane_log" >&2
        printf 'Lane log: %s\n' "$lane_log" >&2
        printf 'Lane failed: %s\n' "$lane" >&2
        exit 1
    fi

    local count
    count="$(rg -c '^warning:' "$lane_log" 2>/dev/null || true)"
    if [[ -z "$count" ]]; then
        count=0
    fi

    printf '%s|%s' "$count" "$lane_log"
}

lib_result="$(run_lane lib cargo clippy --workspace --lib -- -W clippy::all)"
tests_result="$(run_lane tests cargo clippy --workspace --tests -- -W clippy::all -A clippy::unwrap_used -A clippy::expect_used -A clippy::panic)"
examples_result="$(run_lane examples cargo clippy -p wgpui --examples --features desktop -- -W clippy::all)"

current_lib="${lib_result%%|*}"
log_lib="${lib_result#*|}"
current_tests="${tests_result%%|*}"
log_tests="${tests_result#*|}"
current_examples="${examples_result%%|*}"
log_examples="${examples_result#*|}"

status=0

check_lane() {
    local lane="$1"
    local baseline="$2"
    local current="$3"
    local lane_log="$4"

    if (( current > baseline )); then
        printf 'FAIL %s: baseline=%s current=%s (net-new warnings=%s) log=%s\n' \
            "$lane" "$baseline" "$current" "$(( current - baseline ))" "$lane_log" >&2
        status=1
    else
        printf 'PASS %s: baseline=%s current=%s log=%s\n' \
            "$lane" "$baseline" "$current" "$lane_log"
    fi
}

check_lane lib "${LIB_WARNINGS:-0}" "$current_lib" "$log_lib"
check_lane tests "${TEST_WARNINGS:-0}" "$current_tests" "$log_tests"
check_lane examples "${EXAMPLE_WARNINGS:-0}" "$current_examples" "$log_examples"

if (( status != 0 )); then
    printf 'Clippy regression logs preserved at: %s\n' "$LOG_ROOT" >&2
    exit "$status"
fi

if [[ "${CLIPPY_REGRESSION_CLEAN_SUCCESS:-0}" == "1" ]]; then
    rm -f "$log_lib" "$log_tests" "$log_examples"
    rmdir "$LOG_ROOT" 2>/dev/null || true
else
    printf 'Clippy regression logs written to: %s\n' "$LOG_ROOT"
fi

exit 0
