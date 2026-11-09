#!/usr/bin/env bash
# Firm reason: 5. Hands a finished branch to the green-main pipeline's
# integrator instead of pushing main (docs/verse/green-main-pipeline.md,
# #13102): git only.
#
#   coax/remote/submit.sh [--agent ID] [--topic NAME]
#                         queue this worktree's commits since origin/main as
#                         refs/heads/queue/ID/NAME; the integrator checks them
#                         on the head they land on and lands them in order
#   coax/remote/submit.sh status [--agent ID]
#                         the queue, your rejected branches and your messages
#   coax/remote/submit.sh withdraw [--agent ID] [--topic NAME]
#                         take your branch out of the queue
#
# Every commit needs an "Agent: ID" trailer, so a failure on main can reach
# the agent that wrote it. With --agent (or OA_AGENT), submit.sh adds the
# trailer to the commits that lack it. Without them, the ID comes from the
# commits' own trailers.
#
# NAME defaults to the branch name, or else the worktree's directory without
# its "oa-" prefix. The queue runs in the order of each branch's newest commit
# time. Submitting the same NAME again replaces the queued branch.
#
# Settings: see coax/remote/pipeline-lib.sh (OA_PIPELINE_*).
set -uo pipefail
export LC_ALL=C
source "$(cd "$(dirname "$0")" && pwd -P)/pipeline-lib.sh"
die() { echo "submit: $*" >&2; exit 2; }

cmd=submit
case "${1:-}" in status|withdraw) cmd="$1"; shift ;; esac
agent="${OA_AGENT:-}" topic=""
while (($#)); do
  case "$1" in
    --agent) agent="$2"; shift 2 ;;
    --topic) topic="$2"; shift 2 ;;
    -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option $1 (submit.sh --help)" ;;
  esac
done
root="$(git rev-parse --show-toplevel 2> /dev/null)" || die "not in a Git checkout"
cd "$root" || exit 2
git fetch -q "$pl_remote" "$pl_main" "+refs/heads/$pl_queue/*:refs/remotes/$pl_remote/$pl_queue/*" \
  "+refs/heads/$pl_rejected/*:refs/remotes/$pl_remote/$pl_rejected/*" --prune 2> /dev/null ||
  git fetch -q "$pl_remote" "$pl_main" || die "could not fetch $pl_remote"
base="refs/remotes/$pl_remote/$pl_main"

default_topic() {
  local b; b="$(git symbolic-ref -q --short HEAD 2> /dev/null || true)"
  if [[ -n "$b" && "$b" != "$pl_main" && "$b" != main ]]; then echo "${b##*/}"; return; fi
  b="$(basename "$root")"; echo "${b#oa-}"
}
queue_list() {   # "time ref" for each queued branch, oldest first
  git for-each-ref --format='%(committerdate:unix) %(refname:lstrip=3)' "refs/remotes/$pl_remote/$pl_queue/" | sort -n
}

