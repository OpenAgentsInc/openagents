#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASELINE_FILE="${1:-$ROOT_DIR/scripts/perf/microbench-baseline.toml}"

if [[ ! -f "$BASELINE_FILE" ]]; then
    printf 'Missing baseline file: %s\n' "$BASELINE_FILE" >&2
    printf 'Run scripts/perf/microbench-baseline.sh first.\n' >&2
    exit 1
fi

# shellcheck disable=SC1090
source "$BASELINE_FILE"

estimate_file() {
    local rel="$1"
    printf '%s/target/criterion/%s/new/estimates.json' "$ROOT_DIR" "$rel"
}

read_point_estimate() {
    local file="$1"
    local value

    if [[ ! -f "$file" ]]; then
        printf 'Missing estimate file: %s\n' "$file" >&2
        return 1
    fi

    if command -v jq >/dev/null 2>&1; then
        value="$(jq -r '.mean.point_estimate // empty' "$file")"
    else
        value="$(sed -E 's/.*"mean":\\{"confidence_interval":\\{[^}]*\\},"point_estimate":([0-9.]+).*/\1/' "$file")"
    fi
    if [[ -z "$value" ]]; then
        printf 'Could not parse point_estimate from: %s\n' "$file" >&2
        return 1
    fi

    awk "BEGIN { printf \"%.0f\", $value }"
}

check_budget() {
    local lane="$1"
    local baseline="$2"
    local current="$3"
    local factor="$4"
    local budget
    budget="$(((baseline * factor + 99) / 100))"

    if (( current > budget )); then
        printf 'FAIL %s: baseline=%sns budget=%sns current=%sns\n' \
            "$lane" "$baseline" "$budget" "$current" >&2
        return 1
    fi

    printf 'PASS %s: baseline=%sns budget=%sns current=%sns\n' \
        "$lane" "$baseline" "$budget" "$current"
}

factor="${MICRO_BUDGET_FACTOR_PERCENT:-130}"
status=0

printf 'Running microbench suite: text_scene_microbench\n' >&2
(
    cd "$ROOT_DIR" && \
        cargo bench -p wgpui --bench text_scene_microbench -- --noplot >/dev/null
)

current_scene_build_1k_ns="$(read_point_estimate "$(estimate_file 'micro_scene/build_quads/1000')")"
current_text_measure_1kb_ns="$(read_point_estimate "$(estimate_file 'micro_text/measure_styled_mono_1kb')")"
current_text_layout_1kb_ns="$(read_point_estimate "$(estimate_file 'micro_text/layout_styled_mono_1kb')")"

check_budget scene_build_1000_quads "${SCENE_BUILD_1000_QUADS_NS:-0}" "$current_scene_build_1k_ns" "$factor" || status=1
check_budget text_measure_1kb "${TEXT_MEASURE_1KB_NS:-0}" "$current_text_measure_1kb_ns" "$factor" || status=1
check_budget text_layout_1kb "${TEXT_LAYOUT_1KB_NS:-0}" "$current_text_layout_1kb_ns" "$factor" || status=1

exit "$status"
