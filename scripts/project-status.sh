#!/usr/bin/env bash
# Set an issue's Status on the OpenAgents project board
# (https://github.com/orgs/OpenAgentsInc/projects/19). Adds the issue to the
# board if it is not there yet.
#
#   scripts/project-status.sh N STATUS [--blocked-by "N1 N2"]
#
# STATUS: todo | in-progress | blocked | in-review | done (case-insensitive;
# "In progress" and "in_progress" also work).
# --blocked-by records native GitHub "blocked by" dependencies on N for each
# listed issue and writes them to the board's "Blocked by" text field.
#
# Needs `gh` with the `project` scope (`gh auth refresh -s project`).
# IDs are documented in docs/project-board.md.
set -euo pipefail

REPO="OpenAgentsInc/openagents"
OWNER="OpenAgentsInc"
PROJECT_NUMBER=19
PROJECT_ID="PVT_kwDOBubymc4BlgVZ"
STATUS_FIELD="PVTSSF_lADOBubymc4BlgVZzhkM-Bg"
BLOCKED_BY_FIELD="PVTF_lADOBubymc4BlgVZzhkM-L0"

usage() { sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
[ $# -ge 2 ] || usage
N="${1#\#}"; STATUS="$2"; shift 2
BLOCKERS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --blocked-by) BLOCKERS="${2:-}"; shift 2 ;;
    *) usage ;;
  esac
done

case "$(echo "$STATUS" | tr '[:upper:]' '[:lower:]' | tr ' _' '--')" in
  todo) OPT="5cbde1c1"; NAME="Todo" ;;
  in-progress|progress|claimed) OPT="0eb80cca"; NAME="In progress" ;;
  blocked) OPT="c7d7f909"; NAME="Blocked" ;;
  in-review|review) OPT="1868394a"; NAME="In review" ;;
  done|closed) OPT="42d2482f"; NAME="Done" ;;
  *) echo "unknown status: $STATUS" >&2; usage ;;
esac

ITEM=$(gh project item-add "$PROJECT_NUMBER" --owner "$OWNER" \
  --url "https://github.com/$REPO/issues/$N" --format json --jq .id)
gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM" \
  --field-id "$STATUS_FIELD" --single-select-option-id "$OPT" >/dev/null

if [ -n "$BLOCKERS" ]; then
  TEXT=""
  for B in $(echo "$BLOCKERS" | tr ',' ' '); do
    B="${B#\#}"
    BID=$(gh api "repos/$REPO/issues/$B" --jq .id)
    # Already-present dependencies return 422; that is fine.
    gh api -X POST "repos/$REPO/issues/$N/dependencies/blocked_by" \
      -F issue_id="$BID" >/dev/null 2>&1 || true
    TEXT="${TEXT:+$TEXT }#$B"
  done
  gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM" \
    --field-id "$BLOCKED_BY_FIELD" --text "$TEXT" >/dev/null
elif [ "$NAME" != "Blocked" ]; then
  gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM" \
    --field-id "$BLOCKED_BY_FIELD" --clear >/dev/null 2>&1 || true
fi

echo "#$N -> $NAME${BLOCKERS:+ (blocked by $BLOCKERS)}"
