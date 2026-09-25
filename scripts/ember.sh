#!/usr/bin/env bash
# Ember: a pared-down fire loop in one file. It runs policy variants of
# Coder One on Terminal-Bench tasks that Fable 5.1 low passes, stops any
# run that has used up Fable's time on the task, and ranks every variant
# against Fable on pass rate, time, and cost.
#
# Usage:
#   scripts/ember.sh --task TASK... --variant NAME=BASE[:JQ]... [options]
#
#   --task TASK        a TB4 task id; repeat it for more
#   --variant NAME=BASE[:JQ]
#                      a policy variant: BASE is a manifest stem under
#                      crates/coder-one/policies/, and JQ an optional jq
#                      filter applied to it, such as
#                      '.policy.executor.microluna.lean.wall_sec=600'.
#                      The filter sees $fable_sec, Fable low's mean trial
#                      seconds on the task, so a variant can fit its
#                      bounds to the task
#   --attempts N       attempts per task and variant (default 1)
#   --parallel N       trials at once (default 4)
#   --budget-x X       stop a run after X times Fable low's mean trial
#                      time on the task (default 1.0), counted from the
#                      start of agent setup, so image builds don't count
#   --max-sec SEC      never let a run go longer than SEC (default 1800)
#   --idle SEC         stop a run whose job folder hasn't changed for SEC
#                      (default 420)
#   --arm ARM          the agent profile the variants run under (default
#                      coder-one-microluna-v20)
#   --provider P       codex (the default) sends Luna requests through the
#                      Codex login; openrouter sends them through
#                      OpenRouter on OPENROUTER_API_KEY or the api_key in
#                      ~/.openagents/openrouter.json, under the
#                      coder-one-microluna-openrouter arm (issue #9665)
#   --out DIR          where variants, logs, and results.tsv go (default
#                      ~/.cache/openagents/ember/<stamp>)
#   --report DIR       print the ranking of an earlier --out and exit
#
# EMBER_CODEX_AUTH points at a Codex auth.json other than ~/.codex's.
# Every variant's manifest is sent through --arm, or the OpenRouter arm.
#
# Unlike the fire loop, Ember has no Jev judge and no strategy card: the
# only rules are Fable's clock, an idle limit, and a hard ceiling. A run
# Ember stops has no reward and counts as a failure. Everything runs
# in-sample; a result here is a lead, not admission evidence.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bench="$root/bench/terminal-bench"
fable="$bench/experiments/2026-09-25-luna-sized-family/tasks.json"
arm="coder-one-microluna-v20"
attempts=1 parallel=4 budget_x=1.0 max_sec=1800 idle=420
out="" report_only="" provider="codex"
tasks=() variants=()

while [ $# -gt 0 ]; do
  case "$1" in
    --task) tasks+=("$2"); shift 2 ;;
    --variant) variants+=("$2"); shift 2 ;;
    --attempts) attempts="$2"; shift 2 ;;
    --parallel) parallel="$2"; shift 2 ;;
    --budget-x) budget_x="$2"; shift 2 ;;
    --max-sec) max_sec="$2"; shift 2 ;;
    --idle) idle="$2"; shift 2 ;;
    --arm) arm="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --provider) provider="$2"; shift 2 ;;
    --report) report_only="$2"; shift 2 ;;
    -h|--help) sed -n '2,34p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "ember: unknown option $1" >&2; exit 2 ;;
  esac
done

# Fable 5.1 low on a task: passes, mean trial seconds, cost per pass.
fable_row() {
  jq -r --arg t "$1" '.tasks[] | select(.task == $t)
    | [.fable.by_effort.low, .fable.low_mean_trial_sec, .fable.low_cost_per_pass_usd] | @tsv' "$fable"
}

