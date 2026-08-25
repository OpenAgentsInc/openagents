#!/usr/bin/env bash
set -euo pipefail

# Gym suite runner. Packs the working-tree openagents CLI, runs a Terminal-Bench
# suite through Harbor, and posts the graded result to the OpenAgents Gym.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$REPO_ROOT/bench"
CLI_DIR="$REPO_ROOT/packages/openagents-cli"

usage() {
  cat <<'EOF'
Usage: bench/run-suite.sh <suite-file> --model <harbor-model> [options]

Arguments:
  <suite-file>            Path to a suite file: one task name per line. Lines
                          that are empty or start with # (after whitespace) are
                          ignored.

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
  --dry-run               Print the commands that would run without executing.
  -h, --help              Show this help and exit.

Environment:
  OPENAGENTS_TOKEN        Required for the Harbor run unless --model starts
                          with ollama/. Required for posting results.
  OPENAGENTS_CODER_API_URL Overrides the coder container API URL.

Examples:
  bench/run-suite.sh bench/suites/tb2-cross-section.txt \
    --model openai/gpt-5.6-luna --lane proxy --n-concurrent 2

  bench/run-suite.sh bench/suites/local-llm.txt \
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

# Parse tasks: strip comments, trim whitespace, drop blanks.
TASKS=()
while IFS= read -r line; do
  [ -n "$line" ] || continue
  TASKS+=("$line")
done < <(sed -e 's/#.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' "$SUITE_FILE" | sed -n '/./p')

if [ "${#TASKS[@]}" -eq 0 ]; then
  log "Suite file contains no tasks: $SUITE_FILE"
  exit 1
fi

TASK_ARGS=()
for task in "${TASKS[@]}"; do
  TASK_ARGS+=("-i" "$task")
done

SUITE_NAME="$(basename "$SUITE_FILE" .txt)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"

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
  echo "[dry-run] Pack command:"
  echo "[dry-run]   (cd $(printf '%q' packages/openagents-cli) && pnpm build && pnpm pack --pack-destination $(printf '%q' ../../bench))"
  echo
  echo "[dry-run] Harbor command:"
  echo -n "[dry-run]   "
  printf '%q ' \
    "PYTHONPATH=$BENCH_DIR" \
    "OPENAGENTS_TOKEN=$TOKEN_DISPLAY" \
    "OPENAGENTS_CODER_API_URL=$CANDIDATE_CODER_API_URL" \
    harbor run \
    "${HARBOR_ARGS[@]}"
  echo
  echo
  echo "[dry-run] Post command (after locating the job directory under $JOBS_DIR):"
  echo "[dry-run]   python3 $(printf '%q' "$BENCH_DIR/post_gym_run.py") <job-dir> --api-url $(printf '%q' "$API_URL") --lane $(printf '%q' "$LANE") --suite $(printf '%q' "$SUITE_NAME")"
  exit 0
fi

# Require token for the Harbor run unless this is an ollama model.
if [ -z "$OPENAGENTS_TOKEN" ] && [[ "$MODEL" != ollama/* ]]; then
  log "OPENAGENTS_TOKEN is required for non-ollama models."
  exit 1
fi

# Pack the CLI tarball.
log "Packing openagents-cli tarball..."
(
  cd "$CLI_DIR"
  pnpm build
  pnpm pack --pack-destination ../../bench
)

if ! ls "$BENCH_DIR"/openagentsinc-cli-*.tgz >/dev/null 2>&1; then
  log "No openagentsinc-cli-*.tgz tarball found in $BENCH_DIR after pack."
  exit 1
fi

log "Running Harbor suite: $SUITE_NAME (${#TASKS[@]} tasks)..."
(
  export PYTHONPATH="$BENCH_DIR"
  export OPENAGENTS_TOKEN
  if [ -n "${OPENAGENTS_CODER_API_URL:-}" ]; then
    export OPENAGENTS_CODER_API_URL
  else
    export OPENAGENTS_CODER_API_URL="$CANDIDATE_CODER_API_URL"
  fi
  harbor run "${HARBOR_ARGS[@]}"
)

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
  exit 1
fi

log "Harbor job directory: $JOB_DIR"

# Post the graded result.
if [ -z "$OPENAGENTS_TOKEN" ]; then
  log "OPENAGENTS_TOKEN is not set; skipping result post."
  log "To post later, run:"
  log "  OPENAGENTS_TOKEN=... python3 $(printf '%q' "$BENCH_DIR/post_gym_run.py") $(printf '%q' "$JOB_DIR") --api-url $(printf '%q' "$API_URL") --lane $(printf '%q' "$LANE") --suite $(printf '%q' "$SUITE_NAME")"
  exit 0
fi

log "Posting result to $API_URL..."
python3 "$BENCH_DIR/post_gym_run.py" "$JOB_DIR" --api-url "$API_URL" --lane "$LANE" --suite "$SUITE_NAME"

log "Suite run complete: $SUITE_NAME"
