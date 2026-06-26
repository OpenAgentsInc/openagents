#!/usr/bin/env bash
#
# run-khala-tb2.sh — Terminal-Bench 2.0 black-box runner pointed at Khala (#6253)
#
# Stands up a Terminal-Bench 2.0 (Harbor) run that drives the `terminus-2`
# coding agent against the PUBLIC Khala API, consumed strictly as a black box:
#   - base URL https://openagents.com/api/v1
#   - model    openagents/khala
#   - auth     a free key from POST /api/keys/free (or $OPENAGENTS_API_KEY)
#
# It does NOT touch any gateway / GLM-serving / pylon / gym-harness code, and it
# does NOT touch the owner-armed full-89 run on Hydralisk. It uses its own
# isolated jobs dir and an explicit named-task subset, so it cannot collide with
# the broad live run.
#
# Output is public-safe: only aggregate counts + per-task reward + token totals
# are summarized to stdout / the summary JSON. Raw Harbor trajectories, prompts,
# and responses stay in the local jobs dir and are NOT committed.
#
# Honesty note (#6310 / #6319, Phase 0): the coding/tool-calling surface may be
# partially hard-down. If tool calls fail, Terminal-Bench scores will be low.
# This runner reports the measured number truthfully and surfaces error/timeout
# counts separately from honest non-solves — it never fabricates a score.
#
# Usage:
#   run-khala-tb2.sh [--tasks-file FILE] [--all] [--n-tasks N]
#                    [--concurrent N] [--include TASK]... [--model M]
#                    [--jobs-dir DIR] [--summary-out FILE] [--dry-run]
#
# Requirements: harbor (Terminal-Bench/Harbor CLI), docker (running), curl,
# python3. All are checked up front.

set -euo pipefail

# ---- defaults ---------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATASET="terminal-bench/terminal-bench-2"
AGENT="terminus-2"
MODEL="openai/openagents/khala"      # LiteLLM-style provider/model for Harbor
PUBLIC_MODEL="openagents/khala"
API_BASE="https://openagents.com/api/v1"
API_HOST="openagents.com"
CONCURRENT=2
MAX_TURNS=24
TASKS_FILE=""
RUN_ALL=0
N_TASKS=""
DRY_RUN=0
EXTRA_INCLUDES=()
TS="$(date -u +%Y%m%dT%H%M%SZ)"
JOBS_DIR=".tmp/terminalbench-6253-khala/jobs"
JOB_NAME="khala-tb2-${TS}"
SUMMARY_OUT="${SCRIPT_DIR}/last-run-summary.json"

# ---- arg parse --------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tasks-file) TASKS_FILE="$2"; shift 2 ;;
    --all)        RUN_ALL=1; shift ;;
    --n-tasks)    N_TASKS="$2"; shift 2 ;;
    --concurrent) CONCURRENT="$2"; shift 2 ;;
    --include)    EXTRA_INCLUDES+=("$2"); shift 2 ;;
    --model)      MODEL="$2"; shift 2 ;;
    --max-turns)  MAX_TURNS="$2"; shift 2 ;;
    --jobs-dir)   JOBS_DIR="$2"; shift 2 ;;
    --job-name)   JOB_NAME="$2"; shift 2 ;;
    --summary-out) SUMMARY_OUT="$2"; shift 2 ;;
    --dry-run)    DRY_RUN=1; shift ;;
    -h|--help)    sed -n '1,40p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# ---- preflight --------------------------------------------------------------
need() { command -v "$1" >/dev/null 2>&1 || { echo "MISSING DEP: $1" >&2; exit 3; }; }
need harbor; need curl; need python3
if [[ $DRY_RUN -eq 0 ]]; then
  need docker
  if ! docker info >/dev/null 2>&1; then
    echo "BLOCKED: docker daemon not reachable (Terminal-Bench needs containers)." >&2
    exit 4
  fi
fi

# ---- build the include list -------------------------------------------------
INCLUDES=()
if [[ $RUN_ALL -eq 1 ]]; then
  echo "[run-khala-tb2] --all: running the full official task set (no -i filter)."
