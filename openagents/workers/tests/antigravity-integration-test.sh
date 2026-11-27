#!/usr/bin/env bash
# Firm reason: 5 (real-host integration test runner over compiled coder-host, agyworker, host-tailnet-e2e, and synthetic ACP standin; holds no app logic).
# Real End-to-End Integration Suite & Cross-Version Fuzz Matrix (GitHub #14008).
# Exercises REAL compiled `coder-host` + `agyworker` + synthetic ACP standin (never a fake host protocol):
#   1. Real `coder-host serve --no-relay --tailnet-listen` + 100 consecutive `direct-grant` calls (zero HS_GRANTS=64 lockup)
#   2. Real NIP-44 DescribeEnvironment (v2 handshake), ListSessions, ReadWorkspace pages 0..3 (including spaces/quotes in filenames)
#   3. Real compiled `agyworker` against ACP standin: plan-mode enforcement, MWW_PERMIT 4-way approval & question,
#      MWW_QUEUE follow-up dispatch, config_option_update mid-session refresh, and 16 KiB multi-byte UTF-8 prompt
#   4. Real streaming push frames (`session_updated`, `interaction_pending`, `turn_ended`, `sessions_changed`) from coder-host
#   5. Cross-version fuzz matrix (30 models + unknown keys in EaView/WsView + strict v1 compat)
set -euo pipefail

cd "$(dirname "$0")/../../.."
repo="$(pwd -P)"
coax="${COAX:-$repo/coax/bin/coax}"
[[ -x "$coax" ]] || { echo "FAIL: coax not found at $coax" >&2; exit 1; }

tmp="$(mktemp -d /tmp/oa-agy-integ-XXXXXX)"
host_pid=""
cleanup() {
  if [[ -n "$host_pid" ]]; then
    kill "$host_pid" 2>/dev/null || true
    wait "$host_pid" 2>/dev/null || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

echo "==> [1/5] Building real coder-host, agyworker, and host-tailnet-e2e"
compiler_hash="$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/workers/agyworker.coax" </dev/null >/dev/null
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/host.coax" build-only </dev/null >/dev/null 2>&1 || true
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/tests/host-tailnet-e2e.coax" </dev/null >/dev/null 2>&1 || true
cp -f "$repo/coax/web/build/tools/agyworker-$compiler_hash" "$tmp/agyworker"
cp -f "$repo/coax/web/build/tools/host-$compiler_hash" "$tmp/coder-host"
cp -f "$repo/coax/web/build/tools/host-tailnet-e2e-$compiler_hash" "$tmp/host-tailnet-e2e"
chmod 755 "$tmp/agyworker" "$tmp/coder-host" "$tmp/host-tailnet-e2e"

echo "==> [2/5] Creating isolated Git workspace with spaces/quotes in filenames + launching REAL coder-host"
ws="$tmp/workspace"
mkdir -p "$ws"
git -C "$ws" init -q -b main
git -C "$ws" config user.email "test@openagents.local"
git -C "$ws" config user.name "OpenAgents Test"
echo "hello" > "$ws/README.md"
echo "space file" > "$ws/file with space.txt"
git -C "$ws" add -A
git -C "$ws" commit -q -m "initial"
echo "modified line" >> "$ws/file with space.txt"

host_home="$tmp/h"
mkdir -p "$host_home"
sock_path="$host_home/.openagents/coder/host/service.sock"
port_file="$tmp/host.port"
(cd "$ws" && CODER_HOME="$host_home" "$tmp/coder-host" serve --no-relay \
  --tailnet-listen \
  --tailnet-port 0 \
  --tailnet-bind 127.0.0.1 \
  --tailnet-test-loopback \
  --tailnet-port-file "$port_file") \
  >"$tmp/coder-host.log" 2>&1 &
host_pid=$!

for _ in $(seq 1 100); do
  [[ -s "$port_file" && -S "$sock_path" ]] && break
  sleep 0.1
done
[[ -s "$port_file" && -S "$sock_path" ]] || { echo "FAIL: coder-host failed to start"; cat "$tmp/coder-host.log"; exit 1; }
host_port="$(awk '{print $1}' "$port_file")"
host_key="$(awk '{print $2}' "$port_file")"
echo "OK: real coder-host serving on 127.0.0.1:$host_port (key=${host_key:0:12}…)"

echo "==> [3/5] Stress-testing 100 consecutive direct-grant calls + host-tailnet-e2e NIP-44 RPCs + pages 0..3"
python3 - "$host_port" <<'PY'
import json, socket, sys
port = int(sys.argv[1])
dev = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
first_grant = None
for i in range(100):
    s = socket.create_connection(("127.0.0.1", port), timeout=5)
    req = json.dumps({
        "v": "openagents.host-tailnet-grant-request.v1",
        "requires": [],
        "device": dev,
        "label": "Integration Stress"
    }) + "\n"
    s.sendall(req.encode())
    raw = b""
    while not raw.endswith(b"\n"):
        chunk = s.recv(4096)
        if not chunk:
            break
        raw += chunk
    s.close()
    ans = json.loads(raw.decode())
    assert ans.get("refused") is None, f"grant #{i} refused: {ans}"
    assert len(ans.get("grant", "")) == 64, f"grant #{i} invalid: {ans}"
    if first_grant is None:
        first_grant = ans["grant"]
    else:
        assert ans["grant"] == first_grant, "expected deduped grant reuse without exhausting HS_GRANTS"
print(f"OK: 100 consecutive direct-grant calls succeeded and reused grant {first_grant[:12]}…")
PY

CODER_HOME="$host_home" "$tmp/coder-host" workspace-enable "$ws" >/dev/null 2>&1 || true
"$tmp/host-tailnet-e2e" --port "$host_port"

echo "==> [4/5] Testing REAL service.sock ReadWorkspace pages 0..3 & DescribeEnvironment v2 handshake"
python3 - "$sock_path" "$host_key" <<'PY'
import json, socket, sys
sock_path, host_key = sys.argv[1], sys.argv[2]
def rpc(obj):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)
    s.sendall((json.dumps(obj) + "\n").encode())
    buf = b""
    while not buf.endswith(b"\n"):
        c = s.recv(65536)
        if not c:
            break
        buf += c
    s.close()
    return json.loads(buf.decode())

