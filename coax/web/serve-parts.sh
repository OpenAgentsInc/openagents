#!/usr/bin/env bash
# Firm reason: 5. The one list of what coax/web/serve.c is built from (#13447):
# the C files the Coax compiler writes from Coax sources, beside serve.c and
# metaverse/voice-host/ws.c. serve.sh (the dev server), deploy.sh (the
# production image) and check.sh (the after-landing check of that image) all
# read it, so a part serve.c starts to include can't be left out of one.
#
#   coax/web/serve-parts.sh --sources     the Coax sources, one a line (for staleness)
#   coax/web/serve-parts.sh DIR           write each part's .c and .h into DIR; print the .c paths
#   coax/web/serve-parts.sh --image DIR   the production image's folder: DIR/Dockerfile and
#                                         DIR/server/ with serve.c, ws.c, ws.h and every part
#   coax/web/serve-parts.sh --check       stage the image in a scratch folder and run the
#                                         Dockerfile's own compile line on it (gcc, as the image)
#
# COAX: the compiler (default coax/bin/coax in this checkout).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
coax="${COAX:-$root/coax/bin/coax}"
# name and Coax source of each part, relative to the checkout's root
parts=(
  "http-range coax/web/http-range.coax"            # HTTP ranges
  "site-routes coax/web/site-routes.coax"          # the site's routes
  "realm-origin coax/web/realm-origin.coax"        # the realm origin's rules (--realm-host, #13432)
  "code-wire metaverse-engine/editor/code-wire.coax" # the dev server's code route's codec (#13421)
  "world-alias metaverse-engine/editor/world-alias-host.coax" # a renamed world's old ID in /api/worlds/... (#13486)
  "sha256-kernel coax/lib/crypto/sha256-kernel.coax" # a changed pack or texture is told by its SHA-256 (#13410)
)

write_parts() {   # DIR
  local p name src
  [[ -x "$coax" ]] || { echo "serve-parts.sh: no Coax compiler at $coax: build it first (coax/build.sh)" >&2; exit 1; }
  mkdir -p "$1"
  for p in "${parts[@]}"; do
    read -r name src <<< "$p"
    "$coax" c "$root/$src" --library --header "$1/$name.h" > "$1/$name.c.tmp" && mv -f "$1/$name.c.tmp" "$1/$name.c"
    echo "$1/$name.c"
  done
}
image() {   # DIR: what deploy.sh sends to Cloud Build
  mkdir -p "$1/server"
  cp "$here/serve.c" "$root/metaverse/voice-host/ws.c" "$root/metaverse/voice-host/ws.h" "$1/server/"
  sed -i.bak 's#"../../metaverse/voice-host/ws.h"#"ws.h"#' "$1/server/serve.c" && rm -f "$1/server/serve.c.bak"
  write_parts "$1/server" > /dev/null
  cp "$here/Dockerfile" "$1/"
}

case "${1:-}" in
  --sources) for p in "${parts[@]}"; do read -r _ src <<< "$p"; echo "$root/$src"; done ;;
  --image) [[ -n "${2:-}" ]] || { echo "usage: serve-parts.sh --image DIR" >&2; exit 2; }; image "$2" ;;
  --check)
    w="$(mktemp -d "${TMPDIR:-/tmp}/serve-image.XXXXXXXX")"; trap 'rm -rf "${w:?}"' EXIT
    image "$w"
    # The Dockerfile's compile line, with its paths pointed at the scratch folder.
    line="$(sed -n 's/^RUN \(gcc .*\)$/\1/p' "$w/Dockerfile")"
    [[ -n "$line" ]] || { echo "serve-parts: no 'RUN gcc' line in coax/web/Dockerfile" >&2; exit 1; }
    line="${line//\/src\//$w/server/}"; line="${line//-o \/serve /-o $w/serve }"
    (cd "$w/server" && eval "$line") || { echo "serve-parts: the serve image's compile failed: a part serve.c includes is missing from coax/web/serve-parts.sh or the Dockerfile" >&2; exit 1; }
    [[ -x "$w/serve" ]] && echo "serve-parts: the serve image compiles (coax/web/Dockerfile)" ;;
  ""|-*) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
  *) write_parts "$1" ;;
esac
