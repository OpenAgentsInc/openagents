#!/usr/bin/env bash
# worker.sh — one Codex (gpt-5.5) agent works one promise in an isolated git
# worktree, runs check:deploy, commits to a BRANCH, pushes the branch, opens a PR.
#
# PR-PER-AGENT: this script NEVER pushes to main. It pushes codex-fleet/<promise>
# and opens a PR for human review. Same PR shape as the (retired) vertex-fleet so
# the existing gate /tmp/fleet-merge.sh still gates it (branch prefix only differs).
#
# Auth: Codex rides OUR ChatGPT/Codex SUBSCRIPTION. There is NO per-machine
# interactive `codex login`. Instead, scripts/codex-fleet/fetch-codex-auth.mjs
# pulls a Codex OAuth blob from the CENTRAL device-flow provider-account store in
# openagents.com (collected once via the device-code ceremony) and materializes a
# codex-native auth.json under a PER-PROMISE isolated CODEX_HOME. See:
#   scripts/codex-fleet/README.md
#   apps/openagents.com/docs/2026-06-05-chatgpt-device-login-operator-runbook.md
#
# Required env (names only; no secret values printed):
#   OPENAGENTS_ADMIN_API_TOKEN   admin/operator bearer (central lease + grant issue)
#   OPENAGENTS_AGENT_TOKEN       programmatic-agent bearer (grant resolve w/ material)
# Optional:
#   OPENAGENTS_BASE_URL          default https://openagents.com
#   OPENAGENTS_FLEET_EMAIL       default chris@openagents.com
#
# Usage:
#   worker.sh --promise <promiseId> --brief-file <path> [--model <codex model>]
#             [--base origin/main] [--no-pr] [--dry-run]
#
# Emits a single-line JSON result to stdout (last line) and human logs to stderr.

set -uo pipefail

PROMISE=""
BRIEF_FILE=""
MODEL="gpt-5.5"
REASONING_EFFORT="xhigh"
BASE="origin/main"
OPEN_PR=1
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --promise) PROMISE="$2"; shift 2;;
    --brief-file) BRIEF_FILE="$2"; shift 2;;
    --model) MODEL="$2"; shift 2;;
    --reasoning-effort) REASONING_EFFORT="$2"; shift 2;;
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

SAFE="$(printf '%s' "$PROMISE" | tr -c 'a-zA-Z0-9._-' '_')"
WORKTREE="/tmp/cf-${SAFE}"
BRANCH="codex-fleet/${SAFE}"
LOG_DIR="/tmp/cf-logs"
mkdir -p "$LOG_DIR"
AGENT_LOG="${LOG_DIR}/${SAFE}.agent.log"

# Per-promise isolated CODEX_HOME so concurrent workers never share/overwrite
# each other's auth.json (the central store hands one account per lease).
CODEX_HOME_DIR="/tmp/cf-codex-home-${SAFE}"
LAST_MESSAGE_FILE="${LOG_DIR}/${SAFE}.last.txt"

# Full-trajectory trace (codex exec --json events): persisted + indexed.
RUN_ID="$(date +%Y%m%dT%H%M%S)-$$"
CF_TRACE_ROOT="${CF_TRACE_DIR:-$HOME/work/codex-fleet-traces}"
TRACE_DIR="${CF_TRACE_ROOT}/$(date +%Y-%m-%d)"
mkdir -p "$TRACE_DIR"
TRACE="${TRACE_DIR}/${SAFE}.${RUN_ID}.trace.jsonl"
TRACE_INDEX="${CF_TRACE_ROOT}/index.jsonl"

FETCH_AUTH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/fetch-codex-auth.mjs"
RG_GUARD_INSTALLER="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/install-rg-guard.mjs"
LEASE_REF=""

log() { echo "[worker:${PROMISE}] $*" >&2; }

emit() { printf '%s\n' "$1"; }

