#!/usr/bin/env bash
# worker.sh — one Vertex-powered claude agent works one promise in an isolated
# worktree, runs check:deploy, commits to a BRANCH, pushes the branch, opens a PR.
#
# PR-PER-AGENT: this script NEVER pushes to main. It pushes vertex-fleet/<promise>
# and opens a PR for human review.
#
# Auth: Claude rides Google Vertex AI. Locally that uses gcloud Application
# Default Credentials (owner reauthed). On an unattended GCE instance the same
# env works with the service-account metadata-server ADC (no reauth) — identical
# to oa-codex-control.
#
# Usage:
#   worker.sh --promise <promiseId> --brief-file <path> [--model <vertex model>]
#             [--repo-url <git url>] [--base origin/main] [--no-pr] [--dry-run]
#
# Emits a single-line JSON result to stdout (last line) and human logs to stderr.

set -uo pipefail

# ---- Vertex env (names only; no secret values) -------------------------------
export CLAUDE_CODE_USE_VERTEX=1
export ANTHROPIC_VERTEX_PROJECT_ID="${ANTHROPIC_VERTEX_PROJECT_ID:-openagentsgemini}"
export CLOUD_ML_REGION="${CLOUD_ML_REGION:-global}"
export ANTHROPIC_SMALL_FAST_MODEL="${ANTHROPIC_SMALL_FAST_MODEL:-claude-haiku-4-5}"

PROMISE=""
BRIEF_FILE=""
MODEL="claude-sonnet-4-6"
BASE="origin/main"
OPEN_PR=1
DRY_RUN=0
REPO_URL="https://github.com/OpenAgentsInc/openagents.git"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --promise) PROMISE="$2"; shift 2;;
    --brief-file) BRIEF_FILE="$2"; shift 2;;
    --model) MODEL="$2"; shift 2;;
    --base) BASE="$2"; shift 2;;
    --repo-url) REPO_URL="$2"; shift 2;;
    --no-pr) OPEN_PR=0; shift;;
    --dry-run) DRY_RUN=1; shift;;
    *) echo "worker: unknown arg $1" >&2; exit 2;;
  esac
done

if [[ -z "$PROMISE" || -z "$BRIEF_FILE" ]]; then
  echo "worker: --promise and --brief-file are required" >&2; exit 2
fi
if [[ ! -f "$BRIEF_FILE" ]]; then
  echo "worker: brief file not found: $BRIEF_FILE" >&2; exit 2
fi

export ANTHROPIC_MODEL="$MODEL"

SAFE="$(printf '%s' "$PROMISE" | tr -c 'a-zA-Z0-9._-' '_')"
WORKTREE="/tmp/vf-${SAFE}"
BRANCH="vertex-fleet/${SAFE}"
LOG_DIR="/tmp/vf-logs"
mkdir -p "$LOG_DIR"
COST_LOG="${LOG_DIR}/${SAFE}.cost.json"
AGENT_LOG="${LOG_DIR}/${SAFE}.agent.log"

# Full-trajectory trace (stream-json events): persisted + indexed for later traversal.
RUN_ID="$(date +%Y%m%dT%H%M%S)-$$"
VF_TRACE_ROOT="${VF_TRACE_DIR:-$HOME/work/vertex-fleet-traces}"
TRACE_DIR="${VF_TRACE_ROOT}/$(date +%Y-%m-%d)"
mkdir -p "$TRACE_DIR"
TRACE="${TRACE_DIR}/${SAFE}.${RUN_ID}.trace.jsonl"
TRACE_INDEX="${VF_TRACE_ROOT}/index.jsonl"

log() { echo "[worker:${PROMISE}] $*" >&2; }

emit() { # emit final JSON result line
  printf '%s\n' "$1"
}

result_json() {
  local status="$1" pr="$2" check="$3" cost="$4" note="$5"
  # note is already JSON-safe (we pass plain ascii)
  printf '{"promise":"%s","model":"%s","branch":"%s","status":"%s","pr_url":"%s","check_deploy":"%s","cost_usd":%s,"note":"%s"}' \
    "$PROMISE" "$MODEL" "$BRANCH" "$status" "$pr" "$check" "${cost:-null}" "$note"
}

