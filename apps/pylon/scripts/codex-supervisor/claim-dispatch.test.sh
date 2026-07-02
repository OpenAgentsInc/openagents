#!/usr/bin/env bash
#
# claim-dispatch.test.sh — shell-path regression tests for the codex-supervisor
# store-backed in-flight claim registry.
#
# Run: bash apps/pylon/scripts/codex-supervisor/claim-dispatch.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

# --- stub gh: every pool issue is OPEN with no referencing PR ---------------
STUB_DIR="$WORK/fixtures"
mkdir -p "$STUB_DIR"
cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
sub="${1:-}"; shift || true
case "$sub" in
  pr)
    echo "[]"
    ;;
  issue)
    action="${1:-}"; shift || true
    case "$action" in
      view)
        n="${1:-}"
        f="$STUB_DIR/state.$n"
        if [ -f "$f" ]; then cat "$f"; fi
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

export STUB_DIR
export SUP_GH_BIN="$WORK/gh"
export SUP_REPO="OpenAgentsInc/openagents"
export PYLON_HOME="$WORK/pylon-home"
export SUP_STATE_DIR="$WORK/state"
export SUP_LOCKOUT_CACHE_DIR="$WORK/cache"
export SUP_LOCKOUT_TTL_SECS=90
export SUP_CLAIM_TTL_SECS=3600
export SUP_ORCHESTRATION_STATE_BIN="bun $REPO_ROOT/apps/pylon/src/orchestration/supervisor-state.ts --supervisor claim-dispatch-test --kind codex --pylon-home $PYLON_HOME"
mkdir -p "$PYLON_HOME" "$SUP_STATE_DIR" "$SUP_LOCKOUT_CACHE_DIR"

for n in $(seq 700 720); do printf 'OPEN' > "$STUB_DIR/state.$n"; done
printf 'CLOSED' > "$STUB_DIR/state.799"

# shellcheck source=lockout.sh
source "$SCRIPT_DIR/lockout.sh"

# --- shell command expansion regression ------------------------------------
# SUP_ORCHESTRATION_STATE_BIN is intentionally a multi-word command. Quoting it
# makes the shell look for one executable named "bun .../supervisor-state.ts",
# causing every claim to fail before it reaches the store.
if sup_try_claim_issue 700 shell-regression; then ok "multi-word SUP_ORCHESTRATION_STATE_BIN claim succeeds"; else bad "multi-word SUP_ORCHESTRATION_STATE_BIN claim should succeed"; fi
if sup_claim_is_active 700; then ok "#700 reports active claim through store"; else bad "#700 should report active claim through store"; fi
if sup_try_claim_issue 700 shell-regression-2; then bad "second claim of held #700 must fail"; else ok "second claim of held #700 fails (store unique guard)"; fi
sup_release_claim 700
if sup_claim_is_active 700; then bad "#700 should be free after release"; else ok "#700 free after sup_release_claim"; fi

# --- store TTL and refresh behavior ----------------------------------------
SUP_CLAIM_TTL_SECS=1
if sup_try_claim_issue 701 ttl-owner; then ok "#701 short TTL claim succeeds"; else bad "#701 short TTL claim should succeed"; fi
sleep 2
sup_gc_stale_claims
if sup_claim_is_active 701; then bad "expired #701 should be GC'd by store reconcile"; else ok "expired #701 GC'd by store reconcile"; fi

SUP_CLAIM_TTL_SECS=2
if sup_try_claim_issue 702 refresh-owner; then ok "#702 refresh claim succeeds"; else bad "#702 refresh claim should succeed"; fi
sleep 1
sup_refresh_claim 702
sleep 1
sup_gc_stale_claims
if sup_claim_is_active 702; then ok "sup_refresh_claim renews TTL (survives GC)"; else bad "refreshed #702 should survive GC"; fi
sup_release_claim 702
SUP_CLAIM_TTL_SECS=3600