result_json() {
  local result_status="$1" pr="$2" check="$3" tokens="$4" note="$5"
  printf '{"promise":"%s","model":"%s","branch":"%s","status":"%s","pr_url":"%s","check_deploy":"%s","tokens":%s,"note":"%s"}' \
    "$PROMISE" "$MODEL" "$BRANCH" "$result_status" "$pr" "$check" "${tokens:-null}" "$note"
}

release_lease() {
  # Best-effort lease release; never fail the worker on cleanup.
  if [[ -n "$LEASE_REF" ]]; then
    node "$FETCH_AUTH" release --leaseRef "$LEASE_REF" --status "${1:-released}" \
      >>"$AGENT_LOG" 2>&1 || true
    LEASE_REF=""
  fi
}
trap 'release_lease released' EXIT

# Find the source repo (this script lives inside a checkout). We add the
# worktree against origin/main so the agent starts from a clean, current base.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_REPO="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$SRC_REPO" ]]; then
  log "FATAL: not inside a git checkout"
  emit "$(result_json error "" skipped null "not_in_git_checkout")"; exit 1
fi

log "src repo: $SRC_REPO"
log "model:    $MODEL (Codex subscription via central device-flow auth)"
log "worktree: $WORKTREE  branch: $BRANCH"

# Fresh worktree from origin/main on a new branch.
# Serialize git-worktree setup: concurrent `git worktree add` on one repo races
# on git's index/worktree locks. Agent runs stay parallel; only this fast setup
# is mutually exclusive (mkdir = atomic, macOS-safe). Same lock file name as the
# retired vertex-fleet so the two never run a concurrent worktree add together.
CF_GIT_LOCK="${SRC_REPO}/.git/vf-worktree.lock"
_cf_n=0
while ! mkdir "$CF_GIT_LOCK" 2>/dev/null; do sleep 0.5; _cf_n=$((_cf_n+1)); [ "$_cf_n" -gt 600 ] && { log "WARN: worktree lock wait timeout; proceeding"; break; }; done
git -C "$SRC_REPO" fetch origin main --quiet 2>>"$AGENT_LOG" || true
git -C "$SRC_REPO" worktree remove "$WORKTREE" --force >/dev/null 2>&1 || true
git -C "$SRC_REPO" worktree prune >/dev/null 2>&1 || true
git -C "$SRC_REPO" branch -D "$BRANCH" >/dev/null 2>&1 || true
git -C "$SRC_REPO" worktree add -b "$BRANCH" "$WORKTREE" "$BASE" >>"$AGENT_LOG" 2>&1
CF_ADD_RC=$?
rmdir "$CF_GIT_LOCK" 2>/dev/null || true
if [ "$CF_ADD_RC" != 0 ]; then
  log "FATAL: could not create worktree (rc=$CF_ADD_RC)"
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
  log "DRY RUN: skipping codex agent; brief is:"
  sed 's/^/    /' "$BRIEF_FILE" >&2
  emit "$(result_json dry_run "" skipped 0 "dry_run")"; exit 0
fi

# ---- Fetch Codex auth from the CENTRAL device-flow store ---------------------
# Writes a codex-native auth.json into the per-promise CODEX_HOME. Captures the
# leaseRef (public) so we can release the lease on exit. Never prints material.
log "fetching Codex auth from central device-flow store..."
export CODEX_HOME="$CODEX_HOME_DIR"
mkdir -p "$CODEX_HOME"
set +e
LEASE_OUT="$(node "$FETCH_AUTH" lease \
  --action codex_fleet_promise_work \
  --assignmentId "$SAFE" \
  --runId "$RUN_ID" 2>>"$AGENT_LOG")"
FETCH_RC=$?
set -e 2>/dev/null || true
if [ "$FETCH_RC" != 0 ]; then
  log "FATAL: central Codex auth fetch failed (rc=$FETCH_RC); see $AGENT_LOG"
  emit "$(result_json auth_failed "" skipped null "central_auth_fetch_failed")"; exit 0
