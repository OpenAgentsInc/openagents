#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: compiles apps/shared/antigravity/client-core.coax to C and enforces stack-frame budget; holds no logic).
# Generates apps/openagents-ios/Generated/client_core.{c,h} from apps/shared/antigravity/client-core.coax (GitHub #13993).
# Zero Metal shaders, zero GPU component atlases, zero JavaScript or WebAssembly.
set -euo pipefail
cd "$(dirname "$0")/../.."
repo="$(pwd -P)"
gen="$repo/apps/openagents-ios/Generated"
mkdir -p "$gen"
# Remove any legacy GPU or superseded single-platform artifacts.
rm -rf "$gen/Components" "$gen/gallery.c" "$gen/gallery.h" "$gen/mobile_core.c" "$gen/mobile_core.h"

coax="${COAX:-$repo/coax/bin/coax}"
if [[ -x "$coax" ]]; then
  "$coax" c apps/shared/antigravity/client-core.coax --library --header "$gen/client_core.h" > "$gen/client_core.c"
else
  coax/remote/remote.sh run -- bash -ec '
    g=apps/openagents-ios/Generated; mkdir -p "$g"
    coax/bin/coax c apps/shared/antigravity/client-core.coax --library --header "$g/client_core.h" > "$g/client_core.c"
  '
fi

# Frame budget (#13747): no generated C function may hold a stack frame over 512 KB at -Os,
# with and without inlining, matching iOS Release builds. Needs Xcode; skipped elsewhere.
if command -v xcrun > /dev/null && sdk="$(xcrun --sdk iphoneos --show-sdk-path 2> /dev/null)"; then
  for inline in "" -fno-inline; do
    xcrun clang -target arm64-apple-ios18.0 -isysroot "$sdk" -Os $inline -c "$gen/client_core.c" \
      -I "$gen" -I "$repo/apps/shared/antigravity" -o /dev/null -Wframe-larger-than=524288 -Werror=frame-larger-than \
      || { echo "generate.sh: a stack frame is over the 512 KB budget${inline:+ ($inline)}; move the large local into caller-owned state (#13747)" >&2; exit 1; }
  done
fi