if [[ "$cmd" == status ]]; then
  echo "queue (oldest first):"
  n=0
  while read -r t ref; do
    [[ -n "$ref" ]] || continue
    n=$((n + 1)); echo "  $n. $ref  ($(git log -1 --format='%h %s' "refs/remotes/$pl_remote/$ref" | cut -c1-70))"
  done < <(queue_list)
  (( n > 0 )) || echo "  (empty)"
  if [[ -n "$agent" ]]; then
    rej="$(git for-each-ref --format='%(refname:lstrip=3)' "refs/remotes/$pl_remote/$pl_rejected/$agent/")"
    [[ -z "$rej" ]] || { echo "rejected (fix, then submit again):"; sed 's/^/  /' <<< "$rej"; }
    if [[ -s "$pl_dir/inbox/$agent.log" ]]; then echo "messages ($pl_dir/inbox/$agent.log), newest last:"; tail -n 20 "$pl_dir/inbox/$agent.log" | sed 's/^/  /'; fi
  fi
  red="$(jq -r '.[] | select(.status == "open") | "  \(.check): \(.culprit[0:10]) by \(.agent) (#\(.issue // "-"))"' "$pl_red" 2> /dev/null)"
  [[ -z "$red" ]] || { echo "red on main (already owned; don't investigate):"; echo "$red"; }
  exit 0
fi

[[ -n "$topic" ]] || topic="$(default_topic)"
topic="$(tr -c 'A-Za-z0-9._-\n' '-' <<< "$topic")"
pl_valid_name "$topic" || die "the topic \"$topic\" is not a usable name (--topic NAME)"

if [[ "$cmd" == withdraw ]]; then
  [[ -n "$agent" ]] || die "withdraw needs --agent ID (or OA_AGENT)"
  git push -q "$pl_remote" ":refs/heads/$pl_queue/$agent/$topic" && echo "submit: withdrew $pl_queue/$agent/$topic"
  exit $?
fi

[[ -z "$(git status --porcelain --untracked-files=no)" ]] || die "this worktree has uncommitted changes: commit them first"
commits="$(git rev-list --no-merges "$base..HEAD")"
[[ -n "$commits" ]] || die "no commits since $pl_remote/$pl_main: nothing to submit"
[[ -z "$(git rev-list --merges "$base..HEAD")" ]] || die "the branch has merge commits: rebase it on $pl_remote/$pl_main"

# The agent: --agent, or the commits' own trailers (one ID for all of them).
missing=0 ids=""
for c in $commits; do
  a="$(pl_agent_of "$c")"
  if [[ -z "$a" ]]; then missing=$((missing + 1)); else ids+="$a"$'\n'; fi
done
if [[ -z "$agent" ]]; then
  agent="$(sed '/^$/d' <<< "$ids" | sort -u)"
  [[ -n "$agent" ]] || die "no commit has an \"Agent: ID\" trailer: pass --agent ID (or set OA_AGENT) and submit.sh adds it"
  [[ "$(wc -l <<< "$agent" | tr -d ' ')" == 1 ]] || die "the commits name more than one agent ($(tr '\n' ' ' <<< "$agent")): pass --agent ID"
fi
pl_valid_name "$agent" || die "the agent ID \"$agent\" is not a usable name"
if (( missing > 0 )); then
  echo "submit: adding \"Agent: $agent\" to $missing commit(s)"
  GIT_EDITOR=true git -c core.hooksPath=/dev/null rebase -q --exec \
    "git log -1 --format='%(trailers:key=Agent,valueonly)' | grep -q . || git commit -q --amend --no-edit --no-verify --trailer 'Agent: $agent'" \
    "$(git merge-base "$base" HEAD)" || { git rebase --abort 2> /dev/null; die "could not add the trailers"; }
fi

# A branch that can't merge with main goes back at once: rebase it first.
if ! git merge-tree --write-tree "$base" HEAD > /dev/null 2>&1; then
  die "the branch conflicts with $pl_remote/$pl_main: rebase it (git rebase $pl_remote/$pl_main), then submit again"
fi

ref="refs/heads/$pl_queue/$agent/$topic"
git push -q --force "$pl_remote" "HEAD:$ref" || die "could not push $ref"
git push -q "$pl_remote" ":refs/heads/$pl_rejected/$agent/$topic" 2> /dev/null || true
git fetch -q "$pl_remote" "+refs/heads/$pl_queue/*:refs/remotes/$pl_remote/$pl_queue/*" --prune 2> /dev/null || true
pos="$(queue_list | awk -v r="$pl_queue/$agent/$topic" '$2 == r { print NR }')"
echo "submit: queued $(git rev-list --count "$base..HEAD") commit(s) as $pl_queue/$agent/$topic${pos:+, number $pos in the queue}"
echo "submit: the integrator checks it on the head it lands on and lands it; coax/remote/submit.sh status --agent $agent shows where it is"
