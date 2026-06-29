#!/usr/bin/env bash
#
# pr-review-refill.test.sh — focused tests for PR-review refill locking.
#
# Run: bash apps/pylon/scripts/codex-supervisor/pr-review-refill.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

export SUP_PR_REVIEW_STATE_DIR="$WORK/state"
export SUP_PR_REVIEW_LOG_DIR="$WORK/logs"
export SUP_PR_REVIEW_ACTIVE_ASSIGNMENT_DIR="$WORK/active"
export SUP_PR_REVIEW_LOCK_TTL_SECS=2
export SUP_PR_REVIEW_DONE_TTL_SECS=3600
mkdir -p "$SUP_PR_REVIEW_STATE_DIR" "$SUP_PR_REVIEW_LOG_DIR" "$SUP_PR_REVIEW_ACTIVE_ASSIGNMENT_DIR"

# shellcheck source=pr-review-refill.sh
source "$SCRIPT_DIR/pr-review-refill.sh"

# --- atomic claim / release -------------------------------------------------
if sup_pr_review_try_claim 7401 test-a; then ok "first claim of PR #7401 succeeds"; else bad "first claim of PR #7401 should succeed"; fi
if sup_pr_review_try_claim 7401 test-b; then bad "second claim of PR #7401 should fail"; else ok "second claim of PR #7401 fails while lock held"; fi
if sup_pr_review_is_reserved 7401; then ok "held lock reports reserved"; else bad "held lock should report reserved"; fi
sup_pr_review_release 7401
if sup_pr_review_try_claim 7401 test-c; then ok "released PR #7401 can be claimed again"; else bad "released PR #7401 should be claimable"; fi
sup_pr_review_release 7401

# --- failed-before-accept releases immediately -----------------------------
sup_pr_review_try_claim 7402 test >/dev/null || bad "claim #7402 setup failed"
sup_pr_review_release 7402
if sup_pr_review_try_claim 7402 retry; then ok "pre-accept failure release allows immediate retry"; else bad "pre-accept release should retry immediately"; fi
sup_pr_review_release 7402

# --- accepted active assignment survives stale lock GC ----------------------
sup_pr_review_try_claim 7403 test >/dev/null || bad "claim #7403 setup failed"
sup_pr_review_mark_accepted 7403 assignment.public.khala_coding.active7403
printf '{"assignmentRef":"assignment.public.khala_coding.active7403"}\n' > "$SUP_PR_REVIEW_ACTIVE_ASSIGNMENT_DIR/active7403.json"
touch -t 200001010000 "$(sup_pr_review_lock_path 7403)" 2>/dev/null || true
sup_pr_review_gc_locks
if sup_pr_review_is_reserved 7403; then ok "active accepted assignment keeps PR #7403 reserved past TTL"; else bad "active accepted assignment lock was GC'd"; fi
rm -f "$SUP_PR_REVIEW_ACTIVE_ASSIGNMENT_DIR/active7403.json"

# --- accepted inactive stale lock becomes done marker -----------------------
touch -t 200001010000 "$(sup_pr_review_lock_path 7403)" 2>/dev/null || true
sup_pr_review_gc_locks
if [ ! -e "$(sup_pr_review_lock_path 7403)" ] && sup_pr_review_done_is_fresh 7403; then
  ok "inactive accepted PR #7403 becomes a done marker"
else
  bad "inactive accepted PR #7403 should convert lock -> done marker"
fi
if picked="$(sup_pr_review_pick_and_claim 7403 7404)" && [ "$picked" = "7404" ]; then
  ok "fresh done marker skips PR #7403 and picks #7404"
else
  bad "done marker did not skip #7403 (picked '$picked')"
fi
sup_pr_review_release 7404

# --- done TTL expiry makes PR eligible again -------------------------------
touch -t 200001010000 "$(sup_pr_review_done_path 7403)" 2>/dev/null || true
SUP_PR_REVIEW_DONE_TTL_SECS=2 sup_pr_review_gc_locks
if sup_pr_review_try_claim 7403 after-done-expiry; then ok "expired done marker allows later PR #7403 retry"; else bad "expired done marker should allow retry"; fi
sup_pr_review_release 7403
export SUP_PR_REVIEW_DONE_TTL_SECS=3600

# --- parse recent logs into accepted locks ---------------------------------
cat > "$SUP_PR_REVIEW_LOG_DIR/pr-review-20260629T120000Z-codex-3-7410-123.log" <<'LOG'
{"event":"assignment_run.accepted","assignmentRef":"assignment.public.khala_coding.log7410","statusRef":"assignment.accepted.demo"}
LOG
sup_pr_review_sync_recent_logs 10
if [ "$(sup_pr_review_lock_assignment_ref 7410)" = "assignment.public.khala_coding.log7410" ]; then
  ok "sync-logs records accepted assignment ref for PR #7410"
else
  bad "sync-logs failed to record accepted assignment ref"
fi

# --- gh open PR parser skips drafts and prefers mergeable -------------------
cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
if [ "${1:-}" = "pr" ] && [ "${2:-}" = "list" ]; then
  printf '%s\n' '[{"number":7602,"isDraft":false,"mergeable":"UNKNOWN","updatedAt":"2026-06-29T12:00:00Z"},{"number":7600,"isDraft":true,"mergeable":"MERGEABLE","updatedAt":"2026-06-29T12:01:00Z"},{"number":7601,"isDraft":false,"mergeable":"MERGEABLE","updatedAt":"2026-06-29T12:02:00Z"}]'
fi
STUB
chmod +x "$WORK/gh"
export SUP_GH_BIN="$WORK/gh"
open_prs=()
while IFS= read -r pr_number; do
  [ -n "$pr_number" ] && open_prs+=("$pr_number")
done < <(sup_pr_review_open_pr_numbers)
if [ "${open_prs[*]}" = "7601 7602" ]; then
  ok "open PR parser skips drafts and orders mergeable PRs first"
else
  bad "open PR parser returned '${open_prs[*]}'"
fi

# --- CONCURRENCY: same PR list -> distinct winners -------------------------
POOL=(7501 7502 7503 7504 7505 7506 7507 7508)
SLOTS=8
pids=()
for i in $(seq 0 $((SLOTS-1))); do
  ( sup_pr_review_pick_and_claim "${POOL[@]}" > "$WORK/res.$i" 2>/dev/null ) &
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
  ok "concurrent $SLOTS slots claimed $distinct DISTINCT PRs"
else
  bad "concurrent PR claim collision: total=$total distinct=$distinct (want $SLOTS/$SLOTS): ${results[*]}"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
