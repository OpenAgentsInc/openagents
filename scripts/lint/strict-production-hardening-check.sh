#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

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

printf 'Strict production hardening check passed.\n'
