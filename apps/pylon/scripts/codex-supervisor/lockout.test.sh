#!/usr/bin/env bash
#
# lockout.test.sh — focused tests for the codex-supervisor dispatch lockout.
#
# Stubs the `gh` binary (no network) and asserts that:
#   * an issue with an open referencing PR is LOCKED,
#   * an issue with no referencing PR is UNLOCKED,
#   * a near-miss number (#1010 vs #101) does NOT lock,
#   * a missing `gh` fails OPEN (unlocked) for the PR check so the fleet never
#     stalls on a transient GitHub problem,
#   * a CLOSED issue is NOT open (so it is skipped / never re-dispatched),
#   * an OPEN issue is open,
#   * issue_is_open FAILS CLOSED when state is undeterminable (gh missing/error),
#   * pick_unlocked_issue skips CLOSED issues AND open-PR issues, and reports
#     all-locked,
#   * sup_open_issue_numbers returns the live OPEN set and excludes closed ones.
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
# Dispatches on the gh subcommand:
#   pr list ... --search "#N ..."   -> $STUB_DIR/pr.N.json     (default "[]")
#   issue view N --json state -q .. -> contents of $STUB_DIR/state.N
#                                      (absent -> empty == gh error)
#   issue list --state open ...     -> $STUB_DIR/openlist.json (default "[]")
STUB_DIR="$WORK/fixtures"
mkdir -p "$STUB_DIR"
cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
sub="${1:-}"; shift || true
case "$sub" in
  pr)
    search=""
    args=("$@")
    i=0
    while [ $i -lt ${#args[@]} ]; do
      if [ "${args[$i]}" = "--search" ]; then search="${args[$((i+1))]}"; fi
      i=$((i+1))
    done
    n=$(printf '%s' "$search" | sed -n 's/^#\([0-9]*\).*/\1/p')
    f="$STUB_DIR/pr.$n.json"
    if [ -f "$f" ]; then cat "$f"; else echo "[]"; fi
    ;;
  issue)
    action="${1:-}"; shift || true
    case "$action" in
      view)
        n="${1:-}"
        f="$STUB_DIR/state.$n"
        if [ -f "$f" ]; then cat "$f"; fi   # absent -> empty (simulated gh error)
        ;;
      list)
        f="$STUB_DIR/openlist.json"
        if [ -f "$f" ]; then cat "$f"; else echo "[]"; fi
        ;;
    esac
    ;;
esac
STUB
chmod +x "$WORK/gh"

# --- PR fixtures keyed by issue number ------------------------------------
printf '%s' '[{"number":1,"title":"feat(pylon): fix #100","body":""}]' > "$STUB_DIR/pr.100.json"
printf '%s' '[{"number":2,"title":"unrelated","body":"Addresses #1010."}]' > "$STUB_DIR/pr.101.json"
printf '%s' '[{"number":3,"title":"chore: #300","body":""}]' > "$STUB_DIR/pr.300.json"
# pr.200.json / pr.6423.json intentionally absent -> no PR

# --- issue-state fixtures --------------------------------------------------
printf 'OPEN'   > "$STUB_DIR/state.100"
printf 'OPEN'   > "$STUB_DIR/state.200"
printf 'OPEN'   > "$STUB_DIR/state.300"
printf 'OPEN'   > "$STUB_DIR/state.101"
printf 'CLOSED' > "$STUB_DIR/state.6423"   # the bug case: resolved/closed issue
# state.999 intentionally absent -> empty == gh error (must fail CLOSED)

# --- live OPEN issue set (note: 6423 is CLOSED and absent) -----------------
printf '%s' '[{"number":100},{"number":200},{"number":300},{"number":101}]' > "$STUB_DIR/openlist.json"

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

# fail-open when gh is missing (PR check)
if SUP_GH_BIN="$WORK/does-not-exist" issue_has_open_pr 100; then
  bad "missing gh should fail OPEN (unlocked) for PR check"
else
  ok "missing gh fails open (unlocked) for PR check"
fi

# --- issue_is_open ---------------------------------------------------------
if issue_is_open 200; then ok "issue #200 is OPEN"; else bad "issue #200 should be OPEN"; fi
if issue_is_open 6423; then bad "issue #6423 is CLOSED and must NOT be open"; else ok "issue #6423 (CLOSED) is correctly not-open"; fi
if issue_is_open 999; then bad "undeterminable state must fail CLOSED (not open)"; else ok "undeterminable issue #999 fails CLOSED (not open)"; fi

# fail-closed when gh is missing (state check)
if SUP_GH_BIN="$WORK/does-not-exist" issue_is_open 200; then
  bad "missing gh must fail CLOSED for issue_is_open"
else
  ok "missing gh fails closed (not open) for issue_is_open"
fi

# --- pick_unlocked_issue ---------------------------------------------------
# Skips the CLOSED #6423 and lands on the open, PR-free #200.
picked=$(pick_unlocked_issue 0 6423 200)
if [ "$picked" = "200" ]; then ok "pick_unlocked_issue skips CLOSED #6423 -> #200"; else bad "pick_unlocked_issue returned '$picked' (want 200, skipping closed 6423)"; fi

# Skips the open-PR #100 and lands on the open, PR-free #200.
picked=$(pick_unlocked_issue 0 100 200)
if [ "$picked" = "200" ]; then ok "pick_unlocked_issue skips PR-locked #100 -> #200"; else bad "pick_unlocked_issue returned '$picked' (want 200)"; fi

# Everything locked: #100 (PR), #300 (PR), #6423 (closed) -> rc 1.
if pick_unlocked_issue 0 100 300 6423 >/dev/null; then
  bad "pick_unlocked_issue should fail when all issues are locked (PR'd/closed)"
else
  ok "pick_unlocked_issue reports all-locked (rc 1)"
fi

# --- sup_open_issue_numbers ------------------------------------------------
open_set="$(sup_open_issue_numbers | sort -n | tr '\n' ' ')"
if [ "$open_set" = "100 101 200 300 " ]; then
  ok "sup_open_issue_numbers returns the live OPEN set (excludes closed #6423)"
else
  bad "sup_open_issue_numbers returned '$open_set' (want '100 101 200 300 ')"
fi

# --- summary ---------------------------------------------------------------
printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
