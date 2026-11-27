#!/usr/bin/env bash
# Firm reason: 5 (thin E2E harness wrapping coax test, coax build, real agy CLI, compiled agyworker, and compiled coder-host; holds no app logic).
# Comprehensive Antigravity CLI + agyworker + coder-host End-to-End Test Harness (GitHub #13993, 4.3).
# Exercises:
#   Stage 1: Coax unit suites (cursor, antigravity, agyworker, tailnet, client-core) + host typecheck
#   Stage 2: Real `agy` CLI (`--version`, `models`), compiled `agyworker --probe-json`, and native `~/.gemini/antigravity` sync read-only `session/list`
#   Stage 3: Live compiled `agyworker` multimodal vision (`[oa-image ...]` -> "red") + live in-place ACP `session/resume` across processes (isolated in `antigravity-cli` and cleaned up on exit)
#   Stage 4: Live compiled `coder-host serve --no-relay --tailnet-port 0 --tailnet-test-loopback` verified end-to-end via direct-grant + NIP-44 DescribeEnvironment / ListSessions / ReadWorkspace
set -euo pipefail

cd "$(dirname "$0")/../../.."
repo="$(pwd -P)"
coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then
  echo "agy-e2e-test.sh: coax binary not found at $coax" >&2
  exit 1
fi