fi
LEASE_REF="$(printf '%s' "$LEASE_OUT" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{process.stdout.write(JSON.parse(s.trim().split("\n").pop()||"{}").leaseRef||"")}catch{process.stdout.write("")}})')"
if [[ ! -s "$CODEX_HOME/auth.json" ]]; then
  log "FATAL: auth.json was not materialized under CODEX_HOME"
  emit "$(result_json auth_failed "" skipped null "auth_json_missing")"; exit 0
fi
REAL_RG="$(command -v rg 2>/dev/null || true)"
if [[ -n "$REAL_RG" && -x "$REAL_RG" ]]; then
  RG_GUARD_BIN="${CODEX_HOME_DIR}/rg-guard-bin"
  if node "$RG_GUARD_INSTALLER" --bin-dir "$RG_GUARD_BIN" --real-rg "$REAL_RG" >>"$AGENT_LOG" 2>&1; then
    export OPENAGENTS_CODEX_REAL_RG="$REAL_RG"
    export OPENAGENTS_CODEX_RG_GUARD=1
    case ":${PATH:-}:" in
      *":${RG_GUARD_BIN}:"*) ;;
      *) export PATH="${RG_GUARD_BIN}${PATH:+:${PATH}}" ;;
    esac
    log "ripgrep guard installed for Codex agent"
  else
    log "WARN: ripgrep guard install failed; continuing without rg guard"
  fi
else
  log "ripgrep guard skipped: rg not on PATH"
fi
log "Codex auth materialized (lease acquired); running agent..."

# ---- Run the Codex agent non-interactively -----------------------------------
# Mirrors the operator's alias: gpt-5.5, xhigh reasoning, bypass approvals+sandbox
# (the worktree is the externally-managed sandbox). --json streams trace events.
BRIEF="$(cat "$BRIEF_FILE")"
log "running codex exec (model=$MODEL, reasoning=$REASONING_EFFORT)..."
set +e
# NOTE: stdin MUST be /dev/null. `codex exec` appends piped stdin to the prompt;
# in a non-interactive/background context an open stdin makes it block forever
# ("Reading additional input from stdin..."). Closing stdin avoids that hang.
codex exec "$BRIEF" \
  -m "$MODEL" \
  -c model_reasoning_effort="$REASONING_EFFORT" \
  --dangerously-bypass-approvals-and-sandbox \
  --skip-git-repo-check \
  -C "$WORKTREE" \
  --json \
  --output-last-message "$LAST_MESSAGE_FILE" \
  </dev/null >"$TRACE" 2>>"$AGENT_LOG"
AGENT_RC=$?
set -e 2>/dev/null || true

# Extract token usage from codex's JSONL trace (token_count events carry usage).
TOKENS="null"
if [[ -s "$TRACE" ]]; then
  TOKENS="$(node -e '
    try {
      const fs=require("fs");
      const lines=fs.readFileSync(process.argv[1],"utf8").trim().split("\n");
      let inp=0,out=0,total=0,seen=false;
      for (const l of lines) {
        let j; try { j=JSON.parse(l); } catch { continue; }
        // codex emits token usage under a few shapes across versions; scan generically.
        const u = j?.msg?.info?.total_token_usage ?? j?.info?.total_token_usage ?? j?.usage ?? j?.token_usage ?? j?.msg?.usage;
        if (u && typeof u==="object") {
          seen=true;
          const i = u.input_tokens ?? u.prompt_tokens ?? 0;
          const o = u.output_tokens ?? u.completion_tokens ?? 0;
          const t = u.total_tokens ?? (Number(i)+Number(o));
          inp=Number(i)||inp; out=Number(o)||out; total=Number(t)||total;
        }
      }
      process.stdout.write(seen?JSON.stringify({input:inp,output:out,total:total}):"null");
    } catch(e){ process.stdout.write("null"); }
  ' "$TRACE" 2>/dev/null || echo null)"