# --- pick_and_claim_unlocked_issue returns DISTINCT issues ------------------
a=$(pick_and_claim_unlocked_issue 0 710 711 712)
b=$(pick_and_claim_unlocked_issue 0 710 711 712)
c=$(pick_and_claim_unlocked_issue 0 710 711 712)
if [ -n "$a" ] && [ -n "$b" ] && [ -n "$c" ] && [ "$a" != "$b" ] && [ "$a" != "$c" ] && [ "$b" != "$c" ]; then
  ok "sequential picks are distinct ($a,$b,$c)"
else
  bad "sequential picks not distinct (a=$a b=$b c=$c)"
fi
if d=$(pick_and_claim_unlocked_issue 0 710 711 712) && [ -n "$d" ]; then
  bad "exhausted pool should yield nothing (got '$d')"
else
  ok "exhausted claimed pool reports nothing (rc 1)"
fi
sup_release_claim 710; sup_release_claim 711; sup_release_claim 712

# --- pick_and_claim honors CLOSED and queue-governor lockouts ---------------
picked=$(pick_and_claim_unlocked_issue 0 799 713)
if [ "$picked" = "713" ]; then ok "pick_and_claim skips CLOSED #799 -> #713"; else bad "expected 713, got '$picked'"; fi
sup_release_claim 713

sup_labels_for_issue() {
  case "$1" in
    714) printf 'standing-task,prio:4-backstop-burn' ;;
    715) printf 'epic,prio:1-continual-learning' ;;
    *) printf '' ;;
  esac
}
picked=$(pick_and_claim_unlocked_issue 0 714 715 716)
if [ "$picked" = "716" ]; then
  ok "pick_and_claim skips standing-task/epic issues -> #716"
else
  bad "expected standing/epic skip to pick 716, got '$picked'"
fi
sup_release_claim 716
sup_labels_for_issue() { printf ''; }

# --- CONCURRENCY: N slots, same pool -> all DISTINCT, no duplicates ---------
POOL=(710 711 712 713 714 715 716 717)
SLOTS=8
for n in "${POOL[@]}"; do sup_release_claim "$n"; done
pids=()
for i in $(seq 0 $((SLOTS-1))); do
  ( pick_and_claim_unlocked_issue 0 "${POOL[@]}" > "$WORK/res.$i" 2>/dev/null ) &
  pids+=("$!")
done
for p in "${pids[@]}"; do wait "$p"; done

results=()
for i in $(seq 0 $((SLOTS-1))); do
  r="$(cat "$WORK/res.$i" 2>/dev/null)"
  [ -n "$r" ] && results+=("$r")
done
total="${#results[@]}"
distinct="$(printf '%s\n' "${results[@]}" | sort -u | grep -c .)"
if [ "$total" = "$SLOTS" ] && [ "$distinct" = "$SLOTS" ]; then
  ok "concurrent $SLOTS slots claimed $distinct DISTINCT issues (no collisions)"
else
  bad "concurrent claim collision: total=$total distinct=$distinct (want $SLOTS/$SLOTS): ${results[*]}"
fi

for n in "${POOL[@]}"; do sup_release_claim "$n"; done
SMALL=(720 711 712)
MORE_SLOTS=6
pids=()
for i in $(seq 0 $((MORE_SLOTS-1))); do
  ( pick_and_claim_unlocked_issue 0 "${SMALL[@]}" > "$WORK/small.$i" 2>/dev/null ) &
  pids+=("$!")
done
for p in "${pids[@]}"; do wait "$p"; done
sresults=()
for i in $(seq 0 $((MORE_SLOTS-1))); do
  r="$(cat "$WORK/small.$i" 2>/dev/null)"
  [ -n "$r" ] && sresults+=("$r")
done
stotal="${#sresults[@]}"
sdistinct="$(printf '%s\n' "${sresults[@]}" | sort -u | grep -c .)"
if [ "$stotal" = "3" ] && [ "$sdistinct" = "3" ]; then
  ok "6 slots / 3 issues -> exactly 3 distinct winners, 3 empty (no over-claim)"
else
  bad "small-pool over-claim: total=$stotal distinct=$sdistinct (want 3/3): ${sresults[*]}"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