else
  if [[ -n "$TASKS_FILE" ]]; then
    [[ -f "$TASKS_FILE" ]] || { echo "tasks file not found: $TASKS_FILE" >&2; exit 5; }
    while IFS= read -r line; do
      line="${line%%#*}"; line="$(echo "$line" | tr -d '[:space:]')"
      [[ -n "$line" ]] && INCLUDES+=("$line")
    done < "$TASKS_FILE"
  fi
  for t in "${EXTRA_INCLUDES[@]:-}"; do [[ -n "$t" ]] && INCLUDES+=("$t"); done
  if [[ ${#INCLUDES[@]} -eq 0 ]]; then
    echo "no tasks selected; pass --tasks-file, --include, or --all" >&2
    exit 5
  fi
  echo "[run-khala-tb2] selected ${#INCLUDES[@]} task(s): ${INCLUDES[*]}"
fi

# ---- acquire a Khala key as a black box -------------------------------------
KEY="${OPENAGENTS_API_KEY:-}"
if [[ -z "$KEY" ]]; then
  echo "[run-khala-tb2] requesting a free Khala key from POST /api/keys/free ..."
  KEY="$(curl -fsS -X POST "https://${API_HOST}/api/keys/free" \
          -H 'content-type: application/json' -d '{}' \
        | python3 -c 'import sys,json;print(json.load(sys.stdin)["credential"]["token"])')"
fi
[[ -n "$KEY" ]] || { echo "could not obtain a Khala key" >&2; exit 6; }
echo "[run-khala-tb2] key acquired (prefix ${KEY:0:16}...)."

# ---- assemble the harbor invocation -----------------------------------------
LLM_KWARGS='llm_call_kwargs={"extra_headers":{"x-openagents-demand-kind":"internal","x-openagents-demand-source":"harbor_terminal_bench","x-openagents-client":"tb2-6253-blackbox-runner"}}'

CMD=( harbor run
  --dataset "$DATASET"
  --agent "$AGENT"
  --model "$MODEL"
  --n-concurrent "$CONCURRENT"
  --jobs-dir "$JOBS_DIR"
  --job-name "$JOB_NAME"
  --yes
  --allow-agent-host "$API_HOST"
  --allow-environment-host "$API_HOST"
  --agent-kwarg "api_base=${API_BASE}"
  --agent-kwarg "max_turns=${MAX_TURNS}"
  --agent-kwarg "$LLM_KWARGS"
)
# Harbor's package task ids are namespaced as "terminal-bench/<name>"; the
# --include-task-name filter matches the full id, so prefix bare names.
for t in "${INCLUDES[@]:-}"; do
  case "$t" in
    */*) CMD+=( --include-task-name "$t" ) ;;
    *)   CMD+=( --include-task-name "terminal-bench/$t" ) ;;
  esac
done
[[ -n "$N_TASKS" ]] && CMD+=( --n-tasks "$N_TASKS" )

echo "[run-khala-tb2] jobs-dir: $JOBS_DIR"
echo "[run-khala-tb2] job-name: $JOB_NAME"
echo "[run-khala-tb2] command:"
printf '    %q ' "${CMD[@]}"; echo

if [[ $DRY_RUN -eq 1 ]]; then
  echo "[run-khala-tb2] --dry-run: not executing."
  exit 0
fi

# ---- run --------------------------------------------------------------------
export OPENAI_API_KEY="$KEY"
export OPENAI_BASE_URL="$API_BASE"
set +e
"${CMD[@]}"
HARBOR_RC=$?
set -e
echo "[run-khala-tb2] harbor exit code: $HARBOR_RC"

# ---- summarize (public-safe only) -------------------------------------------
RESULT_JSON="$(find "$JOBS_DIR/$JOB_NAME" -name 'result.json' -maxdepth 2 2>/dev/null | head -1 || true)"
if [[ -z "$RESULT_JSON" ]]; then
  RESULT_JSON="$(find "$JOBS_DIR" -name 'result.json' 2>/dev/null | sort | tail -1 || true)"
fi
if [[ -z "$RESULT_JSON" || ! -f "$RESULT_JSON" ]]; then
  echo "[run-khala-tb2] no result.json found; reporting blocked status."
  python3 - "$SUMMARY_OUT" "$JOB_NAME" "$HARBOR_RC" <<'PY'
import json,sys
out,job,rc=sys.argv[1],sys.argv[2],int(sys.argv[3])
json.dump({"schemaVersion":"openagents.tb2.blackbox_summary.v1","status":"blocked_no_result",
          "jobName":job,"harborExit":rc,"model":"openagents/khala"}, open(out,"w"), indent=2)
print("wrote", out)
PY
  exit 0
fi

echo "[run-khala-tb2] summarizing $RESULT_JSON (public-safe fields only)"
python3 - "$RESULT_JSON" "$SUMMARY_OUT" "$JOB_NAME" "$PUBLIC_MODEL" <<'PY'
import json,sys
res_path,out,job,model=sys.argv[1],sys.argv[2],sys.argv[3],sys.argv[4]
r=json.load(open(res_path))
stats=r.get("stats",{}) or {}
# Per-task reward, NO prompts/responses/trajectories — only task id + reward.
# Harbor's result.json nests trials under stats.evals[*].reward_stats.reward,
# a {reward_value: [trial_id, ...]} map. Strip the "__<suffix>" trial hash.
def base_name(trial_id):
    return (trial_id or "").rsplit("__",1)[0] or trial_id
trials=[]
errored=0
evals=stats.get("evals",{}) or {}
mean=None
for ev in evals.values():
    errored+=ev.get("n_errors",0) or 0
    metrics=ev.get("metrics") or []
    if metrics and isinstance(metrics[0],dict) and mean is None:
        mean=metrics[0].get("mean")
    rmap=((ev.get("reward_stats") or {}).get("reward")) or {}
    for reward_str,ids in rmap.items():
        try: reward=float(reward_str)
        except Exception: reward=None
        for tid in ids:
            trials.append({"task":base_name(tid),"reward":reward,"hadException":False})
n_completed=stats.get("n_completed_trials") or len(trials)
passed=len([x for x in trials if (x["reward"] or 0)>=1.0])
if mean is None:
    mean=(passed/n_completed) if n_completed else None
summary={
  "schemaVersion":"openagents.tb2.blackbox_summary.v1",
  "status":"completed",
  "jobName":job,
  "dataset":"terminal-bench@2.0",
  "model":model,
  "servingPath":"public Khala API (black box); tool-bearing requests may route to a non-GLM tool-caller",
  "counts":{
     "selected":len(trials),
     "completed":n_completed,
     "passed":passed,
     "errored":errored,
  },
  "passRateOverCompleted": (passed/n_completed) if n_completed else None,
  "harborMean": mean,
  "tokens":{
     "promptTokens":stats.get("n_input_tokens") or stats.get("prompt_tokens"),
     "completionTokens":stats.get("n_output_tokens") or stats.get("completion_tokens"),
  },
  "perTask":sorted(trials,key=lambda x:(x["task"] or "")),
  "honesty":"errored trials are infra/tool failures, reported separately from honest non-solves; this bounded subset is NOT the decision-grade 89-task denominator",
}
json.dump(summary,open(out,"w"),indent=2)
print(json.dumps({k:summary[k] for k in ("status","counts","passRateOverCompleted","harborMean")},indent=2))
print("wrote",out)
PY