# Ranks every task and variant in results.tsv against Fable.
report() {
  local results="$1/results.tsv"
  [ -s "$results" ] || { echo "ember: no results in $1" >&2; return 1; }
  printf '\n%-28s %-18s %6s %8s %9s %9s %10s %9s %9s\n' \
    task variant passes tests med_min fable_min usd/run usd/pass fable_usd
  tail -n +2 "$results" | sort -t$'\t' -k1,1 -k2,2 | awk -F'\t' '
    function flush() {
      if (key == "") return
      n = asort(times, sorted)
      med = n ? (n % 2 ? sorted[(n + 1) / 2] : (sorted[n / 2] + sorted[n / 2 + 1]) / 2) : 0
      per_pass = passes ? sprintf("%.4f", cost / passes) : "-"
      printf "%-28s %-18s %3d/%-2d %8s %9.1f %9.1f %10.4f %9s %9.2f\n", task, variant, passes, runs,
        (tests_total ? sprintf("%d/%d", tests_passed, tests_total) : "-"),
        med / 60, fsec / 60, cost / runs, per_pass, fcost
      delete times
    }
    {
      k = $1 "\t" $2
      if (k != key) { flush(); key = k; task = $1; variant = $2; runs = passes = cost = 0; tests_passed = tests_total = 0 }
      runs++; if ($4 == "1" || $4 == "1.0") passes++
      cost += $8; tests_passed += $5; tests_total += $6
      times[runs] = $7; fsec = $11; fcost = $12
    }
    END { flush() }'
  echo
  echo "passes: runs with reward 1. tests: verifier tests passed over all runs."
  echo "med_min: median trial minutes, a stopped run counting its time at the stop."
  echo "usd/run: Luna plus Jev, known cost or its lower bound. fable_*: Fable 5.1 low."
}

if [ -n "$report_only" ]; then report "$report_only"; exit 0; fi
[ ${#tasks[@]} -gt 0 ] || { echo "ember: name at least one --task" >&2; exit 2; }
[ ${#variants[@]} -gt 0 ] || variants=("v20=microluna-v20")
for task in "${tasks[@]}"; do
  [ -n "$(fable_row "$task")" ] || { echo "ember: $task has no Fable row in $fable" >&2; exit 2; }
done

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "Credentials and harness"
export PATH="$bench/.venv/bin:$PATH"
command -v harbor >/dev/null || { echo "ember: harbor isn't in $bench/.venv" >&2; exit 2; }
for f in ~/.openagents/bearer ~/.openagents/jev.json; do
  [ -s "$f" ] || { echo "ember: $f is missing" >&2; exit 2; }
done
OPENAGENTS_API_KEY="$(tr -d '\n' < ~/.openagents/bearer)"
TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export OPENAGENTS_API_KEY TYPESAFE_API_KEY
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY CLAUDE_CODE_OAUTH_TOKEN
case "$provider" in
  codex)
    # EMBER_CODEX_AUTH names a Codex sign-in other than the host's own;
    # Microluna only reads it.
    codex_auth="${EMBER_CODEX_AUTH:-$HOME/.codex/auth.json}"
    [ -s "$codex_auth" ] || { echo "ember: $codex_auth is missing" >&2; exit 2; }
    export CODEX_AUTH_JSON_PATH="$codex_auth"
    auth_mode="auth-json"
    ;;
  openrouter)
    if [ -z "${OPENROUTER_API_KEY:-}" ] && [ -s ~/.openagents/openrouter.json ]; then
      OPENROUTER_API_KEY="$(jq -r .api_key ~/.openagents/openrouter.json)"
    fi
    [ -n "${OPENROUTER_API_KEY:-}" ] || { echo "ember: no OpenRouter key" >&2; exit 2; }
    export OPENROUTER_API_KEY
    arm="coder-one-microluna-openrouter"
    auth_mode="openrouter"
    ;;
  *) echo "ember: --provider is codex or openrouter" >&2; exit 2 ;;
esac
echo "ok"

step "The trial binary"
# Shared with the fire loop's cache: the same key, the same folder.
key="$(cd "$root" && {
  git ls-files -s -- crates Cargo.lock rust-toolchain.toml ':!crates/coder-one/src/fire'
  git diff HEAD -- crates Cargo.lock rust-toolchain.toml ':!crates/coder-one/src/fire'
} | sha256sum | cut -c1-16)"
cache="$HOME/.cache/openagents/fire-artifacts/$key"
if ! [ -x "$cache/coder-one" ] || ! [ -s "$cache/sha256" ]; then
  built="$("$root/scripts/build-coder-one-linux.sh")"
  mkdir -p "$cache"
  cp "$(printf '%s\n' "$built" | sed -n 's/^artifact_path=//p')" "$cache/coder-one"
  printf '%s\n' "$built" | sed -n 's/^artifact_sha256=//p' > "$cache/sha256"
