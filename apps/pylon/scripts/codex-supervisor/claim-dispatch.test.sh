#!/usr/bin/env bash
#
# claim-dispatch.test.sh — tests for the codex-supervisor in-flight claim
# registry that makes concurrent worker slots select DISTINCT issues per cycle.
#
# This guards the duplicate-dispatch under-fill bug: N independent worker slots
# ordered the same pool by priority and each picked the FIRST dispatchable issue,
# so multiple slots collided on the same top issue, the dispatch gate admitted
# one and refused the duplicates (409 / duplicate_active_assignment /
# target_pylon_unavailable), and the fleet under-filled. The claim registry makes
# each concurrent slot atomically reserve a distinct unlocked issue.
#
# Covers:
#   * sup_try_claim_issue is atomic: a second claim of a held issue fails.
#   * sup_claim_is_active reflects held vs released vs stale claims.
#   * sup_release_claim frees a claim immediately.
#   * sup_gc_stale_claims removes only entries past SUP_CLAIM_TTL_SECS.
#   * sup_refresh_claim renews a claim's TTL window.
#   * pick_and_claim_unlocked_issue returns DISTINCT issues on repeated calls,
#     still honors the CLOSED/open-PR lockout, and reports rc 1 when nothing
#     distinct+unlocked remains.
#   * CONCURRENCY: many slots racing the SAME ordered pool resolve to distinct
#     issues with no duplicates (the core regression assertion).
#
# Run: bash apps/pylon/scripts/codex-supervisor/claim-dispatch.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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
    # No referencing PRs for any issue in this suite.
    echo "[]"
    ;;
  issue)
    action="${1:-}"; shift || true
    case "$action" in
      view)
        n="${1:-}"
        f="$STUB_DIR/state.$n"
        if [ -f "$f" ]; then cat "$f"; fi   # absent -> empty == gh error
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
export SUP_STATE_DIR="$WORK/state"
export SUP_LOCKOUT_CACHE_DIR="$WORK/cache"
export SUP_LOCKOUT_TTL_SECS=90
export SUP_CLAIM_TTL_SECS=3600
mkdir -p "$SUP_STATE_DIR" "$SUP_LOCKOUT_CACHE_DIR"

# Mark a generous OPEN issue set (700..720) so all pool members are dispatchable.
for n in $(seq 700 720); do printf 'OPEN' > "$STUB_DIR/state.$n"; done
# A CLOSED issue used by the lockout assertion.
printf 'CLOSED' > "$STUB_DIR/state.799"

# shellcheck source=lockout.sh
source "$SCRIPT_DIR/lockout.sh"

# --- sup_try_claim_issue atomicity -----------------------------------------
if sup_try_claim_issue 700; then ok "first claim of #700 succeeds"; else bad "first claim of #700 should succeed"; fi
if sup_try_claim_issue 700; then bad "second claim of held #700 must fail"; else ok "second claim of held #700 fails (atomic)"; fi
if sup_claim_is_active 700; then ok "#700 reports active claim"; else bad "#700 should report active claim"; fi

# --- sup_release_claim ------------------------------------------------------
sup_release_claim 700
if sup_claim_is_active 700; then bad "#700 should be free after release"; else ok "#700 free after sup_release_claim"; fi
if sup_try_claim_issue 700; then ok "#700 re-claimable after release"; else bad "#700 should be re-claimable after release"; fi
sup_release_claim 700

# --- sup_gc_stale_claims ----------------------------------------------------
sup_try_claim_issue 701 >/dev/null
sup_try_claim_issue 702 >/dev/null
# Backdate #701's claim well past the TTL; #702 stays fresh.
touch -t 200001010000 "$(sup_claims_dir)/claim.701" 2>/dev/null || true
sup_gc_stale_claims
if sup_claim_is_active 701; then bad "stale #701 should be GC'd"; else ok "stale #701 GC'd by sup_gc_stale_claims"; fi
if sup_claim_is_active 702; then ok "fresh #702 survives GC"; else bad "fresh #702 should survive GC"; fi
sup_release_claim 702