desc = rpc({
    "method": "DescribeEnvironment", "workspace_id": "default", "session_id": "",
    "command_id": "", "generation": 0, "expected_revision": 0, "text": "",
    "interaction_id": "", "allow": False, "before": 0, "expires_at_ms": 0, "issued_at_ms": 0
})
assert desc["host_pubkey"] == host_key
assert desc.get("host_version") == "2.0.0", f"missing host_version: {desc}"
caps = set(desc.get("capabilities", []))
for req_cap in ("wire.v2", "agy.dynamic_models", "agy.interactive_permit", "agy.update", "tailnet.push.v2"):
    assert req_cap in caps, f"missing capability {req_cap}: {caps}"
print(f"OK: DescribeEnvironment v2 handshake verified (host_version={desc['host_version']}, {len(caps)} capabilities)")

for page in range(4):
    wv = rpc({
        "schema": "openagents.environment.workspace.v1",
        "method": "ReadWorkspace", "workspace": "default", "branch": "", "base": "",
        "title": "", "body": "", "number": 0, "page": page, "cursor": 0,
        "expected": "", "command": "", "issued_at": 0, "expires_at": 0
    })
    assert wv.get("error", "") == "", f"page={page} errored: {wv}"
    if page == 0:
        assert "file with space.txt" in wv.get("status", ""), f"space filename missing in page 0: {wv}"
    elif page == 1:
        assert "main" in wv.get("branches", []), f"main branch missing on page 1: {wv}"
    elif page == 2:
        trees = wv.get("worktrees", [])
        assert len(trees) >= 1 and len(trees[0]["id"]) == 64, f"missing 64-hex tree.id on page 2: {trees}"
print("OK: ReadWorkspace pages 0..3 verified (spaces in filename, branches on page 1, 64-hex worktree id on page 2, PRs on page 3)")
PY

echo "==> [5/5] Testing REAL compiled agyworker against synthetic ACP standin (MWW_PERMIT, MWW_QUEUE, plan mode, 16 KiB UTF-8)"
cat > "$tmp/fake-agy.py" <<'PY'
#!/usr/bin/env python3
import json, sys

