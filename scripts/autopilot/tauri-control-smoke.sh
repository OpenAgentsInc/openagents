#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_DIR="$ROOT/apps/autopilot"
RUN_ROOT="${OPENAGENTS_AUTOPILOT_TAURI_SMOKE_DIR:-$ROOT/target/autopilot-tauri-control-smoke}"
MANIFEST="$RUN_ROOT/tauri-control.json"
LOG="$RUN_ROOT/tauri-dev.log"
STATUS_JSON="$RUN_ROOT/status.json"
SMOKE_JSON="$RUN_ROOT/smoke.json"
FAKE_BIN_DIR="$RUN_ROOT/bin"
FAKE_STATE_DIR="$RUN_ROOT/fake-state"
PYLON_HOME="$RUN_ROOT/pylon-home"
PYLON_CONFIG_PATH="${OPENAGENTS_AUTOPILOT_TAURI_PYLON_CONFIG_PATH:-$PYLON_HOME/config.json}"
PYLON_CONFIG_DIR="$(dirname "$PYLON_CONFIG_PATH")"
PROOF_ROOT="${OPENAGENTS_AUTOPILOT_PROOF_ROOT:-$PYLON_CONFIG_DIR/proof/namespaces}"
PYLON_ADMIN_LISTEN_ADDR="${OPENAGENTS_AUTOPILOT_TAURI_PYLON_ADMIN_LISTEN_ADDR:-}"
NAMESPACE="${OPENAGENTS_AUTOPILOT_TAURI_SMOKE_NAMESPACE:-proof.autopilot.ctl.smoke.$(date +%s)}"
TIMEOUT_MS="${OPENAGENTS_AUTOPILOT_TAURI_SMOKE_TIMEOUT_MS:-180000}"
START_TIMEOUT_SECONDS="${OPENAGENTS_AUTOPILOT_TAURI_START_TIMEOUT_SECONDS:-90}"
KEEP_RUNNING=0
STATUS_ONLY=0
HOMEWORK_MATRIX=0
USE_FAKE_BINARIES=1
REAL_PYLON_BINARY=""
REAL_OA_BINARY=""
TAURI_DEV_PID=""
APP_PID=""

usage() {
  cat <<USAGE
Usage: scripts/autopilot/tauri-control-smoke.sh [options]

Launch the Autopilot Tauri dev app and drive the same Rust command flow through
autopilotctl-tauri.

Options:
  --namespace <value>   Proof namespace or namespace prefix for the run.
  --timeout-ms <value>  Proof wait timeout. Default: $TIMEOUT_MS.
  --manifest <path>     Control manifest path. Default: $MANIFEST.
  --status-only         Launch and verify control status only.
  --homework-matrix     Run clean, replacement, and stale homework proof lanes.
  --real-binaries       Use the machine's real pylon and oa binaries.
  --keep-running        Leave the Tauri app running after the command completes.
  -h, --help            Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --namespace)
      shift
      NAMESPACE="${1:?--namespace requires a value}"
      ;;
    --timeout-ms)
      shift
      TIMEOUT_MS="${1:?--timeout-ms requires a value}"
      ;;
    --manifest)
      shift
      MANIFEST="${1:?--manifest requires a value}"
      ;;
    --status-only)
      STATUS_ONLY=1
      ;;
    --homework-matrix)
      HOMEWORK_MATRIX=1
      ;;
    --real-binaries)
      USE_FAKE_BINARIES=0
      ;;
    --keep-running)
      KEEP_RUNNING=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

mkdir -p "$(dirname "$MANIFEST")"
RUN_ROOT="$(cd "$(dirname "$MANIFEST")" && pwd)"
LOG="$RUN_ROOT/tauri-dev.log"
STATUS_JSON="$RUN_ROOT/status.json"
SMOKE_JSON="$RUN_ROOT/smoke.json"
FAKE_BIN_DIR="$RUN_ROOT/bin"
FAKE_STATE_DIR="$RUN_ROOT/fake-state"
PYLON_HOME="$RUN_ROOT/pylon-home"
PYLON_CONFIG_PATH="${OPENAGENTS_AUTOPILOT_TAURI_PYLON_CONFIG_PATH:-$PYLON_HOME/config.json}"
PYLON_CONFIG_DIR="$(dirname "$PYLON_CONFIG_PATH")"
PROOF_ROOT="${OPENAGENTS_AUTOPILOT_PROOF_ROOT:-$PYLON_CONFIG_DIR/proof/namespaces}"
PYLON_ADMIN_LISTEN_ADDR="${OPENAGENTS_AUTOPILOT_TAURI_PYLON_ADMIN_LISTEN_ADDR:-}"