fi
artifact="$cache/coder-one"
sha="$(cat "$cache/sha256")"
[ "$(sha256sum "$artifact" | cut -d' ' -f1)" = "$sha" ] || { echo "ember: artifact digest mismatch" >&2; exit 2; }
echo "$artifact"

out="${out:-$HOME/.cache/openagents/ember/$(date -u +%Y%m%dT%H%M%SZ)}"
mkdir -p "$out/variants" "$out/logs"
results="$out/results.tsv"
[ -s "$results" ] || printf 'task\tvariant\tattempt\treward\ttests_passed\ttests_total\ttrial_sec\tcost_usd\tstatus\tjob\tfable_sec\tfable_cost_per_pass\n' > "$results"

step "Variants"
# One manifest per variant and task, since a filter may read $fable_sec.
for spec in "${variants[@]}"; do
  name="${spec%%=*}" rest="${spec#*=}"
  base="${rest%%:*}" filter="."
  [ "$rest" != "$base" ] && filter="${rest#*:}"
  src="$root/crates/coder-one/policies/$base.json"
  [ -f "$src" ] || { echo "ember: no manifest $src" >&2; exit 2; }
  for task in "${tasks[@]}"; do
    fsec="$(fable_row "$task" | cut -f2)"
    jq --arg n "ember-$name" --argjson fable_sec "$fsec" "$filter | .name = \$n" "$src" \
      > "$out/variants/$name--$task.json"
  done
  printf '%-18s %s %s\n' "$name" "$base" "$filter"
done

jobs_dir="${TBENCH_JOBS_DIR:-$HOME/.openagents/terminal-bench/jobs}"

# Stops a trial: the harness's process group, then the trial's containers.
stop_trial() {
  local pid="$1" trial="$2"
  kill -INT -"$pid" 2>/dev/null || true
  sleep 3
  if [ -n "$trial" ]; then
    docker ps -q --filter "name=$(printf %s "$trial" | tr '[:upper:]' '[:lower:]')" | xargs -r docker kill >/dev/null 2>&1 || true
  fi
  kill -TERM -"$pid" 2>/dev/null || true
}

