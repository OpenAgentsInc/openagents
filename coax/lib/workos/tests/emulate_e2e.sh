#!/usr/bin/env bash
# Firm reason: 5 (thin E2E test wrapper starting local workos/emulate and running Coax emulate_e2e.coax).
# Offline deterministic E2E verification for coax/lib/workos/ (#14099).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
seed="$here/emulate_seed.yaml"

# Pick a free localhost port.
port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

# Locate workos/emulate (prefers local checkout or installed binary or npx/bunx).
emu_cmd=()
if [[ -n "${WORKOS_EMULATE_BIN:-}" ]]; then
  emu_cmd=("$WORKOS_EMULATE_BIN")
elif command -v workos-emulate >/dev/null 2>&1; then
  emu_cmd=(workos-emulate)
elif [[ -f "/Users/christopherdavid/work/projects/repos/workos/emulate/src/cli.ts" ]] && command -v bun >/dev/null 2>&1; then
  emu_cmd=(bun run /Users/christopherdavid/work/projects/repos/workos/emulate/src/cli.ts)
elif command -v bunx >/dev/null 2>&1; then
  emu_cmd=(bunx @workos/emulate)
else
  emu_cmd=(npx --yes @workos/emulate)
fi

log="$(mktemp)"
cleanup() {
  if [[ -n "${emu_pid:-}" ]]; then kill "$emu_pid" 2>/dev/null || true; wait "$emu_pid" 2>/dev/null || true; fi
  rm -f "$log"
}
trap cleanup EXIT

NO_UPDATE_NOTIFIER=1 WORKOS_EMULATE_DISABLE_UPDATE_CHECK=1 \
  "${emu_cmd[@]}" --port "$port" --seed "$seed" >"$log" 2>&1 &
emu_pid=$!

base="http://127.0.0.1:$port"
ready=0
for _ in $(seq 1 50); do
  if curl -fsS "$base/health" >/dev/null 2>&1; then ready=1; break; fi
  sleep 0.1
done
if [[ "$ready" != 1 ]]; then
  echo "emulate_e2e.sh: workos/emulate failed to start on $base" >&2
  cat "$log" >&2
  exit 1
fi

cd "$repo"
coax/web/tool.sh coax/lib/workos/tests/emulate_e2e.coax "$base" "$(date +%s)"
