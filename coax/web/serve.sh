#!/usr/bin/env bash
# coax serve: builds the Coaxial dev server and runs it on an app.
#   coax/web/serve.sh APP_DIR [--listen HOST:PORT] [--build CMD]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
if [[ ! -x "$here/build/serve" || "$here/serve.c" -nt "$here/build/serve" || "$here/http-range.coax" -nt "$here/build/serve" || "$here/site-routes.coax" -nt "$here/build/serve" || "$here/../bin/coax" -nt "$here/build/serve" ]]; then
  # The Mac's system clang; elsewhere the host C compiler (gcc unless CC says).
  if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); fi
  # Always fortified, as Nix's compiler builds it: a buffer glibc's checks refuse fails at once, everywhere.
  "$here/../bin/coax" c "$here/http-range.coax" --library --header "$here/build/http-range.h" > "$here/build/http-range.c"
  "$here/../bin/coax" c "$here/site-routes.coax" --library --header "$here/build/site-routes.h" > "$here/build/site-routes.c"
  "${cc[@]}" -std=c11 -O2 -U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=2 -Wall -Wextra -I"$here/build" -o "$here/build/serve" "$here/serve.c" "$here/build/http-range.c" "$here/build/site-routes.c" "$here/../../verse/voice-host/ws.c" -lm
fi
COAXIAL_MODE=dev exec "$here/build/serve" "$@"
