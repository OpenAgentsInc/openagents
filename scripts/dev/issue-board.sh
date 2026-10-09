#!/usr/bin/env bash
# Keep an issue's project Status in step with what an agent does to it
# (#11108), on every open project of the repository's owner the issue is on:
#
#   scripts/dev/issue-board.sh claim N               Status -> In Progress
#   scripts/dev/issue-board.sh block N "reason"      comments "Blocked: reason", Status -> Blocked
#   scripts/dev/issue-board.sh close N ["comment"]   closes the issue (with the comment), Status -> Done
#   scripts/dev/issue-board.sh set N "Status name"   any Status value, e.g. Todo
#
# Uses only the REST API, so it keeps working when other tools have spent the
# GraphQL limit. A board that cannot be read or written is said and skipped;
# only a failed close exits non-zero.
# ISSUE_BOARD_REPO (default OpenAgentsInc/openagents) names the repository;
# ISSUE_BOARD_PROJECTS ("22 19") limits the projects looked at.
set -uo pipefail

usage() { sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }
[[ $# -ge 2 ]] || usage
verb=$1
issue=${2#\#}
[[ $issue =~ ^[0-9]+$ ]] || usage
repo=${ISSUE_BOARD_REPO:-OpenAgentsInc/openagents}
owner=${repo%%/*}

say() { printf '%s\n' "$*"; }

# Sets Status to "$1" (matched without regard to case) on each project.
move() {
  local target=$1 projects scope="orgs" number base hit item fields field option moved=0
  if [[ -n ${ISSUE_BOARD_PROJECTS:-} ]]; then
    projects=$ISSUE_BOARD_PROJECTS
  elif ! projects=$(gh api --paginate "/orgs/$owner/projectsV2?per_page=100" \
      --jq '.[]|select(.state=="open")|.number' 2>/dev/null); then
    scope="users"
    if ! projects=$(gh api --paginate "/users/$owner/projectsV2?per_page=100" \
        --jq '.[]|select(.state=="open")|.number' 2>/dev/null); then
      say "board: could not list $owner's projects; #$issue's Status stays as it is."
      return 0
    fi
  fi
  for number in $projects; do
    base="/$scope/$owner/projectsV2/$number"
    if ! hit=$(gh api -X GET "$base/items" -f per_page=100 -f "q=$issue" \
        --jq ".[]|select(.content.number==$issue and ((.content.repository_url // \"\")|endswith(\"/repos/$repo\")))|.id" 2>/dev/null); then
      say "board: could not read project $number; skipped."
      continue
    fi
    item=${hit%%$'\n'*}
    [[ -n $item ]] || continue
    if ! fields=$(gh api "$base/fields" 2>/dev/null); then
      say "board: could not read project $number's fields; skipped."
      continue
    fi
    field=$(jq -r '.[]|select(.name=="Status")|.id' <<<"$fields" | head -n1)
    option=$(jq -r --arg n "$target" '.[]|select(.name=="Status")|.options[]
      |select(((.name.raw // .name)|ascii_downcase)==($n|ascii_downcase))|.id' <<<"$fields" | head -n1)
    if [[ -z $field || -z $option ]]; then
      say "board: project $number has no Status \"$target\"; skipped."
      continue
    fi
    if gh api -X PATCH "$base/items/$item" --input - >/dev/null 2>&1 \
        <<<"{\"fields\":[{\"id\":$field,\"value\":\"$option\"}]}"; then
      say "board: #$issue -> $target on project $number."
      moved=1
    else
      say "board: could not set #$issue to $target on project $number."
    fi
  done
  [[ $moved == 1 ]] || say "board: #$issue is on no project with a Status \"$target\"."
  return 0
}

case $verb in
  claim) move "In Progress" ;;
  block)
    reason=${3:-}
    [[ -n $reason ]] || { say "block needs a reason: what it waits on." >&2; exit 2; }
    gh issue comment "$issue" -R "$repo" --body "Blocked: $reason" >/dev/null \
      || say "Could not comment the block reason on #$issue."
    move "Blocked"
    ;;
  close)
    args=(issue close "$issue" -R "$repo" --reason completed)
    [[ -n ${3:-} ]] && args+=(--comment "$3")
    if ! gh "${args[@]}" >/dev/null; then
      say "Could not close #$issue."
      exit 1
    fi
    say "Closed #$issue."
    move "Done"
    ;;
  set)
    [[ -n ${3:-} ]] || usage
    move "$3"
    ;;
  *) usage ;;
esac