manifest_pid() {
  if [[ -s "$MANIFEST" ]]; then
    sed -n 's/.*"pid": \([0-9][0-9]*\).*/\1/p' "$MANIFEST" | head -n 1
  fi
}

autopilotctl_tauri() {
  if [[ -x "$ROOT/target/debug/autopilotctl-tauri" ]]; then
    "$ROOT/target/debug/autopilotctl-tauri" "$@"
  else
    cargo run -p autopilot --bin autopilotctl-tauri -- "$@"
  fi
}

stop_control_runtime() {
  [[ -s "$MANIFEST" ]] || return
  if [[ "$HOMEWORK_MATRIX" == "1" ]]; then
    for suffix in clean replacement stale; do
      autopilotctl_tauri --manifest "$MANIFEST" proof stop "$NAMESPACE.$suffix" >/dev/null 2>&1 || true
    done
  else
    autopilotctl_tauri --manifest "$MANIFEST" proof stop "$NAMESPACE" >/dev/null 2>&1 || true
  fi
  autopilotctl_tauri --manifest "$MANIFEST" pylon stop >/dev/null 2>&1 || true
}

cleanup() {
  if [[ "$KEEP_RUNNING" == "1" ]]; then
    return
  fi
  stop_control_runtime
  APP_PID="$(manifest_pid || true)"
  if [[ -n "$APP_PID" ]]; then
    kill "$APP_PID" 2>/dev/null || true
  fi
  if [[ -n "$TAURI_DEV_PID" ]]; then
    pkill -TERM -P "$TAURI_DEV_PID" 2>/dev/null || true
    kill "$TAURI_DEV_PID" 2>/dev/null || true
    wait "$TAURI_DEV_PID" 2>/dev/null || true
  fi
  if [[ "$USE_FAKE_BINARIES" != "1" && -n "$PYLON_CONFIG_PATH" ]]; then
    while IFS= read -r pid; do
      [[ -n "$pid" ]] || continue
      [[ "$pid" == "$$" ]] && continue
      kill "$pid" 2>/dev/null || true
    done < <(pgrep -f "$PYLON_CONFIG_PATH" 2>/dev/null || true)
    while IFS= read -r pid; do
      [[ -n "$pid" ]] || continue
      [[ "$pid" == "$$" ]] && continue
      kill "$pid" 2>/dev/null || true
    done < <(pgrep -f "$RUN_ROOT" 2>/dev/null || true)
  fi
}
trap cleanup EXIT

rm -f "$MANIFEST" "$STATUS_JSON" "$SMOKE_JSON"

resolve_real_binary() {
  local env_var="$1"
  local name="$2"
  local package="$3"
  local configured="${!env_var:-}"
  if [[ -n "$configured" ]]; then
    if [[ -x "$configured" ]]; then
      printf '%s' "$configured"
      return 0
    fi
    echo "$env_var points at a non-executable path: $configured" >&2
    exit 1
  fi

  local candidate="$ROOT/target/debug/$name"
  cargo build -p "$package" --bin "$name" >/dev/null
  if [[ ! -x "$candidate" ]]; then
    echo "failed to build or locate $name at $candidate" >&2
    exit 1
  fi
  printf '%s' "$candidate"
}

choose_loopback_listen_addr() {
  python3 - <<'PY'
import socket

sock = socket.socket()
sock.bind(("127.0.0.1", 0))
host, port = sock.getsockname()
sock.close()
print(f"{host}:{port}")
PY
}

