#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

HOST_HOME="${HOME:-$ROOT_DIR}"
RUN_DIR="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_RUN_DIR:-$ROOT_DIR/target/gpt-oss-nvidia-seller-lane-perf}"
APP_HOME="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_APP_HOME:-$RUN_DIR/app-home}"
APP_LOG_DIR="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_APP_LOG_DIR:-$RUN_DIR/app-logs}"
APP_EXECUTABLE="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_APP_BIN:-$ROOT_DIR/target/release/autopilot-desktop}"
AUTOPILOTCTL_BIN="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_AUTOPILOTCTL_BIN:-$ROOT_DIR/target/release/autopilotctl}"
PSIONIC_SERVER_BIN="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_PSIONIC_BIN:-$ROOT_DIR/target/release/psionic-gpt-oss-server}"
BENCHMARK_SCRIPT="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCHMARK_SCRIPT:-$ROOT_DIR/crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh}"
ARTIFACT_PATH="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_ARTIFACT:-$RUN_DIR/seller-lane-perf.json}"
SUMMARY_PATH="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_SUMMARY:-$RUN_DIR/summary.txt}"
APP_START_TIMEOUT_SECONDS="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_APP_START_TIMEOUT_SECONDS:-90}"
WAIT_TIMEOUT_MS="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_WAIT_TIMEOUT_MS:-180000}"
BENCHMARK_HOST="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_HOST:-127.0.0.1}"
BENCHMARK_PORT="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_PORT:-8099}"
BENCHMARK_CTX="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_CTX:-4096}"
BENCHMARK_NGL="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_NGL:-999}"
BENCHMARK_MAX_TOKENS="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_MAX_TOKENS:-64}"
BENCHMARK_STARTUP_TIMEOUT_SECONDS="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_STARTUP_TIMEOUT_SECONDS:-60}"
BENCHMARK_REQUEST_TIMEOUT_SECONDS="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_BENCH_REQUEST_TIMEOUT_SECONDS:-60}"
SKIP_BUILD="${OPENAGENTS_GPT_OSS_NVIDIA_PERF_SKIP_BUILD:-0}"
GPT_OSS_BACKEND="${OPENAGENTS_GPT_OSS_BACKEND:-cuda}"
GPT_OSS_MODEL_PATH="${OPENAGENTS_GPT_OSS_MODEL_PATH:-$HOST_HOME/models/gpt-oss/gpt-oss-20b-mxfp4.gguf}"
BASELINE_ARTIFACT="${OPENAGENTS_GPT_OSS_NVIDIA_BASELINE_ARTIFACT:-}"

APP_PID=""

log() {
  echo "[check-gpt-oss-nvidia-seller-lane-perf] $*"
}

