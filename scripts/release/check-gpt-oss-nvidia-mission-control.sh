#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

HOST_HOME="${HOME:-$ROOT_DIR}"
RUN_DIR="${OPENAGENTS_GPT_OSS_NVIDIA_RUN_DIR:-$ROOT_DIR/target/gpt-oss-nvidia-mission-control}"
APP_HOME="${OPENAGENTS_GPT_OSS_NVIDIA_APP_HOME:-$RUN_DIR/app-home}"
APP_LOG_DIR="${OPENAGENTS_GPT_OSS_NVIDIA_APP_LOG_DIR:-$RUN_DIR/app-logs}"
APP_EXECUTABLE="${OPENAGENTS_GPT_OSS_NVIDIA_APP_BIN:-$ROOT_DIR/target/release/autopilot-desktop}"
AUTOPILOTCTL_BIN="${OPENAGENTS_GPT_OSS_NVIDIA_AUTOPILOTCTL_BIN:-$ROOT_DIR/target/release/autopilotctl}"
APP_START_TIMEOUT_SECONDS="${OPENAGENTS_GPT_OSS_NVIDIA_APP_START_TIMEOUT_SECONDS:-90}"
WAIT_TIMEOUT_MS="${OPENAGENTS_GPT_OSS_NVIDIA_WAIT_TIMEOUT_MS:-180000}"
SKIP_BUILD="${OPENAGENTS_GPT_OSS_NVIDIA_SKIP_BUILD:-0}"
GPT_OSS_BACKEND="${OPENAGENTS_GPT_OSS_BACKEND:-cuda}"
GPT_OSS_MODEL_PATH="${OPENAGENTS_GPT_OSS_MODEL_PATH:-$HOST_HOME/models/gpt-oss/gpt-oss-20b-mxfp4.gguf}"

APP_PID=""

log() {
  echo "[check-gpt-oss-nvidia-mission-control] $*"
}

die() {
  echo "[check-gpt-oss-nvidia-mission-control] ERROR: $*" >&2
  exit 1
}

require_command() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Missing required command: $cmd"
}

require_graphical_session() {
  case "$(uname -s)" in
    Linux)
      if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
        die "Linux NVIDIA smoke check requires DISPLAY or WAYLAND_DISPLAY"
      fi
      ;;
  esac
}

