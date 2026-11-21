#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: Coax to C and Coax to Metal on a build host, in one job; holds no logic).
# component and scene shaders as Metal, with the kit's fonts and icons beside them. Nothing is
# compiled to JavaScript or WebAssembly.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax/remote/remote.sh run -- bash -ec '
g=apps/openagents-ios/Generated; mkdir -p $g/Components
coax/bin/coax c apps/openagents-ios/app.coax --library --header $g/gallery.h > $g/gallery.c
coax/bin/coax metal components/backend/gpu-shader.coax --vertex chat_fx_vs --fragment chat_fx_fs -o $g/Components/components.metal
coax/bin/coax metal components/backend/composition-shader.coax --vertex uc_vs --fragment uc_fs -o $g/Components/composition.metal
rm -rf $g/Components/fonts $g/Components/icons
mkdir -p $g/Components/fonts $g/Components/icons
cp components/fonts/Geist* $g/Components/fonts/
cp components/icons/*.svg $g/Components/icons/
cp components/NOTICE $g/Components/NOTICE
'
# Frame budget (#13747): the generated C may hold no stack frame over 512 KB, at -Os with and
# without inlining, as the iOS release build compiles it. Needs Xcode; skipped elsewhere.
if command -v xcrun > /dev/null && sdk="$(xcrun --sdk iphoneos --show-sdk-path 2> /dev/null)"; then
  for inline in "" -fno-inline; do
    xcrun clang -target arm64-apple-ios17.0 -isysroot "$sdk" -Os $inline -c apps/openagents-ios/Generated/gallery.c \
      -I apps/openagents-ios/Generated -I apps/openagents-ios -o /dev/null -Wframe-larger-than=524288 -Werror=frame-larger-than \
      || { echo "generate.sh: a stack frame is over the 512 KB budget${inline:+ ($inline)}; move the large local into caller-owned state (#13747)" >&2; exit 1; }
  done
fi
