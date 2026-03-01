#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# CAD clippy policy:
# - Production lane: strict, deny warnings and panic/unwrap/expect usage.
# - Test lane: allow panic/expect/unwrap while we migrate high-noise inline tests.
#   This is intentionally temporary and tracked by CAD health issues.
#
# Modes:
# - strict (default): fail if any lane fails.
# - advisory: never fail; prints failures and remediation hints.

mode="strict"
if [[ "${1:-}" == "--advisory" ]]; then
    mode="advisory"
elif [[ "${1:-}" == "--strict" || -z "${1:-}" ]]; then
    mode="strict"
else
    printf 'Unknown flag: %s\n' "${1:-}" >&2
    printf 'Usage: %s [--strict|--advisory]\n' "$0" >&2
    exit 2
fi

run_lane() {
    local lane="$1"
    shift
    local tmp
    tmp="$(mktemp)"
    printf 'Running CAD clippy lane: %s\n' "$lane" >&2

    if ! (cd "$ROOT_DIR" && "$@" >"$tmp" 2>&1); then
        printf 'Lane failed: %s\n' "$lane" >&2
        cat "$tmp" >&2
        rm -f "$tmp"
        return 1
    fi

    rm -f "$tmp"
    return 0
}

status=0

if ! run_lane \
    cad-lib-and-bin-strict \
    cargo clippy -p openagents-cad --lib --bins -- \
        -D warnings \
        -D clippy::unwrap_used \
        -D clippy::expect_used \
        -D clippy::panic; then
    status=1
fi

if ! run_lane \
    cad-tests-transitional \
    cargo clippy -p openagents-cad --tests -- \
        -W clippy::all \
        -A clippy::unwrap_used \
        -A clippy::expect_used \
        -A clippy::panic; then
    status=1
fi

if (( status == 0 )); then
    printf 'CAD clippy check passed.\n'
    exit 0
fi

if [[ "$mode" == "advisory" ]]; then
    printf 'CAD clippy advisory mode: failures reported but not failing.\n' >&2
    printf 'Remediation: fix lint findings or run with --strict after cleanup issues land.\n' >&2
    exit 0
fi

printf 'CAD clippy strict mode failed.\n' >&2
exit 1
