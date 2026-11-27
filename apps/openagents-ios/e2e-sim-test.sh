#!/usr/bin/env bash
# Firm reason: 5 (thin Simulator E2E test harness orchestrating Coax wire/fuzz tests, dual-mode legacy-v1 + v2 NIP-44 TCP hosts, real iOS Simulator unit + UI tests, and live coder-host smoke checks).
# Usage: bash apps/openagents-ios/e2e-sim-test.sh [SIMULATOR_UDID]
set -euo pipefail

cd "$(dirname "$0")"
ios_dir="$(pwd -P)"
repo="$(cd ../.. && pwd -P)"
coax="${COAX:-$repo/coax/bin/coax}"

echo "==> [1/6] Running Coax shared client-core unit & 256-case #forall fuzz checks..."
if [[ -x "$coax" ]]; then
  "$coax" test "$repo/apps/shared/antigravity/client-core.coax"
else
  "$repo/coax/remote/remote.sh" run -- coax/bin/coax test apps/shared/antigravity/client-core.coax
fi

echo "==> [2/6] Building OpenAgents iOS for Simulator & running binary cleanliness check..."
./build.sh
./check.sh

sim_id="${1:-}"
if [[ -z "$sim_id" ]]; then
  sim_id="$(xcrun simctl list devices booted -j | /usr/bin/python3 -c '
import json, sys
d = json.load(sys.stdin).get("devices", {})
for rt, devs in d.items():
    if "iOS" not in rt:
        continue
    for dev in devs:
        if dev.get("state") == "Booted" and dev.get("isAvailable", True):
            print(dev["udid"]); sys.exit(0)
' 2>/dev/null || true)"
fi
if [[ -z "$sim_id" ]]; then
  sim_id="$(xcrun simctl list devices available -j | /usr/bin/python3 -c '
import json, sys
d = json.load(sys.stdin).get("devices", {})
for rt, devs in sorted(d.items(), reverse=True):
    if "iOS" not in rt:
        continue
    for dev in devs:
        if "iPhone" in dev.get("name", "") and dev.get("isAvailable", True):
            print(dev["udid"]); sys.exit(0)
' 2>/dev/null || true)"
  [[ -n "$sim_id" ]] || { echo "e2e-sim-test: no available iOS Simulator found" >&2; exit 2; }
  xcrun simctl boot "$sim_id" || true
fi
xcrun simctl bootstatus "$sim_id" -b >/dev/null 2>&1 || true
echo "    Using iOS Simulator: $sim_id"

echo "==> [3/6] Running OpenAgentsTests (unit, strict v1 wire compat, error humanization, 120-case fuzz)..."
"$repo/coax/remote/remote.sh" run --local -- xcodebuild \
  -project OpenAgents.xcodeproj \
  -scheme OpenAgentsTests \
  -configuration Debug \
  -destination "platform=iOS Simulator,id=$sim_id" \
  -derivedDataPath build \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- \
  OA_IOS_BUNDLE_ID=com.openagents.app.e2etest \
  test

echo "==> [4/6] Starting real Coax NIP-44 E2E hosts (strict 16-key --legacy-v1 AND modern v2)..."
run_dir="$(mktemp -d /tmp/oa-ios-e2e.XXXXXX)"
shot_dir="$run_dir/screenshots"
mkdir -p "$shot_dir"
p_legacy=""
p_v2=""
p_real_host=""
cleanup() {
  for pid in "$p_legacy" "$p_v2" "$p_real_host"; do
    if [[ -n "$pid" ]]; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done
}
trap cleanup EXIT

# Pre-build e2e-host once cleanly before spawning both servers in parallel.
bash "$repo/coax/web/tool.sh" "$ios_dir/Tests/e2e-host.coax" --port-file "$run_dir/warmup.txt" --idle-ms 50 >/dev/null

bash "$repo/coax/web/tool.sh" "$ios_dir/Tests/e2e-host.coax" --legacy-v1 --port-file "$run_dir/legacy.port" --idle-ms 180000 >"$run_dir/legacy.log" 2>&1 &
p_legacy=$!
bash "$repo/coax/web/tool.sh" "$ios_dir/Tests/e2e-host.coax" --port-file "$run_dir/v2.port" --idle-ms 180000 >"$run_dir/v2.log" 2>&1 &
p_v2=$!

wait_port() {
  local f="$1"
  for _ in $(seq 1 60); do
    if [[ -s "$f" ]]; then
      awk '{print $1}' "$f"
      return 0
    fi
    sleep 0.2
  done
  echo "e2e-sim-test: timed out waiting for $f" >&2
  return 1
}

legacy_port="$(wait_port "$run_dir/legacy.port")"
v2_port="$(wait_port "$run_dir/v2.port")"
echo "    Legacy v1 host listening on 127.0.0.1:$legacy_port"
echo "    Modern v2 host listening on 127.0.0.1:$v2_port"

echo "==> [5/6] Running OpenAgentsUITests LIVE on Simulator against BOTH legacy-v1 and v2 hosts..."
TEST_RUNNER_OA_E2E_LEGACY_PORT="$legacy_port" \
TEST_RUNNER_OA_E2E_V2_PORT="$v2_port" \
TEST_RUNNER_OA_E2E_SCREENSHOT_DIR="$shot_dir" \
"$repo/coax/remote/remote.sh" run --local -- xcodebuild \
  -project OpenAgents.xcodeproj \
  -scheme OpenAgentsUITests \
  -configuration Debug \
  -destination "platform=iOS Simulator,id=$sim_id" \
  -derivedDataPath build \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- \
  OA_IOS_BUNDLE_ID=com.openagents.app.e2etest \
  test

