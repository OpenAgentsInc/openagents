#!/usr/bin/env bash
# Build and run the System One smoke against Baseten. The key comes from
# BASETEN_API_KEY, or from ~/work/.secrets/baseten.env when that is unset;
# it is never printed. Two small requests spend credit; the refusals don't.
#
#   coax/lib/systemone/smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"
if [ -z "${BASETEN_API_KEY:-}" ] && [ -f "$HOME/work/.secrets/baseten.env" ]; then
  set -a; . "$HOME/work/.secrets/baseten.env"; set +a
fi
[ -n "${BASETEN_API_KEY:-}" ] || { echo "set BASETEN_API_KEY or write it to ~/work/.secrets/baseten.env"; exit 1; }
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$repo/coax/bin/coax" c "$here/smoke.coax" > "$work/smoke.c"
"$cc" -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" -lm
export SO_SMOKE_BAD_KEY=not.a.key
"$work/smoke" | tee "$work/out"
tail -1 "$work/out" | grep -q '^smoke: ok$'