# Find the source repo (this script lives inside a checkout). We add the
# worktree against origin/main so the agent starts from a clean, current base.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_REPO="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$SRC_REPO" ]]; then
  log "FATAL: not inside a git checkout"
  emit "$(result_json error "" skipped null "not_in_git_checkout")"; exit 1
fi

log "src repo: $SRC_REPO"
log "model:    $MODEL (Vertex, region=$CLOUD_ML_REGION, project=$ANTHROPIC_VERTEX_PROJECT_ID)"
log "worktree: $WORKTREE  branch: $BRANCH"

# Fresh worktree from origin/main on a new branch.
# Serialize git-worktree setup: concurrent `git worktree add` on one repo races
# on git's index/worktree locks (the real cause of parallel worktree_add_failed).
# Agent runs stay parallel; only this fast setup is mutually exclusive (mkdir = atomic, macOS-safe).
VF_GIT_LOCK="${SRC_REPO}/.git/vf-worktree.lock"
_vf_n=0
while ! mkdir "$VF_GIT_LOCK" 2>/dev/null; do sleep 0.5; _vf_n=$((_vf_n+1)); [ "$_vf_n" -gt 600 ] && { log "WARN: worktree lock wait timeout; proceeding"; break; }; done
git -C "$SRC_REPO" fetch origin main --quiet 2>>"$AGENT_LOG" || true
git -C "$SRC_REPO" worktree remove "$WORKTREE" --force >/dev/null 2>&1 || true
git -C "$SRC_REPO" worktree prune >/dev/null 2>&1 || true
git -C "$SRC_REPO" branch -D "$BRANCH" >/dev/null 2>&1 || true
git -C "$SRC_REPO" worktree add -b "$BRANCH" "$WORKTREE" "$BASE" >>"$AGENT_LOG" 2>&1
VF_ADD_RC=$?
rmdir "$VF_GIT_LOCK" 2>/dev/null || true
if [ "$VF_ADD_RC" != 0 ]; then
  log "FATAL: could not create worktree (rc=$VF_ADD_RC)"
  emit "$(result_json error "" skipped null "worktree_add_failed")"; exit 1
fi

cd "$WORKTREE" || { emit "$(result_json error "" skipped null "cd_worktree_failed")"; exit 1; }

# Pin the base to a fixed SHA. BASE (e.g. "origin/main") is a moving ref: a
# concurrent push or a later `git fetch` advances it mid-run, which would make
# the ahead-of-base count below misfire and a real commit look like no_changes.
BASE_SHA="$(git rev-parse HEAD)"
log "base pinned: ${BASE_SHA}"

# Install deps so check:deploy can run. Reuse bun's global cache (fast).
log "installing deps (bun install)..."
bun install >>"$AGENT_LOG" 2>&1 || log "WARN: bun install returned nonzero (continuing)"

if [[ "$DRY_RUN" == "1" ]]; then
  log "DRY RUN: skipping claude agent; brief is:"
  sed 's/^/    /' "$BRIEF_FILE" >&2
  emit "$(result_json dry_run "" skipped 0 "dry_run")"; exit 0
fi

# ---- Run the Vertex-powered claude agent ------------------------------------
BRIEF="$(cat "$BRIEF_FILE")"
log "running claude --bare -p (acceptEdits) on Vertex..."
set +e
claude --bare -p "$BRIEF" \
  --permission-mode acceptEdits \
  --allowedTools "Bash,Read,Edit,Write" \
  --output-format stream-json --verbose \
  >"$TRACE" 2>>"$AGENT_LOG"
AGENT_RC=$?
set -e 2>/dev/null || true

# Extract cost from claude's JSON result (--output-format json -> total_cost_usd).
COST="null"
if [[ -s "$TRACE" ]]; then
  COST="$(node -e '
    try {
      const fs=require("fs");
      const lines=fs.readFileSync(process.argv[1],"utf8").trim().split("\n");
      let c=null;
      for (const l of lines) { try { const j=JSON.parse(l); if (j.type==="result" && (j.total_cost_usd!=null||j.cost_usd!=null)) c=j.total_cost_usd??j.cost_usd; } catch(e){} }
      process.stdout.write(c==null?"null":String(c));
    } catch(e){ process.stdout.write("null"); }
  ' "$TRACE" 2>/dev/null || echo null)"
