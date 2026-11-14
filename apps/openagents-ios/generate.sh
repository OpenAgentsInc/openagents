#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: generate the Coax Native starter on a build host).
set -euo pipefail
cd "$(dirname "$0")/../.."
coax/remote/remote.sh run -- bash coax/native/generate.sh
coax/web/tool.sh coax/native/compile.coax apps/openagents-ios/Starter/build/app.wasm apps/openagents-ios/Starter/build/app-mvp.wasm apps/openagents-ios/Starter/build/app.js
coax/web/tool.sh coax/native/compile.coax apps/openagents-ios/Starter/build/extras.wasm apps/openagents-ios/Starter/build/extras-mvp.wasm apps/openagents-ios/Starter/build/extras.js