# --- sup_gc_orphan_claims removes claims owned by dead supervisor pids -------
sup_try_claim_issue 704 "$$" >/dev/null
sup_try_claim_issue 705 99999999 >/dev/null
sup_gc_orphan_claims
if sup_claim_is_active 704; then ok "startup orphan GC preserves live-owner claim"; else bad "live-owner #704 should survive orphan GC"; fi
if sup_claim_is_active 705; then bad "dead-owner #705 should be removed by orphan GC"; else ok "startup orphan GC removes dead-owner claim"; fi
sup_release_claim 704

# --- sup_refresh_claim renews TTL ------------------------------------------
sup_try_claim_issue 703 >/dev/null
touch -t 200001010000 "$(sup_claims_dir)/claim.703" 2>/dev/null || true
sup_refresh_claim 703
sup_gc_stale_claims
if sup_claim_is_active 703; then ok "sup_refresh_claim renews TTL (survives GC)"; else bad "refreshed #703 should survive GC"; fi
sup_release_claim 703

# --- pick_and_claim_unlocked_issue returns DISTINCT issues ------------------
# Sequential calls against the same pool must hand out different issues.
a=$(pick_and_claim_unlocked_issue 0 710 711 712)
b=$(pick_and_claim_unlocked_issue 0 710 711 712)
c=$(pick_and_claim_unlocked_issue 0 710 711 712)
if [ -n "$a" ] && [ -n "$b" ] && [ -n "$c" ] && [ "$a" != "$b" ] && [ "$a" != "$c" ] && [ "$b" != "$c" ]; then
  ok "sequential picks are distinct ($a,$b,$c)"
else
  bad "sequential picks not distinct (a=$a b=$b c=$c)"
fi
# Pool exhausted (all three claimed) -> rc 1 / empty.
if d=$(pick_and_claim_unlocked_issue 0 710 711 712) && [ -n "$d" ]; then
  bad "exhausted pool should yield nothing (got '$d')"
else
  ok "exhausted claimed pool reports nothing (rc 1)"
fi
sup_release_claim 710; sup_release_claim 711; sup_release_claim 712

# --- pick_and_claim honors the CLOSED lockout ------------------------------
# #799 is CLOSED, #713 is open -> must pick #713, never the closed one.
picked=$(pick_and_claim_unlocked_issue 0 799 713)
if [ "$picked" = "713" ]; then ok "pick_and_claim skips CLOSED #799 -> #713"; else bad "expected 713, got '$picked'"; fi
sup_release_claim 713

# --- pick_and_claim never claims epics / standing tasks ----------------------
sup_labels_for_issue() {
  case "$1" in
    730) printf 'standing-task,prio:4-backstop-burn' ;;
    731) printf 'epic,prio:1-continual-learning' ;;
    *) printf '' ;;
  esac
}
printf 'OPEN' > "$STUB_DIR/state.730"
printf 'OPEN' > "$STUB_DIR/state.731"
printf 'OPEN' > "$STUB_DIR/state.732"
picked=$(pick_and_claim_unlocked_issue 0 730 731 732)
if [ "$picked" = "732" ]; then ok "pick_and_claim skips standing-task/epic labels -> #732"; else bad "expected 732 after standing/epic exclusions, got '$picked'"; fi
if sup_claim_is_active 730 || sup_claim_is_active 731; then
  bad "standing-task/epic issues must not be claimed"
else
  ok "standing-task/epic issues have no claim directories"
fi
sup_release_claim 732

picked=$(pick_and_claim_unlocked_issue 0 730 731)
if [ "$picked" = "730" ]; then ok "standing-task can dispatch as fallback when no concrete issue remains"; else bad "expected fallback standing-task #730, got '$picked'"; fi
if sup_claim_is_active 730; then bad "fallback standing-task dispatch must remain unclaimed"; else ok "fallback standing-task dispatch creates no claim"; fi

# --- CONCURRENCY: N slots, same pool -> all DISTINCT, no duplicates ---------
# This is the core regression assertion for the duplicate-dispatch bug.
POOL=(710 711 712 713 714 715 716 717)   # 8 distinct unlocked issues
SLOTS=8
# Clear any residual claims so every pool member is free.
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

# --- CONCURRENCY: more slots than issues -> exactly pool-size distinct ------
for n in "${POOL[@]}"; do sup_release_claim "$n"; done
SMALL=(720 711 712)   # 3 distinct unlocked issues
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
