#!/usr/bin/env bash
# The web app for Design Lab's Web board: builds openagents-web, runs it on
# 127.0.0.1:${DESIGN_LAB_WEB_PORT:-4380} with an empty store of its own (the
# screens gallery uses sample data only, so no account or service is needed),
# and rebuilds and restarts it whenever a source file changes. The board
# (`cargo run -p design-lab -- serve`, then `#app=web`) sees the restart and
# reloads its frames.
#
#   crates/design-lab/scripts/web-dev.sh [CHECKOUT]
#
# CHECKOUT is the openagents checkout to build and watch (default: the one
# this script is in).
#
# A build that fails keeps the last good server running and prints why; the
# next save tries again. Ctrl-C stops the server too.
set -u

repo="$(cd "${1:-$(dirname "$0")/../../..}" && pwd)"
port="${DESIGN_LAB_WEB_PORT:-4380}"
state="${TMPDIR:-/tmp}/design-lab-web"
mkdir -p "$state/store" "$state/chats"
stamp="$state/.built"
cd "$repo" || exit 1
# What a change to the web app can touch: the web crate and the crates in
# this checkout it is built from (all of crates/ if cargo can't say).
watch=()
while IFS= read -r dir; do
  case "$dir" in "$repo"/*) watch+=("$dir") ;; esac
done < <(cargo tree -p openagents-web-site -e normal,build --prefix none 2>/dev/null \
  | sed -n 's/.*(\(\/[^)]*\)).*/\1/p' | sort -u)
if [ "${#watch[@]}" -eq 0 ]; then
  for dir in "$repo"/crates/*/; do
    [ -f "$dir/Cargo.toml" ] && watch+=("$dir")
  done
fi
watch+=("$repo/Cargo.toml" "$repo/Cargo.lock")
echo "web-dev: watching ${#watch[@]} folders and files in $repo"
target="${CARGO_TARGET_DIR:-$repo/target}"
# Rebuild only what a save changed: incremental builds, without the shared
# compile cache's wrapper (it turns incremental off, so every save would
# recompile all of openagents-web). Dependencies still come from the cache.
export CARGO_BUILD_RUSTC_WRAPPER="${DESIGN_LAB_WEB_WRAPPER:-}"
export CARGO_INCREMENTAL=1
server=""

stop() {
  if [ -n "$server" ] && kill -0 "$server" 2>/dev/null; then
    kill "$server" 2>/dev/null
    wait "$server" 2>/dev/null
  fi
  server=""
}
trap 'stop; exit 0' INT TERM
trap stop EXIT

changed() {
  find "${watch[@]}" \( -name target -o -name node_modules -o -name .git \) -prune -o \
    -type f \( -name '*.rs' -o -name '*.css' -o -name '*.js' -o -name '*.html' -o -name '*.md' \
    -o -name '*.json' -o -name '*.toml' -o -name '*.svg' \) -newer "$stamp" -print -quit 2>/dev/null
}

build_and_start() {
  touch "$stamp"
  echo "web-dev: building openagents-web…"
  local started=$SECONDS
  # One build: rendered errors to the log, the built binary's path from JSON.
  if ! cargo build -p openagents-web-site --bin openagents-web --message-format json-render-diagnostics \
    >"$state/build.json" 2>"$state/build.log"; then
    grep -E -A12 '^error' "$state/build.log" | head -60
    echo "web-dev: the build failed; the last good server keeps running. Save again to retry."
    return
  fi
  local bin
  bin="$(sed -n 's/.*"executable":"\([^"]*openagents-web\)".*/\1/p' "$state/build.json" | tail -1)"
  [ -x "$bin" ] || bin="$target/debug/openagents-web"
  stop
  "$bin" --listen "127.0.0.1:$port" --store "$state/store" --chat-store "$state/chats" &
  server=$!
  for _ in $(seq 1 100); do
    curl -fs "http://127.0.0.1:$port/ui/screens.json" >/dev/null 2>&1 && break
    sleep 0.2
  done
  echo "web-dev: serving http://127.0.0.1:$port/ui/screens ($((SECONDS - started))s). Watching for changes…"
}

build_and_start
while true; do
  sleep 1
  if [ -n "$(changed)" ]; then
    # Let an editor finish writing a batch of files.
    sleep 0.3
    build_and_start
  elif [ -n "$server" ] && ! kill -0 "$server" 2>/dev/null; then
    echo "web-dev: the server stopped. Save a file to build and start it again."
    server=""
  fi
done