fi
log "agent rc=$AGENT_RC cost_usd=$COST  trace=$TRACE"
# Index this run for later traversal.
printf '{"promise":"%s","model":"%s","run_id":"%s","branch":"%s","trace":"%s","agent_rc":%s,"cost_usd":%s,"ts":"%s"}\n' \
  "$PROMISE" "$MODEL" "$RUN_ID" "$BRANCH" "$TRACE" "$AGENT_RC" "${COST:-null}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$TRACE_INDEX" 2>/dev/null || true

if [[ "$AGENT_RC" != "0" ]]; then
  log "agent failed (rc=$AGENT_RC); see $AGENT_LOG"
  emit "$(result_json agent_failed "" skipped "$COST" "agent_rc_${AGENT_RC}")"; exit 0
fi

# ---- Did the agent actually change anything? --------------------------------
if git diff --quiet HEAD 2>/dev/null && [[ -z "$(git status --porcelain)" ]]; then
  # Maybe the agent already committed. Check for commits beyond base.
  AHEAD="$(git rev-list --count "${BASE_SHA}..HEAD" 2>/dev/null || echo 0)"
  if [[ "$AHEAD" == "0" ]]; then
    log "agent produced no changes"
    emit "$(result_json no_changes "" skipped "$COST" "agent_made_no_changes")"; exit 0
  fi
fi

# Commit anything the agent left uncommitted.
if [[ -n "$(git status --porcelain)" ]]; then
  git add -A
  git commit -q -m "vertex-fleet(${PROMISE}): agent changes (${MODEL} on Vertex)" || true
fi

AHEAD="$(git rev-list --count "${BASE_SHA}..HEAD" 2>/dev/null || echo 0)"
if [[ "$AHEAD" == "0" ]]; then
  log "no commits ahead of base after commit attempt"
  emit "$(result_json no_changes "" skipped "$COST" "no_commits_ahead")"; exit 0
fi

# ---- check:deploy (the merge gate) ------------------------------------------
log "running check:deploy..."
CHECK="fail"
if bun run check:deploy >>"$AGENT_LOG" 2>&1; then
  CHECK="pass"
  log "check:deploy PASSED"
else
  CHECK="fail"
  log "check:deploy FAILED (see $AGENT_LOG)"
fi

# ---- push branch + open PR (PR-per-agent; NEVER main) ------------------------
if [[ "$OPEN_PR" != "1" ]]; then
  emit "$(result_json built_no_pr "" "$CHECK" "$COST" "pr_skipped_by_flag")"; exit 0
fi

log "pushing branch $BRANCH..."
if ! git push -u origin "$BRANCH" --force-with-lease >>"$AGENT_LOG" 2>&1; then
  log "branch push failed"
  emit "$(result_json push_failed "" "$CHECK" "$COST" "branch_push_failed")"; exit 0
fi

PR_TITLE="vertex-fleet: advance ${PROMISE}"
PR_BODY="Automated PR opened by a Vertex-powered \`claude -p\` agent (model: ${MODEL}, region: ${CLOUD_ML_REGION}).

Promise: \`${PROMISE}\`

check:deploy: **${CHECK}**

Guardrails honored by the worker: PR-per-agent (branch only, never main), no green flips, isolated worktree. Review before merge.

DO NOT MERGE automatically — owner/Raynor reviews and merges.

🤖 Generated with [Claude Code](https://claude.com/claude-code)"

set +e
PR_URL="$(gh pr create --title "$PR_TITLE" --body "$PR_BODY" --base main --head "$BRANCH" --repo OpenAgentsInc/openagents 2>>"$AGENT_LOG")"
PR_RC=$?
set -e 2>/dev/null || true

if [[ "$PR_RC" != "0" || -z "$PR_URL" ]]; then
  log "gh pr create failed (rc=$PR_RC)"
  emit "$(result_json pr_failed "" "$CHECK" "$COST" "gh_pr_create_failed")"; exit 0
fi

log "PR opened: $PR_URL"
emit "$(result_json pr_open "$PR_URL" "$CHECK" "$COST" "ok")"
