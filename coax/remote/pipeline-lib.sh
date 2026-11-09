#!/usr/bin/env bash
# Firm reason: 5. Settings and helpers shared by the green-main pipeline's
# scripts (docs/verse/green-main-pipeline.md, #13102): submit.sh,
# integrate.sh and after.sh source it. Git, gh and the files under the
# pipeline's directory.
#
# Settings (environment):
#   OA_PIPELINE_REMOTE   the Git remote (default origin)
#   OA_PIPELINE_MAIN     the branch the integrator lands on (default main)
#   OA_PIPELINE_GREEN    the last commit that passed the full check (default main-green)
#   OA_PIPELINE_QUEUE    the queue's ref prefix under refs/heads/ (default queue)
#   OA_PIPELINE_REJECTED where a red branch goes back to, under refs/heads/ (default rejected)
#   OA_PIPELINE_DIR      logs, results and the red list (default ~/.openagents/pipeline)
#   OA_PIPELINE_ISSUES   0: file no GitHub issues and write no comments (scratch runs)
#   OA_PIPELINE_NOTIFY   a command run as CMD AGENT MESSAGE-FILE for each message to an agent
#
# A scratch run points every name elsewhere, for example
#   OA_PIPELINE_MAIN=scratch/gm/main OA_PIPELINE_GREEN=scratch/gm/main-green \
#   OA_PIPELINE_QUEUE=scratch/gm/queue OA_PIPELINE_REJECTED=scratch/gm/rejected \
#   OA_PIPELINE_DIR=/tmp/gm OA_PIPELINE_ISSUES=0
pl_remote="${OA_PIPELINE_REMOTE:-origin}"
pl_main="${OA_PIPELINE_MAIN:-main}"
pl_green="${OA_PIPELINE_GREEN:-main-green}"
pl_queue="${OA_PIPELINE_QUEUE:-queue}"
pl_rejected="${OA_PIPELINE_REJECTED:-rejected}"
pl_dir="${OA_PIPELINE_DIR:-$HOME/.openagents/pipeline}"
pl_issues="${OA_PIPELINE_ISSUES:-1}"
pl_repo="${OA_PIPELINE_GH_REPO:-OpenAgentsInc/openagents}"
pl_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
mkdir -p "$pl_dir/logs" "$pl_dir/inbox" "$pl_dir/releases/after"
pl_red="$pl_dir/releases/after/red.json"
[[ -f "$pl_red" ]] || echo '[]' > "$pl_red"

pl_say() { echo "$(date +%H:%M:%S) $*" >&2; }
pl_lock() {   # FILE CMD...: CMD under an exclusive lock
  if command -v flock > /dev/null; then flock "$@"; else lockf -k "$@"; fi
}
pl_trylock() {   # FILE CMD...: CMD under the lock, or status 75 when another holds it
  if command -v flock > /dev/null; then flock -n -E 75 "$@"; else lockf -k -t 0 "$@" || return 75; fi
}
# A commit's agent, from its last "Agent:" trailer ("" when none).
pl_agent_of() {
  git log -1 --format='%(trailers:key=Agent,valueonly,separator=%x0a)' "$1" 2> /dev/null | sed '/^$/d' | tail -1 | tr -d '[:space:]'
}
pl_valid_name() { [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]]; }

# A message to an agent: its inbox file (OA_PIPELINE_DIR/inbox/AGENT.log, which
# `submit.sh status` prints), and OA_PIPELINE_NOTIFY when set.
pl_message() {   # agent text
  local agent="${1:-unknown}" file
  file="$(mktemp "${TMPDIR:-/tmp}/oa-pipeline-msg.XXXXXXXX")"
  printf '%s\n' "$2" > "$file"
  { echo "== $(date '+%Y-%m-%d %H:%M %Z')"; cat "$file"; echo; } >> "$pl_dir/inbox/$agent.log"
  if [[ -n "${OA_PIPELINE_NOTIFY:-}" ]]; then
    # shellcheck disable=SC2086
    $OA_PIPELINE_NOTIFY "$agent" "$file" || pl_say "OA_PIPELINE_NOTIFY failed for $agent"
  fi
  rm -f "$file"
}

# The red list (releases/after/red.json): one entry per failing check on main
# that has a culprit. Fields: id, check, culprit, subject, agent, issue,
# opened (Unix time), status (open, fixed or reverted), revert (the revert
# branch, once queued).
pl_red_update() {   # JQ-PROGRAM [ARGS...]: rewrite the red list under its lock
  local prog="$1"; shift
  pl_lock "$pl_red.lock" bash -c '
    f="$1"; prog="$2"; shift 2
    jq "$@" "$prog" "$f" > "$f.new" && mv "$f.new" "$f"' _ "$pl_red" "$prog" "$@"
}

pl_issue_comment() {   # issue-number text
  [[ "$pl_issues" == 1 && -n "$1" ]] || return 0
  gh issue comment "$1" --repo "$pl_repo" --body "$2" > /dev/null 2>&1 || pl_say "could not comment on #$1"
}
