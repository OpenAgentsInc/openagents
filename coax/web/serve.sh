#!/usr/bin/env bash
# coax serve: builds the Coaxial dev server and runs it on an app.
#   coax/web/serve.sh APP_DIR [--listen HOST:PORT] [--build CMD]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
if [[ ! -x "$here/build/serve" || "$here/serve.c" -nt "$here/build/serve" ]]; then
  /usr/bin/clang -std=c11 -O2 -Wall -Wextra -D_DARWIN_C_SOURCE -o "$here/build/serve" "$here/serve.c" "$here/../../metareal/voice-host/ws.c"
fi
COAXIAL_MODE=dev exec "$here/build/serve" "$@"
