#!/usr/bin/env bash
# Firm reason: 5 (thin compiler/host-compiler wrapper; all app and update rules are Coax).
set -euo pipefail
mkdir -p apps/openagents-ios/Generated apps/openagents-ios/Starter/build
"${COAX:-coax/bin/coax}" c coax/native/updates-policy.coax --library --header apps/openagents-ios/Generated/NativePolicy.h > apps/openagents-ios/Generated/NativePolicy.c
"${COAX:-coax/bin/coax}" c coax/native/updates-wire.coax --library --header apps/openagents-ios/Generated/NativeWire.h > apps/openagents-ios/Generated/NativeWire.c
"${COAX:-coax/bin/coax}" c metaverse/phone-signin.coax --library --header apps/openagents-ios/Generated/PhoneSignin.h > apps/openagents-ios/Generated/PhoneSignin.c
coax/web/tool.sh coax/native/runtime.coax apps/openagents-ios/Generated/runtime.txt apps/shared/native/NativeHost.swift apps/shared/native/NativeUpdates.swift apps/shared/host/apple/AppAuth.swift apps/shared/host/apple/DeviceKey.swift apps/openagents-ios/OpenAgents/SignIn/PhoneSignIn.swift apps/openagents-ios/OpenAgents/SignIn/NostrKey.swift apps/openagents-ios/NativeBridge.h apps/openagents-ios/StarterApp.swift apps/openagents-ios/OpenAgents/Info.plist apps/openagents-ios/OpenAgents/OpenAgents.entitlements apps/openagents-ios/OpenAgents/Platform/NostrSign.c apps/openagents-ios/OpenAgents/Platform/NostrSign.h apps/openagents-ios/project.yml coax/native/openagents-public-key.txt apps/openagents-ios/Generated/NativePolicy.c apps/openagents-ios/Generated/NativeWire.c apps/openagents-ios/Generated/PhoneSignin.c
coax/web/wasm.sh coax/native/starter/app.coax coax/native/starter/face.c apps/openagents-ios/Starter/build/app.wasm
coax/web/tool.sh coax/native/prepare.coax "$@"
