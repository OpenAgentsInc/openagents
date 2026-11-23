#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: compiles apps/shared/antigravity/client-core.coax to C and checks the 512 KB stack-frame budget for macOS; holds no logic).
# Generates apps/openagents-macos/Generated/client_core.{c,h} from apps/shared/antigravity/client-core.coax (GitHub #13993).
set -euo pipefail
cd "$(dirname "$0")/../.."
repo="$(pwd -P)"
gen="$repo/apps/openagents-macos/Generated"
mkdir -p "$gen"

coax="${COAX:-$repo/coax/bin/coax}"
if [[ -x "$coax" ]]; then
  "$coax" c apps/shared/antigravity/client-core.coax --library --header "$gen/client_core.h" > "$gen/client_core.c"
else
  coax/remote/remote.sh run -- bash -ec '
    g=apps/openagents-macos/Generated; mkdir -p "$g"
    coax/bin/coax c apps/shared/antigravity/client-core.coax --library --header "$g/client_core.h" > "$g/client_core.c"
  '
fi

if command -v xcrun > /dev/null && sdk="$(xcrun --sdk macosx --show-sdk-path 2> /dev/null)"; then
  for inline in "" -fno-inline; do
    xcrun clang -target arm64-apple-macos15.0 -isysroot "$sdk" -Os $inline -c "$gen/client_core.c" \
      -I "$gen" -I "$repo/apps/shared/antigravity" -o /dev/null -Wframe-larger-than=524288 -Werror=frame-larger-than \
      || { echo "generate.sh: a stack frame is over the 512 KB budget${inline:+ ($inline)}" >&2; exit 1; }
  done
fi
