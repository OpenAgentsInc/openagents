#!/usr/bin/env bash
set -euo pipefail

# Gym suite runner. Packs the working-tree openagents CLI, runs a Terminal-Bench
# suite through Harbor, and posts the graded result to the OpenAgents Gym.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$REPO_ROOT/bench"

usage() {
  cat <<'EOF'
Usage: bench/run-suite.sh <suite-file> --model <harbor-model> [options]

Arguments:
  <suite-file>            A suite manifest (*.suite.json) or a plain task list.

                          Prefer the manifest. It pins each task by content —
                          dataset, git url, commit, path — so the digest a run
                          records means something, and it is what
                          `coder-effectiveness report --suite-manifest` scores
                          the run against. Every pinned task is included, so a
                          run of a manifest is a full run or it is not that
                          suite; the report says which.

                          A plain task list is one task name per line, with
                          empty lines and lines starting with # ignored. It
                          still runs, and a run of one cannot be recorded as a
                          score: nothing in it says what the suite was.

Required options:
  --model <model>         Harbor model string, e.g. openai/gpt-5.6-luna,
                          google/gemini-3.7-flash, ollama/qwen3.8:27b-mtp-q8_0.

Optional options:
  --lane <proxy|local>    Gym lane. Defaults to 'local' for ollama/... models,
                          'proxy' for all other models.
  --api-url <URL>         OpenAgents API base URL for posting the run.
                          Defaults to http://localhost:4000. The container
                          side uses host.docker.internal where localhost/127.0.0.1
                          is given, unless OPENAGENTS_CODER_API_URL is set.
  --jobs-dir <DIR>        Harbor jobs directory. Defaults to a fresh
                          timestamped directory under /tmp/gym-jobs-YYYYMMDD-HHMMSS.
  --n-concurrent <N>      Number of concurrent trials. Defaults to 1.
  --timeout-multiplier <X> Passed through to harbor run if provided.
  --plugins               Install the working-tree digest-pinned plugin
                          catalog at /plugins inside each Harbor container
                          (OPENAGENTS_CODER_PLUGINS=1). Omit for the A/B
                          absent row. Issue OpenAgentsInc/openagents#120.
  --dry-run               Print the commands that would run without executing.
  -h, --help              Show this help and exit.

Environment:
  OPENAGENTS_TOKEN        Required for the Harbor run unless --model starts
                          with ollama/. Required for posting results and for
                          registering the run against the Gym lifecycle API;
                          without it registration is skipped.
  OPENAGENTS_CODER_API_URL Overrides the coder container API URL.

With OPENAGENTS_TOKEN set, the runner registers the run at
POST <api-url>/api/v1/gym/runs/start before Harbor starts, exports
OPENAGENTS_GYM_RUN_ID and OPENAGENTS_GYM_API_URL into the Harbor run so the
adapter reports each trial live, and finalizes through post_gym_run.py
--run-id. A suite that fails before grading patches the run to abandoned.

Examples:
  bench/run-suite.sh bench/suites/tb2-cross-section.suite.json \
    --model openai/gpt-5.6-luna --lane proxy --n-concurrent 2

  bench/run-suite.sh bench/suites/tb2-quick.suite.json \
    --model ollama/qwen3.8:27b-mtp-q8_0 --lane local --dry-run
EOF
}

log() {
  echo "[run-suite] $*" >&2
}

# Replace localhost / 127.0.0.1 with host.docker.internal so the same --api-url
# works from the host (posting) and from inside the Harbor container (coder).
coder_api_url_for() {
  local url="$1"
  echo "$url" | sed -E 's#(https?://)(localhost|127\.0\.0\.1)(:[0-9]+)?#\1host.docker.internal\3#'
}

