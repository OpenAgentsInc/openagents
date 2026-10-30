#!/usr/bin/env bash
# coax serve: builds the Coaxial dev server and runs it on an app.
#   coax/web/serve.sh APP_DIR [--listen HOST:PORT] [--build CMD]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
if [[ ! -x "$here/build/serve" || "$here/serve.c" -nt "$here/build/serve" ]]; then
  # The Mac's system clang; elsewhere the host C compiler (gcc unless CC says).
  if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); fi
  "${cc[@]}" -std=c11 -O2 -Wall -Wextra -o "$here/build/serve" "$here/serve.c" "$here/../../verse/voice-host/ws.c"
fi
COAXIAL_MODE=dev exec "$here/build/serve" "$@"