wait_for_file() {
  local path="$1"
  local timeout_seconds="$2"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    if [[ -f "$path" ]]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_manifest_status() {
  local manifest="$1"
  local timeout_seconds="$2"
  local output_path="$3"
  local deadline=$((SECONDS + timeout_seconds))
  while (( SECONDS < deadline )); do
    if "$AUTOPILOTCTL_BIN" --manifest "$manifest" --json status >"$output_path" 2>/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

json_field() {
  local file="$1"
  local field="$2"
  python3 - "$file" "$field" <<'PY'
import json, pathlib, re, sys

text = pathlib.Path(sys.argv[1]).read_text()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
decoder = json.JSONDecoder()
value = None
for index, ch in enumerate(text):
    if ch not in "{[":
        continue
    try:
        value, _ = decoder.raw_decode(text[index:])
        break
    except json.JSONDecodeError:
        continue
if value is None:
    raise SystemExit(f"failed to decode JSON payload from {sys.argv[1]}")
node = value
for key in sys.argv[2].split("."):
    if not key:
        continue
    node = node[key]
if isinstance(node, bool):
    print("true" if node else "false")
else:
    print(node)
PY
}

assert_json_field() {
  local file="$1"
  local field="$2"
  local expected="$3"
  local actual
  actual="$(json_field "$file" "$field")"
  if [[ "$actual" != "$expected" ]]; then
    die "Expected ${field}=${expected}, got ${actual} (${file})"
  fi
}

cleanup() {
  set +e
  if [[ -n "$APP_PID" ]]; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

require_command cargo
require_command python3
require_graphical_session

[[ "$GPT_OSS_BACKEND" == "cuda" ]] || die "OPENAGENTS_GPT_OSS_BACKEND must be cuda for this smoke check"
[[ -f "$GPT_OSS_MODEL_PATH" ]] || die "Missing GPT-OSS GGUF at ${GPT_OSS_MODEL_PATH}"

if [[ "$SKIP_BUILD" != "1" ]]; then
  log "Building release desktop binaries"
  cargo build -p autopilot-desktop --release --bin autopilot-desktop --bin autopilotctl
fi

[[ -x "$APP_EXECUTABLE" ]] || die "Missing desktop app executable at ${APP_EXECUTABLE}"
[[ -x "$AUTOPILOTCTL_BIN" ]] || die "Missing autopilotctl binary at ${AUTOPILOTCTL_BIN}"

rm -rf "$RUN_DIR"
mkdir -p "$APP_HOME" "$APP_LOG_DIR"

MANIFEST_PATH="${APP_LOG_DIR}/desktop-control.json"
INITIAL_STATUS_PATH="${RUN_DIR}/initial-status.json"
LOCAL_RUNTIME_PATH="${RUN_DIR}/local-runtime.json"
GPT_OSS_STATUS_PATH="${RUN_DIR}/gpt-oss.json"
FINAL_STATUS_PATH="${RUN_DIR}/final-status.json"

log "Launching desktop app with GPT-OSS CUDA env"
HOME="$APP_HOME" \
OPENAGENTS_AUTOPILOT_LOG_DIR="$APP_LOG_DIR" \
OPENAGENTS_GPT_OSS_BACKEND="$GPT_OSS_BACKEND" \
OPENAGENTS_GPT_OSS_MODEL_PATH="$GPT_OSS_MODEL_PATH" \
"$APP_EXECUTABLE" >"${RUN_DIR}/app.stdout.log" 2>"${RUN_DIR}/app.stderr.log" &
APP_PID=$!

wait_for_file "$MANIFEST_PATH" "$APP_START_TIMEOUT_SECONDS" \
  || die "Timed out waiting for desktop control manifest ${MANIFEST_PATH}"
wait_for_manifest_status "$MANIFEST_PATH" "$APP_START_TIMEOUT_SECONDS" "$INITIAL_STATUS_PATH" \
  || die "Timed out waiting for desktop control status"

log "Refreshing active local runtime lane"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" local-runtime refresh >"${RUN_DIR}/local-runtime-refresh.json"

log "Warming configured GPT-OSS GGUF"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" gpt-oss warm --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/gpt-oss-warm.json"

log "Waiting for Mission Control local runtime readiness"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" wait local-runtime-ready --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/local-runtime-ready.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json local-runtime status >"$LOCAL_RUNTIME_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json gpt-oss status >"$GPT_OSS_STATUS_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json status >"$FINAL_STATUS_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" logs --tail 100 >"${RUN_DIR}/logs-tail.txt"

assert_json_field "$LOCAL_RUNTIME_PATH" "lane" "gpt_oss"
assert_json_field "$LOCAL_RUNTIME_PATH" "policy" "gpt_oss_cuda"
assert_json_field "$LOCAL_RUNTIME_PATH" "runtime_ready" "true"
assert_json_field "$LOCAL_RUNTIME_PATH" "go_online_ready" "true"
assert_json_field "$LOCAL_RUNTIME_PATH" "supports_sell_compute" "true"
assert_json_field "$LOCAL_RUNTIME_PATH" "supports_model_management" "true"
assert_json_field "$LOCAL_RUNTIME_PATH" "action" "unload_gpt_oss"

assert_json_field "$GPT_OSS_STATUS_PATH" "detected" "true"
assert_json_field "$GPT_OSS_STATUS_PATH" "backend" "cuda"
assert_json_field "$GPT_OSS_STATUS_PATH" "ready" "true"
assert_json_field "$GPT_OSS_STATUS_PATH" "loaded" "true"
assert_json_field "$GPT_OSS_STATUS_PATH" "artifact_present" "true"
assert_json_field "$GPT_OSS_STATUS_PATH" "supports_sell_compute" "true"

assert_json_field "$FINAL_STATUS_PATH" "snapshot.mission_control.can_go_online" "true"
assert_json_field "$FINAL_STATUS_PATH" "snapshot.local_runtime.lane" "gpt_oss"
assert_json_field "$FINAL_STATUS_PATH" "snapshot.local_runtime.go_online_ready" "true"
assert_json_field "$FINAL_STATUS_PATH" "snapshot.gpt_oss.ready" "true"

log "NVIDIA GPT-OSS Mission Control smoke passed"
