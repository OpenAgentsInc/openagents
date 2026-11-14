#!/usr/bin/env bash
# Firm reason: 5 (thin remote Coax generation and macOS SDK/signing wrapper).
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax/remote/remote.sh run -- bash coax/native/generate.sh
coax/web/tool.sh coax/native/compile.coax apps/openagents-ios/Starter/build/app.wasm apps/openagents-ios/Starter/build/app-mvp.wasm apps/openagents-ios/Starter/build/app.js
coax/remote/remote.sh run -- bash -ec 'mkdir -p apps/openagents-desktop/build/native-c; coax/bin/coax c apps/openagents-desktop/lab-sign.coax --library --run-steps 250000000 > apps/openagents-desktop/build/native-c/lab-sign.c'
work="$PWD/apps/openagents-desktop/build"
app="$work/Native/OpenAgents.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for unit in NativePolicy NativeText; do
  coax/remote/remote.sh run --local -- /usr/bin/clang -O2 -w -c "apps/openagents-ios/Generated/$unit.c" -o "$work/native-c/$unit.o"
done
coax/remote/remote.sh run --local -- /usr/bin/clang -O2 -w -c "$work/native-c/lab-sign.c" -o "$work/native-c/lab-sign.o"
coax/remote/remote.sh run --local -- xcrun swiftc -O -swift-version 5 -suppress-warnings -parse-as-library -import-objc-header apps/openagents-desktop/native/NativeBridge.h \
  apps/openagents-desktop/native/App.swift apps/openagents-desktop/native/EnvironmentWindow.swift \
  apps/shared/native/NativeProgram.swift apps/shared/native/NativeTypes.swift apps/openagents-desktop/Account.swift \
  apps/shared/host/apple/AppAuth.swift apps/shared/host/apple/DeviceKey.swift \
  "$work/native-c/lab-sign.o" "$work/native-c/NativePolicy.o" "$work/native-c/NativeText.o" \
  -framework AppKit -framework JavaScriptCore -framework CryptoKit -framework Security -framework UserNotifications \
  -o "$app/Contents/MacOS/OpenAgents"
cp apps/openagents-desktop/Info.plist "$app/Contents/Info.plist"
cp apps/openagents-desktop/native/OpenAgents.icns "$app/Contents/Resources/OpenAgents.icns"
rm -rf "$app/Contents/Resources/Starter"
cp -R apps/openagents-ios/Starter "$app/Contents/Resources/Starter"
codesign --force --options runtime --entitlements apps/openagents-desktop/native/entitlements.plist \
  --sign "${OA_DESKTOP_SIGNING_IDENTITY:-Developer ID Application: OpenAgents, Inc. (HQWSG26L43)}" "$app"
"$app/Contents/MacOS/OpenAgents" --inspect-program
echo "built $app"