tmp="$(mktemp -d /tmp/oa-agy-e2e-XXXXXX)"
host_pid=""
cleanup() {
  if [[ -n "$host_pid" ]]; then
    kill "$host_pid" 2>/dev/null || true
    wait "$host_pid" 2>/dev/null || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

echo "==> [1/4] Running Coax unit suites & host typecheck"
"$coax" test coax/lib/agent/engine/cursor.coax
"$coax" test coax/lib/agent/engine/antigravity.coax
"$coax" test openagents/workers/agyworker.coax
"$coax" test coax/lib/nostr/host/tailnet.coax
"$coax" test apps/shared/antigravity/client-core.coax
"$coax" check openagents/daemon/host.coax --run-steps 250000000

echo "==> [2/4] Building agyworker + coder-host & verifying real agy CLI + native Google Antigravity sync"
compiler_hash="$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/workers/agyworker.coax" </dev/null >/dev/null
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/host.coax" build-only </dev/null >/dev/null 2>&1 || true
COAX="$coax" bash "$repo/coax/web/tool.sh" "$repo/openagents/daemon/tests/host-tailnet-e2e.coax" </dev/null >/dev/null 2>&1 || true
cp -f "$repo/coax/web/build/tools/agyworker-$compiler_hash" "$tmp/agyworker"
cp -f "$repo/coax/web/build/tools/host-$compiler_hash" "$tmp/coder-host"
cp -f "$repo/coax/web/build/tools/host-tailnet-e2e-$compiler_hash" "$tmp/host-tailnet-e2e"
chmod 755 "$tmp/agyworker" "$tmp/coder-host" "$tmp/host-tailnet-e2e"

agy_bin="${AGY_BIN:-$(command -v agy || echo "$HOME/.local/bin/agy")}"
if [[ ! -x "$agy_bin" ]]; then
  echo "FAIL: agy CLI not found at $agy_bin" >&2
  exit 1
fi

agy_ver="$("$agy_bin" --version)"
echo "agy --version: $agy_ver"
"$agy_bin" models | head -20

probe_json="$("$tmp/agyworker" --probe-json)"
echo "agyworker --probe-json: $probe_json"
python3 - "$probe_json" <<'PY'
import json, sys
p = json.loads(sys.argv[1])
assert p.get("installed") is True, f"expected installed=true, got {p}"
assert p.get("authStatus") == "authenticated", f"expected authenticated, got {p}"
models = [m.get("slug") or m.get("id", "") for m in p.get("models", [])]
assert any(m.startswith("gemini-") for m in models), f"expected gemini-* models, got {models}"
print(f"OK: probe-json verified {len(models)} models (authStatus={p['authStatus']})")
PY

# Verify read-only Google Antigravity app sync (`--app-data-dir antigravity` session/list)
if [[ -d "$HOME/.gemini/antigravity" ]]; then
  python3 - "$agy_bin" "$repo" <<'PY'
import json, subprocess, sys
agy, cwd = sys.argv[1], sys.argv[2]
p = subprocess.Popen([agy, "acp", "--app-data-dir", "antigravity"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
def rpc(obj):
    p.stdin.write(json.dumps(obj) + "\n")
    p.stdin.flush()
    while True:
        line = p.stdout.readline()
        if not line:
            raise RuntimeError("agy acp exited early: " + p.stderr.read())
        msg = json.loads(line)
        if msg.get("id") == obj["id"]:
            return msg
init = rpc({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {}}})
caps = init["result"]["agentCapabilities"]
assert caps.get("promptCapabilities", {}).get("image") is True, f"image cap missing: {caps}"
assert "resume" in caps.get("sessionCapabilities", {}), f"resume cap missing: {caps}"
listed = rpc({"jsonrpc": "2.0", "id": 2, "method": "session/list", "params": {"cwd": cwd}})
sessions = listed.get("result", {}).get("sessions", [])
p.terminate()
p.wait(timeout=5)
print(f"OK: native ~/.gemini/antigravity sync verified ({len(sessions)} Google Antigravity sessions visible, image+resume caps active)")
PY
fi

echo "==> [3/4] Live compiled agyworker multimodal vision ([oa-image ...]) + cross-process in-place session/resume"
python3 - "$tmp/agyworker" "$tmp" "$agy_bin" <<'PY'
import base64, glob, json, os, select, struct, subprocess, sys, time, zlib

worker, workdir, agy_bin = sys.argv[1], sys.argv[2], sys.argv[3]
conv_dir = os.path.expanduser("~/.gemini/antigravity-cli/conversations")
before = set(glob.glob(os.path.join(conv_dir, "*.db"))) if os.path.isdir(conv_dir) else set()

# Synthesize a crisp 64x64 solid red PNG with pure stdlib
def make_red_png(w=64, h=64):
    raw = b"".join(b"\x00" + (b"\xe8\x12\x12" * w) for _ in range(h))
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")

b64 = base64.b64encode(make_red_png()).decode("ascii")
prompt1 = f"What single color is this square? Reply with one word only.\n\n[oa-image name=swatch.png mime=image/png base64={b64}]"

def run_worker_turn(prompt, resume_id=""):
    env = os.environ.copy()
    env["CODER_AGY_PROGRAM"] = agy_bin
    env["CODER_AGY_APP_DATA_DIR"] = "antigravity-cli"
    env["CODER_AGY_MODEL"] = "gemini-3.8-flash"
    env["CODER_AGY_EFFORT"] = "low"
    env.pop("CODER_AGY_PROFILE", None)
    if resume_id:
        env["CODER_AGY_RESUME_SESSION"] = resume_id
    else:
        env.pop("CODER_AGY_RESUME_SESSION", None)
    p = subprocess.Popen([worker], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0, env=env)
    fd = p.stdout.fileno()
    buf = b""
    def read_line(timeout):
        nonlocal buf
        end = time.time() + timeout
        while b"\n" not in buf:
            left = end - time.time()
            if left <= 0:
                return None
            r, _, _ = select.select([fd], [], [], min(1.0, left))
            if not r:
                if p.poll() is not None:
                    return None
                continue
            chunk = os.read(fd, 65536)
            if not chunk:
                return None
            buf += chunk
        line_b, buf = buf.split(b"\n", 1)
        return line_b.decode("utf-8", "replace")

    ready_line = read_line(10.0)
    assert ready_line is not None and '"ready"' in ready_line, f"expected ready line, got: {ready_line!r}"
    req = {"task": prompt, "cwd": workdir, "model": "gemini-3.8-flash", "effort": "low"}
    p.stdin.write((json.dumps(req) + "\n").encode("utf-8"))
    p.stdin.flush()
    reply_chunks = []
    ended = None
    deadline = time.time() + 60
    while time.time() < deadline:
        line = read_line(max(0.1, deadline - time.time()))
        if line is None:
            break
        ev = json.loads(line)
        kind = ev.get("kind", "")
        if kind == "reply" and ev.get("text"):
            reply_chunks.append(ev["text"])
        if kind == "ended":
            ended = ev
            break
    p.stdin.close()
    try:
        p.wait(timeout=10)
    except subprocess.TimeoutExpired:
        p.terminate()
    assert ended is not None, f"worker did not finish turn; stderr={p.stderr.read().decode('utf-8', 'replace')}"
    assert ended.get("ending") == "finished", f"worker turn failed: {ended}"
    return (ended.get("text") or "".join(reply_chunks)).strip()

new_files = []
try:
    reply1 = run_worker_turn(prompt1)
    print(f"Turn 1 (vision) reply: {reply1!r}")
    assert "red" in reply1.lower(), f"expected 'red' from real ACP vision turn, got: {reply1!r}"

    after = set(glob.glob(os.path.join(conv_dir, "*.db"))) if os.path.isdir(conv_dir) else set()
    new_files = sorted(after - before, key=os.path.getmtime, reverse=True)
    assert new_files, f"expected a new session .db file under {conv_dir} for session/resume test"
    sess_uuid = os.path.splitext(os.path.basename(new_files[0]))[0]
    print(f"Turn 1 session UUID: {sess_uuid}")

    reply2 = run_worker_turn("What color was the square I just showed you? Reply with one word only.", resume_id=sess_uuid)
    print(f"Turn 2 (in-place session/resume) reply: {reply2!r}")
    assert "red" in reply2.lower(), f"expected 'red' from in-place ACP session/resume, got: {reply2!r}"
    print("OK: live agyworker multimodal vision + cross-process in-place session/resume verified!")
finally:
    for f in new_files:
        for suf in ("", "-wal", "-shm"):
            try:
                os.remove(f + suf)
            except OSError:
                pass
PY

echo "==> [4/4] Live compiled coder-host native Tailscale TCP listener (direct-grant + NIP-44 RPCs)"
host_home="$tmp/coder-home"
mkdir -p "$host_home"
port_file="$tmp/host-tailnet.port"
CODER_HOME="$host_home" "$tmp/coder-host" serve --no-relay \
  --workspace "$repo" \
  --tailnet-port 0 \
  --tailnet-bind 127.0.0.1 \
  --tailnet-test-loopback \
  --tailnet-port-file "$port_file" \
  >"$tmp/coder-host.log" 2>&1 &
host_pid=$!

for _ in $(seq 1 100); do
  if [[ -s "$port_file" && -S "$host_home/host/service.sock" ]]; then break; fi
  sleep 0.1
done
if [[ ! -s "$port_file" ]]; then
  echo "FAIL: coder-host did not write tailnet port file; log:" >&2
  cat "$tmp/coder-host.log" >&2
  exit 1
fi
host_port="$(awk '{print $1}' "$port_file")"
echo "Live coder-host listening on 127.0.0.1:$host_port (and $host_home/host/service.sock)"

# Enable workspace RPCs on the isolated test host so ReadWorkspace succeeds cleanly
CODER_HOME="$host_home" "$tmp/coder-host" workspace-enable "$repo" >/dev/null 2>&1 || true

"$tmp/host-tailnet-e2e" --port "$host_port"

echo ""
echo "ALL 4 ANTIGRAVITY CLI + AGYWORKER + CODER-HOST E2E STAGES PASSED!"
