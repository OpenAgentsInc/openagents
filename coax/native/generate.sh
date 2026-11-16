#!/usr/bin/env bash
# Firm reason: 5 (thin compiler/host-compiler wrapper; all app rules are Coax).
# Generates what the macOS native desktop app builds from (apps/openagents-desktop/native/build.sh):
# the installed policy and the text host as C, the Coax Native application and its extras as
# WebAssembly, and the Starter folder the app loads.
set -euo pipefail
out=apps/openagents-desktop/build
mkdir -p $out/native-gen $out/Starter/build
"${COAX:-coax/bin/coax}" c coax/native/updates-policy.coax --library --header $out/native-gen/NativePolicy.h > $out/native-gen/NativePolicy.c
"${COAX:-coax/bin/coax}" c coax/native/text-host.coax --library --header $out/native-gen/NativeText.h > $out/native-gen/NativeText.c
coax/web/wasm.sh apps/openagents-desktop/home/app.coax apps/openagents-desktop/home/face.c $out/Starter/build/app.wasm
coax/web/wasm.sh apps/openagents-desktop/extras/app.coax coax/native/starter/face.c $out/Starter/build/extras.wasm
coax/web/tool.sh coax/native/prepare.coax "$@"
