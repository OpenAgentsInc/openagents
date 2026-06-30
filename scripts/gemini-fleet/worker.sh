#!/usr/bin/env bash
# worker.sh — one opencode agent driven by a GEMINI model on Google Vertex AI
# works one promise in an isolated worktree, runs check:deploy, commits to a
# BRANCH, pushes the branch, opens a PR.
#
# PR-PER-AGENT: this script NEVER pushes to main. It pushes gemini-fleet/<promise>
# and opens a PR for human review. Same PR shape as scripts/vertex-fleet, so
# /tmp/fleet-merge.sh gates these identically.
#
# Engine:  opencode (provider-agnostic coding agent), run HEADLESS via
#          `opencode run -m google-vertex/<gemini model> "<brief>"`.
# Model:   a first-party Google Gemini model on Vertex AI (default
#          gemini-2.5-pro). First-party Google SKU => billed to the GFS cloud
#          credit, NOT cards.
# Auth:    Google Cloud Application Default Credentials (ADC). opencode's
#          google-vertex provider auto-loads when GOOGLE_VERTEX_PROJECT /
#          GOOGLE_CLOUD_PROJECT is set and signs requests with
#          google-auth-library (cloud-platform scope). Locally that's the
#          owner's `gcloud auth application-default` creds; on an unattended GCE
#          instance the same code path uses the service-account metadata ADC
#          (no reauth). NO key material is read or printed by this script.
#
# Usage:
#   worker.sh --promise <promiseId> --brief-file <path>
#             [--model <gemini model, e.g. gemini-2.5-pro>]
#             [--base origin/main] [--no-pr] [--dry-run]
#
# Emits a single-line JSON result to stdout (last line) and human logs to stderr.

set -uo pipefail

# ---- Vertex / Gemini env (names only; no secret values) ----------------------
# opencode google-vertex provider reads these. We export both the
# GOOGLE_VERTEX_* names (what models.dev advertises) and the wider GOOGLE_CLOUD_*
# names so existing ADC setups autoload regardless of which the provider checks.
export GOOGLE_VERTEX_PROJECT="${GOOGLE_VERTEX_PROJECT:-${GOOGLE_CLOUD_PROJECT:-openagentsgemini}}"
export GOOGLE_CLOUD_PROJECT="${GOOGLE_CLOUD_PROJECT:-$GOOGLE_VERTEX_PROJECT}"
# global endpoint = better availability at no extra cost; us-central1 is the
# safe default for Gemini availability. Keep both names in sync.
export GOOGLE_VERTEX_LOCATION="${GOOGLE_VERTEX_LOCATION:-${VERTEX_LOCATION:-us-central1}}"
export VERTEX_LOCATION="${VERTEX_LOCATION:-$GOOGLE_VERTEX_LOCATION}"

PROMISE=""
BRIEF_FILE=""
MODEL="gemini-2.5-pro"
BASE="origin/main"
OPEN_PR=1
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --promise) PROMISE="$2"; shift 2;;
    --brief-file) BRIEF_FILE="$2"; shift 2;;
    --model) MODEL="$2"; shift 2;;
    --base) BASE="$2"; shift 2;;
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

