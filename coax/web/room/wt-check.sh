#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds wt-echo with the host C compiler, makes a short-lived
# certificate with the openssl command every host has, and runs the Coax check against it)
# The WebTransport prototype's check (#13175), in the room's checks: wt-echo (wt-echo.c, wt.c and
# the vendored QUIC stack) on 127.0.0.1:PORT, a new ECDSA P-256 certificate valid for 10 days
# (Chrome's serverCertificateHashes takes P-256 and under 14 days), and a headless Chrome
# (wt-check.coax) that opens a session, echoes 1,000 datagrams and a 100 kB stream, and closes.
#   coax/web/room/wt-check.sh [PORT]                      local (default port 9441)
#   coax/web/room/wt-check.sh --remote URL HASH [PORT]    a WebTransport server elsewhere (the room
#                                                         VM's staging port), from a local page
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"; root="$(cd "$here/../../.." && pwd -P)"
remote_url=""; remote_hash=""
if [[ "${1:-}" == --remote ]]; then remote_url="${2:?URL}"; remote_hash="${3:?HASH}"; shift 3; fi
port="${1:-9441}"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-wtcheck.XXXXXX")"
pid=""
cleanup() { [[ -z "$pid" ]] || kill "$pid" 2>/dev/null || true; rm -rf "${work:?}"; }
trap cleanup EXIT
"$here/third_party/build.sh" "$here/build/quic" >/dev/null || exit 1
T="$here/third_party"
"${CC:-cc}" -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -DNGTCP2_STATICLIB -DNGHTTP3_STATICLIB \
  -I"$T/ngtcp2/lib/includes" -I"$T/ngtcp2/crypto/includes" -I"$T/nghttp3/lib/includes" -I"$T/picotls/include" \
  "$here/wt.c" "$here/wt-echo.c" "$here/build/quic/libquic.a" -o "$work/wt-echo" || exit 1
"$here/wt-cert.sh" "$work/cert.pem" "$work/key.pem" > "$work/hash" || exit 1
hash="$(cat "$work/hash")"
"$work/wt-echo" "$port" "$work/cert.pem" "$work/key.pem" > "$work/echo.log" 2>&1 &
pid=$!
for _ in $(seq 1 50); do grep -q listening "$work/echo.log" && break; kill -0 "$pid" 2>/dev/null || { cat "$work/echo.log" >&2; exit 1; }; read -r -t 0.1 < /dev/zero || true; done
url="https://127.0.0.1:$port/wt"
if [[ -n "$remote_url" ]]; then url="$remote_url"; hash="$remote_hash"; fi
cd "$root"
status=0
coax/web/tool.sh coax/web/room/wt-check.coax "http://127.0.0.1:$port/" "$url" "$hash" 2>&1 | tee "$work/check.log" || status=$?
sed 's/^/  /' "$work/echo.log"
# A build host without a browser still builds the stack and starts the server; the browser half is
# said to be skipped, never passed.
if (( status != 0 )) && grep -q 'no headless Chromium' "$work/check.log"; then echo "wt-check: skipped the browser half (no headless Chromium here); wt-echo built and listened"; exit 0; fi
exit "$status"
