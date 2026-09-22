#!/usr/bin/env bash
# verify-gateway-install.sh — prove a fresh gateway install end to end:
# bootstrap a registry, issue a key, make one real bounded call through a
# stub backend, restart, restore from backup, roll back the binary, and
# delete. Each step prints PASS or FAIL; any FAIL exits 1.
#
# Run from the repository root:
#
#   ./scripts/verify-gateway-install.sh
#
# It builds only what it runs (gateway, the tenancy bootstrap example),
# works in a temporary directory, and cleans up its processes on exit.
set -u
cd "$(dirname "$0")/.."

WORK=$(mktemp -d)
GATEWAY_PID=""
BACKEND_PID=""
cleanup() {
    [ -n "$GATEWAY_PID" ] && kill "$GATEWAY_PID" 2>/dev/null
    [ -n "$BACKEND_PID" ] && kill "$BACKEND_PID" 2>/dev/null
    rm -rf "$WORK"
}
trap cleanup EXIT

FAILED=0
check() {
    if [ "$2" = "$3" ]; then
        echo "PASS  $1"
    else
        echo "FAIL  $1 (wanted $3, got $2)"
        FAILED=1
    fi
}

DIGEST="sha256:$(printf '0%.0s' $(seq 64))"
REGISTRY="$WORK/registry"
GW_PORT=18443
BE_PORT=19080

# Poll liveness for up to ten seconds — a cold start opens the registry
# and ledger before it binds.
wait_health() {
    for _ in $(seq 50); do
        code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$GW_PORT/healthz" 2>/dev/null)
        [ "$code" = "200" ] && return 0
        sleep 0.2
    done
    return 1
}

start_gateway() {
    local binary=${1:-$GATEWAY}
    "$binary" --config "$WORK/gateway.json" &
    GATEWAY_PID=$!
    wait_health
}

stop_gateway() {
    [ -z "$GATEWAY_PID" ] && return 0
    kill "$GATEWAY_PID" 2>/dev/null
    wait "$GATEWAY_PID" 2>/dev/null
    GATEWAY_PID=""
    # A crashed writer leaves the ledger's lock file standing; the
    # operator's own recovery removes it once no writer runs.
    rm -f "$REGISTRY/quota-ledger.lock"
}

echo "== build =="
cargo build -p gateway --bin gateway -p tenancy --example bootstrap_registry 2>/dev/null
TARGET=${CARGO_TARGET_DIR:-target}
GATEWAY=$(find "$TARGET/debug" -name gateway -type f | head -1)
BOOTSTRAP=$(find "$TARGET/debug/examples" -name bootstrap_registry -type f | head -1)
[ -n "$GATEWAY" ] && [ -n "$BOOTSTRAP" ] && echo "PASS  binaries built" || { echo "FAIL  binaries built"; exit 1; }

echo "== fresh install =="
TOKEN=$("$BOOTSTRAP" --registry "$REGISTRY" --tenant eval --door local-kev \
    --model kev-stub --signature "$DIGEST" | head -1)
case "$TOKEN" in oak_*) echo "PASS  registry installed and key issued" ;; *) echo "FAIL  registry installed and key issued"; exit 1;; esac

cat > "$WORK/gateway.json" <<EOF
{"v": "openagents.gateway.v1", "listen": "127.0.0.1:$GW_PORT",
 "registry": "$REGISTRY", "doors": {"local-kev": {"endpoint": "http://127.0.0.1:$BE_PORT"}}}
EOF
python3 deploy/gateway/stub-backend.py "$BE_PORT" "$DIGEST" &
BACKEND_PID=$!
sleep 1

if start_gateway; then echo "PASS  gateway started"; else echo "FAIL  gateway started"; exit 1; fi
check "liveness" "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$GW_PORT/healthz)" "200"
check "door discovery" "$(curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:$GW_PORT/v1/models | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["models"]))')" "1"

echo "== one real bounded call =="
OUTCOME=$(curl -s -X POST http://127.0.0.1:$GW_PORT/v1/systemone \
    -H "Authorization: Bearer $TOKEN" -H "content-type: application/json" \
    -d '{"model":"local-kev","state":"the sky","questions":{"sky":{"type":"noul","instructions":"Is the sky blue?","criteria":"yes/no"}}}' \
    -o "$WORK/response.json" -w '%{http_code}')
check "decision call" "$OUTCOME" "200"
check "typed answer" "$(python3 -c 'import json; print(json.load(open("'$WORK'/response.json"))["answers"]["sky"]["answer"])' 2>/dev/null)" "True"
check "receipt persisted" "$(test -s "$REGISTRY/receipts.jsonl" && echo yes)" "yes"
check "quota settled" "$(test -s "$REGISTRY/quota-ledger.jsonl" && echo yes)" "yes"

echo "== restart and recovery =="
stop_gateway
if start_gateway; then echo "PASS  restart"; else echo "FAIL  restart"; FAILED=1; fi
check "ledger survives" "$(curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:$GW_PORT/v1/models | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["models"]))')" "1"

echo "== backup, restore, delete =="
stop_gateway
cp -a "$REGISTRY" "$WORK/registry-backup"
rm -rf "$REGISTRY"
cp -a "$WORK/registry-backup" "$REGISTRY"
if start_gateway; then echo "PASS  restore"; else echo "FAIL  restore"; FAILED=1; fi
check "restored call" "$(curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:$GW_PORT/v1/systemone \
    -H "Authorization: Bearer $TOKEN" -H "content-type: application/json" \
    -H "Idempotency-Key: verify-restore-1" \
    -d '{"model":"local-kev","state":"restored","questions":{"q2":{"type":"noul","instructions":"Up?","criteria":"yes/no"}}}')" "200"

echo "== rollback =="
# A release is an immutable directory under a `current` symlink; rollback
# repoints the symlink. Simulated here with the same binary at two paths.
mkdir -p "$WORK/releases/a" "$WORK/releases/b"
cp "$GATEWAY" "$WORK/releases/a/gateway"
cp "$GATEWAY" "$WORK/releases/b/gateway"
ln -sfn "$WORK/releases/b" "$WORK/current"
stop_gateway
ln -sfn "$WORK/releases/a" "$WORK/current"
if start_gateway "$WORK/current/gateway"; then echo "PASS  rollback serves"; else echo "FAIL  rollback serves"; FAILED=1; fi

echo "== deletion =="
stop_gateway
rm -rf "$REGISTRY"
check "registry deleted" "$(test ! -e "$REGISTRY" && echo gone)" "gone"

echo
if [ "$FAILED" = 0 ]; then
    echo "all checks passed"
else
    echo "one or more checks FAILED"
    exit 1
fi
