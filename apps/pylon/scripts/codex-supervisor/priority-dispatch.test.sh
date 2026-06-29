#!/usr/bin/env bash
#
# priority-dispatch.test.sh — tests for the codex-supervisor label-priority
# dispatch ordering (#6711).
#
# Covers:
#   * sup_priority_rank_for_labels maps each prio:* tier to its rank and
#     unlabelled / unknown-only label sets to 99, taking the highest tier when
#     several prio:* labels are present.
#   * sup_order_pool_by_priority orders a mixed pool prio:0 -> ... -> prio:4 ->
#     unlabelled, preserves intra-tier round-robin rotation, and never drops or
#     duplicates a pool member.
#   * sup_fetch_issue_label_map parses gh JSON into the cached TSV label map and
#     sup_labels_for_issue reads it back (stub gh, no network).
#
# Run: bash apps/pylon/scripts/codex-supervisor/priority-dispatch.test.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
ok()  { PASS=$((PASS+1)); printf 'ok   - %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf 'FAIL - %s\n' "$1"; }

export SUP_STATE_DIR="$WORK/state"
export SUP_LOCKOUT_CACHE_DIR="$WORK/cache"
export SUP_REPO="OpenAgentsInc/openagents"

# shellcheck source=priority-dispatch.sh
source "$SCRIPT_DIR/priority-dispatch.sh"

# --- sup_priority_rank_for_labels -----------------------------------------
[ "$(sup_priority_rank_for_labels 'prio:0-pr-burndown')" = "0" ]        && ok "rank prio:0 -> 0"        || bad "rank prio:0"
[ "$(sup_priority_rank_for_labels 'prio:1-continual-learning')" = "1" ] && ok "rank prio:1 -> 1"        || bad "rank prio:1"
[ "$(sup_priority_rank_for_labels 'prio:2-issue-triage')" = "2" ]       && ok "rank prio:2 -> 2"        || bad "rank prio:2"
[ "$(sup_priority_rank_for_labels 'prio:3-product-promises')" = "3" ]   && ok "rank prio:3 -> 3"        || bad "rank prio:3"
[ "$(sup_priority_rank_for_labels 'prio:4-backstop-burn')" = "4" ]      && ok "rank prio:4 -> 4"        || bad "rank prio:4"
[ "$(sup_priority_rank_for_labels '')" = "99" ]                         && ok "rank empty -> 99"        || bad "rank empty"
[ "$(sup_priority_rank_for_labels 'bug,enhancement')" = "99" ]          && ok "rank no-prio -> 99"      || bad "rank no-prio"
# Highest (lowest-numbered) prio wins when several are present.
[ "$(sup_priority_rank_for_labels 'bug,prio:4-backstop-burn,prio:1-continual-learning')" = "1" ] \
  && ok "rank takes highest tier (prio:1 over prio:4)" || bad "rank highest-tier"
# A prio substring that is not a full label must NOT match.
[ "$(sup_priority_rank_for_labels 'prio:0-pr-burndown-extra')" = "99" ] && ok "rank ignores non-exact prio label" || bad "rank non-exact"

# --- sup_order_pool_by_priority -------------------------------------------
# Inject labels directly (override the map lookup; no gh needed). Use a case
# statement instead of associative arrays so stock macOS Bash 3 can run this.
sup_labels_for_issue() {
  case "$1" in
    10) printf 'prio:4-backstop-burn' ;;
    11) printf 'prio:0-pr-burndown' ;;
    12) printf 'bug' ;;
    13) printf 'prio:2-issue-triage' ;;
    14) printf 'prio:0-pr-burndown' ;;
  esac
}

# start=0: tiers ascending; within prio:0, input order (11 then 14) preserved.
order="$(sup_order_pool_by_priority 0 10 11 12 13 14 | tr '\n' ' ')"
[ "$order" = "11 14 13 10 12 " ] && ok "order prio:0(11,14) -> prio:2(13) -> prio:4(10) -> none(12)" \
  || bad "order start=0 returned '$order' (want '11 14 13 10 12 ')"

# Rotation spreads intra-tier order: start=2 rotates the input (12,13,14,10,11)
# so within the prio:0 tier 14 now leads 11.
order2="$(sup_order_pool_by_priority 2 10 11 12 13 14 | tr '\n' ' ')"
[ "$order2" = "14 11 13 10 12 " ] && ok "order intra-tier rotation (start=2 -> 14 before 11)" \
  || bad "order start=2 returned '$order2' (want '14 11 13 10 12 ')"

# No member dropped or duplicated regardless of start.
count="$(sup_order_pool_by_priority 3 10 11 12 13 14 | grep -c .)"
[ "$count" = "5" ] && ok "order preserves pool size (5)" || bad "order pool size '$count'"

# --- sup_fetch_issue_label_map + sup_labels_for_issue (stub gh) -----------
# Restore the real map-backed lookup (the block above overrode it).
# shellcheck source=priority-dispatch.sh
source "$SCRIPT_DIR/priority-dispatch.sh"
STUB_DIR="$WORK/fixtures"; mkdir -p "$STUB_DIR"
cat > "$WORK/gh" <<'STUB'
#!/usr/bin/env bash
# Only supports: issue list --state open --json number,labels
cat "$STUB_DIR/openlist.json"
STUB
chmod +x "$WORK/gh"
export STUB_DIR
export SUP_GH_BIN="$WORK/gh"
printf '%s' '[{"number":11,"labels":[{"name":"prio:0-pr-burndown"},{"name":"bug"}]},{"number":13,"labels":[{"name":"prio:2-issue-triage"}]},{"number":12,"labels":[]}]' \
  > "$STUB_DIR/openlist.json"

if sup_fetch_issue_label_map; then ok "sup_fetch_issue_label_map built the map"; else bad "sup_fetch_issue_label_map failed"; fi
[ "$(sup_labels_for_issue 11)" = "prio:0-pr-burndown,bug" ] && ok "labels for #11 read back from map" || bad "labels #11 '$(sup_labels_for_issue 11)'"
[ "$(sup_labels_for_issue 13)" = "prio:2-issue-triage" ]    && ok "labels for #13 read back from map" || bad "labels #13"
[ -z "$(sup_labels_for_issue 12)" ]                          && ok "labels for #12 empty (no labels)" || bad "labels #12 not empty"
[ -z "$(sup_labels_for_issue 999)" ]                         && ok "labels for unknown #999 empty"    || bad "labels #999 not empty"

# End-to-end: map-backed ordering ranks #11 (prio:0) ahead of #13 (prio:2) ahead of #12 (none).
order3="$(sup_order_pool_by_priority 0 12 13 11 | tr '\n' ' ')"
[ "$order3" = "11 13 12 " ] && ok "map-backed order prio:0 -> prio:2 -> none" || bad "map-backed order '$order3'"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