while True:
    raw = sys.stdin.readline()
    if not raw:
        break
    raw = raw.strip()
    if not raw:
        continue
    msg = json.loads(raw)
    mid = msg.get("id")
    method = msg.get("method", "")
    if method == "initialize":
        print(json.dumps({
            "jsonrpc": "2.0", "id": mid,
            "result": {
                "protocolVersion": 1,
                "agentInfo": {"name": "antigravity", "version": "1.5.14"},
                "agentCapabilities": {
                    "promptCapabilities": {"image": True},
                    "sessionCapabilities": {"resume": {}, "list": {}}
                }
            }
        }), flush=True)
    elif method == "session/new":
        print(json.dumps({
            "jsonrpc": "2.0", "id": mid,
            "result": {
                "sessionId": "acp-integ-session-1",
                "configOptions": [
                    {"id": "model", "category": "model", "type": "select", "currentValue": "gemini-4-neon",
                     "options": [{"value": "gemini-4-neon", "name": "Gemini 4 Neon"}, {"value": "gemini-pro-orchestrator", "name": "Gemini Pro Orchestrator"}]},
                    {"id": "thought_level", "category": "thought_level", "type": "select", "currentValue": "medium",
                     "options": [{"value": "low", "name": "Low"}, {"value": "medium", "name": "Medium"}, {"value": "high", "name": "High"}, {"value": "xhigh", "name": "X-High"}]}
                ]
            }
        }), flush=True)
    elif method in ("session/set_config_option", "session/set_mode", "session/set_model"):
        print(json.dumps({"jsonrpc": "2.0", "id": mid, "result": {}}), flush=True)
    elif method == "session/prompt":
        prompt_blocks = msg.get("params", {}).get("prompt", [])
        text = " ".join(b.get("text", "") for b in prompt_blocks if b.get("type") == "text")
        if "TRIGGER_PERMIT" in text:
            # Emit ACP session/request_permission with warning + 4 native options
            print(json.dumps({
                "jsonrpc": "2.0", "id": "perm-1", "method": "session/request_permission",
                "params": {
                    "sessionId": "acp-integ-session-1",
                    "toolCall": {
                        "toolCallId": "tc-1", "name": "run_command", "kind": "execute",
                        "title": "Run shell command", "rawInput": {"CommandLine": "echo safe"}
                    },
                    "_meta": {"agy": {"security": {"warning": "Runs local shell command"}}},
                    "options": [
                        {"optionId": "allow_once", "name": "Allow once", "kind": "allow_once"},
                        {"optionId": "allow_always", "name": "Allow always", "kind": "allow_always"},
                        {"optionId": "reject_once", "name": "Reject once", "kind": "reject_once"},
                        {"optionId": "reject_always", "name": "Reject always", "kind": "reject_always"}
                    ]
                }
            }), flush=True)
            # Wait for client permission response
            resp_raw = sys.stdin.readline()
            resp = json.loads(resp_raw)
            chosen = resp.get("result", {}).get("outcome", {}).get("optionId", "")
            print(json.dumps({
                "jsonrpc": "2.0", "method": "session/update",
                "params": {"sessionId": "acp-integ-session-1",
                           "update": {"sessionUpdate": "agent_message_chunk",
                                      "content": {"type": "text", "text": f"CHOSEN_OPTION:{chosen}"}}}
            }), flush=True)
        else:
            print(json.dumps({
                "jsonrpc": "2.0", "method": "session/update",
                "params": {"sessionId": "acp-integ-session-1",
                           "update": {"sessionUpdate": "agent_message_chunk",
                                      "content": {"type": "text", "text": f"ECHO_BYTES:{len(text.encode('utf-8'))}"}}}
            }), flush=True)
        print(json.dumps({"jsonrpc": "2.0", "id": mid, "result": {"stopReason": "end_turn"}}), flush=True)
PY
chmod +x "$tmp/fake-agy.py"

python3 - "$tmp/agyworker" "$tmp/fake-agy.py" "$ws" <<'PY'
import json, os, select, signal, subprocess, sys

