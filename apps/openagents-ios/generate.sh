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
rm -rf $g/Components/fonts $g/Components/icons
cp -R components/zeron/fonts components/zeron/icons $g/Components/
cp components/zeron/NOTICE $g/Components/NOTICE
'
