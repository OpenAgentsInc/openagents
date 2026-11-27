#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: compiles apps/openagents-ios/ios-app.coax to C, emits Metal shader + atlas assets, enforces 512 KB stack-frame budget).
# Generates apps/openagents-ios/Generated/gallery.{c,h}, Generated/SparkSDK, and Generated/Components/.
# Zero JavaScript, zero WebKit, zero WebAssembly.
set -euo pipefail
cd "$(dirname "$0")/../.."
repo="$(pwd -P)"
gen="$repo/apps/openagents-ios/Generated"
mkdir -p "$gen/Components/fonts" "$gen/Components/icons"
rm -f "$gen/client_core.c" "$gen/client_core.h" "$gen/mobile_core.c" "$gen/mobile_core.h"

# Stage pinned Breez Spark SDK local SwiftPM package (breez_sdk_sparkFFI.xcframework + BigNumber + BreezSdkSpark).
"$repo/apps/shared/host/apple/spark-sdk.sh" --ios-package "$gen/SparkSDK" > /dev/null
cp "$repo/metaverse/stage-resources/bip39-english.txt" "$gen/bip39-english.txt"

coax="${COAX:-$repo/coax/bin/coax}"
if [[ -x "$coax" ]]; then
  "$coax" c apps/openagents-ios/ios-app.coax --library --header "$gen/gallery.h" > "$gen/gallery.c"
  "$coax" metal components/backend/gpu-shader.coax --vertex chat_fx_vs --fragment chat_fx_fs -o "$gen/Components/components.metal"
else
  coax/remote/remote.sh run -- bash -ec '
    g=apps/openagents-ios/Generated; mkdir -p "$g/Components/fonts" "$g/Components/icons"
    coax/bin/coax c apps/openagents-ios/ios-app.coax --library --header "$g/gallery.h" > "$g/gallery.c"
    coax/bin/coax metal components/backend/gpu-shader.coax --vertex chat_fx_vs --fragment chat_fx_fs -o "$g/Components/components.metal"
  '
fi

rm -rf "$gen/Components/fonts" "$gen/Components/icons"
mkdir -p "$gen/Components/fonts" "$gen/Components/icons"
cp "$repo/components/fonts/Geist"* "$gen/Components/fonts/"
cp "$repo/components/icons/"*.svg "$gen/Components/icons/"
cp "$repo/components/NOTICE" "$gen/Components/NOTICE"

# Frame budget (#13747): no generated C function may hold a stack frame over 512 KB at -Os,
# with and without inlining, matching iOS Release builds. Needs Xcode; skipped elsewhere.
if command -v xcrun > /dev/null && sdk="$(xcrun --sdk iphoneos --show-sdk-path 2> /dev/null)"; then
  for inline in "" -fno-inline; do
    xcrun clang -target arm64-apple-ios18.0 -isysroot "$sdk" -Os $inline -c "$gen/gallery.c" \
      -I "$gen" -o /dev/null -Wframe-larger-than=524288 -Werror=frame-larger-than \
      || { echo "generate.sh: a stack frame is over the 512 KB budget${inline:+ ($inline)}; move the large local into caller-owned state (#13747)" >&2; exit 1; }
  done
fi