echo "==> [6/6] Running real compiled coder-host + oa-worker daemon smoke check (isolated CODER_HOME)..."
host_home="$run_dir/coder-home"
worker_dir="$run_dir/workers"
mkdir -p "$host_home" "$worker_dir"
compiler_hash="$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/runtime/oa-worker.coax" </dev/null >/dev/null
cp -f "$repo/coax/web/build/tools/oa-worker-$compiler_hash" "$worker_dir/oa-worker"
chmod 755 "$worker_dir/oa-worker"
CODER_HOME="$host_home" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/host.coax" --help >/dev/null
grant_json="$(CODER_HOME="$host_home" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/host.coax" direct-grant \
  --device 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --workspace "$repo" --label "E2E Real Host" --client-label "OpenAgents iOS E2E")"
/usr/bin/python3 -c '
import json, sys
d = json.loads(sys.argv[1])
assert len(d.get("host", "")) == 64, d
assert len(d.get("grant", "")) == 64, d
assert not d.get("refused"), d
print("    real coder-host direct-grant verified: host=" + d["host"][:12] + "...")
' "$grant_json"

CODER_HOME="$host_home" CODER_WORKER_DIR="$worker_dir" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/host.coax" serve --no-relay --for 45 >"$run_dir/coder-host.log" 2>&1 &
p_real_host=$!
sock="$host_home/.openagents/coder/host/service.sock"
for _ in $(seq 1 80); do
  [[ -S "$sock" ]] && break
  sleep 0.25
done
[[ -S "$sock" ]] || { echo "e2e-sim-test: coder-host did not create $sock" >&2; cat "$run_dir/coder-host.log" >&2; exit 1; }
/usr/bin/python3 - "$sock" "$host_home" <<'PY'
import json, os, socket, sys, time
sock_path, host_home = sys.argv[1], sys.argv[2]
def rpc(payload):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(10)
    s.connect(sock_path)
    s.sendall((json.dumps(payload) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        chunk = s.recv(65536)
        if not chunk:
            break
        buf += chunk
    s.close()
    return json.loads(buf.decode().strip())
def ea_req(method, **extra):
    base = {
        "method": method, "workspace_id": "default", "session_id": "", "command_id": "",
        "generation": 0, "expected_revision": 0, "text": "", "interaction_id": "",
        "allow": False, "before": 0, "expires_at_ms": 0, "issued_at_ms": 0,
    }
    base.update(extra)
    return base
ws_read = {
    "schema": "openagents.environment.workspace.v1", "method": "ReadWorkspace",
    "workspace": "default", "branch": "", "base": "", "title": "", "body": "",
    "number": 0, "page": 0, "cursor": 0, "expected": "", "command": "",
    "issued_at": 0, "expires_at": 0,
}
desc = rpc(ea_req("DescribeEnvironment"))
assert desc.get("error", "") == "" and desc.get("method") == "DescribeEnvironment", desc
assert "oa.core.v1" in desc.get("capabilities", []), f"missing oa.core.v1: {desc}"
for payload in (
    ea_req("ListSessions"),
    ea_req("ListSessions", model="antigravity"),
    ws_read,
):
    resp = rpc(payload)
    assert resp.get("error", "") == "", f"coder-host {payload['method']} returned error: {resp!r}"
    assert resp.get("method") == payload["method"], f"method mismatch: {resp}"
models = [m.get("model_id") for m in desc.get("models", [])]
if "oa:auto" in models:
    now_ms = int(time.time() * 1000)
    sid = "11112222333344445555666677778888"
    cid = "aaaabbbbccccddddeeeeffff00001111"
    op = rpc(ea_req(
        "OpenSession", session_id=sid, command_id=cid, generation=1, expected_revision=0,
        text="Reply with exactly one word: PONG", model="oa:auto", effort="low",
        issued_at_ms=now_ms, expires_at_ms=now_ms + 300000,
    ))
    assert op.get("error", "") == "" and op.get("command_status") == "accepted", op
    runs_dir = os.path.join(host_home, ".openagents/coder/host/runs")
    phone_meta = json.loads(open(os.path.join(runs_dir, f"{sid}.phone")).read())
    assert phone_meta.get("engine") == "openagents", phone_meta
    assert phone_meta.get("model") == "oa:auto", phone_meta
    out_path = os.path.join(runs_dir, f"{sid}.out")
    ready_seen = False
    for _ in range(50):
        if os.path.exists(out_path) and '"kind":"ready"' in open(out_path).read():
            ready_seen = True
            break
        time.sleep(0.2)
    assert ready_seen, f"oa-worker did not emit ready line in {out_path}"
    print("    real coder-host + oa-worker live OpenSession verified: engine=openagents, kind=ready")
print("    real coder-host service.sock verified: DescribeEnvironment(oa.core.v1), ListSessions, ListSessions(antigravity), ReadWorkspace all returned error=''")
PY

echo "e2e-sim-test: ALL 6 STAGES PASSED! Screenshots saved in: $shot_dir"
ls -la "$shot_dir" || true
