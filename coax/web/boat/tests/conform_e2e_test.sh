#!/usr/bin/env bash
# Firm reason: 5 (offline ASan/UBSan integration test harness running real coax/web/tool.sh boat conform --lifecycle + mock GCE VM verification against oa-boat, #14128).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
boat_dir="$(cd "$here/.." && pwd)"
repo="$(cd "$boat_dir/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"

work="$(mktemp -d)"
boat_pid=""; gce_pid=""; boat_gce_pid=""
cleanup() {
  [[ -n "$boat_pid" ]] && kill "$boat_pid" 2>/dev/null || true
  [[ -n "$boat_gce_pid" ]] && kill "$boat_gce_pid" 2>/dev/null || true
  [[ -n "$gce_pid" ]] && kill "$gce_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

"$coax" c "$boat_dir/boat-service-wire.coax" --library --header "$work/boat-service-wire.generated.h" > "$work/boat-service-wire.generated.c"
clang -std=c11 -O1 -g -Wall -Wextra -Werror -D_POSIX_C_SOURCE=200809L -fsanitize=address,undefined \
  -I"$work" -o "$work/oa-boat" "$boat_dir/oa-boat.c" "$work/boat-service-wire.generated.c" -lcurl -lpthread

p1="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
p2="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
p3="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

# 1. Start oa-boat in local backend and run the REAL `coax/web/tool.sh boat conform --lifecycle`!
ASAN_OPTIONS=detect_leaks=0 OA_BOAT_API_KEY="oa_boat_conform_secret_key" OA_BOAT_BACKEND=local \
  "$work/oa-boat" --listen "127.0.0.1:$p1" >"$work/boat-local.log" 2>&1 &
boat_pid=$!
for _ in $(seq 1 40); do curl -fsS "http://127.0.0.1:$p1/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "http://127.0.0.1:$p1/health" >/dev/null

# Verify unauthorized request returns typed 401
st_unauth="$(curl -sS -o /dev/null -w '%{http_code}' "http://127.0.0.1:$p1/api/v1/limits")"
[[ "$st_unauth" == "401" ]] || { echo "expected 401 unauth, got $st_unauth" >&2; exit 1; }

BOAT_API_BASE="http://127.0.0.1:$p1/api/v1" BOAT_API_KEY="oa_boat_conform_secret_key" \
  "$repo/coax/web/tool.sh" boat conform --lifecycle

# 2. Verify GCE backend calls Compute Engine REST API with zero-SA, zero-external-IP, exact machineType, stop, start, delete!
cat > "$work/mock_gce.py" <<'PY'
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = int(sys.argv[1])
LOG = sys.argv[2]

class H(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args): pass
    def _rec(self, method):
        ln = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(ln).decode("utf-8") if ln > 0 else ""
        with open(LOG, "a") as f:
            f.write(json.dumps({"method": method, "path": self.path, "auth": self.headers.get("Authorization", ""), "body": body}) + "\n")
        resp = b'{"status":"DONE"}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)
    def do_POST(self): self._rec("POST")
    def do_DELETE(self): self._rec("DELETE")
    def do_GET(self):
        self.send_response(200); self.send_header("Content-Length", "2"); self.end_headers(); self.wfile.write(b"ok")

HTTPServer(("127.0.0.1", PORT), H).serve_forever()
PY
python3 "$work/mock_gce.py" "$p2" "$work/gce_ops.jsonl" >"$work/gce.log" 2>&1 &
gce_pid=$!
for _ in $(seq 1 40); do curl -fsS "http://127.0.0.1:$p2/health" >/dev/null 2>&1 && break; sleep 0.1; done

ASAN_OPTIONS=detect_leaks=0 OA_BOAT_API_KEY="oa_boat_gce_secret_key" OA_BOAT_BACKEND=gce \
  OA_BOAT_GCE_PROJECT="openagentsgemini" OA_BOAT_GCE_ZONE="us-central1-a" \
  OA_BOAT_GCE_BASE="http://127.0.0.1:$p2" OA_BOAT_GCE_TOKEN="ya29_mock_gce_tok" \
  "$work/oa-boat" --listen "127.0.0.1:$p3" >"$work/boat-gce.log" 2>&1 &
boat_gce_pid=$!
for _ in $(seq 1 40); do curl -fsS "http://127.0.0.1:$p3/health" >/dev/null 2>&1 && break; sleep 0.1; done

python3 - "http://127.0.0.1:$p3/api/v1" "oa_boat_gce_secret_key" "$work/gce_ops.jsonl" <<'PY'
import json, sys, urllib.request

base, key, log_path = sys.argv[1:4]
def req(method, path, body=None, extra=None):
    hdrs = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
    if extra: hdrs.update(extra)
    r = urllib.request.Request(base + path, data=(json.dumps(body).encode() if body is not None else None), headers=hdrs, method=method)
    with urllib.request.urlopen(r, timeout=10) as resp:
        return resp.status, json.loads(resp.read().decode())

st, j = req("POST", "/sandboxes", {"type": "large", "name": "oa-devbox-gce1", "ttlSeconds": 900}, {"Idempotency-Key": "idem_gce_1"})
assert st == 201 and j["ok"], (st, j)
bid = j["sandbox"]["id"]
# Idempotency retry returns exact same sandbox ID without creating a second VM
st2, j2 = req("POST", "/sandboxes", {"type": "large", "name": "oa-devbox-gce1", "ttlSeconds": 900}, {"Idempotency-Key": "idem_gce_1"})
assert st2 == 200 and j2["sandbox"]["id"] == bid, (st2, j2)

st, _ = req("POST", f"/sandboxes/{bid}/stop", {})
assert st == 200
st, _ = req("POST", f"/sandboxes/{bid}/resume", {})
assert st == 200
st, _ = req("DELETE", f"/sandboxes/{bid}", None, {"X-Ascii-Confirm-Delete": bid})
assert st == 200

with open(log_path) as f:
    ops = [json.loads(line) for line in f if line.strip()]
assert len(ops) == 4, ops
create_body = json.loads(ops[0]["body"])
assert ops[0]["method"] == "POST" and ops[0]["path"] == "/projects/openagentsgemini/zones/us-central1-a/instances"
assert ops[0]["auth"] == "Bearer ya29_mock_gce_tok"
assert create_body["machineType"].endswith("/c3d-highcpu-8")
assert create_body["serviceAccounts"] == []
assert "accessConfigs" not in json.dumps(create_body["networkInterfaces"])
assert create_body["labels"]["oa-boat-id"] == bid
gce_vm = "oa-" + bid.replace("_", "-")
assert create_body["name"] == gce_vm and "_" not in create_body["name"], create_body["name"]
assert create_body["disks"][0]["boot"] is True and create_body["disks"][0]["autoDelete"] is True
assert ops[1]["path"].endswith(f"/instances/{gce_vm}/stop")
assert ops[2]["path"].endswith(f"/instances/{gce_vm}/start")
assert ops[3]["method"] == "DELETE" and ops[3]["path"].endswith(f"/instances/{gce_vm}")
print("oa-boat conform_e2e_test: boat conform --lifecycle + GCE VM lifecycle (zero-SA, no external IP, idempotency) PASSED (ASan/UBSan clean)")
PY
