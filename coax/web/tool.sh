#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds a Coax tool with the host C compiler, then runs it)
# Runs a command-line tool written in Coax: coax/web/NAME.coax (or any FILE.coax
# whose `tool_main :: () -> int #export_c` is the program), built with the
# platform effects of coax/lib/sys and coax/lib/net/cdp.coax into
# coax/web/build/tools/NAME, rebuilt when a Coax or platform source is newer.
# The entropy policy (coax/lib/crypto/entropy.coax) is linked here; a tool
# does not #load it.
#   coax/web/tool.sh shot URL OUT.png [WxH] [WAIT_MS] [--eval JS]
#   coax/web/tool.sh clip URL OUT.mp4 [--size WxH] [--seconds S] ...
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
[[ $# -ge 1 ]] || { echo "usage: coax/web/tool.sh NAME|FILE.coax [ARGS...]" >&2; exit 2; }
src="$here/$1.coax"; [[ "$1" == *.coax ]] && src="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
shift
name="$(basename "$src" .coax)"; bin="$here/build/tools/$name"; p="$repo/coax/platform"
# Imports may live beside web tools or in Verse, Psionic, apps and protocol
# sources. Conservatively watch those Coax trees as well as the entry/native
# inputs; a cached executable must not silently retain an older #load.
if [[ ! -x "$bin" || -n "$(find "$src" "$repo/coax/lib" "$p" "$repo/coax/bin/coax" -newer "$bin" -print -quit)" ||
      -n "$(find "$repo/coax" "$repo/verse" "$repo/psionic" "$repo/apps" "$repo/proto" -name '*.coax' -newer "$bin" -print -quit)" ]]; then
  if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); fi
  work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
  "$repo/coax/bin/coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
  "$repo/coax/bin/coax" c "$src" --library > "$work/$name.c"
  mkdir -p "$(dirname "$bin")"
  "${cc[@]}" -std=c11 -O2 -I"$work" -I"$p" -o "$work/$name" "$work/$name.c" "$p/tool-native.c" "$p/process-native.c" \
    "$p/store-file-native.c" "$p/socket-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" "${libs[@]}" -lm
  mv -f "$work/$name" "$bin"; rm -rf "$work"; trap - EXIT
fi
exec "$bin" "$@"
