#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, swiftc and codesign; holds no logic)
# Builds OpenAgents.app for this Mac: the page (app.coax, page.coax) compiled to C on the shared
# component carrier (carrier.c, host.c) and Mac host (apps/shared/host/apple/ui-host.m), with the
# component shader emitted as Metal, the Microcoder worker (coder/tui/mcworker.coax) it talks to, and the daemon (coder/tui/host.coax) that runs it.
#   apps/openagents-desktop/build.sh [--open]
#   CODER_BUILD=/tmp/coder apps/openagents-desktop/build.sh   builds into another folder
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="${CODER_BUILD:-$here/build}"   # CODER_BUILD=DIR builds a second copy and leaves a running OpenAgents's files alone
app="$work/OpenAgents.app"
res="$app/Contents/Resources"
mkdir -p "$work/c" "$app/Contents/MacOS" "$res/Components"
source "$here/sign.sh"
sign_prepare "$work"
"$coax" c "$here/app.coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$res/Components/components.metal" 2> "$work/c/components.metal.bind"
"$coax" metal "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs -o "$res/Components/composition.metal" 2> "$work/c/composition.metal.bind"
cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux")
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$here/carrier.c" -o "$work/c/carrier.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/gallery.c" -o "$work/c/gallery.o"
/usr/bin/clang -fobjc-arc "${cflags[@]}" -c "$repo/apps/shared/host/apple/ui-host.m" -o "$work/c/ui-host.o"
"$coax" c "$repo/apps/shared/auth/lab-sign.coax" --run-steps 250000000 --library --header "$work/c/lab-sign.h" > "$work/c/lab-sign.c"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -DS_hash=oa_lab_S_hash -DS_hex=oa_lab_S_hex -c "$work/c/lab-sign.c" -o "$work/c/lab-sign-all.o"
ld -r "$work/c/lab-sign-all.o" -exported_symbol _coaxial_lab_nip98 -exported_symbol '_coaxial_account_*' -o "$work/c/lab-sign.o"
"$coax" c "$here/desktop-core.coax" --library --header "$work/c/desktop-core.h" > "$work/c/desktop-core.generated.c"
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h" > "$work/c/entropy-policy.c"
cp "$here/desktop-core-unit.c" "$work/c/desktop-core-unit.c"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$work/c/desktop-core-unit.c" -o "$work/c/desktop-core-all.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$work/c/entropy-policy.c" -o "$work/c/entropy-policy.o"
for _dc_f in process-native.c store-file-native.c read-file-native.c socket-native.c entropy-native.c entropy-foreign.c; do
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$repo/coax/platform/$_dc_f" -o "$work/c/$_dc_f.o"
done
ld -r "$work/c/desktop-core-all.o" "$work/c/entropy-policy.o" "$work/c/process-native.c.o" "$work/c/store-file-native.c.o" "$work/c/read-file-native.c.o" "$work/c/socket-native.c.o" "$work/c/entropy-native.c.o" "$work/c/entropy-foreign.c.o" -exported_symbol '_oa_dc_*' -o "$work/c/desktop-core.o"
"$coax" c "$repo/pay/wallet.coax" --library --header "$work/c/pay-wallet.h" > "$work/c/pay-wallet.c"
"$coax" c "$repo/pay/wallet-backup.coax" --library --header "$work/c/pay-wallet-backup.h" > "$work/c/pay-wallet-backup.c"
"$coax" c "$repo/pay/wallet-rpc.coax" --library --header "$work/c/pay-wallet-rpc.h" > "$work/c/pay-wallet-rpc.c"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/pay-wallet.c" -o "$work/c/pay-wallet-all.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/pay-wallet-backup.c" -o "$work/c/pay-wallet-backup-all.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/pay-wallet-rpc.c" -o "$work/c/pay-wallet-rpc-all.o"
ld -r "$work/c/pay-wallet-all.o" -exported_symbol '_metaverse_wallet_*' -o "$work/c/pay-wallet.o"
ld -r "$work/c/pay-wallet-backup-all.o" -exported_symbol '_metaverse_backup_*' -exported_symbol '_metaverse_limit_*' -o "$work/c/pay-wallet-backup.o"
ld -r "$work/c/pay-wallet-rpc-all.o" -exported_symbol '_wallet_rpc_*' -o "$work/c/pay-wallet-rpc.o"
spark_sdk="$("$repo/apps/shared/host/apple/spark-sdk.sh")"
mkdir -p "$app/Contents/Frameworks"
rm -rf "$app/Contents/Frameworks/breez_sdk_sparkFFI.framework"
cp -R "$spark_sdk/breez_sdk_sparkFFI.framework" "$app/Contents/Frameworks/breez_sdk_sparkFFI.framework"
xcrun -sdk macosx swiftc -O -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 ${swift_defs[@]+"${swift_defs[@]}"} -import-objc-header "$here/Bridge.h" \
  -I "$spark_sdk" -F "$spark_sdk" \
  "$here/DesktopCore.swift" "$here/Worker.swift" "$here/Store.swift" "$here/DefaultsMigration.swift" "$here/Notifications.swift" "$here/Activation.swift" "$here/ScheduledTasks.swift" "$here/ThreadProof.swift" "$here/Window.swift" "$here/Window+Runs.swift" "$here/Window+FollowUps.swift" "$here/Window+Update.swift" "$here/Window+Foreman.swift" "$here/Window+Chats.swift" "$here/Window+Settings.swift" "$here/Window+Environments.swift" "$here/Window+McpSkills.swift" "$here/Window+Providers.swift" "$here/Window+TitleBar.swift" "$here/Window+Dialogs.swift" "$here/Window+Menus.swift" "$here/Window+Palette.swift" "$here/Window+Attach.swift" "$here/Window+Files.swift" "$here/Window+GitDiff.swift" "$here/Window+Simulator.swift" "$here/Window+Terminal.swift" "$here/SceneModel.swift" "$here/SceneView.swift" "$here/ScenePanelView.swift" "$here/GymDiscovery.swift" "$here/GymWorkbenchView.swift" "$here/GymWorkbenchView+Plan.swift" "$here/Window+Gym.swift" "$here/Window+ForemanAPI.swift" "$here/Window+ForemanChats.swift" "$here/Window+Worktrees.swift" "$here/Window+GitActions.swift" "$here/Window+Checkpoints.swift" "$here/Window+Wallet.swift" "$here/Window+WalletRPC.swift" "$here/Window+Usage.swift" "$here/Window+Voice.swift" "$here/Window+Rich.swift" "$here/Window+Scheduled.swift" "$here/Window+Preview.swift" "$here/AppDelegate.swift" \
  "$here/Foreman.swift" "$here/ForemanOperator.swift" "$here/Updater.swift" "$here/DaemonClient.swift" "$here/Worktree.swift" "$here/Tailnet.swift" "$repo/apps/shared/native/AppTailnet.swift" \
  "$here/Account.swift" "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$repo/apps/shared/native/KitPasskey.swift" \
  "$repo/apps/shared/host/apple/BreezKey.swift" "$repo/apps/shared/host/apple/BIP39.swift" "$repo/apps/shared/host/apple/WalletKeychain.swift" "$repo/apps/shared/host/apple/SparkWallet.swift" \
  "$work/c/carrier.o" "$work/c/gallery.o" "$work/c/ui-host.o" "$work/c/lab-sign.o" "$work/c/desktop-core.o" "$work/c/pay-wallet.o" "$work/c/pay-wallet-backup.o" "$work/c/pay-wallet-rpc.o" \
  "$spark_sdk/libBreezSdkSpark.a" "$spark_sdk/libBigNumber.a" \
  -framework breez_sdk_sparkFFI -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
  -framework AppKit -framework Metal -framework QuartzCore -framework CoreText -framework ImageIO -framework UniformTypeIdentifiers -framework CoreGraphics \
  -framework CryptoKit -framework Security -framework UserNotifications -framework AuthenticationServices -framework Speech -framework AVFoundation -framework IOKit -framework WebKit \
  -o "$app/Contents/MacOS/OpenAgents.new"