# Register the run against the Gym lifecycle API and print its id
# (OpenAgentsInc/openagents#38). Prints nothing on failure: the suite still
# runs, and the post-hoc one-shot path still posts the graded result.
register_gym_run() {
  python3 - "$API_URL" "$SUITE_NAME" "$CATALOG_MODEL" "$LANE" "$1" <<'PY'
import json, os, sys, urllib.request

api_url, suite, model, lane, tasks_total = sys.argv[1:6]
payload = {
    "suite": suite,
    "agent": "openagents-coder",
    "model": model,
    "lane": lane,
    "tasks_total": int(tasks_total),
}
request = urllib.request.Request(
    f"{api_url}/api/v1/gym/runs/start",
    data=json.dumps(payload).encode(),
    headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {os.environ.get('OPENAGENTS_TOKEN', '')}",
    },
    method="POST",
)
try:
    with urllib.request.urlopen(request, timeout=15) as response:
        body = json.loads(response.read() or b"{}")
        run_id = (body.get("run") or {}).get("id")
        if run_id:
            print(run_id)
except Exception as error:
    print(f"gym run registration failed: {error}", file=sys.stderr)
PY
}

# Close a registered run without grades. A crashed suite is not a grade, but
# it should not be a forever-running row on /gym either.
abandon_gym_run() {
  [ -n "${GYM_RUN_ID:-}" ] || return 0
  log "Marking Gym run $GYM_RUN_ID abandoned..."
  python3 - "$API_URL" "$GYM_RUN_ID" <<'PY'
import json, os, sys, urllib.request

api_url, run_id = sys.argv[1:3]
request = urllib.request.Request(
    f"{api_url}/api/v1/gym/runs/{run_id}",
    data=json.dumps({"status": "abandoned"}).encode(),
    headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {os.environ.get('OPENAGENTS_TOKEN', '')}",
    },
    method="PATCH",
)
try:
    with urllib.request.urlopen(request, timeout=15):
        pass
except Exception as error:
    print(f"gym run abandon failed: {error}", file=sys.stderr)
PY
}

# Argument defaults
SUITE_FILE=""
MODEL=""
LANE=""
LANE_SET=0
API_URL="http://localhost:4000"
JOBS_DIR=""
N_CONCURRENT="1"
TIMEOUT_MULTIPLIER=""
DRY_RUN=0
PLUGINS=0

while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --model)
      MODEL="$2"
      shift 2
      ;;
    --lane)
      LANE="$2"
      LANE_SET=1
      shift 2
      ;;
    --api-url)
      API_URL="$2"
      shift 2
      ;;
    --jobs-dir)
      JOBS_DIR="$2"
      shift 2
      ;;
    --n-concurrent)
      N_CONCURRENT="$2"
      shift 2
      ;;
    --timeout-multiplier)
      TIMEOUT_MULTIPLIER="$2"
      shift 2
      ;;
    --plugins)
      PLUGINS=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      log "Unknown option: $1"
      usage
      exit 1
      ;;
    *)
      if [ -n "$SUITE_FILE" ]; then
        log "Unexpected extra argument: $1"
        usage
        exit 1
      fi
      SUITE_FILE="$1"
      shift
      ;;
  esac
done

# Validate required arguments
if [ -z "$SUITE_FILE" ]; then
  log "Missing required <suite-file> argument."
  usage
  exit 1
fi

if [ ! -f "$SUITE_FILE" ]; then
  log "Suite file not found: $SUITE_FILE"
  exit 1
fi

if [ -z "$MODEL" ]; then
  log "Missing required --model."
  usage
  exit 1
fi