fi
log "agent rc=$AGENT_RC tokens=$TOKENS  trace=$TRACE"
# Index this run for later traversal.
printf '{"promise":"%s","model":"%s","run_id":"%s","branch":"%s","trace":"%s","agent_rc":%s,"tokens":%s,"ts":"%s"}\n' \
  "$PROMISE" "$MODEL" "$RUN_ID" "$BRANCH" "$TRACE" "$AGENT_RC" "${TOKENS:-null}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$TRACE_INDEX" 2>/dev/null || true

# We are done with the subscription seat for this promise; release the lease now
# so other workers / accounts are not starved while we run check:deploy + push.
release_lease succeeded

if [[ "$AGENT_RC" != "0" ]]; then
  log "agent failed (rc=$AGENT_RC); see $AGENT_LOG"
  emit "$(result_json agent_failed "" skipped "$TOKENS" "agent_rc_${AGENT_RC}")"; exit 0
fi

# ---- Did the agent actually change anything? --------------------------------
if git diff --quiet HEAD 2>/dev/null && [[ -z "$(git status --porcelain)" ]]; then
  AHEAD="$(git rev-list --count "${BASE_SHA}..HEAD" 2>/dev/null || echo 0)"
  if [[ "$AHEAD" == "0" ]]; then
    log "agent produced no changes"
    emit "$(result_json no_changes "" skipped "$TOKENS" "agent_made_no_changes")"; exit 0
  fi
fi

# Commit anything the agent left uncommitted.
if [[ -n "$(git status --porcelain)" ]]; then
  git add -A
  git commit -q -m "codex-fleet(${PROMISE}): agent changes (${MODEL} via subscription)" || true
fi

AHEAD="$(git rev-list --count "${BASE_SHA}..HEAD" 2>/dev/null || echo 0)"
if [[ "$AHEAD" == "0" ]]; then
  log "no commits ahead of base after commit attempt"
  emit "$(result_json no_changes "" skipped "$TOKENS" "no_commits_ahead")"; exit 0
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
  emit "$(result_json built_no_pr "" "$CHECK" "$TOKENS" "pr_skipped_by_flag")"; exit 0
fi

log "pushing branch $BRANCH..."
if ! git push -u origin "$BRANCH" --force-with-lease >>"$AGENT_LOG" 2>&1; then
  log "branch push failed"
  emit "$(result_json push_failed "" "$CHECK" "$TOKENS" "branch_push_failed")"; exit 0
fi

PR_TITLE="codex-fleet: advance ${PROMISE}"
PR_BODY="Automated PR opened by a Codex (\`codex exec\`) agent running on the OpenAgents ChatGPT/Codex subscription (model: ${MODEL}).

Promise: \`${PROMISE}\`

check:deploy: **${CHECK}**

Auth: the worker pulled a Codex OAuth token from the CENTRAL device-flow provider-account store in openagents.com (no per-machine interactive login). Guardrails honored: PR-per-agent (branch only, never main), no green flips, isolated worktree, per-promise CODEX_HOME, lease released after the run. Review before merge.

DO NOT MERGE automatically — owner/Raynor reviews and merges.

🤖 Generated with [Claude Code](https://claude.com/claude-code)"

set +e
PR_URL="$(gh pr create --title "$PR_TITLE" --body "$PR_BODY" --base main --head "$BRANCH" --repo OpenAgentsInc/openagents 2>>"$AGENT_LOG")"
PR_RC=$?
set -e 2>/dev/null || true

if [[ "$PR_RC" != "0" || -z "$PR_URL" ]]; then
  log "gh pr create failed (rc=$PR_RC)"
  emit "$(result_json pr_failed "" "$CHECK" "$TOKENS" "gh_pr_create_failed")"; exit 0
fi

log "PR opened: $PR_URL"
emit "$(result_json pr_open "$PR_URL" "$CHECK" "$TOKENS" "ok")"
