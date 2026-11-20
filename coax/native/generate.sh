#!/usr/bin/env bash
# Firm reason: 5 (thin compiler/host-compiler wrapper; all app rules are Coax).
# Generates the Coax Native starter artifacts: the installed policy and text host as C,
# the reusable starter application as WebAssembly, and the Starter folder.
set -euo pipefail
out=coax/native/build
mkdir -p $out/native-gen $out/Starter/build
"${COAX:-coax/bin/coax}" c coax/native/updates-policy.coax --library --header $out/native-gen/NativePolicy.h > $out/native-gen/NativePolicy.c
"${COAX:-coax/bin/coax}" c coax/native/text-host.coax --library --header $out/native-gen/NativeText.h > $out/native-gen/NativeText.c
coax/web/wasm.sh coax/native/starter/app.coax coax/native/starter/face.c $out/Starter/build/starter.wasm
coax/web/tool.sh coax/native/prepare.coax "$@"