mv "$app/Contents/MacOS/OpenAgents.new" "$app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$app/Contents/Info.plist"
cp "$here/resources/OpenAgents.icns" "$res/OpenAgents.icns"
cp "$repo/metaverse/stage-resources/bip39-english.txt" "$res/bip39-english.txt"
# The Microcoder worker (coder/tui/mcworker.coax): tool.sh builds its image, named by the compiler's hash; the app runs the copy.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/mcworker.coax" < /dev/null > /dev/null
cp "$repo/coax/web/build/tools/mcworker-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/mcworker"
# The Claude Code worker (coder/tui/claudeworker.coax), for a sub-agent on a Claude model.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/claudeworker.coax" < /dev/null > /dev/null
cp "$repo/coax/web/build/tools/claudeworker-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/claudeworker"
# The Antigravity worker (coder/tui/agyworker.coax), for Antigravity ACP runs (GitHub #13802).
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/agyworker.coax" < /dev/null > /dev/null
cp "$repo/coax/web/build/tools/agyworker-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/agyworker"
# Its git guard (coder/tui/gitguard.coax), the PreToolUse hook claudeworker finds beside it (GitHub #13771).
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/gitguard.coax" < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/gitguard-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/gitguard"
# oasim, the iOS Simulator's command and its helper (coder/tui/oasim.coax, GitHub #13811): agents run it
# from their shell (the workers' PATH ends in Contents/Resources) and the pane starts it as the helper.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/oasim.coax" help < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/oasim-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/oasim"
# The unified daemon (coder/tui/host.coax, `coder host serve`): Coder starts it when none answers, and it
# owns the worker runs so they go on after Coder quits (GitHub #13694). An unknown verb only builds it.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/host.coax" build-only < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/host-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/coder-host"
# ple-run (ple-run.coax): a PLE run a chat starts and the pictures it shows inline (docs/desktop/steer-ple-from-chat.md).
COAX="$coax" "$repo/coax/web/tool.sh" "$here/ple-run.coax" < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/ple-run-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/ple-run"
# proof-export (coax/lib/agent/desktop/proof-export.coax): canonical ATIF-v1.8 trace exporter & verifier (#13895).
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/proof-export.coax" help < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/proof-export-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/proof-export"
cp "$here/foreman-ctl" "$res/foreman-ctl"
chmod +x "$res/foreman-ctl"
# Native Psionic Metal runtime for Inkling 2 Mini (psionic/lib/inkling/kernels.coax + psionic/platform/inkling-serve.c).
COAX="$coax" "$repo/psionic/platform/inkling-build.sh" "$work/psionic-inkling" > /dev/null
cp "$work/psionic-inkling/psionic-inkling-serve" "$res/psionic-inkling-serve"
chmod +x "$res/psionic-inkling-serve"
cp "$work/psionic-inkling/inkling-kernels.metal" "$res/inkling-kernels.metal"
rm -rf "$res/Components/fonts" "$res/Components/icons"
mkdir -p "$res/Components/fonts" "$res/Components/icons"
cp "$repo/components/fonts/"Geist* "$res/Components/fonts/"
cp "$repo/components/icons/"*.svg "$res/Components/icons/"
cp "$repo/components/NOTICE" "$res/Components/NOTICE"
mkdir -p "$res/fixtures"
[[ -f "$repo/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json" ]] && cp "$repo/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json" "$res/fixtures/ple-data-hall-sample.ple-scene.json"
sign_bundle "$app" "$work"
echo "built $app"
[[ "${1:-}" == --open ]] && open "$app"
exit 0
