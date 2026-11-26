#!/usr/bin/env bash
# Firm reason: 5 (thin Harbor CLI invocation wrapper for apples-to-apples Terminal-Bench 4.1 subset runs)
# Run a Terminal-Bench 4.1 (v4.1.0) subset fairly on either Antigravity CLI or OpenAgents Flawed
# using the exact same model (google/gemini-4-neon), reasoning_effort (high), GEMINI_API_KEY, and timeout cap.
set -euo pipefail

AGENT="${1:?usage: run_tb41_subset.sh <antigravity|flawed> <job_name> <task1> [task2 ...]}"
JOB_NAME="${2:?missing job_name}"
shift 2
TASKS=("$@")
if [ "${#TASKS[@]}" -eq 0 ]; then
  echo "error: at least one TB 4.1 task name required" >&2
  exit 2
fi

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
TB_TASKS_DIR="${TB41_TASKS_DIR:-$HOME/work/projects/repos/terminal-bench/tasks}"
JOBS_DIR="${TB41_JOBS_DIR:-$HOME/.openagents/tbench/jobs}"
mkdir -p "$JOBS_DIR"

export DOCKER_HOST="${DOCKER_HOST:-unix://$HOME/.colima/oa-agent0/docker.sock}"
export PYTHONPATH="$REPO/evals/terminal-bench/harbor_adapter${PYTHONPATH:+:$PYTHONPATH}"

if [ -z "${GEMINI_API_KEY:-}" ] && [ -f "$HOME/work/.secrets/openagents-gemini-provider.env" ]; then
  GEMINI_API_KEY="$(grep '^GEMINI_API_KEY=' "$HOME/work/.secrets/openagents-gemini-provider.env" | head -n1 | cut -d= -f2- | tr -d '\"'\'' ')"
  export GEMINI_API_KEY
fi
if [ -z "${GEMINI_API_KEY:-}" ]; then
  echo "error: GEMINI_API_KEY not set" >&2
  exit 2
fi

MODEL="${TB41_MODEL:-gemini-4-neon}"
EFFORT="${TB41_EFFORT:-high}"
# 28800s * 0.02083333 = 600s (10 min per task)
AGENT_TIMEOUT_MULT="${TB41_AGENT_TIMEOUT_MULT:-0.02083333}"
N_CONCURRENT="${TB41_CONCURRENT:-2}"

TASK_ARGS=()
for t in "${TASKS[@]}"; do
  TASK_ARGS+=(-i "$t")
done

if [ "$AGENT" = "antigravity" ]; then
  AGENT_ARGS=(--agent antigravity-cli --model "$MODEL" --ak "reasoning_effort=$EFFORT")
  if [ -n "${AGY_LINUX_BIN:-}" ] && [ -f "${AGY_LINUX_BIN}" ]; then
    AGENT_ARGS+=(--ak "binary_path=$AGY_LINUX_BIN")
  fi
elif [ "$AGENT" = "flawed" ]; then
  BUDGET_S="${TB41_BUDGET_S:-600}"
  AGENT_ARGS=(--agent "flawed_agent:FlawedAgent" --model "$MODEL" --ak "reasoning_effort=$EFFORT" --ak "budget_s=$BUDGET_S")
  if [ -n "${FLAWED_EXTRA_AK:-}" ]; then
    for ak in $FLAWED_EXTRA_AK; do
      AGENT_ARGS+=(--ak "$ak")
    done
  fi
else
  echo "error: unknown agent '$AGENT' (expected antigravity or flawed)" >&2
  exit 2
fi

echo "=== TB 4.1 RUN: agent=$AGENT job=$JOB_NAME model=$MODEL effort=$EFFORT timeout_mult=$AGENT_TIMEOUT_MULT conc=$N_CONCURRENT tasks=(${TASKS[*]}) ==="
harbor run \
  -p "$TB_TASKS_DIR" \
  "${TASK_ARGS[@]}" \
  "${AGENT_ARGS[@]}" \
  --jobs-dir "$JOBS_DIR" \
  --job-name "$JOB_NAME" \
  --n-concurrent "$N_CONCURRENT" \
  --agent-timeout-multiplier "$AGENT_TIMEOUT_MULT" \
  --max-retries 1 \
  --yes
