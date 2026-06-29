#!/usr/bin/env bash
#
# fleet-offload.test.sh — no-network tests for the multi-host Codex profile
# offload planner.
#
# Run: bash apps/pylon/scripts/codex-supervisor/fleet-offload.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()   { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

PYLON_HOME="$WORK/pylon"
mkdir -p \
  "$PYLON_HOME/accounts/codex/codex-4" \
  "$PYLON_HOME/accounts/codex/codex-5" \
  "$PYLON_HOME/accounts/codex/codex-6" \
  "$PYLON_HOME/accounts/codex/codex-7"

OUT="$WORK/out.txt"
PYLON_HOME="$PYLON_HOME" \
REMOTE_PYLON_HOME="/Users/operator/.pylon-fable" \
REMOTE_REPO="/Users/operator/work/openagents" \
SUP_PYLON_REF="pylon.live" \
SUP_MAX_SLOTS_PER_HOST=4 \
SUP_PER_ACCOUNT=2 \
DATE_BIN=/bin/date \
  bash "$SCRIPT_DIR/fleet-offload.sh" \
    --hosts imac-pro-bertha,macbook-pro-m2 \
    --accounts codex-4,codex-5,codex-6,codex-7 \
    > "$OUT"
rc=$?

if [ "$rc" -eq 0 ]; then ok "dry-run exits zero"; else bad "dry-run rc=$rc"; fi

if grep -q 'host=imac-pro-bertha accounts=codex-4,codex-6' "$OUT"; then
  ok "bertha receives alternating account refs"
else
  bad "bertha assignment missing"
fi

if grep -q 'host=macbook-pro-m2 accounts=codex-5,codex-7' "$OUT"; then
  ok "m2 receives alternating account refs"
else
  bad "m2 assignment missing"
fi

if grep -E -q "'tar' '-C' '.*/accounts/codex' '-czf' '/tmp/openagents-codex-4-" "$OUT"; then
  ok "archives isolated codex profile directory"
else
  bad "codex-4 tar command missing"
fi

if grep -q "SUP_ACCOUNT_REFS=.*codex-4,codex-6" "$OUT" &&
   grep -q "SUP_ACCOUNT_REFS=.*codex-5,codex-7" "$OUT"; then
  ok "remote launches constrain supervisor account refs"
else
  bad "SUP_ACCOUNT_REFS launch constraints missing"
fi

if grep -q 'codex login\|pylon auth codex' "$OUT"; then
  bad "offload dry-run must not include login commands"
else
  ok "offload plan never invokes login"
fi

BAD_OUT="$WORK/bad.txt"
PYLON_HOME="$PYLON_HOME" bash "$SCRIPT_DIR/fleet-offload.sh" \
  --hosts imac-pro-bertha \
  --accounts codex-4,../../unsafe \
  > "$BAD_OUT" 2>&1
bad_rc=$?
if [ "$bad_rc" -ne 0 ] && grep -q 'unsafe account ref' "$BAD_OUT"; then
  ok "unsafe account refs are rejected"
else
  bad "unsafe account ref was not rejected"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
