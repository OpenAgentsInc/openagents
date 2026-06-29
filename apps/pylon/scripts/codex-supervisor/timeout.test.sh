#!/usr/bin/env bash
#
# timeout.test.sh — focused tests for sup_run_timeout, the #6646 wedge guard.
#
# The supervisor's #1 token-burn failure mode is an external gh/network call in
# the dispatch loop hanging with no timeout. sup_run_timeout (defined in
# lockout.sh) bounds every such call. This asserts:
#   * a fast command passes through (rc + stdout preserved),
#   * a nonzero exit code is preserved,
#   * an overrunning command is killed and reports the timeout (rc 124),
#   * the timeout fires promptly (it does not wait for the full command).
#
# These exercise the pure-bash watchdog fallback on hosts without coreutils
# `timeout`/`gtimeout` (e.g. stock macOS), which is the real production path.
#
# Run: bash apps/pylon/scripts/codex-supervisor/timeout.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lockout.sh
source "$SCRIPT_DIR/lockout.sh"

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

# --- fast command passes through (rc 0, stdout preserved) ------------------
out=$(sup_run_timeout 5 bash -c 'echo hello; exit 0'); rc=$?
if [ "$rc" -eq 0 ] && [ "$out" = "hello" ]; then
  ok "fast command passes through (rc=0, stdout='hello')"
else
  bad "fast command rc=$rc out='$out' (want rc=0 out='hello')"
fi

# --- nonzero exit code is preserved ---------------------------------------
sup_run_timeout 5 bash -c 'exit 7' >/dev/null 2>&1; rc=$?
if [ "$rc" -eq 7 ]; then
  ok "nonzero exit code preserved (7)"
else
  bad "expected rc 7, got $rc"
fi

# --- overrunning command is killed and reports timeout (rc 124) -----------
start=$(date +%s)
sup_run_timeout 1 bash -c 'sleep 30' >/dev/null 2>&1; rc=$?
end=$(date +%s)
if [ "$rc" -eq 124 ]; then
  ok "overrunning command times out (rc=124)"
else
  bad "expected rc 124 (timeout), got $rc"
fi
if [ $(( end - start )) -lt 10 ]; then
  ok "timeout fired promptly ($(( end - start ))s, not the full 30s)"
else
  bad "timeout took too long ($(( end - start ))s)"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
