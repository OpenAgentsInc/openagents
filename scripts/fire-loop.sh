#!/usr/bin/env bash
# The fire loop: runs Coder One on Terminal-Bench tasks that Fable 5.1
# passes, prints everything each run does as it happens, has Jev judge
# every action against how Fable won the task, and stops the run the
# moment it drifts, with a report that says why.
#
# Usage:
#   scripts/fire-loop.sh [--arm ARM] [--task TASK]... [--parallel] [-- WATCH_OPTIONS]
#
#   --arm ARM     the agent profile to run (default coder-one-microluna-v18)
#   --task TASK   a task with a card in bench/terminal-bench/fire/cards/;
#                 repeat it for more; every card's task when omitted
#   --parallel    start every task at once and write each stream to a
#                 file instead of the terminal
#   -- ...        options for `coder-one fire watch`, such as --clip 400,
#                 --no-stop, or --budget-x 5 (`coder-one fire help`)
#
# Each task runs as its own `tbench try` job named fire--<task>--<stamp>,
# from the task's kept image, with the live log copied to the host every
# 2 seconds. When the loop stops a run, it interrupts that job and kills
# the trial's containers, so no verifier runs. Reports go to
# <job>/<trial>/fire/report.md.
#
# The cards are read only by the judge on the host; nothing in them
# reaches the run. Every fire loop task is in-sample by construction.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bench="$root/bench/terminal-bench"
cards="$bench/fire/cards"
arm="coder-one-microluna-v18"
tasks=()
parallel=0
watch_args=()

while [ $# -gt 0 ]; do
  case "$1" in
    --arm) arm="$2"; shift 2 ;;
    --task) tasks+=("$2"); shift 2 ;;
    --parallel) parallel=1; shift ;;
    --) shift; watch_args=("$@"); break ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "fire-loop: unknown option $1" >&2; exit 2 ;;
  esac
done
if [ ${#tasks[@]} -eq 0 ]; then
  for card in "$cards"/*.json; do tasks+=("$(basename "$card" .json)"); done
fi
for task in "${tasks[@]}"; do
  [ -f "$cards/$task.json" ] || { echo "fire-loop: no card for $task in $cards" >&2; exit 2; }
done

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "Credentials"
[ -s ~/.openagents/bearer ] || { echo "fire-loop: ~/.openagents/bearer is missing" >&2; exit 2; }
[ -s ~/.openagents/jev.json ] || { echo "fire-loop: ~/.openagents/jev.json is missing" >&2; exit 2; }
[ -s ~/.codex/auth.json ] || { echo "fire-loop: ~/.codex/auth.json is missing; sign in to Codex first" >&2; exit 2; }
OPENAGENTS_API_KEY="$(tr -d '\n' < ~/.openagents/bearer)"
TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export OPENAGENTS_API_KEY TYPESAFE_API_KEY CODEX_FORCE_AUTH_JSON=1
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY
echo "door key, Jev key, and Codex sign-in found"

step "Building the watcher"
watch_target="${FIRE_TARGET_DIR:-$root/target-fire}"
CARGO_TARGET_DIR="$watch_target" cargo build -q --release -p coder-one --manifest-path "$root/Cargo.toml"
watcher="$watch_target/release/coder-one"
echo "$watcher"

step "Building the trial binary"
built="$("$root/scripts/build-coder-one-linux.sh")"
artifact_path="$(printf '%s\n' "$built" | sed -n 's/^artifact_path=//p')"
artifact_sha256="$(printf '%s\n' "$built" | sed -n 's/^artifact_sha256=//p')"
printf '%s\n' "$built"

policy_rel="$(jq -r --arg arm "$arm" '.agents[$arm].kwargs.policy // empty' "$bench/profiles/agents.json")"
[ -n "$policy_rel" ] || { echo "fire-loop: arm $arm has no policy in profiles/agents.json" >&2; exit 2; }
policy="$root/$policy_rel"
jobs_dir="${TBENCH_JOBS_DIR:-$HOME/.openagents/terminal-bench/jobs}"
logs_dir="$HOME/.openagents/terminal-bench/fire"
mkdir -p "$logs_dir"

# Starts one task's trial and watches it. Prints the job directory and
# returns the watcher's exit code.
fire() {
  local task="$1" stamp job harness pid code=0 on_stop
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  job="fire--$task--$stamp"
  harness="$logs_dir/$job.harness.log"
  (
    cd "$bench"
    exec setsid "$bench/.venv/bin/python" -m tbench try \
      --arm "$arm" --task "$task" --job-name "$job" --auth-mode auth-json \
      --no-retain --interval 5 \
      --agent-kwarg "artifact_path=$artifact_path" \
      --agent-kwarg "artifact_sha256=$artifact_sha256" \
      --agent-kwarg "live_interval_sec=2"
  ) > "$harness" 2>&1 &
  pid=$!
  # The job runs in a process group of its own, so the stop reaches all of it.
  on_stop="kill -INT -$pid 2>/dev/null; sleep 3; \
docker ps -q --filter \"name=\$(printf %s \"\$FIRE_TRIAL\" | tr '[:upper:]' '[:lower:]')\" | xargs -r docker kill >/dev/null 2>&1; \
kill -TERM -$pid 2>/dev/null; true"
  echo "Task $task: job $jobs_dir/$job, harness log $harness"
  "$watcher" fire watch --job "$jobs_dir/$job" --card "$cards/$task.json" \
    --policy "$policy" --on-stop "$on_stop" "${watch_args[@]}" || code=$?
  wait "$pid" 2>/dev/null || true
  return "$code"
}

declare -A results
if [ "$parallel" -eq 1 ]; then
  step "Starting ${#tasks[@]} tasks at once"
  declare -A pids
  for task in "${tasks[@]}"; do
    stream="$logs_dir/fire--$task--$(date -u +%Y%m%dT%H%M%SZ).stream.txt"
    ( fire "$task" ) > "$stream" 2>&1 &
    pids[$task]=$!
    echo "$task: streaming to $stream"
  done
  for task in "${tasks[@]}"; do
    code=0; wait "${pids[$task]}" || code=$?; results[$task]=$code
  done
else
  for task in "${tasks[@]}"; do
    step "Fire loop: $task"
    code=0; fire "$task" || code=$?; results[$task]=$code
  done
fi

step "Summary"
for task in "${tasks[@]}"; do
  case "${results[$task]}" in
    0) outcome="finished and passed" ;;
    3) outcome="stopped by the fire loop" ;;
    1) outcome="finished without a pass, or the result is unknown" ;;
    *) outcome="the watcher failed (exit ${results[$task]})" ;;
  esac
  printf '%-28s %s\n' "$task" "$outcome"
done