# Runs one trial, watches it, and appends its row to results.tsv.
run_one() {
  # A watcher must outlive a failed probe, such as a find on a job
  # folder the harness hasn't made yet, and still write its row.
  set +e +o pipefail
  local task="$1" variant="$2" attempt="$3"
  local job="ember--$task--$variant--a$attempt--$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM"
  local log="$out/logs/$job.log" status="finished"
  local fpasses fsec fcost budget
  IFS=$'\t' read -r fpasses fsec fcost < <(fable_row "$task")
  budget="$(awk -v s="$fsec" -v x="$budget_x" -v m="$max_sec" 'BEGIN { b = s * x; if (b > m) b = m; if (b < 300) b = 300; printf "%d", b }')"
  (
    cd "$bench"
    exec setsid python -m tbench try --arm "$arm" --task "$task" --job-name "$job" \
      --auth-mode "$auth_mode" --no-retain --interval 10 \
      --agent-kwarg "artifact_path=$artifact" --agent-kwarg "artifact_sha256=$sha" \
      --agent-kwarg "policy=$out/variants/$variant--$task.json"
  ) > "$log" 2>&1 &
  local pid=$! launched=$SECONDS started="" trial="" last_change=$SECONDS last_count=""
  echo "start $task $variant a$attempt: budget ${budget}s after agent setup, log $log"
  while kill -0 "$pid" 2>/dev/null; do
    sleep 5
    if [ -z "$trial" ]; then
      trial="$(find "$jobs_dir/$job" -mindepth 1 -maxdepth 1 -type d -name "${task}__*" -printf '%f\n' 2>/dev/null | head -1)"
    fi
    if [ -z "$started" ] && [ -n "$trial" ] && [ -d "$jobs_dir/$job/$trial/agent" ]; then
      started=$SECONDS
    fi
    local count
    count="$(find "$jobs_dir/$job" "$log" -newermt "@$(( $(date +%s) - 60 ))" -type f 2>/dev/null | head -1 | wc -l)"
    [ "$count" -gt 0 ] && last_change=$SECONDS
    if [ -n "$started" ] && [ $((SECONDS - started)) -gt "$budget" ]; then
      status="over_fable_time"; stop_trial "$pid" "$trial"; break
    fi
    if [ $((SECONDS - last_change)) -gt "$idle" ]; then
      status="idle"; stop_trial "$pid" "$trial"; break
    fi
    if [ $((SECONDS - launched)) -gt $((max_sec + 1800)) ]; then
      status="ceiling"; stop_trial "$pid" "$trial"; break
    fi
  done
  wait "$pid" 2>/dev/null || true

  # The attempt record holds the reward, timing, and cost when the run
  # finished; a stopped run keeps its own clock and its logged usage.
  local rec reward="" tests_passed=0 tests_total=0 trial_sec cost=0
  rec="$(find "$jobs_dir/$job/tbench/attempts" -name '*.json' 2>/dev/null | head -1)"
  trial_sec=$((SECONDS - launched))
  if [ -n "$rec" ] && [ "$status" = "finished" ]; then
    reward="$(jq -r '.outcome.reward // empty' "$rec")"
    trial_sec="$(jq -r '((.timing.total_ms // 0) / 1000 | floor)' "$rec")"
    cost="$(jq -r '(.cost.amount_usd // .cost.lower_bound_usd // 0)' "$rec")"
  fi
  if [ -n "$trial" ]; then
    local ctrf="$jobs_dir/$job/$trial/verifier/ctrf.json" usage="$jobs_dir/$job/$trial/agent/episode/evaluation/usage.json"
    if [ -f "$ctrf" ]; then
      tests_passed="$(jq -r '.results.summary.passed // 0' "$ctrf")"
      tests_total="$(jq -r '.results.summary.tests // 0' "$ctrf")"
    fi
    if [ "$cost" = "0" ] && [ -f "$usage" ]; then
      cost="$(jq -r '(.cost.amount_usd // .cost.lower_bound_usd // 0)' "$usage")"
    fi
  fi
  # A run whose model provider refused it says nothing about the policy,
  # and the next run would meet the same refusal: halt the round.
  if [ -n "$trial" ] && grep -rqs "lost its provider" "$jobs_dir/$job/$trial/agent"; then
    status="provider_lost"; : > "$out/.halt"
  fi
  [ -n "$reward" ] || reward="none"
  [ "$status" = "finished" ] && [ "$reward" = "none" ] && status="no_reward"
  (
    flock 8
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$task" "$variant" "$attempt" "$reward" \
      "$tests_passed" "$tests_total" "$trial_sec" "$cost" "$status" "$job" "$fsec" "$fcost" >> "$results"
  ) 8>"$out/.lock"
  echo "done  $task $variant a$attempt: reward=$reward tests=$tests_passed/$tests_total ${trial_sec}s \$$cost $status"
}

step "Running ${#tasks[@]} task(s) x ${#variants[@]} variant(s) x $attempts attempt(s), $parallel at once"
echo "results: $results"
running=0
for attempt in $(seq 1 "$attempts"); do
  for task in "${tasks[@]}"; do
    for spec in "${variants[@]}"; do
      if [ "$running" -ge "$parallel" ]; then wait -n || true; running=$((running - 1)); fi
      if [ -e "$out/.halt" ]; then echo "ember: a run lost its model provider; not starting more" >&2; break 3; fi
      run_one "$task" "${spec%%=*}" "$attempt" &
      running=$((running + 1))
      sleep 20 # keeps image builds and setup from landing at once
    done
  done
done
wait || true

step "Ranking against Fable 5.1 low"
report "$out"
