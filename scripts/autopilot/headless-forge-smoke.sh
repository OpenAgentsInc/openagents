#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMPDIR="$(mktemp -d)"
MANIFEST_PATH="$TMPDIR/forge-desktop-control.json"
LOG_PATH="$TMPDIR/forge-headless.log"
SESSIONS_JSON="$TMPDIR/forge-sessions.json"
STATUS_STDERR="$TMPDIR/forge-status.stderr"
REQUEST_STDERR="$TMPDIR/forge-handoff-request.stderr"

cleanup() {
  if [[ -n "${FORGE_HOST_PID:-}" ]] && kill -0 "$FORGE_HOST_PID" >/dev/null 2>&1; then
    kill "$FORGE_HOST_PID" >/dev/null 2>&1 || true
    wait "$FORGE_HOST_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMPDIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "building standalone Forge binaries"
(
  cd "$ROOT"
  cargo build -p autopilot-desktop --bin autopilotctl --bin autopilot_headless_forge
)

AUTOPILOTCTL_BIN="$ROOT/target/debug/autopilotctl"

echo "running standalone Forge autostart smoke"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" forge hosted sessions --json >"$SESSIONS_JSON"

if [[ ! -s "$SESSIONS_JSON" ]]; then
  echo "Forge hosted sessions output is empty" >&2
  exit 1
fi

python3 - <<'PY' "$SESSIONS_JSON"
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
assert isinstance(payload, dict), payload
assert "sessions" in payload, payload
assert isinstance(payload["sessions"], list), payload
print(json.dumps(payload, indent=2, sort_keys=True))
PY

if [[ ! -f "$MANIFEST_PATH" ]]; then
  echo "Forge manifest was not created at $MANIFEST_PATH" >&2
  exit 1
fi

FORGE_HOST_PID="$(pgrep -f "autopilot_headless_forge --manifest-path $MANIFEST_PATH" | head -n 1 || true)"
if [[ -z "$FORGE_HOST_PID" ]]; then
  echo "Could not find autostarted Forge host process" >&2
  cat "$LOG_PATH" >&2 || true
  exit 1
fi

echo "checking honest no-thread failures"
if "$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" forge status --json > /dev/null 2>"$STATUS_STDERR"; then
  echo "Forge status unexpectedly succeeded without an active shared session" >&2
  exit 1
fi
if ! rg -q "No Forge thread id was supplied" "$STATUS_STDERR"; then
  echo "Forge status failure did not surface the expected no-thread reason" >&2
  cat "$STATUS_STDERR" >&2
  exit 1
fi

if "$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" forge handoff request "smoke request" \
  > /dev/null 2>"$REQUEST_STDERR"; then
  echo "Forge handoff request unexpectedly succeeded without an active shared session" >&2
  exit 1
fi
if ! rg -q "No Forge thread id was supplied" "$REQUEST_STDERR"; then
  echo "Forge handoff failure did not surface the expected no-thread reason" >&2
  cat "$REQUEST_STDERR" >&2
  exit 1
fi

echo
echo "standalone Forge smoke passed"
echo "manifest: $MANIFEST_PATH"
echo "log: $LOG_PATH"
echo "host pid: $FORGE_HOST_PID"
