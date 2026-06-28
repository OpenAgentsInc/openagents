#!/usr/bin/env bash
#
# lockout.test.sh — focused tests for the codex-supervisor dispatch lockout.
#
# Stubs the `gh` binary (no network) and asserts that:
#   * an issue with an open referencing PR is LOCKED,
#   * an issue with no referencing PR is UNLOCKED,
#   * a near-miss number (#1010 vs #101) does NOT lock,
#   * a missing `gh` fails OPEN (unlocked) so the fleet never stalls,
#   * pick_unlocked_issue skips locked issues and reports all-locked.
#
# Run: bash apps/pylon/scripts/codex-supervisor/lockout.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()   { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

# --- stub gh ---------------------------------------------------------------
STUB_DIR="$WORK/fixtures"
mkdir -p "$STUB_DIR"
cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
search=""
args=("$@")
i=0
while [ $i -lt ${#args[@]} ]; do
  if [ "${args[$i]}" = "--search" ]; then search="${args[$((i+1))]}"; fi
  i=$((i+1))
done
n=$(printf '%s' "$search" | sed -n 's/^#\([0-9]*\).*/\1/p')
f="$STUB_DIR/$n.json"
if [ -f "$f" ]; then cat "$f"; else echo "[]"; fi
STUB
chmod +x "$WORK/gh"

# fixtures keyed by issue number
printf '%s' '[{"number":1,"title":"feat(pylon): fix #100","body":""}]' > "$STUB_DIR/100.json"
printf '%s' '[{"number":2,"title":"unrelated","body":"Addresses #1010."}]' > "$STUB_DIR/101.json"
printf '%s' '[{"number":3,"title":"chore: #300","body":""}]' > "$STUB_DIR/300.json"
# 200.json intentionally absent -> open

# --- environment for the lib ----------------------------------------------
export SUP_GH_BIN="$WORK/gh"
export STUB_DIR
export SUP_REPO="OpenAgentsInc/openagents"
export SUP_STATE_DIR="$WORK/state"
export SUP_LOCKOUT_CACHE_DIR="$WORK/cache"
export SUP_LOCKOUT_TTL_SECS=90

# shellcheck source=lockout.sh
source "$SCRIPT_DIR/lockout.sh"

# --- issue_has_open_pr -----------------------------------------------------
if issue_has_open_pr 100; then ok "issue #100 is locked (open PR references it)"; else bad "issue #100 should be locked"; fi
if issue_has_open_pr 200; then bad "issue #200 should be unlocked (no PR)"; else ok "issue #200 is unlocked (no PR)"; fi
if issue_has_open_pr 101; then bad "issue #101 should NOT lock on near-miss #1010"; else ok "issue #101 unlocked (near-miss #1010 ignored)"; fi

# fail-open when gh is missing
if SUP_GH_BIN="$WORK/does-not-exist" issue_has_open_pr 100; then
  bad "missing gh should fail OPEN (unlocked)"
else
  ok "missing gh fails open (unlocked)"
fi

# --- pick_unlocked_issue ---------------------------------------------------
picked=$(pick_unlocked_issue 0 100 200)
if [ "$picked" = "200" ]; then ok "pick_unlocked_issue skips locked #100 -> #200"; else bad "pick_unlocked_issue returned '$picked' (want 200)"; fi

if pick_unlocked_issue 0 100 300 >/dev/null; then
  bad "pick_unlocked_issue should fail when all issues locked"
else
  ok "pick_unlocked_issue reports all-locked (rc 1)"
fi

# --- summary ---------------------------------------------------------------
printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
