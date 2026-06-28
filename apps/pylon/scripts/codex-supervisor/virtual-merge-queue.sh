#!/usr/bin/env bash
#
# virtual-merge-queue.sh — local projected-base helper for codex-supervisor.
#
# The supervisor still opens normal GitHub PRs, but it can stop creating work
# from a stale base by maintaining a local "virtual HEAD": real main plus the
# pylon/assignment-* branches that are already in flight and can be replayed
# cleanly. New assignments are pinned to that projected commit, so the actual PR
# merge path stays closer to trivial after main advances.
#
# This file is sourceable and test-friendly. It performs no work at source time.

: "${SUP_VMQ_ENABLED:=1}"
: "${SUP_VMQ_DIR:=${SUP_STATE_DIR:-$HOME/.codex-supervisor}/virtual-merge-queue}"
: "${SUP_VMQ_BRANCH_PREFIX:=pylon/assignment-}"
: "${SUP_VMQ_REMOTE:=origin}"
: "${SUP_VMQ_BASE_BRANCH:=main}"
: "${SUP_VMQ_MAX_BRANCHES:=24}"
: "${SUP_GH_BIN:=gh}"

sup_vmq_log() {
  if declare -F log >/dev/null 2>&1; then
    log "$*"
  else
    printf '%s\n' "$*" >&2
  fi
}

sup_vmq_git() {
  git -C "$SUP_VMQ_DIR" "$@"
}

sup_vmq_sha() {
  git -C "$1" rev-parse "$2" 2>/dev/null
}

sup_vmq_sync_repo() {
  local source_repo="$1"
  [ -d "$source_repo/.git" ] || return 1
  mkdir -p "$(dirname "$SUP_VMQ_DIR")" 2>/dev/null || true

  local remote_url
  remote_url="$(git -C "$source_repo" remote get-url "$SUP_VMQ_REMOTE" 2>/dev/null || true)"
  [ -n "$remote_url" ] || remote_url="$source_repo"

  if [ ! -d "$SUP_VMQ_DIR/.git" ]; then
    rm -rf "$SUP_VMQ_DIR"
    git clone --quiet "$remote_url" "$SUP_VMQ_DIR" >/dev/null 2>&1 || return 1
  fi

  sup_vmq_git remote set-url "$SUP_VMQ_REMOTE" "$remote_url" >/dev/null 2>&1 || true
  sup_vmq_git fetch --quiet "$SUP_VMQ_REMOTE" \
    "+refs/heads/*:refs/remotes/$SUP_VMQ_REMOTE/*" >/dev/null 2>&1 || return 1
  return 0
}

sup_vmq_candidate_branches() {
  if command -v "$SUP_GH_BIN" >/dev/null 2>&1 && [ -n "${SUP_REPO:-}" ]; then
    local json branches
    if declare -F sup_run_timeout >/dev/null 2>&1; then
      json=$(sup_run_timeout "${SUP_GH_TIMEOUT_SECS:-15}" "$SUP_GH_BIN" pr list \
        --repo "$SUP_REPO" --state open --json headRefName \
        --limit "$SUP_VMQ_MAX_BRANCHES" 2>/dev/null)
    else
      json=$("$SUP_GH_BIN" pr list --repo "$SUP_REPO" --state open --json headRefName \
        --limit "$SUP_VMQ_MAX_BRANCHES" 2>/dev/null)
    fi
    branches=$(printf '%s' "$json" | python3 -c "import json,os,sys
prefix=os.environ.get('SUP_VMQ_BRANCH_PREFIX','pylon/assignment-')
try:
    rows=json.load(sys.stdin)
except Exception:
    sys.exit(0)
if not isinstance(rows,list):
    sys.exit(0)
for row in rows:
    head=row.get('headRefName') if isinstance(row,dict) else None
    if isinstance(head,str) and head.startswith(prefix):
        print(head)" 2>/dev/null)
    if [ -n "$branches" ]; then
      printf '%s\n' "$branches"
      return 0
    fi
  fi

  sup_vmq_git for-each-ref \
    --format='%(refname:short)' \
    --sort=committerdate \
    "refs/remotes/$SUP_VMQ_REMOTE/$SUP_VMQ_BRANCH_PREFIX*" 2>/dev/null \
    | sed "s#^$SUP_VMQ_REMOTE/##" \
    | tail -n "$SUP_VMQ_MAX_BRANCHES"
}

sup_vmq_project_head() {
  local source_repo="$1"
  local source_base="${2:-HEAD}"
  [ "${SUP_VMQ_ENABLED:-1}" = "1" ] || return 1

  local real_base
  real_base="$(sup_vmq_sha "$source_repo" "$source_base")" || return 1
  [ -n "$real_base" ] || return 1

  if ! sup_vmq_sync_repo "$source_repo"; then
    sup_vmq_log "VMQ unavailable: sync failed; using real base $real_base"
    return 1
  fi

  local work_branch="pylon/virtual-merge-queue"
  sup_vmq_git checkout --quiet -B "$work_branch" "$real_base" >/dev/null 2>&1 || return 1

  local applied=0 skipped=0 branch
  while IFS= read -r branch; do
    [ -n "$branch" ] || continue
    # Skip branches already contained by the real base.
    if sup_vmq_git merge-base --is-ancestor "$SUP_VMQ_REMOTE/$branch" HEAD >/dev/null 2>&1; then
      continue
    fi
    if sup_vmq_git merge --quiet --no-ff --no-edit "$SUP_VMQ_REMOTE/$branch" >/dev/null 2>&1; then
      applied=$((applied + 1))
    else
      sup_vmq_git merge --abort >/dev/null 2>&1 || true
      skipped=$((skipped + 1))
    fi
  done < <(sup_vmq_candidate_branches)

  local projected
  projected="$(sup_vmq_git rev-parse HEAD 2>/dev/null)" || return 1
  [ -n "$projected" ] || return 1
  sup_vmq_log "VMQ projected_head=$projected real_base=$real_base applied=$applied skipped=$skipped"
  printf '%s' "$projected"
  return 0
}
