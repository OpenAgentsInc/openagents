#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds a Coax tool with the host C compiler, then runs it)
# Runs a command-line tool written in Coax: coax/web/NAME.coax (or any FILE.coax
# whose `tool_main :: () -> int #export_c` is the program), built with the
# platform effects of coax/lib/sys and coax/lib/net/cdp.coax into
# coax/web/build/tools/NAME, rebuilt when a Coax or platform source is newer.
# The entropy policy (coax/lib/crypto/entropy.coax) is linked here; a tool
# does not #load it. A tool with a "// tool.sh: tls" line (in its file or a
# file it #loads from its own directory) also links tls-native.c with coax/platform/tls-env.sh's flags,
# for wss:// and https:// (coax/lib/net/relay_conn.coax, http.coax).
#   coax/web/tool.sh shot URL OUT.png [WxH] [WAIT_MS] [--eval JS]
#   coax/web/tool.sh clip URL OUT.mp4 [--size WxH] [--seconds S] ...
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
[[ $# -ge 1 ]] || { echo "usage: coax/web/tool.sh NAME|FILE.coax [ARGS...]" >&2; exit 2; }
src="$here/$1.coax"; [[ "$1" == *.coax ]] && src="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
shift
name="$(basename "$src" .coax)"; bin="$here/build/tools/$name"; p="$repo/coax/platform"
# Imports may live beside the entry file, beside web tools or in Verse,
# Psionic, apps, protocol and OS sources. Conservatively watch those Coax trees as well as the entry/native
# inputs when no source list was recorded; a cached executable must not silently retain an older #load.
# A build records the sources it read (coax deps) in NAME.sources; with that
# list, only those files, the compiler and the platform sources can make it
# stale, so a change elsewhere in the repository doesn't rebuild the tool.
sources="$bin.sources"
why=""
if [[ ! -x "$bin" ]]; then why="first build"
elif changed="$(find "$repo/coax/bin/coax" "$p" -newer "$bin" -print -quit)"; [[ -n "$changed" ]]; then why="$changed changed"
elif [[ -s "$sources" ]]; then
  while IFS= read -r f; do
    if [[ ! -e "$f" || "$f" -nt "$bin" ]]; then why="$f changed"; break; fi
  done < "$sources"
  [[ -n "$why" ]] || { changed="$(find "$(dirname "$src")" -maxdepth 1 -name '*.coax' -newer "$bin" -print -quit)"; [[ -z "$changed" ]] || why="$changed changed"; }
elif changed="$(find "$src" "$(dirname "$src")" "$repo/coax" "$repo/verse" "$repo/psionic" "$repo/apps" "$repo/proto" "$repo/os" -name '*.coax' -newer "$bin" -print -quit)"; [[ -n "$changed" ]]; then
  why="$changed changed"
fi
if [[ -n "$why" ]]; then
  started=$SECONDS
  # Say why the tool isn't up yet: a rebuild takes a while (Coder about 30 s).
  [[ ! -t 2 ]] || printf '%s: rebuilding (%s) ...\n' "$name" "${why#"$repo/"}" >&2
  if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); fi
  tls=()
  near=("$src"); while read -r f; do near+=("$(dirname "$src")/$f"); done < <(sed -n 's/^#load "\([^/"]*\.coax\)";.*/\1/p' "$src")
  if grep -qs '^// tool.sh: tls' "${near[@]}"; then
    # shellcheck source=/dev/null
    source "$p/tls-env.sh"; tls=("$p/tls-native.c" $CXTLS_CFLAGS); libs+=($CXTLS_LIBS)
  fi
  work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
  "$repo/coax/bin/coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
  "$repo/coax/bin/coax" deps "$src" > "$work/deps.json" 2> /dev/null & deps_pid=$!
  "$repo/coax/bin/coax" c "$src" --library > "$work/$name.c"
  mkdir -p "$(dirname "$bin")"
  "${cc[@]}" -std=c11 -O2 -I"$work" -I"$p" -o "$work/$name" "$work/$name.c" "$p/tool-native.c" "$p/process-native.c" \
    "$p/store-file-native.c" "$p/read-file-native.c" "$p/socket-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" ${tls[@]+"${tls[@]}"} "${libs[@]}" -lm
  mv -f "$work/$name" "$bin"
  # the sources this build read, for the next staleness check (none: the broad check)
  if wait "$deps_pid" && grep -o '"sources":\[[^]]*\]' "$work/deps.json" | grep -o '"/[^"]*"' | tr -d '"' > "$work/sources" && [[ -s "$work/sources" ]]; then
    mv -f "$work/sources" "$sources"
  else rm -f "$sources"; fi
  rm -rf "$work"; trap - EXIT
  [[ ! -t 2 ]] || printf '%s: built in %d s\n' "$name" $((SECONDS - started)) >&2
fi
exec "$bin" "$@"