bootstrap_real_pylon_config() {
  mkdir -p "$PYLON_HOME" "$PYLON_CONFIG_DIR" "$PROOF_ROOT"
  REAL_PYLON_BINARY="$(resolve_real_binary OPENAGENTS_PYLON_BINARY pylon pylon)"
  REAL_OA_BINARY="$(resolve_real_binary OPENAGENTS_OA_BINARY oa pylon)"
  cargo build -p nexus-relay --bin nexus-relay >/dev/null
  if [[ -z "$PYLON_ADMIN_LISTEN_ADDR" ]]; then
    PYLON_ADMIN_LISTEN_ADDR="$(choose_loopback_listen_addr)"
  fi

  OPENAGENTS_PYLON_HOME="$PYLON_HOME" \
    "$REAL_PYLON_BINARY" --config-path "$PYLON_CONFIG_PATH" init >/dev/null
  OPENAGENTS_PYLON_HOME="$PYLON_HOME" \
    "$REAL_PYLON_BINARY" --config-path "$PYLON_CONFIG_PATH" config set admin_listen_addr "$PYLON_ADMIN_LISTEN_ADDR" >/dev/null
  OPENAGENTS_PYLON_HOME="$PYLON_HOME" \
    "$REAL_PYLON_BINARY" --config-path "$PYLON_CONFIG_PATH" config set local_gemma_base_url http://127.0.0.1:9 >/dev/null
  OPENAGENTS_PYLON_HOME="$PYLON_HOME" \
    "$REAL_PYLON_BINARY" --config-path "$PYLON_CONFIG_PATH" config set training.run_root "$PYLON_CONFIG_DIR/training" >/dev/null
}

if [[ "$USE_FAKE_BINARIES" == "1" ]]; then
  mkdir -p "$FAKE_BIN_DIR" "$FAKE_STATE_DIR" "$PYLON_HOME" "$PROOF_ROOT"

  cat >"$FAKE_BIN_DIR/pylon" <<'PYLON'
#!/usr/bin/env bash
set -euo pipefail

CONFIG_PATH=""
if [[ "${1:-}" == "--config-path" ]]; then
  CONFIG_PATH="${2:-}"
  shift 2
fi
COMMAND="${1:-status}"
STATE_FILE="${OPENAGENTS_AUTOPILOT_FAKE_PYLON_STATE:?missing fake pylon state}"
mkdir -p "$(dirname "$STATE_FILE")"
if [[ -n "$CONFIG_PATH" ]]; then
  mkdir -p "$(dirname "$CONFIG_PATH")"
  [[ -f "$CONFIG_PATH" ]] || printf '{"fake":true}\n' >"$CONFIG_PATH"
fi

mode="$(cat "$STATE_FILE" 2>/dev/null || printf 'offline')"

write_mode() {
  mode="$1"
  printf '%s\n' "$mode" >"$STATE_FILE"
}

status_json() {
  cat <<JSON
{
  "listen_addr": "127.0.0.1:9468",
  "desired_mode": "$mode",
  "snapshot": {
    "runtime": {
      "authoritative_status": "$mode",
      "execution_backend_label": "fake-control-smoke",
      "queue_depth": 0,
      "online_uptime_seconds": 1,
      "provider_blocker_codes": [],
      "last_action": "fake pylon $COMMAND",
      "last_error": null
    },
    "availability": {
      "local_gemma": {
        "ready_model": "fake:gemma"
      }
    },
    "inventory_rows": [
      {"eligible": true}
    ]
  }
}
JSON
}

case "$COMMAND" in
  serve)
    write_mode "$mode"
    while true; do
      sleep 60
    done
    ;;
  status)
    status_json
    ;;
  online|offline|pause|resume)
    write_mode "$COMMAND"
    status_json
    ;;
  *)
    echo "unsupported fake pylon command: $COMMAND" >&2
    exit 2
    ;;
esac
PYLON

  cat >"$FAKE_BIN_DIR/oa" <<'OA'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "proof" ]]; then
  echo "unsupported fake oa command: $*" >&2
  exit 2
fi
shift
SUBCOMMAND="${1:-}"
shift || true

PROOF_ROOT="${OPENAGENTS_AUTOPILOT_PROOF_ROOT:?missing fake proof root}"

arg_value() {
  local flag="$1"
  shift
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == "$flag" ]]; then
      shift
      printf '%s' "${1:-}"
      return 0
    fi
    shift
  done
  return 1
}