die() {
  echo "[check-gpt-oss-nvidia-seller-lane-perf] ERROR: $*" >&2
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
        die "Linux NVIDIA perf harness requires DISPLAY or WAYLAND_DISPLAY"
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
    if isinstance(node, list):
        node = node[int(key)]
    else:
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
require_command jq
require_command python3
require_graphical_session

[[ "$GPT_OSS_BACKEND" == "cuda" ]] || die "OPENAGENTS_GPT_OSS_BACKEND must be cuda for this perf harness"
[[ -f "$GPT_OSS_MODEL_PATH" ]] || die "Missing GPT-OSS GGUF at ${GPT_OSS_MODEL_PATH}"
[[ -f "$BENCHMARK_SCRIPT" ]] || die "Missing benchmark script at ${BENCHMARK_SCRIPT}"
if [[ -n "$BASELINE_ARTIFACT" && ! -f "$BASELINE_ARTIFACT" ]]; then
  die "Missing baseline artifact at ${BASELINE_ARTIFACT}"
fi

if [[ "$SKIP_BUILD" != "1" ]]; then
  log "Building release desktop and Psionic benchmark binaries"
  cargo build -p autopilot-desktop --release --bin autopilot-desktop --bin autopilotctl
  cargo build -p psionic-serve --release --bin psionic-gpt-oss-server
fi

[[ -x "$APP_EXECUTABLE" ]] || die "Missing desktop app executable at ${APP_EXECUTABLE}"
[[ -x "$AUTOPILOTCTL_BIN" ]] || die "Missing autopilotctl binary at ${AUTOPILOTCTL_BIN}"
[[ -x "$PSIONIC_SERVER_BIN" ]] || die "Missing Psionic server binary at ${PSIONIC_SERVER_BIN}"

rm -rf "$RUN_DIR"
mkdir -p "$APP_HOME" "$APP_LOG_DIR" "$RUN_DIR/benchmark"

MANIFEST_PATH="${APP_LOG_DIR}/desktop-control.json"
INITIAL_STATUS_PATH="${RUN_DIR}/initial-status.json"
INITIAL_LOCAL_RUNTIME_PATH="${RUN_DIR}/initial-local-runtime.json"
INITIAL_GPT_OSS_PATH="${RUN_DIR}/initial-gpt-oss.json"
FINAL_STATUS_PATH="${RUN_DIR}/final-status.json"
FINAL_LOCAL_RUNTIME_PATH="${RUN_DIR}/final-local-runtime.json"
FINAL_GPT_OSS_PATH="${RUN_DIR}/final-gpt-oss.json"
BENCHMARK_JSON_DIR="${RUN_DIR}/benchmark"

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
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json local-runtime status >"$INITIAL_LOCAL_RUNTIME_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json gpt-oss status >"$INITIAL_GPT_OSS_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json status >"$INITIAL_STATUS_PATH"

assert_json_field "$INITIAL_LOCAL_RUNTIME_PATH" "lane" "gpt_oss"
assert_json_field "$INITIAL_LOCAL_RUNTIME_PATH" "policy" "gpt_oss_cuda"
assert_json_field "$INITIAL_LOCAL_RUNTIME_PATH" "supports_sell_compute" "true"
assert_json_field "$INITIAL_GPT_OSS_PATH" "detected" "true"
assert_json_field "$INITIAL_GPT_OSS_PATH" "backend" "cuda"
assert_json_field "$INITIAL_GPT_OSS_PATH" "artifact_present" "true"

log "Running canonical Psionic CUDA seller-lane benchmark"
bash "$BENCHMARK_SCRIPT" \
  --server psionic \
  --model "$GPT_OSS_MODEL_PATH" \
  --psionic-bin "$PSIONIC_SERVER_BIN" \
  --psionic-backend "$GPT_OSS_BACKEND" \
  --host "$BENCHMARK_HOST" \
  --port "$BENCHMARK_PORT" \
  --ctx "$BENCHMARK_CTX" \
  --ngl "$BENCHMARK_NGL" \
  --max-tokens "$BENCHMARK_MAX_TOKENS" \
  --startup-timeout-seconds "$BENCHMARK_STARTUP_TIMEOUT_SECONDS" \
  --request-timeout-seconds "$BENCHMARK_REQUEST_TIMEOUT_SECONDS" \
  --json-out "$BENCHMARK_JSON_DIR" \
  >"${RUN_DIR}/benchmark.stdout.log" 2>"${RUN_DIR}/benchmark.stderr.log"

log "Warming Mission Control seller lane"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" gpt-oss warm --wait --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/gpt-oss-warm.json"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" wait local-runtime-ready --timeout-ms "$WAIT_TIMEOUT_MS" >"${RUN_DIR}/local-runtime-ready.json"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json local-runtime status >"$FINAL_LOCAL_RUNTIME_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json gpt-oss status >"$FINAL_GPT_OSS_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json status >"$FINAL_STATUS_PATH"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" logs --tail 100 >"${RUN_DIR}/logs-tail.txt"

assert_json_field "$FINAL_LOCAL_RUNTIME_PATH" "lane" "gpt_oss"
assert_json_field "$FINAL_LOCAL_RUNTIME_PATH" "policy" "gpt_oss_cuda"
assert_json_field "$FINAL_LOCAL_RUNTIME_PATH" "runtime_ready" "true"
assert_json_field "$FINAL_LOCAL_RUNTIME_PATH" "go_online_ready" "true"
assert_json_field "$FINAL_LOCAL_RUNTIME_PATH" "supports_sell_compute" "true"
assert_json_field "$FINAL_GPT_OSS_PATH" "detected" "true"
assert_json_field "$FINAL_GPT_OSS_PATH" "backend" "cuda"
assert_json_field "$FINAL_GPT_OSS_PATH" "ready" "true"
assert_json_field "$FINAL_GPT_OSS_PATH" "loaded" "true"
assert_json_field "$FINAL_STATUS_PATH" "snapshot.mission_control.can_go_online" "true"
assert_json_field "$FINAL_STATUS_PATH" "snapshot.local_runtime.lane" "gpt_oss"
assert_json_field "$FINAL_STATUS_PATH" "snapshot.gpt_oss.ready" "true"
json_field "$FINAL_LOCAL_RUNTIME_PATH" "diagnostics.selected_devices.0.stable_device_id" >/dev/null

log "Writing machine-readable seller-lane artifact"
python3 - \
  "$ARTIFACT_PATH" \
  "$SUMMARY_PATH" \
  "$INITIAL_LOCAL_RUNTIME_PATH" \
  "$INITIAL_GPT_OSS_PATH" \
  "$INITIAL_STATUS_PATH" \
  "$FINAL_LOCAL_RUNTIME_PATH" \
  "$FINAL_GPT_OSS_PATH" \
  "$FINAL_STATUS_PATH" \
  "$BENCHMARK_JSON_DIR" \
  "$GPT_OSS_MODEL_PATH" \
  "$BENCHMARK_HOST" \
  "$BENCHMARK_PORT" \
  "$BENCHMARK_CTX" \
  "$BENCHMARK_NGL" \
  "$BENCHMARK_MAX_TOKENS" \
  "$BENCHMARK_STARTUP_TIMEOUT_SECONDS" \
  "$BENCHMARK_REQUEST_TIMEOUT_SECONDS" \
  "$BASELINE_ARTIFACT" <<'PY'
import datetime as dt
import json
import platform
import sys
from pathlib import Path


def load_json(path: str):
    return json.loads(Path(path).read_text())


def ratio(current: float, previous: float):
    if previous == 0:
        return None
    return current / previous


def delta_percent(current: float, previous: float):
    if previous == 0:
        return None
    return ((current - previous) / previous) * 100.0


def scheduler_posture(diagnostics: dict):
    observability = diagnostics.get("observability")
    if not observability:
        return None
    execution_profile = observability.get("execution_profile") or {}
    batch_posture = execution_profile.get("batch_posture")
    queue_policy = execution_profile.get("queue_policy") or {}
    discipline = queue_policy.get("discipline")
    if batch_posture and discipline:
        return f"{batch_posture}/{discipline}"
    return batch_posture or discipline


artifact_path = Path(sys.argv[1])
summary_path = Path(sys.argv[2])
initial_local_runtime = load_json(sys.argv[3])
initial_gpt_oss = load_json(sys.argv[4])
initial_status = load_json(sys.argv[5])
final_local_runtime = load_json(sys.argv[6])
final_gpt_oss = load_json(sys.argv[7])
final_status = load_json(sys.argv[8])
benchmark_dir = Path(sys.argv[9])
model_path = sys.argv[10]
benchmark_host = sys.argv[11]
benchmark_port = int(sys.argv[12])
benchmark_ctx = int(sys.argv[13])
benchmark_ngl = int(sys.argv[14])
benchmark_max_tokens = int(sys.argv[15])
benchmark_startup_timeout_seconds = int(sys.argv[16])
benchmark_request_timeout_seconds = int(sys.argv[17])
baseline_path = sys.argv[18]

case_names = ("cold", "warm_non_hit", "prompt_cache_hit")
benchmark_cases = {
    case_name: load_json(str(benchmark_dir / f"psionic.{case_name}.summary.json"))
    for case_name in case_names
}

final_diagnostics = final_local_runtime["diagnostics"]
selected_devices = final_diagnostics.get("selected_devices", [])

artifact = {
    "artifact_version": 1,
    "generated_at_rfc3339": dt.datetime.now(dt.timezone.utc).isoformat(),
    "host": {
        "platform": platform.system(),
        "platform_release": platform.release(),
        "machine": platform.machine(),
    },
    "seller_lane": {
        "lane": final_local_runtime.get("lane"),
        "policy": final_local_runtime.get("policy"),
        "backend": final_gpt_oss.get("backend"),
        "model_path": model_path,
        "configured_model": final_gpt_oss.get("configured_model"),
        "configured_model_path": final_gpt_oss.get("configured_model_path"),
        "execution_posture": final_diagnostics.get("posture"),
        "scheduler_posture": scheduler_posture(final_diagnostics),
        "selected_devices": selected_devices,
        "runtime_resources": final_diagnostics.get("runtime_resources"),
        "compile_path": final_diagnostics.get("last_compile_path"),
        "last_cold_compile_duration_ns": final_diagnostics.get("last_cold_compile_duration_ns"),
        "last_warm_refresh_duration_ns": final_diagnostics.get("last_warm_refresh_duration_ns"),
        "cache_invalidation": final_diagnostics.get("last_cache_invalidation"),
        "compile_failure": final_diagnostics.get("last_compile_failure"),
    },
    "preflight": {
        "initial": {
            "runtime_ready": initial_local_runtime.get("runtime_ready"),
            "go_online_ready": initial_local_runtime.get("go_online_ready"),
            "supports_sell_compute": initial_local_runtime.get("supports_sell_compute"),
            "execution_posture": initial_local_runtime["diagnostics"].get("posture"),
            "gpt_oss_ready": initial_gpt_oss.get("ready"),
            "gpt_oss_loaded": initial_gpt_oss.get("loaded"),
            "can_go_online": initial_status.get("snapshot", {})
            .get("mission_control", {})
            .get("can_go_online"),
        },
        "final": {
            "runtime_ready": final_local_runtime.get("runtime_ready"),
            "go_online_ready": final_local_runtime.get("go_online_ready"),
            "supports_sell_compute": final_local_runtime.get("supports_sell_compute"),
            "execution_posture": final_diagnostics.get("posture"),
            "gpt_oss_ready": final_gpt_oss.get("ready"),
            "gpt_oss_loaded": final_gpt_oss.get("loaded"),
            "can_go_online": final_status.get("snapshot", {})
            .get("mission_control", {})
            .get("can_go_online"),
        },
    },
    "benchmark_assumptions": {
        "server": "psionic-gpt-oss-server",
        "backend": final_gpt_oss.get("backend"),
        "host": benchmark_host,
        "port": benchmark_port,
        "ctx": benchmark_ctx,
        "ngl": benchmark_ngl,
        "max_tokens": benchmark_max_tokens,
        "startup_timeout_seconds": benchmark_startup_timeout_seconds,
        "request_timeout_seconds": benchmark_request_timeout_seconds,
        "reasoning_budget": 0,
    },
    "benchmark_cases": benchmark_cases,
    "capability_envelope_observation": {
        "tokens_per_second": {
            case_name: benchmark_cases[case_name]["tokens_per_second"]
            for case_name in case_names
        },
        "elapsed_seconds": {
            case_name: benchmark_cases[case_name]["elapsed_seconds"]
            for case_name in case_names
        },
        "warm_non_hit_speedup_over_cold": ratio(
            benchmark_cases["warm_non_hit"]["tokens_per_second"],
            benchmark_cases["cold"]["tokens_per_second"],
        ),
        "prompt_cache_hit_speedup_over_warm_non_hit": ratio(
            benchmark_cases["prompt_cache_hit"]["tokens_per_second"],
            benchmark_cases["warm_non_hit"]["tokens_per_second"],
        ),
        "warm_non_hit_elapsed_delta_vs_cold_percent": delta_percent(
            benchmark_cases["warm_non_hit"]["elapsed_seconds"],
            benchmark_cases["cold"]["elapsed_seconds"],
        ),
        "prompt_cache_hit_elapsed_delta_vs_warm_non_hit_percent": delta_percent(
            benchmark_cases["prompt_cache_hit"]["elapsed_seconds"],
            benchmark_cases["warm_non_hit"]["elapsed_seconds"],
        ),
    },
    "raw_paths": {
        "initial_local_runtime": str(Path(sys.argv[3]).resolve()),
        "initial_gpt_oss": str(Path(sys.argv[4]).resolve()),
        "initial_status": str(Path(sys.argv[5]).resolve()),
        "final_local_runtime": str(Path(sys.argv[6]).resolve()),
        "final_gpt_oss": str(Path(sys.argv[7]).resolve()),
        "final_status": str(Path(sys.argv[8]).resolve()),
        "benchmark_dir": str(benchmark_dir.resolve()),
    },
}

if baseline_path:
    baseline = load_json(baseline_path)
    baseline_cases = baseline.get("benchmark_cases", {})
    case_deltas = {}
    regressions = []
    for case_name in case_names:
      current_case = benchmark_cases[case_name]
      baseline_case = baseline_cases.get(case_name)
      if not baseline_case:
          continue
      tokps_delta_percent = delta_percent(
          current_case["tokens_per_second"],
          baseline_case["tokens_per_second"],
      )
      elapsed_delta_percent = delta_percent(
          current_case["elapsed_seconds"],
          baseline_case["elapsed_seconds"],
      )
      case_deltas[case_name] = {
          "current_tokens_per_second": current_case["tokens_per_second"],
          "baseline_tokens_per_second": baseline_case["tokens_per_second"],
          "delta_tokens_per_second": current_case["tokens_per_second"] - baseline_case["tokens_per_second"],
          "delta_tokens_per_second_percent": tokps_delta_percent,
          "current_elapsed_seconds": current_case["elapsed_seconds"],
          "baseline_elapsed_seconds": baseline_case["elapsed_seconds"],
          "delta_elapsed_seconds": current_case["elapsed_seconds"] - baseline_case["elapsed_seconds"],
          "delta_elapsed_seconds_percent": elapsed_delta_percent,
      }
      if tokps_delta_percent is not None and tokps_delta_percent < 0:
          regressions.append(
              f"{case_name} tokens_per_second regressed by {abs(tokps_delta_percent):.2f}%"
          )
      if elapsed_delta_percent is not None and elapsed_delta_percent > 0:
          regressions.append(
              f"{case_name} elapsed_seconds regressed by {elapsed_delta_percent:.2f}%"
          )
    artifact["baseline_comparison"] = {
        "baseline_artifact": str(Path(baseline_path).resolve()),
        "case_deltas": case_deltas,
        "regressions": regressions,
    }

artifact_path.parent.mkdir(parents=True, exist_ok=True)
artifact_path.write_text(json.dumps(artifact, indent=2) + "\n")

summary_lines = [
    f"artifact={artifact_path}",
    f"backend={artifact['seller_lane']['backend']}",
    f"model_path={artifact['seller_lane']['model_path']}",
    f"device_count={len(selected_devices)}",
    f"initial_posture={artifact['preflight']['initial']['execution_posture']}",
    f"final_posture={artifact['preflight']['final']['execution_posture']}",
    f"can_go_online={artifact['preflight']['final']['can_go_online']}",
    "tokens_per_second:"
]
for case_name in case_names:
    summary_lines.append(
        f"  {case_name}={benchmark_cases[case_name]['tokens_per_second']:.2f}"
    )
summary_lines.append("elapsed_seconds:")
for case_name in case_names:
    summary_lines.append(
        f"  {case_name}={benchmark_cases[case_name]['elapsed_seconds']:.3f}"
    )
if baseline_path:
    regressions = artifact["baseline_comparison"]["regressions"]
    summary_lines.append(f"baseline_artifact={baseline_path}")
    summary_lines.append(f"baseline_regressions={len(regressions)}")
summary_path.write_text("\n".join(summary_lines) + "\n")
PY

log "Seller-lane perf harness passed"
log "Artifact: ${ARTIFACT_PATH}"
log "Summary: ${SUMMARY_PATH}"
