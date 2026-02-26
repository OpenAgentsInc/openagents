#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASELINE_FILE="${1:-$ROOT_DIR/scripts/perf/compile-baseline.toml}"

if [[ ! -f "$BASELINE_FILE" ]]; then
    printf 'Missing baseline file: %s\n' "$BASELINE_FILE" >&2
    printf 'Run scripts/perf/compile-baseline.sh first.\n' >&2
    exit 1
fi

# shellcheck disable=SC1090
source "$BASELINE_FILE"

factor="${BUDGET_FACTOR_PERCENT:-125}"

run_lane() {
    local lane="$1"
    shift

    local start end elapsed
    start="$(date +%s)"
    printf 'Running compile lane: %s\n' "$lane" >&2
    (cd "$ROOT_DIR" && "$@" >/dev/null)
    end="$(date +%s)"
    elapsed="$((end - start))"
    printf '%s' "$elapsed"
}

check_budget() {
    local lane="$1"
    local baseline="$2"
    local current="$3"
    local budget
    budget="$(((baseline * factor + 99) / 100))"

    if (( current > budget )); then
        printf 'FAIL %s: baseline=%ss budget=%ss current=%ss\n' \
            "$lane" "$baseline" "$budget" "$current" >&2
        return 1
    fi

    printf 'PASS %s: baseline=%ss budget=%ss current=%ss\n' \
        "$lane" "$baseline" "$budget" "$current"
}

status=0

current_wgpui_check_s="$(run_lane wgpui_check cargo check -p wgpui)"
current_desktop_check_s="$(run_lane desktop_check cargo check -p autopilot-desktop)"
current_wgpui_smoke_test_s="$(run_lane wgpui_smoke_test cargo test -p wgpui markdown::renderer::tests -- --nocapture)"

check_budget wgpui_check "${WGPUI_CHECK_SECONDS:-0}" "$current_wgpui_check_s" || status=1
check_budget desktop_check "${DESKTOP_CHECK_SECONDS:-0}" "$current_desktop_check_s" || status=1
check_budget wgpui_smoke_test "${WGPUI_SMOKE_TEST_SECONDS:-0}" "$current_wgpui_smoke_test_s" || status=1

exit "$status"
