#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the Coax CLI to C and links it with coax/platform).
# Builds the gamebuilder command line (gamebuilder/cli/main.coax) into
# gamebuilder/build/cli/gamebuilder (and installs ~/.openagents/bin/gamebuilder with --install).
#   gamebuilder/cli/build.sh [--force] [--install]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
gb="$(cd "$here/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
[[ -x "$coax" ]] || { echo "build the compiler first: coax/build.sh" >&2; exit 2; }
force=false; install_bin=false
for arg in "$@"; do
  case "$arg" in
    --force) force=true ;;
    --install) install_bin=true ;;
    *) echo "usage: gamebuilder/cli/build.sh [--force] [--install]" >&2; exit 2 ;;
  esac
done
bin="$gb/build/cli/gamebuilder"
mkdir -p "$gb/build/cli"
head_sha="$(git -C "$repo" rev-parse HEAD 2>/dev/null || echo "0000000000000000000000000000000000000000")"
printf 'GB_ENGINE_PIN :: "%s";\n' "$head_sha" > "$gb/build/cli/pin.coax"
printf 'GB_COMMIT :: "%s";\nGB_DIRTY :: false;\n' "${head_sha:0:10}" > "$gb/build/stamp.coax"
if ! $force && [[ -x "$bin" ]] && [[ -z "$(find "$gb/core" "$gb/shared" "$gb/cli" "$coax" "$repo/apps/openagents-cli/cli-native.c" -newer "$bin" -print -quit)" ]]; then
  if $install_bin; then
    mkdir -p "$HOME/.openagents/bin"
    cp -f "$bin" "$HOME/.openagents/bin/gamebuilder"
  fi
  echo "$bin"; exit 0
fi
p="$repo/coax/platform"
. "$p/tls-env.sh"
if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security -framework CoreFoundation)
else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); fi
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
"$coax" c "$gb/cli/main.coax" --library --release > "$work/gamebuilder.c"
units=("$repo/apps/openagents-cli/cli-native.c" "$p/tls-native.c" "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/read-file-native.c" "$p/socket-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c")
# shellcheck disable=SC2086
"${cc[@]}" -std=c11 -O2 $CXTLS_CFLAGS -I"$work" -I"$p" -o "$work/gamebuilder" "$work/gamebuilder.c" "${units[@]}" "$work/entropy.c" "${libs[@]}" $CXTLS_LIBS -lm
mv -f "$work/gamebuilder" "$bin"
if $install_bin; then
  mkdir -p "$HOME/.openagents/bin"
  cp -f "$bin" "$HOME/.openagents/bin/gamebuilder"
fi
echo "$bin"
