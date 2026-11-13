#!/usr/bin/env bash
# coax serve: builds the Coaxial dev server and runs it on an app.
#   coax/web/serve.sh APP_DIR [--listen HOST:PORT] [--build CMD] [--api PORT]
# On apps/openagents-web with no --api (and not --static), it starts the local API first
# (coax/web/api/test.sh --serve: its own Postgres, you as its admin, Gemini on a token that
# refreshes itself) and Coder's local side (coder/tui/coder.sh mcp --http, behind --local:
# the agents on this machine, #13465), and stops both with the server, so /chat and the
# editor's agent answer (#13463). OA_SERVE_NO_API=1 leaves both out, OA_SERVE_NO_CODER=1 Coder.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
if [[ ! -x "$here/build/serve" || "$here/serve.c" -nt "$here/build/serve" || "$here/http-range.coax" -nt "$here/build/serve" || "$here/site-routes.coax" -nt "$here/build/serve" || "$here/realm-origin.coax" -nt "$here/build/serve" || "$here/../../metaverse-engine/editor/code-wire.coax" -nt "$here/build/serve" || "$here/../lib/crypto/sha256-kernel.coax" -nt "$here/build/serve" || "$here/../bin/coax" -nt "$here/build/serve" ]]; then
  # The Mac's system clang; elsewhere the host C compiler (gcc unless CC says).
  if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); fi
  # Always fortified, as Nix's compiler builds it: a buffer glibc's checks refuse fails at once, everywhere.
  "$here/../bin/coax" c "$here/http-range.coax" --library --header "$here/build/http-range.h" > "$here/build/http-range.c"
  "$here/../bin/coax" c "$here/site-routes.coax" --library --header "$here/build/site-routes.h" > "$here/build/site-routes.c"
  # The realm origin's rules (#13432): which addresses it answers and the headers that sandbox them.
  "$here/../bin/coax" c "$here/realm-origin.coax" --library --header "$here/build/realm-origin.h" > "$here/build/realm-origin.c"
  # A realm's code from the browser, on the dev server only (#13421): the one codec and the rule for a path.
  "$here/../bin/coax" c "$here/../../metaverse-engine/editor/code-wire.coax" --library --header "$here/build/code-wire.h" > "$here/build/code-wire.c"
  # A changed pack or texture is told by its SHA-256 (#13410).
  "$here/../bin/coax" c "$here/../lib/crypto/sha256-kernel.coax" --library --header "$here/build/sha256-kernel.h" > "$here/build/sha256-kernel.c"
  "${cc[@]}" -std=c11 -O2 -U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=2 -Wall -Wextra -I"$here/build" -o "$here/build/serve" "$here/serve.c" "$here/build/http-range.c" "$here/build/site-routes.c" "$here/build/realm-origin.c" "$here/build/code-wire.c" "$here/build/sha256-kernel.c" "$here/../../metaverse/voice-host/ws.c" -lm
fi
want_api=0
if [[ "$(cd "${1:-.}" 2>/dev/null && pwd -P)" == "$(cd "$here/../../apps/openagents-web" && pwd -P)" && -z "${OA_SERVE_NO_API:-}" ]]; then
  want_api=1; listen=127.0.0.1:8800; prev=""
  for a in "$@"; do
    [[ "$a" == --api || "$a" == --static ]] && want_api=0
    [[ "$prev" == --listen ]] && listen="$a"
    prev="$a"
  done
fi
[[ $want_api == 1 ]] || COAXIAL_MODE=dev exec "$here/build/serve" "$@"
port_file="$(mktemp)"; coder_log="$(mktemp)"
trap 'kill ${serve_pid:-} ${api_pid:-} ${coder_pid:-} 2>/dev/null || true; rm -f "$port_file" "$coder_log"' EXIT
echo "serve: starting the local API for /chat and the editor's agent (a minute or two the first time)"
# The editor's Gemini turn opens attachments from the dev server's own store (serve.c files_local, #13468).
OA_WORLD_FILES_DIR="$(cd "$1" && pwd -P)/build/.worlds" OA_SERVE_PORT_FILE="$port_file" "$here/api/test.sh" --serve "http://$listen" & api_pid=$!
# Coder's local routes for the agents on this machine, from this checkout, on a port of its own. A harness
# never opens a browser sign-in from here: an agent with no login shows as not ready (Settings, This machine).
if [[ -z "${OA_SERVE_NO_CODER:-}" ]]; then
  echo "serve: starting Coder's local side for the agents on this machine"
  (cd "$here/../.." && BROWSER=/usr/bin/true exec "$here/../../coder/tui/coder.sh" mcp --http 0) > "$coder_log" 2>&1 & coder_pid=$!
fi
while [[ ! -s "$port_file" ]]; do
  kill -0 "$api_pid" 2>/dev/null || { echo "serve: the local API didn't start (above); OA_SERVE_NO_API=1 serves the pages without it" >&2; exit 1; }
  sleep 0.5
done
local_args=()
while [[ -n "${coder_pid:-}" ]]; do
  coder_port="$(sed -n 's|^coder mcp: http://127.0.0.1:\([0-9][0-9]*\)/mcp$|\1|p' "$coder_log" | head -n 1)"
  if [[ -n "$coder_port" ]]; then local_args=(--local "$coder_port"); echo "serve: Coder answers on 127.0.0.1:$coder_port"; break; fi
  kill -0 "$coder_pid" 2>/dev/null || { echo "serve: Coder didn't start (its output: $(tail -n 3 "$coder_log" | tr '\n' ' ')); the page offers our models only" >&2; coder_pid=""; break; }
  sleep 0.5
done
COAXIAL_MODE=dev "$here/build/serve" "$@" --api "$(cat "$port_file")" ${local_args[@]+"${local_args[@]}"} & serve_pid=$!
wait "$serve_pid"
