#!/usr/bin/env bash
# Keep a GitHub project's Status in step with its issues: a closed issue is
# Done; an open issue still marked Done goes back to In Progress; an item with
# no Status gets Todo. Uses the REST API (two reads per run) so it keeps
# working when the GraphQL limit is spent by other tools.
# Usage: scripts/dev/project-status-sync.sh [PROJECT_NUMBER] (default 22)
set -euo pipefail
owner=OpenAgentsInc
number=${1:-22}
base="/orgs/$owner/projectsV2/$number"
status=$(gh api "$base/fields" --jq '.[]|select(.name=="Status")')
field=$(jq -r .id <<<"$status")
option() { jq -r --arg n "$1" '.options[]|select((.name.raw // .name)==$n)|.id' <<<"$status"; }
done_id=$(option Done)
progress_id=$(option "In Progress")
todo_id=$(option Todo)
set_status() {
  gh api -X PATCH "$base/items/$1" --input - >/dev/null \
    <<<"{\"fields\":[{\"id\":$field,\"value\":\"$2\"}]}"
}
gh api --paginate "$base/items?per_page=100&fields[]=$field" \
  --jq '.[]|select(.content_type=="Issue")
        |"\(.id) \(.content.number) \(.content.state) \(((.fields[]?|select(.name=="Status")|.value.name.raw // .value.name) // "none")|gsub(" ";"_"))"' |
  while read -r item issue state current; do
    if [[ $state == closed && $current != Done ]]; then
      set_status "$item" "$done_id" && echo "#$issue: $current -> Done"
    elif [[ $state == open && $current == Done ]]; then
      set_status "$item" "$progress_id" && echo "#$issue: Done -> In Progress (still open)"
    elif [[ $current == none ]]; then
      set_status "$item" "$todo_id" && echo "#$issue: no status -> Todo"
    fi
  done