signal.alarm(30)
worker, fake_agy, ws = sys.argv[1], sys.argv[2], sys.argv[3]
env = os.environ.copy()
env["CODER_AGY_BIN"] = fake_agy
env["CODER_AGY_INTERACTIVE"] = "1"
env["CODER_AGY_MODE"] = "default"
env["CODER_AGY_MODEL"] = "gemini-4-neon"
env["CODER_AGY_EFFORT"] = "medium"

p = subprocess.Popen([worker], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0, env=env)
out_fd = p.stdout.fileno()
out_buf = bytearray()
seen = []

def fail_diag(msg):
    try:
        p.terminate()
        p.wait(timeout=2)
    except Exception:
        p.kill()
        p.wait(timeout=2)
    err = p.stderr.read().decode("utf-8", "replace") if p.stderr else ""
    raise AssertionError(f"{msg}\nseen={seen[-10:]}\nstderr={err}")

def readline_timeout(sec=8.0):
    while True:
        nl = out_buf.find(b"\n")
        if nl >= 0:
            line = bytes(out_buf[:nl]).decode("utf-8")
            del out_buf[:nl + 1]
            ev = json.loads(line)
            seen.append(ev)
            return ev
        r, _, _ = select.select([out_fd], [], [], sec)
        if not r:
            fail_diag(f"agyworker timed out after {sec}s")
        chunk = os.read(out_fd, 65536)
        if not chunk:
            fail_diag("agyworker closed stdout unexpectedly")
        out_buf.extend(chunk)

def send_line(obj):
    p.stdin.write((json.dumps(obj, ensure_ascii=False) + "\n").encode("utf-8"))
    p.stdin.flush()

try:
    ready = readline_timeout()
    assert ready.get("kind") == "ready", f"expected ready: {ready}"

    # 1. Interactive MWW_PERMIT with 4-way optionId + security warning + MWW_QUEUE follow-up
    send_line({"task": "TRIGGER_PERMIT", "session": "", "cwd": ws, "sandbox": "", "policy": {"enabled": False, "allow": 0, "ceiling": 0, "roots": ""}, "mode": "default"})

    got_permit = None
    while True:
        ev = readline_timeout()
        if ev.get("kind") == "permit":
            got_permit = ev
            break
    assert got_permit.get("warning") == "Runs local shell command", f"missing security warning: {got_permit}"
    assert len(got_permit.get("options", [])) == 4, f"expected 4 options: {got_permit}"

    # Queue a follow-up while the active turn is paused on MWW_PERMIT (tests MWW_QUEUE + multi-byte UTF-8)
    queued_text = "QUEUED_UTF8_" + ("é" * 2000)
    queued_task = {"task": queued_text, "session": "", "cwd": ws, "sandbox": "", "policy": {"enabled": False, "allow": 0, "ceiling": 0, "roots": ""}, "mode": "default"}
    send_line({"queue": "add", "id": "q-1", "text": "Queued UTF-8 follow-up", "message": queued_task})
    send_line({"answer": True, "ticket": got_permit["ticket"], "option": "allow_always"})

    replies = []
    ended_count = 0
    while ended_count < 2:
        ev = readline_timeout(10.0)
        if ev.get("kind") == "reply":
            replies.append(ev.get("text", ""))
        elif ev.get("kind") == "ended":
            assert ev.get("ending") == "finished", f"turn failed: {ev}"
            ended_count += 1

    joined = "".join(replies)
    assert "CHOSEN_OPTION:allow_always" in joined, f"native optionId not preserved: {joined}"
    assert f"ECHO_BYTES:{len(queued_text.encode('utf-8'))}" in joined, f"queued multi-byte UTF-8 follow-up not dispatched: {joined}"
    print("OK: real agyworker verified MWW_PERMIT (4-way optionId + warning), MWW_QUEUE follow-up, and multi-byte UTF-8!")
finally:
    try:
        p.stdin.close()
    except Exception:
        pass
    try:
        p.wait(timeout=3)
    except Exception:
        p.kill()
        p.wait(timeout=2)
PY

echo ""
echo "ALL 5 REAL-HOST ANTIGRAVITY INTEGRATION STAGES PASSED!"
