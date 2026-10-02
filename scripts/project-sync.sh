#!/usr/bin/env bash
# Reconcile the OpenAgents project board
# (https://github.com/orgs/OpenAgentsInc/projects/19) with GitHub state.
#
#   scripts/project-sync.sh [--dry-run]
#
# Rules, applied per issue:
#   - every open issue in the repo is on the board (added as Todo if missing);
#   - closed issue                                   -> Done
#   - open, live Coder claim marker in its comments  -> In progress
#     (latest `<!-- openagents-coder-claim` newer than the latest
#      `<!-- openagents-coder-release -->` and younger than CLAIM_HOURS, default 6)
#   - open, native "blocked by" issue still open     -> Blocked (unless In progress
#     or In review), and the "Blocked by" text field lists the open blockers
#   - Blocked, all native blockers closed            -> Todo, "Blocked by" cleared
# In progress / In review set by hand are left alone unless the issue closed.
set -euo pipefail

REPO="OpenAgentsInc/openagents"
OWNER="OpenAgentsInc"
PROJECT_NUMBER=19
PROJECT_ID="PVT_kwDOBubymc4BlgVZ"
STATUS_FIELD="PVTSSF_lADOBubymc4BlgVZzhkM-Bg"
BLOCKED_BY_FIELD="PVTF_lADOBubymc4BlgVZzhkM-L0"
opt() { case "$1" in Todo) echo 5cbde1c1;; "In progress") echo 0eb80cca;; Blocked) echo c7d7f909;;
  "In review") echo 1868394a;; Done) echo 42d2482f;; esac; }
CLAIM_HOURS="${CLAIM_HOURS:-6}"
DRY=0; [ "${1:-}" = "--dry-run" ] && DRY=1

set_status() { # item number from to
  echo "#$2: ${3:-none} -> $4"
  [ $DRY = 1 ] && return
  gh project item-edit --project-id "$PROJECT_ID" --id "$1" \
    --field-id "$STATUS_FIELD" --single-select-option-id "$(opt "$4")" >/dev/null
}
set_blocked_text() { # item text
  [ $DRY = 1 ] && return
  if [ -n "$2" ]; then
    gh project item-edit --project-id "$PROJECT_ID" --id "$1" --field-id "$BLOCKED_BY_FIELD" --text "$2" >/dev/null
  else
    gh project item-edit --project-id "$PROJECT_ID" --id "$1" --field-id "$BLOCKED_BY_FIELD" --clear >/dev/null 2>&1 || true
  fi
}

# Add any open issue missing from the board.
ON_BOARD=$(gh project item-list "$PROJECT_NUMBER" --owner "$OWNER" --limit 1000 --format json \
  --jq '[.items[] | select(.content.type=="Issue" and .content.repository=="'"$REPO"'") | .content.number] | sort | .[]')
for N in $(gh issue list -R "$REPO" --state open --limit 1000 --json number --jq '.[].number'); do
  if ! grep -qx "$N" <<<"$ON_BOARD"; then
    echo "#$N: added to board"
    if [ $DRY = 0 ]; then
      ITEM=$(gh project item-add "$PROJECT_NUMBER" --owner "$OWNER" --url "https://github.com/$REPO/issues/$N" --format json --jq .id)
      gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM" --field-id "$STATUS_FIELD" --single-select-option-id "$(opt Todo)" >/dev/null
    fi
  fi
done

NOW=$(date -u +%s)
gh project item-list "$PROJECT_NUMBER" --owner "$OWNER" --limit 1000 --format json \
  --jq '.items[] | select(.content.type=="Issue" and .content.repository=="'"$REPO"'") | [.id, .content.number, (.status // ""), (.["blocked by"] // "")] | @tsv' |
while IFS=$'\t' read -r ITEM N STATUS BLOCKED_TEXT; do
  STATE=$(gh issue view "$N" -R "$REPO" --json state --jq .state)
  if [ "$STATE" != "OPEN" ]; then
    [ "$STATUS" != "Done" ] && set_status "$ITEM" "$N" "$STATUS" Done
    continue
  fi

  # Live claim: latest claim marker after the latest release marker, within CLAIM_HOURS.
  LAST=$(gh api --paginate "repos/$REPO/issues/$N/comments" \
    --jq '.[] | select(.body | test("<!-- openagents-coder-(claim|release)")) | [.created_at, (if (.body|test("<!-- openagents-coder-release")) then "release" else "claim" end)] | @tsv' \
    | sort | tail -1)
  CLAIMED=0
  if [ "$(cut -f2 <<<"$LAST")" = "claim" ]; then
    AT=$(cut -f1 <<<"$LAST")
    T=$(date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$AT" +%s 2>/dev/null || date -u -d "$AT" +%s)
    [ $(( NOW - T )) -lt $(( CLAIM_HOURS * 3600 )) ] && CLAIMED=1
  fi

  OPEN_BLOCKERS=$(gh api "repos/$REPO/issues/$N/dependencies/blocked_by" \
    --jq '[.[] | select(.state=="open") | "#\(.number)"] | join(" ")' 2>/dev/null || true)
  [ "$OPEN_BLOCKERS" != "$BLOCKED_TEXT" ] && { echo "#$N: blocked by '${OPEN_BLOCKERS}'"; set_blocked_text "$ITEM" "$OPEN_BLOCKERS"; }

  if [ $CLAIMED = 1 ]; then
    [ "$STATUS" != "In progress" ] && [ "$STATUS" != "In review" ] && set_status "$ITEM" "$N" "$STATUS" "In progress"
  elif [ -n "$OPEN_BLOCKERS" ]; then
    case "$STATUS" in "In progress"|"In review"|Blocked) ;; *) set_status "$ITEM" "$N" "$STATUS" Blocked ;; esac
  elif [ "$STATUS" = "Blocked" ] || [ -z "$STATUS" ]; then
    set_status "$ITEM" "$N" "$STATUS" Todo
  fi
  :
done
