#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

lane_remediation_hint() {
    local lane="$1"
    case "$lane" in
        cad-clippy)
            printf 'Remediation (%s): run scripts/lint/cad-clippy.sh --strict and fix lint policy violations.\n' "$lane" >&2
            ;;
        cad-step-checker)
            printf 'Remediation (%s): run scripts/cad/step-checker-ci.sh and inspect checker artifact logs.\n' "$lane" >&2
            ;;
        cad-headless-script-harness)
            printf 'Remediation (%s): run scripts/cad/headless-script-ci.sh and verify deterministic CadIntent script receipts.\n' "$lane" >&2
            ;;
        cad-20s-reliability)
            printf 'Remediation (%s): run scripts/cad/reliability-20s-ci.sh and inspect reliability fixture drift.\n' "$lane" >&2
            ;;
        cad-demo-release-gates)
            printf 'Remediation (%s): run scripts/cad/release-gate-checklist.sh and fix failing gate checks.\n' "$lane" >&2
            ;;
        cad-perf-benchmark-suite)
            printf 'Remediation (%s): run scripts/cad/perf-benchmark-ci.sh and compare budget regressions.\n' "$lane" >&2
            ;;
        *)
            printf 'Remediation (%s): see docs/cad/CAD_CODE_HEALTH.md for lane-specific fix workflow.\n' "$lane" >&2
            ;;
    esac
}

run_strict_lane() {
    local lane="$1"
    shift

    local tmp
    tmp="$(mktemp)"
    printf 'Running strict production lane: %s\n' "$lane" >&2

    if ! (cd "$ROOT_DIR" && "$@" >"$tmp" 2>&1); then
        cat "$tmp" >&2
        rm -f "$tmp"
        printf 'Strict production lane failed: %s\n' "$lane" >&2
        lane_remediation_hint "$lane"
        exit 1
    fi

    rm -f "$tmp"
}

run_strict_lane \
    nostr-lib \
    cargo clippy -p nostr --lib --no-deps -- \
        -D clippy::unwrap_used \
        -D clippy::expect_used \
        -D clippy::panic

run_strict_lane \
    autopilot-desktop-bin \
    cargo clippy -p autopilot-desktop --bin autopilot-desktop --no-deps -- \
        -D clippy::unwrap_used \
        -D clippy::expect_used \
        -D clippy::panic

run_strict_lane \
    cad-clippy \
    "$ROOT_DIR/scripts/lint/cad-clippy.sh" --strict

run_strict_lane \
    cad-step-checker \
    "$ROOT_DIR/scripts/cad/step-checker-ci.sh"

run_strict_lane \
    cad-headless-script-harness \
    "$ROOT_DIR/scripts/cad/headless-script-ci.sh"

run_strict_lane \
    cad-20s-reliability \
    "$ROOT_DIR/scripts/cad/reliability-20s-ci.sh"

run_strict_lane \
    cad-demo-release-gates \
    "$ROOT_DIR/scripts/cad/release-gate-checklist.sh"

run_strict_lane \
    cad-perf-benchmark-suite \
    "$ROOT_DIR/scripts/cad/perf-benchmark-ci.sh"

printf 'Strict production hardening check passed.\n'