# opencode model spec: provider/model. The Gemini models live under google-vertex.
# Accept a bare model id (gemini-2.5-pro) OR a fully-qualified one (google-vertex/...).
if [[ "$MODEL" == */* ]]; then
  OC_MODEL="$MODEL"
else
  OC_MODEL="google-vertex/${MODEL}"
fi

SAFE="$(printf '%s' "$PROMISE" | tr -c 'a-zA-Z0-9._-' '_')"
WORKTREE="/tmp/gf-${SAFE}"
BRANCH="gemini-fleet/${SAFE}"
LOG_DIR="/tmp/gf-logs"
mkdir -p "$LOG_DIR"
AGENT_LOG="${LOG_DIR}/${SAFE}.agent.log"

# Full-trajectory trace (opencode --format json events): persisted + indexed.
RUN_ID="$(date +%Y%m%dT%H%M%S)-$$"
GF_TRACE_ROOT="${GF_TRACE_DIR:-$HOME/work/gemini-fleet-traces}"
TRACE_DIR="${GF_TRACE_ROOT}/$(date +%Y-%m-%d)"
mkdir -p "$TRACE_DIR"
TRACE="${TRACE_DIR}/${SAFE}.${RUN_ID}.trace.jsonl"
TRACE_INDEX="${GF_TRACE_ROOT}/index.jsonl"

log() { echo "[worker:${PROMISE}] $*" >&2; }

emit() { # emit final JSON result line
  printf '%s\n' "$1"
}

result_json() {
  local result_status="$1" pr="$2" check="$3" cost="$4" note="$5"
  printf '{"promise":"%s","engine":"opencode","model":"%s","branch":"%s","status":"%s","pr_url":"%s","check_deploy":"%s","cost_usd":%s,"note":"%s"}' \
    "$PROMISE" "$OC_MODEL" "$BRANCH" "$result_status" "$pr" "$check" "${cost:-null}" "$note"
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
log "engine:   opencode ($(command -v opencode 2>/dev/null || echo 'not on PATH'))"
log "model:    $OC_MODEL (Vertex, location=$GOOGLE_VERTEX_LOCATION, project=$GOOGLE_VERTEX_PROJECT)"
log "worktree: $WORKTREE  branch: $BRANCH"

if ! command -v opencode >/dev/null 2>&1; then
  log "FATAL: opencode not on PATH"
  emit "$(result_json error "" skipped null "opencode_not_installed")"; exit 1
fi

# Fresh worktree from origin/main on a new branch.
# Serialize git-worktree setup: concurrent `git worktree add` on one repo races
# on git's index/worktree locks. Agent runs stay parallel; only this fast setup
# is mutually exclusive (mkdir = atomic, macOS-safe).
#
# The lock BLOCKS until acquired (no "proceed on timeout" — that caused the
# concurrent add that 8/9 workers failed on). The fetch + index-touching steps
# AND the `git worktree add` all run under the lock; we release immediately
# after the add. Bound is generous (~120s) so a slow add by one worker never
# starves the others; if we somehow can't acquire in that window we fail loudly
# rather than racing.
GF_GIT_LOCK="${SRC_REPO}/.git/gf-worktree.lock"
gf_lock_acquire() {
  local _n=0
  # 240 * 0.5s = 120s blocking bound. mkdir is atomic; a stale lock from a
  # killed worker is the only way this spins forever, so we cap and fail.
  while ! mkdir "$GF_GIT_LOCK" 2>/dev/null; do
    sleep 0.5; _n=$((_n+1))
    [ "$_n" -gt 240 ] && return 1
  done
  return 0
}
gf_lock_release() { rmdir "$GF_GIT_LOCK" 2>/dev/null || true; }

# Everything inside the lock: serialize the index/worktree-touching git steps.
gf_worktree_add() {
  git -C "$SRC_REPO" fetch origin main --quiet 2>>"$AGENT_LOG" || true
  git -C "$SRC_REPO" worktree remove "$WORKTREE" --force >/dev/null 2>&1 || true
  git -C "$SRC_REPO" worktree prune >/dev/null 2>&1 || true
  git -C "$SRC_REPO" branch -D "$BRANCH" >/dev/null 2>&1 || true
  git -C "$SRC_REPO" worktree add -b "$BRANCH" "$WORKTREE" "$BASE" >>"$AGENT_LOG" 2>&1
}

if ! gf_lock_acquire; then
  log "FATAL: could not acquire worktree lock within bound"
  emit "$(result_json error "" skipped null "worktree_lock_timeout")"; exit 1
fi
gf_worktree_add
GF_ADD_RC=$?
if [ "$GF_ADD_RC" != 0 ]; then
  # Transient failure (stale worktree admin state, leftover lock). Prune and
  # retry ONCE, still under the lock, before declaring failure.
  log "WARN: worktree add failed (rc=$GF_ADD_RC); pruning + one retry under lock"
  git -C "$SRC_REPO" worktree prune >/dev/null 2>&1 || true
  gf_worktree_add
  GF_ADD_RC=$?
fi
gf_lock_release
if [ "$GF_ADD_RC" != 0 ]; then
  log "FATAL: could not create worktree (rc=$GF_ADD_RC)"
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
  log "DRY RUN: skipping opencode agent; brief is:"
  sed 's/^/    /' "$BRIEF_FILE" >&2
  emit "$(result_json dry_run "" skipped 0 "dry_run")"; exit 0
fi

# ---- Run the Gemini-on-Vertex opencode agent (HEADLESS) ----------------------
# `opencode run` is the non-interactive headless path. We:
#   --model google-vertex/<gemini>   pick the first-party Gemini model on Vertex
#   --format json                    emit raw JSON events -> the trace file
#   --dangerously-skip-permissions   auto-approve edits/bash so the agent can
#                                    actually do work unattended (opencode
#                                    defaults to allow, but this is explicit and
#                                    survives any restrictive global config)
# opencode runs IN the worktree because we cd'd into it above (it uses cwd).
BRIEF="$(cat "$BRIEF_FILE")"
log "running opencode run (headless, --format json) on $OC_MODEL..."
set +e
opencode run \
  --model "$OC_MODEL" \
  --format json \
  --dangerously-skip-permissions \
  "$BRIEF" \
  >"$TRACE" 2>>"$AGENT_LOG"
AGENT_RC=$?
set -e 2>/dev/null || true

# Extract cost from opencode's JSON event stream. Assistant message.updated
# events carry properties.info.cost (USD). Sum the max-seen per message; if
# models.dev has no Gemini pricing yet, cost is 0 (still billed to the GFS
# credit on the Vertex side, not to this number).
COST="null"
if [[ -s "$TRACE" ]]; then
  COST="$(node -e '
    try {
      const fs=require("fs");
      const lines=fs.readFileSync(process.argv[1],"utf8").trim().split("\n");
      const seen=new Map();
      for (const l of lines) {
        try {
          const j=JSON.parse(l);
          const info = j?.properties?.info ?? j?.info;
          if (info && info.role==="assistant" && typeof info.cost==="number") {
            seen.set(info.id ?? Math.random(), info.cost);
          }
        } catch(e){}
      }
      let c=0; for (const v of seen.values()) c+=v;
      process.stdout.write(seen.size? String(c) : "null");
    } catch(e){ process.stdout.write("null"); }
  ' "$TRACE" 2>/dev/null || echo null)"
fi
log "agent rc=$AGENT_RC cost_usd=$COST  trace=$TRACE"
# Index this run for later traversal.
printf '{"promise":"%s","engine":"opencode","model":"%s","run_id":"%s","branch":"%s","trace":"%s","agent_rc":%s,"cost_usd":%s,"ts":"%s"}\n' \
  "$PROMISE" "$OC_MODEL" "$RUN_ID" "$BRANCH" "$TRACE" "$AGENT_RC" "${COST:-null}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$TRACE_INDEX" 2>/dev/null || true

if [[ "$AGENT_RC" != "0" ]]; then
  log "agent failed (rc=$AGENT_RC); see $AGENT_LOG"
  emit "$(result_json agent_failed "" skipped "$COST" "agent_rc_${AGENT_RC}")"; exit 0
fi

# ---- Did the agent actually change anything? --------------------------------
if git diff --quiet HEAD 2>/dev/null && [[ -z "$(git status --porcelain)" ]]; then
  AHEAD="$(git rev-list --count "${BASE_SHA}..HEAD" 2>/dev/null || echo 0)"
  if [[ "$AHEAD" == "0" ]]; then
    log "agent produced no changes"
    emit "$(result_json no_changes "" skipped "$COST" "agent_made_no_changes")"; exit 0
  fi
fi

# Commit anything the agent left uncommitted.
if [[ -n "$(git status --porcelain)" ]]; then
  git add -A
  git commit -q -m "gemini-fleet(${PROMISE}): agent changes (${OC_MODEL} via opencode on Vertex)" || true
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

PR_TITLE="gemini-fleet: advance ${PROMISE}"
PR_BODY="Automated PR opened by an opencode agent driven by a first-party Google Gemini model on Vertex AI (model: ${OC_MODEL}, location: ${GOOGLE_VERTEX_LOCATION}).

First-party Google SKU => billed to the GFS cloud credit, not cards.

Promise: \`${PROMISE}\`

check:deploy: **${CHECK}**

Guardrails honored by the worker: PR-per-agent (branch only, never main), no green flips, isolated worktree. Review before merge.

DO NOT MERGE automatically — owner/Raynor reviews and merges.

🤖 Generated with opencode + Gemini on GCP"

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
