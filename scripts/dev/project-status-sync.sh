#!/usr/bin/env bash
# Keep a GitHub project's Status in step with its issues: a closed issue is
# Done; an open issue still marked Done goes back to In Progress.
# Usage: scripts/dev/project-status-sync.sh [PROJECT_NUMBER] (default 22)
set -euo pipefail
owner=OpenAgentsInc
number=${1:-22}
project=$(gh project view "$number" --owner "$owner" --format json --jq .id)
field=$(gh project field-list "$number" --owner "$owner" --format json \
  --jq '.fields[]|select(.name=="Status")|.id')
option() {
  gh project field-list "$number" --owner "$owner" --format json \
    --jq ".fields[]|select(.name==\"Status\")|.options[]|select(.name==\"$1\")|.id"
}
done_id=$(option Done)
progress_id=$(option "In Progress")
gh project item-list "$number" --owner "$owner" --format json --limit 500 \
  --jq '.items[]|select(.content.type=="Issue")|"\(.id) \(.content.number) \(.status // "none")"' |
  while read -r item issue status; do
    state=$(gh issue view "$issue" -R "$owner/openagents" --json state --jq .state)
    if [[ $state == CLOSED && $status != Done ]]; then
      gh project item-edit --project-id "$project" --id "$item" --field-id "$field" \
        --single-select-option-id "$done_id" >/dev/null
      echo "#$issue: $status -> Done"
    elif [[ $state == OPEN && $status == Done ]]; then
      gh project item-edit --project-id "$project" --id "$item" --field-id "$field" \
        --single-select-option-id "$progress_id" >/dev/null
      echo "#$issue: Done -> In Progress (still open)"
    fi
  done