# Default lane based on model family when the user did not explicitly set --lane.
if [ "$LANE_SET" -eq 0 ]; then
  if [[ "$MODEL" == ollama/* ]]; then
    LANE="local"
  else
    LANE="proxy"
  fi
fi

if [ "$LANE" != "proxy" ] && [ "$LANE" != "local" ]; then
  log "--lane must be 'proxy' or 'local', got: $LANE"
  exit 1
fi

if ! [[ "$N_CONCURRENT" =~ ^[1-9][0-9]*$ ]]; then
  log "--n-concurrent must be a positive integer, got: $N_CONCURRENT"
  exit 1
fi

if [ -n "$TIMEOUT_MULTIPLIER" ] && ! [[ "$TIMEOUT_MULTIPLIER" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
  log "--timeout-multiplier must be a number, got: $TIMEOUT_MULTIPLIER"
  exit 1
fi

# Normalize the suite file path to an absolute path.
SUITE_FILE="$(cd "$(dirname "$SUITE_FILE")" && pwd)/$(basename "$SUITE_FILE")"

# Parse tasks. A manifest carries them under .tasks[].id; a plain list is one
# name per line with comments stripped.
TASKS=()
case "$SUITE_FILE" in
  *.suite.json)
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      TASKS+=("$line")
    done < <(python3 -c '
import json, sys
manifest = json.load(open(sys.argv[1]))
for task in manifest["tasks"]:
    print(task["id"])
' "$SUITE_FILE")
    ;;
  *)
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      TASKS+=("$line")
    done < <(sed -e 's/#.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' "$SUITE_FILE" | sed -n '/./p')
    ;;
esac

if [ "${#TASKS[@]}" -eq 0 ]; then
  log "Suite file contains no tasks: $SUITE_FILE"
  exit 1
fi

TASK_ARGS=()
for task in "${TASKS[@]}"; do
  TASK_ARGS+=("-i" "$task")
done

SUITE_NAME="$(basename "$(basename "$SUITE_FILE" .txt)" .suite.json)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
BENCH_CLI_VERSION="0.0.0-bench.${TIMESTAMP//-/}"
BENCH_CLI_PLATFORM="${OPENAGENTS_CODER_PLATFORM:-linux-x86_64}"
BENCH_CLI_BINARY="$REPO_ROOT/dist/releases/$BENCH_CLI_VERSION/openagents-$BENCH_CLI_VERSION-$BENCH_CLI_PLATFORM"

# The catalog name the run registers under: Harbor spells models
# provider/name, the catalog id is the name, and an ollama/ model is the
# coder's `ollama:<name>` local-lane shape — the same mapping the adapter
# applies.
if [[ "$MODEL" == ollama/* ]]; then
  CATALOG_MODEL="ollama:${MODEL#ollama/}"
else
  CATALOG_MODEL="${MODEL#*/}"
fi

if [ -z "$JOBS_DIR" ]; then
  JOBS_DIR="/tmp/gym-jobs-${TIMESTAMP}"
fi

mkdir -p "$JOBS_DIR"

# Token may be empty; -u would otherwise fail on reference.
OPENAGENTS_TOKEN="${OPENAGENTS_TOKEN:-}"

TOKEN_DISPLAY="<not set>"
if [ -n "$OPENAGENTS_TOKEN" ]; then
  TOKEN_DISPLAY="<redacted>"
fi

CANDIDATE_CODER_API_URL="${OPENAGENTS_CODER_API_URL:-}"
if [ -z "$CANDIDATE_CODER_API_URL" ]; then
  CANDIDATE_CODER_API_URL="$(coder_api_url_for "$API_URL")"
fi

# Build the harbor argument list.
HARBOR_ARGS=(
  --dataset "terminal-bench@2.0"
  --agent-import-path "adapters.openagents_coder:OpenAgentsCoder"
  -m "$MODEL"
  "${TASK_ARGS[@]}"
  --jobs-dir "$JOBS_DIR"
  --n-concurrent "$N_CONCURRENT"
)
if [ -n "$TIMEOUT_MULTIPLIER" ]; then
  HARBOR_ARGS+=(--timeout-multiplier "$TIMEOUT_MULTIPLIER")
fi

if [ "$DRY_RUN" -eq 1 ]; then
  echo "[dry-run] Native CLI build command:"
  echo "[dry-run]   ops/release-cli.sh --version $(printf '%q' "$BENCH_CLI_VERSION") --targets $(printf '%q' "$BENCH_CLI_PLATFORM") --bench"
  echo
  if [ -n "$OPENAGENTS_TOKEN" ]; then
    echo "[dry-run] Register command:"
    echo "[dry-run]   POST $API_URL/api/v1/gym/runs/start {\"suite\": \"$SUITE_NAME\", \"agent\": \"openagents-coder\", \"model\": \"$CATALOG_MODEL\", \"lane\": \"$LANE\", \"tasks_total\": ${#TASKS[@]}}"
    echo "[dry-run]   (exports OPENAGENTS_GYM_RUN_ID=<run-id> and OPENAGENTS_GYM_API_URL=$API_URL into the Harbor run)"
  else
    echo "[dry-run] Register command: skipped (OPENAGENTS_TOKEN is not set)"
  fi
  GYM_ENV_ARGS=()
  RUN_ID_SUFFIX=""
  if [ -n "$OPENAGENTS_TOKEN" ]; then
    GYM_ENV_ARGS=("OPENAGENTS_GYM_RUN_ID=<run-id>" "OPENAGENTS_GYM_API_URL=$API_URL")
    RUN_ID_SUFFIX=" --run-id '<run-id>'"
  fi
  if [ "$PLUGINS" -eq 1 ]; then
    GYM_ENV_ARGS+=("OPENAGENTS_CODER_PLUGINS=1")
  fi
  echo
  echo "[dry-run] Harbor command:"
  echo -n "[dry-run]   "
  printf '%q ' \
    "PYTHONPATH=$BENCH_DIR" \
    "OPENAGENTS_TOKEN=$TOKEN_DISPLAY" \
    "OPENAGENTS_CODER_BINARY=$BENCH_CLI_BINARY" \
    "OPENAGENTS_CODER_API_URL=$CANDIDATE_CODER_API_URL" \
    ${GYM_ENV_ARGS[@]+"${GYM_ENV_ARGS[@]}"} \
    harbor run \
    "${HARBOR_ARGS[@]}"
  echo
  echo
  echo "[dry-run] Post command (after locating the job directory under $JOBS_DIR):"
  echo "[dry-run]   python3 $(printf '%q' "$BENCH_DIR/post_gym_run.py") <job-dir> --api-url $(printf '%q' "$API_URL") --lane $(printf '%q' "$LANE") --suite $(printf '%q' "$SUITE_NAME")$RUN_ID_SUFFIX"
  if [ -n "$OPENAGENTS_TOKEN" ]; then
    echo
    echo "[dry-run] On a suite failure before grading:"
    echo "[dry-run]   PATCH $API_URL/api/v1/gym/runs/<run-id> {\"status\": \"abandoned\"}"
  fi
  exit 0
fi

# Require token for the Harbor run unless this is an ollama model.
if [ -z "$OPENAGENTS_TOKEN" ] && [[ "$MODEL" != ollama/* ]]; then
  log "OPENAGENTS_TOKEN is required for non-ollama models."
  exit 1
fi

# Build the same native binary the installer publishes. The adapter uploads
# this exact working-tree artifact into each Harbor container.
log "Building native OpenAgents CLI for $BENCH_CLI_PLATFORM..."
"$REPO_ROOT/ops/release-cli.sh" \
  --version "$BENCH_CLI_VERSION" \
  --targets "$BENCH_CLI_PLATFORM" \
  --bench

if [ ! -x "$BENCH_CLI_BINARY" ]; then
  log "Native CLI build did not produce $BENCH_CLI_BINARY."
  exit 1
fi

# Register the run so /gym shows it while the trials are still executing.
# Without a token (ollama dry runs) there is nothing to register against;
# a failed registration degrades to the post-hoc one-shot path.
GYM_RUN_ID=""
if [ -n "$OPENAGENTS_TOKEN" ]; then
  log "Registering Gym run at $API_URL..."
  GYM_RUN_ID="$(register_gym_run "${#TASKS[@]}" || true)"
  if [ -n "$GYM_RUN_ID" ]; then
    log "Gym run id: $GYM_RUN_ID"
  else
    log "Gym run registration failed; continuing without live run reporting."
  fi
else
  log "OPENAGENTS_TOKEN is not set; skipping Gym run registration."
fi

log "Running Harbor suite: $SUITE_NAME (${#TASKS[@]} tasks)..."
if ! (
  export PYTHONPATH="$BENCH_DIR"
  export OPENAGENTS_TOKEN
  export OPENAGENTS_CODER_BINARY="$BENCH_CLI_BINARY"
  if [ -n "${OPENAGENTS_CODER_API_URL:-}" ]; then
    export OPENAGENTS_CODER_API_URL
  else
    export OPENAGENTS_CODER_API_URL="$CANDIDATE_CODER_API_URL"
  fi
  if [ -n "$GYM_RUN_ID" ]; then
    # The adapter reports trials host-side, so it takes the host api url,
    # not the container-rewritten OPENAGENTS_CODER_API_URL.
    export OPENAGENTS_GYM_RUN_ID="$GYM_RUN_ID"
    export OPENAGENTS_GYM_API_URL="$API_URL"
  fi
  if [ "$PLUGINS" -eq 1 ]; then
    export OPENAGENTS_CODER_PLUGINS=1
    export OPENAGENTS_CODER_PLUGINS_DIR="${OPENAGENTS_CODER_PLUGINS_DIR:-$REPO_ROOT/plugins}"
  fi
  harbor run "${HARBOR_ARGS[@]}"
); then
  log "Harbor run failed."
  abandon_gym_run
  exit 1
fi

# Locate the completed job directory. Harbor creates a child directory under
# --jobs-dir, but if it writes directly into the directory, use that.
find_job_dir() {
  local root="$1"
  if [ -f "$root/result.json" ] && [ -f "$root/config.json" ]; then
    echo "$root"
    return 0
  fi
  python3 - "$root" <<'PY'
import os, sys
root = sys.argv[1]
candidates = []
for name in os.listdir(root):
    path = os.path.join(root, name)
    if os.path.isdir(path):
        if os.path.isfile(os.path.join(path, "result.json")) and os.path.isfile(os.path.join(path, "config.json")):
            candidates.append((os.path.getmtime(path), path))
if not candidates:
    print("No Harbor job directory with result.json and config.json found", file=sys.stderr)
    sys.exit(1)
candidates.sort(key=lambda x: x[0], reverse=True)
print(candidates[0][1])
PY
}

if ! JOB_DIR="$(find_job_dir "$JOBS_DIR")"; then
  JOB_DIR=""
fi

if [ -z "$JOB_DIR" ] || [ ! -d "$JOB_DIR" ]; then
  log "Could not locate Harbor job directory under $JOBS_DIR"
  abandon_gym_run
  exit 1
fi

log "Harbor job directory: $JOB_DIR"

# Post the graded result. With a registered run this is the finalize step:
# post_gym_run.py upserts each trial's final state and patches the run to
# graded, or to abandoned when no verifier ran.
if [ -z "$OPENAGENTS_TOKEN" ]; then
  log "OPENAGENTS_TOKEN is not set; skipping result post."
  log "To post later, run:"
  log "  OPENAGENTS_TOKEN=... python3 $(printf '%q' "$BENCH_DIR/post_gym_run.py") $(printf '%q' "$JOB_DIR") --api-url $(printf '%q' "$API_URL") --lane $(printf '%q' "$LANE") --suite $(printf '%q' "$SUITE_NAME")"
  exit 0
fi

RUN_ID_ARGS=()
if [ -n "$GYM_RUN_ID" ]; then
  RUN_ID_ARGS=(--run-id "$GYM_RUN_ID")
fi

log "Posting result to $API_URL..."
if ! python3 "$BENCH_DIR/post_gym_run.py" "$JOB_DIR" --api-url "$API_URL" --lane "$LANE" --suite "$SUITE_NAME" ${RUN_ID_ARGS[@]+"${RUN_ID_ARGS[@]}"}; then
  log "Result post failed. Harbor grades still stand in $JOB_DIR."
  log "Not abandoning a run whose verifier already ran."
  exit 1
fi

log "Suite run complete: $SUITE_NAME"

# Score it. Left as an instruction rather than run here: the report needs a
# thresholds file and a store path, both of which are choices about what this
# run is for, and running a gate the operator did not ask for would make a
# suite run exit non-zero for reasons the runner did not decide.
case "$SUITE_FILE" in
  *.suite.json)
    log "To score and record this run:"
    log "  openagents gym results score --suite $(printf '%q' "$SUITE_NAME") --lane $(printf '%q' "$LANE") --append $(printf '%q' "$JOB_DIR")"
    ;;
  *)
    log "This run used a plain task list, so it carries no suite pin and cannot"
    log "be recorded as a score. Re-run it against bench/suites/${SUITE_NAME}.suite.json"
    log "to produce a recordable row."
    ;;
esac
