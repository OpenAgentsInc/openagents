#!/usr/bin/env bash
# run.sh — orchestrator for the CODEX fleet (replaces the retired vertex-fleet).
#
# 1. assign: pick N non-green promises with buildable, non-owner-gated blockers.
# 2. fan out: run one worker.sh per promise (each = one `codex exec` agent on the
#    OpenAgents ChatGPT/Codex SUBSCRIPTION, in its own worktree on its own branch,
#    with a per-promise CODEX_HOME and a leased subscription seat).
# 3. report: print resulting PR URLs + per-worker check:deploy status + tokens.
#
# PR-PER-AGENT only. NO green flips. Workers push BRANCHES and open PRs; nothing
# touches main. Same PR shape as the retired vertex-fleet (branch prefix only
# differs: codex-fleet/<promise>), so the existing gate /tmp/fleet-merge.sh works.
#
# Auth note: this fleet uses OUR Codex subscription via the CENTRAL device-flow
# provider-account store. Workers do NOT spend pay-per-token Vertex money. They
# DO consume shared subscription quota and lease one Codex account per promise —
# keep waves modest. Required env (names only): OPENAGENTS_ADMIN_API_TOKEN,
# OPENAGENTS_AGENT_TOKEN. See scripts/codex-fleet/README.md.
#
# Usage:
#   run.sh [--count N] [--state red|yellow|planned|any]
#          [--model gpt-5.5] [--ids a,b,c] [--priority business|any]
#          [--parallel] [--dry-run] [--no-pr]
#
# Default: 3 workers, gpt-5.5, sequential, business-fulfillment priority.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COUNT=3
STATE="any"
MODEL="gpt-5.5"
IDS=""
PRIORITY="business"
PARALLEL=0
DRY_RUN=0
NO_PR=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --count) COUNT="$2"; shift 2;;
    --state) STATE="$2"; shift 2;;
    --model) MODEL="$2"; shift 2;;
    --ids) IDS="$2"; shift 2;;
    --priority) PRIORITY="$2"; shift 2;;
    --parallel) PARALLEL=1; shift;;
    --dry-run) DRY_RUN=1; shift;;
    --no-pr) NO_PR=1; shift;;
    *) echo "run: unknown arg $1" >&2; exit 2;;
  esac
done

OUT_DIR="/tmp/cf-assignments"
RESULTS="/tmp/cf-results.jsonl"
: > "$RESULTS"

echo "==> codex-fleet orchestrator" >&2
echo "    count=$COUNT state=$STATE model=$MODEL priority=$PRIORITY parallel=$PARALLEL dry_run=$DRY_RUN no_pr=$NO_PR" >&2

# 1. assignments
ASSIGN_ARGS=(--count "$COUNT" --state "$STATE" --model "$MODEL" --priority "$PRIORITY" --out "$OUT_DIR")
[[ -n "$IDS" ]] && ASSIGN_ARGS+=(--ids "$IDS")
node "$SCRIPT_DIR/assign.mjs" "${ASSIGN_ARGS[@]}" >/dev/null || { echo "run: assign failed" >&2; exit 1; }

# bash 3.2 compatible (macOS): no mapfile.
PROMISE_IDS=()
while IFS= read -r line; do
  [[ -n "$line" ]] && PROMISE_IDS+=("$line")
done < <(node -e '
  const a=require("'"$OUT_DIR"'/assignments.json");
  for (const x of a) console.log(x.promiseId);
')

if [[ "${#PROMISE_IDS[@]}" == "0" ]]; then
  echo "run: no assignments selected" >&2; exit 1
fi

echo "==> selected ${#PROMISE_IDS[@]} promise(s):" >&2
for p in "${PROMISE_IDS[@]}"; do echo "      - $p" >&2; done

run_one() {
  local promise="$1"
  local safe; safe="$(printf '%s' "$promise" | tr -c 'a-zA-Z0-9._-' '_')"
  local brief="$OUT_DIR/${safe}.brief.txt"
  local wargs=(--promise "$promise" --brief-file "$brief" --model "$MODEL")
  [[ "$DRY_RUN" == "1" ]] && wargs+=(--dry-run)
  [[ "$NO_PR" == "1" ]] && wargs+=(--no-pr)
  local line
  line="$(bash "$SCRIPT_DIR/worker.sh" "${wargs[@]}")"
  echo "$line" >> "$RESULTS"
  echo "$line"
}

# 2. fan out
if [[ "$PARALLEL" == "1" ]]; then
  echo "==> running ${#PROMISE_IDS[@]} workers in PARALLEL" >&2
  pids=()
  for p in "${PROMISE_IDS[@]}"; do run_one "$p" & pids+=($!); done
  for pid in "${pids[@]}"; do wait "$pid"; done
else
  echo "==> running ${#PROMISE_IDS[@]} workers SEQUENTIALLY" >&2
  for p in "${PROMISE_IDS[@]}"; do run_one "$p"; done
fi

# 3. report
echo "" >&2
echo "================ CODEX FLEET RESULTS ================" >&2
node -e '
  const fs=require("fs");
  const lines=fs.readFileSync("'"$RESULTS"'","utf8").trim().split("\n").filter(Boolean);
  let total=0, n=0;
  for (const l of lines) {
    let r; try { r=JSON.parse(l); } catch { console.error("  [bad result line]", l); continue; }
    let tok="n/a";
    if (r.tokens && typeof r.tokens==="object") { tok=`${r.tokens.total} (in ${r.tokens.input}/out ${r.tokens.output})`; total+=Number(r.tokens.total)||0; n++; }
    console.error(`  ${r.promise}`);
    console.error(`      status=${r.status}  check:deploy=${r.check_deploy}  tokens=${tok}`);
    console.error(`      PR=${r.pr_url||"(none)"}`);
  }
  console.error("  ---------------------------------------------------");
  console.error(`  workers=${lines.length}  with_tokens=${n}  total_tokens=${total}`);
'
echo "====================================================" >&2
echo "results: $RESULTS" >&2
