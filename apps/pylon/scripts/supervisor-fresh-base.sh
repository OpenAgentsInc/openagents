#!/usr/bin/env bash
#
# supervisor-fresh-base.sh — resolve the CURRENT origin/main commit so every
# dispatched assignment bases on FRESH origin/main, never a stale local HEAD.
#
# ROOT CAUSE THIS FIXES (#6719 deletion-poison; recurred #6734/#6736, nearly
# #6761): the codex/claude supervisors pinned each assignment's `--commit` to the
# LOCAL checkout HEAD (`git -C "$REPO_ROOT" rev-parse HEAD`). The supervisor's
# $REPO_ROOT can lag far behind origin/main (an idle clone, or another agent's
# dirty worktree that is never fast-forwarded). main moves fast, so a stale local
# HEAD makes every worker materialize its bounded workspace from an OLD base; the
# resulting PR diff vs current main then looks like a mass deletion of everything
# added since that old base — i.e. squashing it would DELETE clients/,
# INVARIANTS.md, PRODUCT.md, apps/, etc.
#
# The fix anchors dispatch on the live remote main SHA via `git ls-remote` — a
# pure network READ that NEVER mutates the working tree or local refs, so it is
# safe in a dirty or shared checkout (no fetch/reset/checkout of the supervisor's
# own repo, which would clobber a concurrent agent's work).
#
# This file performs no work at source time; it only defines functions.

: "${SUP_REPO:=OpenAgentsInc/openagents}"
: "${SUP_BASE_BRANCH:=main}"
: "${SUP_GIT_BIN:=git}"

# supervisor_resolve_fresh_origin_main_sha [full_name] [base_branch]
#
# Echo the 40-char lowercase commit SHA of refs/heads/<base_branch> for the given
# GitHub <full_name> (defaults: $SUP_REPO, $SUP_BASE_BRANCH), or echo nothing and
# return 1 on any failure. Pure read; never mutates local git state.
#
# Callers MUST treat a non-zero return / empty output as "do not dispatch": it is
# always safer to skip a dispatch than to pin work to a stale base and poison the
# resulting PR into a mass-deletion diff.
supervisor_resolve_fresh_origin_main_sha() {
  local full_name="${1:-$SUP_REPO}"
  local base_branch="${2:-${SUP_BASE_BRANCH:-main}}"
  [ -n "$full_name" ] || return 1

  local out sha
  out=$("$SUP_GIT_BIN" ls-remote "https://github.com/${full_name}.git" \
    "refs/heads/${base_branch}" 2>/dev/null) || return 1
  sha=$(printf '%s\n' "$out" | awk 'NR==1{print $1}')
  sha=$(printf '%s' "$sha" | tr '[:upper:]' '[:lower:]')
  if [[ "$sha" =~ ^[0-9a-f]{40}$ ]]; then
    printf '%s' "$sha"
    return 0
  fi
  return 1
}
