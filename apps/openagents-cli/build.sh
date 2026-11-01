#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the Coax CLI to C and links it with the host C files)
# Builds the openagents CLI into apps/openagents-cli/build/openagents.
#   apps/openagents-cli/build.sh            then: apps/openagents-cli/build/openagents help
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"; p="$repo/coax/platform"
coax="$repo/coax/bin/coax"
[[ -x "$coax" ]] || { echo "build the compiler first: coax/build.sh" >&2; exit 2; }
if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security -framework CoreFoundation); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); fi
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
"$coax" c "$here/main.coax" --library --run-steps 250000000 > "$work/openagents.c"
mkdir -p "$here/build"
"${cc[@]}" -std=c11 -O2 -I"$work" -I"$p" -o "$work/openagents" "$work/openagents.c" "$here/cli-native.c" \
  "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" \
  "$work/entropy.c" "${libs[@]}" -lm
mv -f "$work/openagents" "$here/build/openagents"
echo "$here/build/openagents"
