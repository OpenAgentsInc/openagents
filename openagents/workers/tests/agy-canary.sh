#!/usr/bin/env bash
# Firm reason: 5 (live upstream agy CLI/ACP canary check verifying version floor >= 1.5.10, ACP initialize/session/list/session/new, and model catalog schema drift).
# Nightly / On-Demand Live Antigravity CLI Canary (GitHub #14008).
set -euo pipefail

agy_bin="${AGY_BIN:-$(command -v agy || echo "$HOME/.local/bin/agy")}"
if [[ ! -x "$agy_bin" ]]; then
  echo "SKIP/FAIL: agy CLI not found at $agy_bin" >&2
  exit 1
fi

ver_out="$("$agy_bin" --version 2>&1 || true)"
echo "agy --version: $ver_out"

python3 - "$agy_bin" <<'PY'
import glob, json, os, re, select, signal, subprocess, sys, tempfile

signal.alarm(45)
agy = sys.argv[1]
ver_raw = subprocess.check_output([agy, "--version"], text=True, timeout=10).strip()
m = re.search(r"(\d+)\.(\d+)\.(\d+)", ver_raw)
if m:
    maj, mn, pat = map(int, m.groups())
    assert (maj, mn, pat) >= (1, 5, 10), f"agy version {ver_raw} below ACP floor 1.5.10"
    print(f"OK: version floor satisfied ({maj}.{mn}.{pat} >= 1.5.10)")
else:
    assert "agy_acp_server_" in ver_raw, f"unrecognized agy version: {ver_raw}"
    print(f"OK: date-stamped RC build accepted ({ver_raw})")

conv_dir = os.path.expanduser("~/.gemini/antigravity-cli/conversations")
before = set(glob.glob(os.path.join(conv_dir, "*.db"))) if os.path.isdir(conv_dir) else set()

tmp_cwd = tempfile.mkdtemp(prefix="oa-agy-canary-")
p = subprocess.Popen(
    [agy, "acp", "--app-data-dir", "antigravity-cli"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0,
)
out_fd = p.stdout.fileno()
out_buf = bytearray()
def rpc(obj, timeout=15.0):
    p.stdin.write((json.dumps(obj) + "\n").encode("utf-8"))
    p.stdin.flush()
    while True:
        nl = out_buf.find(b"\n")
        if nl >= 0:
            line = bytes(out_buf[:nl]).decode("utf-8")
            del out_buf[:nl + 1]
            if not line.strip():
                continue
            msg = json.loads(line)
            if msg.get("id") == obj["id"]:
                return msg
            continue
        r, _, _ = select.select([out_fd], [], [], timeout)
        if not r:
            raise RuntimeError(f"agy acp timed out waiting for {obj['method']}")
        chunk = os.read(out_fd, 65536)
        if not chunk:
            p.terminate()
            try:
                p.wait(timeout=2)
            except Exception:
                p.kill()
            raise RuntimeError("agy acp exited early: " + p.stderr.read().decode("utf-8", "replace"))
        out_buf.extend(chunk)

try:
    init = rpc({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {}}})
    res = init["result"]
    caps = res.get("agentCapabilities", {})
    assert caps.get("promptCapabilities", {}).get("image") is True, f"promptCapabilities.image missing: {caps}"
    assert "resume" in caps.get("sessionCapabilities", {}), f"sessionCapabilities.resume missing: {caps}"
    assert "list" in caps.get("sessionCapabilities", {}), f"sessionCapabilities.list missing: {caps}"

    listed = rpc({"jsonrpc": "2.0", "id": 2, "method": "session/list", "params": {"cwd": tmp_cwd}})
    assert "result" in listed, f"session/list failed: {listed}"

    created = rpc({"jsonrpc": "2.0", "id": 3, "method": "session/new", "params": {"cwd": tmp_cwd, "mcpServers": []}})
    sres = created.get("result", {})
    assert sres.get("sessionId"), f"session/new failed: {created}"

    model_ids = []
    for opt in sres.get("configOptions", []):
        if opt.get("id") == "model" or opt.get("category") == "model":
            for entry in opt.get("options", []):
                if "value" in entry:
                    model_ids.append(entry["value"])
                for sub in entry.get("options", []):
                    if "value" in sub:
                        model_ids.append(sub["value"])
    if not model_ids:
        model_ids = [x.get("modelId") for x in sres.get("models", {}).get("availableModels", []) if x.get("modelId")]

    assert len(model_ids) > 0, f"no models advertised in session/new: {sres}"
    assert len(model_ids) <= 64, f"model count {len(model_ids)} exceeds EA_MODELS=64 cap!"
    print(f"OK: ACP canary passed — protocolVersion={res.get('protocolVersion')}, agent={res.get('agentInfo')}, models={len(model_ids)} ({', '.join(model_ids[:6])}…)")
finally:
    p.terminate()
    try:
        p.wait(timeout=5)
    except Exception:
        p.kill()
    after = set(glob.glob(os.path.join(conv_dir, "*.db"))) if os.path.isdir(conv_dir) else set()
    for f in after - before:
        for suf in ("", "-wal", "-shm"):
            try:
                os.remove(f + suf)
            except OSError:
                pass
    try:
        os.rmdir(tmp_cwd)
    except OSError:
        pass
PY