write_artifacts() {
  local lane="$1"
  local namespace="$2"
  local detail="fake Autopilot Tauri control smoke completed"
  local closeout_stage="rewarded"
  if [[ "$lane" == "cs336-a1-replacement-attempt" ]]; then
    detail="fake replacement attempt sealed and reconciled locally"
    closeout_stage="delivered"
  fi
  local root="$PROOF_ROOT/$namespace"
  local fleet="$root/fleet"
  local artifacts="$root/artifacts"
  mkdir -p "$fleet" "$artifacts"
  cat >"$fleet/run-report.json" <<JSON
{
  "lane": "$lane",
  "status": "completed",
  "detail": "$detail",
  "observed_run": {
    "run": {
      "training_run_id": "fake-run-$namespace"
    }
  }
}
JSON
  cat >"$fleet/authority-state-trace.json" <<JSON
{
  "lane": "$lane",
  "status": "completed",
  "transport": {
    "authority_front_door": [{"ok": true}],
    "relay": {"authority_running": true},
    "artifact_store": [{"ok": true}],
    "node_surfaces": [{"ok": true}]
  },
  "node_traces": [
    {
      "role": "worker",
      "index": 0,
      "node_label": "fake-worker-0",
      "eligibility": {
        "eligibility": "eligible",
        "hard_gate_reasons": []
      },
      "retained_state_fixture_id": "fake-retained-state",
      "training_status": {
        "status": "completed"
      }
    },
    {
      "role": "validator",
      "index": 0,
      "node_label": "fake-validator-0",
      "eligibility": {
        "eligibility": "eligible",
        "hard_gate_reasons": []
      },
      "retained_state_fixture_id": "fake-validator-state",
      "training_status": {
        "status": "completed"
      }
    }
  ]
}
JSON
  cat >"$fleet/proof-summary.json" <<JSON
{
  "lane": "$lane",
  "status": "completed",
  "detail": "$detail",
  "window_id": "fake-window",
  "assignment_id": "fake-assignment",
  "lease_id": "fake-lease",
  "membership_revision": "fake-membership",
  "closeout_stage": "$closeout_stage",
  "closeout_next_action": "none",
  "closeout_last_error": null
}
JSON
  printf '{"event":"fake-object-trace","namespace":"%s"}\n' "$namespace" >"$artifacts/object-trace.jsonl"
}

transport_json() {
  cat <<JSON
{
  "authority_front_door": [{"ok": true}],
  "relay": {"authority_running": true},
  "artifact_store": [{"ok": true}],
  "node_surfaces": [{"ok": true}]
}
JSON
}

case "$SUBCOMMAND" in
  run)
    lane="${1:-cs336-a1-replacement-attempt}"
    shift || true
    namespace="$(arg_value --namespace "$@" || true)"
    namespace="${namespace:-proof.autopilot.fake}"
    write_artifacts "$lane" "$namespace"
    printf '{"lane":"%s","namespace":"%s","status":"completed"}\n' "$lane" "$namespace"
    ;;
  doctor)
    namespace="$(arg_value --namespace "$@" || true)"
    namespace="${namespace:-proof.autopilot.fake}"
    printf '{"configured":true,"namespace":"%s","transport":' "$namespace"
    transport_json
    printf '}\n'
    ;;
  fleet)
    action="${1:-}"
    shift || true
    namespace="$(arg_value --namespace "$@" || true)"
    namespace="${namespace:-proof.autopilot.fake}"
    printf '{"namespace":"%s","action":"fleet-%s","status":"ok"}\n' "$namespace" "$action"
    ;;
  authority)
    action="${1:-}"
    shift || true
    namespace="$(arg_value --namespace "$@" || true)"
    namespace="${namespace:-proof.autopilot.fake}"
    printf '{"namespace":"%s","action":"authority-%s","authority_process":{"running":false},"artifact_store_process":{"running":false}}\n' "$namespace" "$action"
    ;;
  *)
    echo "unsupported fake oa proof command: $SUBCOMMAND" >&2
    exit 2
    ;;
esac
OA

  chmod +x "$FAKE_BIN_DIR/pylon" "$FAKE_BIN_DIR/oa"
else
  bootstrap_real_pylon_config
fi

(
  cd "$APP_DIR"
  if [[ "$USE_FAKE_BINARIES" == "1" ]]; then
    OPENAGENTS_AUTOPILOT_CONTROL_MANIFEST="$MANIFEST" \
      OPENAGENTS_AUTOPILOT_CONTROL_BIND="${OPENAGENTS_AUTOPILOT_CONTROL_BIND:-127.0.0.1:0}" \
      OPENAGENTS_AUTOPILOT_FAKE_PYLON_STATE="$FAKE_STATE_DIR/pylon-mode" \
      OPENAGENTS_AUTOPILOT_PYLON_HOME="$PYLON_HOME" \
      OPENAGENTS_AUTOPILOT_PYLON_CONFIG_PATH="$PYLON_CONFIG_PATH" \
      OPENAGENTS_AUTOPILOT_PROOF_ROOT="$PROOF_ROOT" \
      OPENAGENTS_PYLON_HOME="$PYLON_HOME" \
      OPENAGENTS_PYLON_CONFIG_PATH="$PYLON_CONFIG_PATH" \
      OPENAGENTS_PYLON_BINARY="$FAKE_BIN_DIR/pylon" \
      OPENAGENTS_OA_BINARY="$FAKE_BIN_DIR/oa" \
      bun run tauri dev
  else
    OPENAGENTS_AUTOPILOT_CONTROL_MANIFEST="$MANIFEST" \
      OPENAGENTS_AUTOPILOT_CONTROL_BIND="${OPENAGENTS_AUTOPILOT_CONTROL_BIND:-127.0.0.1:0}" \
      OPENAGENTS_AUTOPILOT_PYLON_HOME="$PYLON_HOME" \
      OPENAGENTS_AUTOPILOT_PYLON_CONFIG_PATH="$PYLON_CONFIG_PATH" \
      OPENAGENTS_AUTOPILOT_PROOF_ROOT="$PROOF_ROOT" \
      OPENAGENTS_PYLON_HOME="$PYLON_HOME" \
      OPENAGENTS_PYLON_CONFIG_PATH="$PYLON_CONFIG_PATH" \
      OPENAGENTS_PYLON_BINARY="$REAL_PYLON_BINARY" \
      OPENAGENTS_OA_BINARY="$REAL_OA_BINARY" \
      bun run tauri dev
  fi
) >"$LOG" 2>&1 &
TAURI_DEV_PID=$!

started_at=$SECONDS
until [[ -s "$MANIFEST" ]]; do
  if ! kill -0 "$TAURI_DEV_PID" 2>/dev/null; then
    echo "Autopilot Tauri dev process exited before writing $MANIFEST" >&2
    tail -n 120 "$LOG" >&2 || true
    exit 1
  fi
  if (( SECONDS - started_at > START_TIMEOUT_SECONDS )); then
    echo "Timed out after ${START_TIMEOUT_SECONDS}s waiting for $MANIFEST" >&2
    tail -n 120 "$LOG" >&2 || true
    exit 1
  fi
  sleep 1
done

cargo run -p autopilot --bin autopilotctl-tauri -- \
  --manifest "$MANIFEST" \
  --json \
  status >"$STATUS_JSON"

if [[ "$STATUS_ONLY" == "1" ]]; then
  cat "$STATUS_JSON"
  exit 0
fi

if [[ "$HOMEWORK_MATRIX" == "1" ]]; then
  cargo run -p autopilot --bin autopilotctl-tauri -- \
    --manifest "$MANIFEST" \
    --json \
    homework matrix \
    --namespace-prefix "$NAMESPACE" \
    --timeout-ms "$TIMEOUT_MS" | tee "$SMOKE_JSON"
else
  cargo run -p autopilot --bin autopilotctl-tauri -- \
    --manifest "$MANIFEST" \
    --json \
    smoke \
    --namespace "$NAMESPACE" \
    --timeout-ms "$TIMEOUT_MS" | tee "$SMOKE_JSON"
fi

if [[ "$HOMEWORK_MATRIX" == "1" ]]; then
  echo "Autopilot Tauri homework proof matrix complete"
else
  echo "Autopilot Tauri control smoke complete"
fi
if [[ "$USE_FAKE_BINARIES" == "1" ]]; then
  echo "binaries: fake deterministic pylon/oa"
else
  echo "binaries: machine real pylon/oa"
fi
echo "manifest: $MANIFEST"
echo "pylon_config: $PYLON_CONFIG_PATH"
echo "pylon_home: $PYLON_HOME"
echo "pylon_admin: ${PYLON_ADMIN_LISTEN_ADDR:-fake}"
echo "status: $STATUS_JSON"
echo "result: $SMOKE_JSON"
echo "log: $LOG"
