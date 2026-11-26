#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, swiftc and codesign; holds no logic)
# Builds OpenAgents.app for this Mac: the page (app.coax / app-v1.coax) compiled to C on the shared
# component carrier (carrier.c, host.c) and Mac host (apps/shared/host/apple/ui-host.m), with the
# component shader emitted as Metal, the Inky local worker (coder/tui/inky.coax) it talks to, and the daemon (coder/tui/host.coax) that runs it.
#   apps/openagents-desktop/build.sh [--minimal|--full] [--fast] [--open]
#   OA_RELEASE_MINIMAL=0 apps/openagents-desktop/build.sh          builds the full developer app
#   CODER_BUILD=/tmp/coder apps/openagents-desktop/build.sh        builds into another folder
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
minimal="${OA_RELEASE_MINIMAL:-1}"
swift_opt="-O"
open_app=0
for arg in "$@"; do
  case "$arg" in
    --minimal) minimal=1 ;;
    --full) minimal=0 ;;
    --fast) swift_opt="-Onone" ;;
    --open) open_app=1 ;;
    *) echo "build.sh: unknown argument: $arg (usage: build.sh [--minimal|--full] [--fast] [--open])" >&2; exit 2 ;;
  esac
done
work="${CODER_BUILD:-$here/build}"   # CODER_BUILD=DIR builds a second copy and leaves a running OpenAgents's files alone
app="$work/OpenAgents.app"
res="$app/Contents/Resources"
mkdir -p "$work/c" "$app/Contents/MacOS" "$res/Components"
update_if_changed() {
  if [[ -f "$2" ]] && cmp -s "$1" "$2"; then rm -f "$1"; else mv -f "$1" "$2"; fi
}
obj_stale() {
  local obj="$1"; shift
  [[ -f "$obj" ]] || return 0
  local dep
  for dep in "$@"; do [[ -e "$dep" && "$dep" -nt "$obj" ]] && return 0; done
  return 1
}
if [[ "$minimal" == "1" ]]; then
  rm -rf "$app/Contents/Frameworks" \
    "$res/bip39-english.txt" "$res/mcworker" "$res/claudeworker" "$res/oasim" \
    "$res/gym-evals" "$res/ple-run" "$res/lean-run" "$res/tbench-run" "$res/gbench-run" \
    "$res/tbench-harbor" "$res/tbench-policies" "$res/lean-checker" "$res/fixtures" \
    "$res/proof-export" "$res/capability-eval" "$res/agents-catalog" "$res/tester-cli" \
    "$res/foreman-ctl" "$res/gamebuilder" "$res/Metaverse" \
    "$res/Components/metaverse-model.metal" "$res/Components/metaverse-ple.metal" "$res/Components/gb-instances.metal" "$res/Components/splat-sort.metal"
fi
source "$here/sign.sh"
sign_prepare "$work"
if [[ "$minimal" == "1" ]]; then
  swift_defs+=(-D OA_MINIMAL -Xcc -DOA_MINIMAL)
else
  swift_defs+=(-D OA_FULL -D OA_WALLET -Xcc -DOA_FULL -Xcc -DOA_WALLET)
fi
app_coax="$here/app.coax"
if [[ "$minimal" == "1" && -f "$here/app-v1.coax" ]]; then
  app_coax="$here/app-v1.coax"
fi
# Build independent Coax workers & Psionic Metal runtime in parallel with page C/Swift compiles.
# COAX_TOOL_BUILD_ONLY=1 compiles without executing workers (prevents 0-byte .ajr leaks).
worker_pids=()
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/oa-worker.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/inky.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/agyworker.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/gitguard.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/host.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX="$coax" "$repo/psionic/platform/inkling-build.sh" "$work/psionic-inkling" > /dev/null & worker_pids+=($!)

gen_pids=()
"$coax" c "$app_coax" --library --header "$work/c/gallery.h.tmp" > "$work/c/gallery.c.tmp" & gen_pids+=($!)
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$res/Components/components.metal" 2> "$work/c/components.metal.bind" & gen_pids+=($!)
"$coax" metal "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs -o "$res/Components/composition.metal" 2> "$work/c/composition.metal.bind" & gen_pids+=($!)
"$coax" c "$here/desktop-core.coax" --library --header "$work/c/desktop-core.h.tmp" > "$work/c/desktop-core.generated.c.tmp" & gen_pids+=($!)
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h.tmp" > "$work/c/entropy-policy.c.tmp" & gen_pids+=($!)
for p in "${gen_pids[@]}"; do wait "$p"; done
update_if_changed "$work/c/gallery.h.tmp" "$work/c/gallery.h"
update_if_changed "$work/c/gallery.c.tmp" "$work/c/gallery.c"
update_if_changed "$work/c/desktop-core.h.tmp" "$work/c/desktop-core.h"
update_if_changed "$work/c/desktop-core.generated.c.tmp" "$work/c/desktop-core.generated.c"
update_if_changed "$work/c/entropy-policy.h.tmp" "$work/c/entropy-policy.h"
update_if_changed "$work/c/entropy-policy.c.tmp" "$work/c/entropy-policy.c"
if [[ ! -f "$work/c/desktop-core-unit.c" ]] || ! cmp -s "$here/desktop-core-unit.c" "$work/c/desktop-core-unit.c"; then
  cp "$here/desktop-core-unit.c" "$work/c/desktop-core-unit.c"
fi

cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux")
cc_pids=()
if obj_stale "$work/c/carrier.o" "$here/carrier.c" "$here/host.c" "$here/host.h" "$repo/components/gallery/linux/main.c" "$work/c/gallery.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$here/carrier.c" -o "$work/c/carrier.o" & cc_pids+=($!)
fi
if obj_stale "$work/c/gallery.o" "$work/c/gallery.c" "$work/c/gallery.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/gallery.c" -o "$work/c/gallery.o" & cc_pids+=($!)
fi
if obj_stale "$work/c/ui-host.o" "$repo/apps/shared/host/apple/ui-host.m" "$repo/apps/shared/host/linux/ui-host.h" "$repo/apps/shared/host/apple/ui-host-apple.h"; then
  /usr/bin/clang -fobjc-arc "${cflags[@]}" -c "$repo/apps/shared/host/apple/ui-host.m" -o "$work/c/ui-host.o" & cc_pids+=($!)
fi
if obj_stale "$work/c/desktop-core-all.o" "$work/c/desktop-core-unit.c" "$work/c/desktop-core.generated.c" "$work/c/desktop-core.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$work/c/desktop-core-unit.c" -o "$work/c/desktop-core-all.o" & cc_pids+=($!)
fi
if obj_stale "$work/c/entropy-policy.o" "$work/c/entropy-policy.c" "$work/c/entropy-policy.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$work/c/entropy-policy.c" -o "$work/c/entropy-policy.o" & cc_pids+=($!)
fi
dc_platform_hdrs=("$repo/coax/platform"/*.h)
for _dc_f in process-native.c store-file-native.c read-file-native.c socket-native.c entropy-native.c entropy-foreign.c; do
  if obj_stale "$work/c/$_dc_f.o" "$repo/coax/platform/$_dc_f" "${dc_platform_hdrs[@]}"; then
    /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$repo/coax/platform/$_dc_f" -o "$work/c/$_dc_f.o" & cc_pids+=($!)
  fi
done
for p in ${cc_pids[@]+"${cc_pids[@]}"}; do wait "$p"; done
dc_inputs=("$work/c/desktop-core-all.o" "$work/c/entropy-policy.o" "$work/c/process-native.c.o" "$work/c/store-file-native.c.o" "$work/c/read-file-native.c.o" "$work/c/socket-native.c.o" "$work/c/entropy-native.c.o" "$work/c/entropy-foreign.c.o")
if obj_stale "$work/c/desktop-core.o" "${dc_inputs[@]}"; then
  ld -r "${dc_inputs[@]}" -exported_symbol '_oa_dc_*' -o "$work/c/desktop-core.o"
fi

c_objs=("$work/c/carrier.o" "$work/c/gallery.o" "$work/c/ui-host.o" "$work/c/desktop-core.o")
link_flags=(
  -framework AppKit -framework Metal -framework QuartzCore -framework CoreText -framework ImageIO
  -framework UniformTypeIdentifiers -framework CoreGraphics -framework CryptoKit -framework Security
  -framework UserNotifications -framework IOKit -framework Speech -framework AVFoundation
)
core_swift=(
  "$here/Activation.swift" "$here/AntigravityProvider.swift" "$here/AppDelegate.swift" "$here/DaemonClient.swift"
  "$here/DefaultsMigration.swift" "$here/DesktopCore.swift" "$here/LocalModels.swift"
  "$here/Notifications.swift" "$here/Store.swift" "$here/Tailnet.swift" "$repo/apps/shared/native/AppTailnet.swift" "$here/Updater.swift"
  "$here/Window.swift" "$here/Window+Attach.swift" "$here/Window+Chats.swift"
  "$here/Window+Dialogs.swift" "$here/Window+Files.swift" "$here/Window+FollowUps.swift"
  "$here/Window+Menus.swift" "$here/Window+Palette.swift" "$here/Window+Runs.swift"
  "$here/Window+Settings.swift" "$here/Window+TitleBar.swift" "$here/Window+Update.swift"
  "$here/Window+Worktrees.swift" "$here/Worker.swift" "$here/Worktree.swift" "$here/KeepAwake.swift"
  "$here/Window+GitDiff.swift" "$here/Window+GitActions.swift" "$here/Window+Checkpoints.swift"
  "$here/AgentSettings.swift" "$here/TitleGenerator.swift" "$here/Window+Usage.swift" "$here/Window+Voice.swift"
)
swift_files=("${core_swift[@]}")

if [[ "$minimal" != "1" ]]; then
  "$coax" c "$repo/apps/shared/auth/lab-sign.coax" --run-steps 250000000 --library --header "$work/c/lab-sign.h" > "$work/c/lab-sign.c"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -DS_hash=oa_lab_S_hash -DS_hex=oa_lab_S_hex -c "$work/c/lab-sign.c" -o "$work/c/lab-sign-all.o"
  ld -r "$work/c/lab-sign-all.o" -exported_symbol _coaxial_lab_nip98 -exported_symbol '_coaxial_account_*' -o "$work/c/lab-sign.o"
  "$coax" c "$repo/pay/wallet.coax" --library --header "$work/c/pay-wallet.h" > "$work/c/pay-wallet.c"
  "$coax" c "$repo/pay/wallet-backup.coax" --library --header "$work/c/pay-wallet-backup.h" > "$work/c/pay-wallet-backup.c"
  "$coax" c "$repo/pay/wallet-rpc.coax" --library --header "$work/c/pay-wallet-rpc.h" > "$work/c/pay-wallet-rpc.c"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/pay-wallet.c" -o "$work/c/pay-wallet-all.o"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/pay-wallet-backup.c" -o "$work/c/pay-wallet-backup-all.o"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/pay-wallet-rpc.c" -o "$work/c/pay-wallet-rpc-all.o"
  ld -r "$work/c/pay-wallet-all.o" -exported_symbol '_metaverse_wallet_*' -o "$work/c/pay-wallet.o"
  ld -r "$work/c/pay-wallet-backup-all.o" -exported_symbol '_metaverse_backup_*' -exported_symbol '_metaverse_limit_*' -o "$work/c/pay-wallet-backup.o"
  ld -r "$work/c/pay-wallet-rpc-all.o" -exported_symbol '_wallet_rpc_*' -o "$work/c/pay-wallet-rpc.o"
  "$coax" c "$repo/metaverse/rh-flythrough.coax" --library --header "$work/c/rh-flythrough.h" > "$work/c/rh-flythrough.c"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/rh-flythrough.c" -o "$work/c/rh-flythrough-all.o"
  ld -r "$work/c/rh-flythrough-all.o" -exported_symbol '_oa_dc_rh3d_*' -o "$work/c/rh-flythrough.o"
  "$coax" metal "$repo/metaverse-engine/gpu/model-view-shader.coax" --vertex mv_vs --fragment mv_fs -o "$res/Components/metaverse-model.metal" 2> "$work/c/metaverse-model.metal.bind"
  "$coax" metal "$repo/metaverse-engine/gpu/ple-shader.coax" --vertex ple_vs --fragment ple_fs -o "$res/Components/metaverse-ple.metal" 2> "$work/c/metaverse-ple.metal.bind"
  "$coax" metal "$repo/gamebuilder/core/gpu/gb-instances.coax" --vertex gb_vs --fragment gb_fs -o "$res/Components/gb-instances.metal" 2> "$work/c/gb-instances.metal.bind"
  "$coax" metal-compute "$repo/metaverse-engine/gpu/splat-sort.coax" > "$res/Components/splat-sort.metal"
  "$coax" c "$repo/metaverse-engine/content/desktop-viewer-bridge.coax" --library --header "$work/c/metaverse-viewer.h" > "$work/c/metaverse-viewer.c"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/metaverse-viewer.c" -o "$work/c/metaverse-viewer-all.o"
  ld -r "$work/c/metaverse-viewer-all.o" -exported_symbol '_oa_mv_*' -o "$work/c/metaverse-viewer.o"
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$here" -c "$here/mv_host.c" -o "$work/c/mv_host.o"
  if obj_stale "$work/c/gamebuilder-core.o" "$repo/gamebuilder/core/desktop-bridge.coax" "$repo/gamebuilder/core/controls.coax" "$repo/gamebuilder/core/editor.coax" "$repo/gamebuilder/core/commands.coax" "$repo/gamebuilder/core/world.coax"; then
    "$coax" c "$repo/gamebuilder/core/desktop-bridge.coax" --run-steps 250000000 --library --header "$work/c/gamebuilder-core.h" > "$work/c/gamebuilder-core.c"
    /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/gamebuilder-core.c" -o "$work/c/gamebuilder-core-all.o"
    ld -r "$work/c/gamebuilder-core-all.o" "$work/c/socket-native.c.o" "$work/c/process-native.c.o" -exported_symbol '_oa_gb_*' -o "$work/c/gamebuilder-core.o"
  fi
  if obj_stale "$work/c/gb_host.o" "$here/gb_host.c" "$here/gb_host.h" "$work/c/gamebuilder-core.h"; then
    /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$here" -c "$here/gb_host.c" -o "$work/c/gb_host.o"
  fi
  spark_sdk="$("$repo/apps/shared/host/apple/spark-sdk.sh")"
  mkdir -p "$app/Contents/Frameworks"
  rm -rf "$app/Contents/Frameworks/breez_sdk_sparkFFI.framework"
  cp -R "$spark_sdk/breez_sdk_sparkFFI.framework" "$app/Contents/Frameworks/breez_sdk_sparkFFI.framework"
  c_objs+=(
    "$work/c/lab-sign.o" "$work/c/pay-wallet.o" "$work/c/pay-wallet-backup.o"
    "$work/c/pay-wallet-rpc.o" "$work/c/rh-flythrough.o"
    "$work/c/metaverse-viewer.o" "$work/c/mv_host.o"
    "$work/c/gamebuilder-core.o" "$work/c/gb_host.o"
  )
  full_swift=(
    "$here/ScheduledTasks.swift" "$here/ThreadProof.swift" "$here/CapabilityStudio.swift" "$here/AgentsPage.swift"
    "$here/Window+Foreman.swift" "$here/Window+Customize.swift" "$here/Window+Environments.swift"
    "$here/Window+McpSkills.swift" "$here/Window+Providers.swift"
    "$here/Window+Simulator.swift" "$here/Window+Terminal.swift"
    "$here/GameBuilderSession.swift" "$here/GtaWorkbenchSession.swift" "$here/MetaverseViewportView.swift" "$here/Window+Metaverse.swift"
    "$here/SceneModel.swift" "$here/SceneView.swift" "$here/ScenePanelView.swift"
    "$here/GymDiscovery.swift" "$here/GymBenchmarkView.swift" "$here/GymTerminalBenchView.swift" "$here/GymGBenchView.swift"
    "$here/RhFlythroughView.swift" "$here/GymWorkbenchView.swift" "$here/GymWorkbenchView+Plan.swift" "$here/Window+Gym.swift"
    "$here/Window+ForemanAPI.swift" "$here/Window+ForemanChats.swift"
    "$here/Window+Wallet.swift" "$here/Window+WalletRPC.swift"
    "$here/Window+Rich.swift"
    "$here/Window+Scheduled.swift" "$here/Window+Preview.swift" "$here/Window+Forge.swift"
    "$here/Foreman.swift" "$here/ForemanOperator.swift"
    "$here/Account.swift" "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$repo/apps/shared/native/KitPasskey.swift"
    "$repo/apps/shared/host/apple/BreezKey.swift" "$repo/apps/shared/host/apple/BIP39.swift" "$repo/apps/shared/host/apple/WalletKeychain.swift" "$repo/apps/shared/host/apple/SparkWallet.swift"
  )
  swift_files+=("${full_swift[@]}")
  link_flags+=(
    -I "$spark_sdk" -F "$spark_sdk"
    "$spark_sdk/libBreezSdkSpark.a" "$spark_sdk/libBigNumber.a"
    -framework breez_sdk_sparkFFI -Xlinker -rpath -Xlinker @executable_path/../Frameworks
    -framework AuthenticationServices -framework Speech -framework AVFoundation
    -framework CoreMedia -framework CoreVideo -framework WebKit -framework ModelIO
  )
fi

xcrun -sdk macosx swiftc "$swift_opt" -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 ${swift_defs[@]+"${swift_defs[@]}"} -import-objc-header "$here/Bridge.h" \
  "${swift_files[@]}" \
  "${c_objs[@]}" \
  "${link_flags[@]}" \
  -o "$app/Contents/MacOS/OpenAgents.new"
mv "$app/Contents/MacOS/OpenAgents.new" "$app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$app/Contents/Info.plist"
build_profile="full"; [[ "$minimal" == "1" ]] && build_profile="minimal"
/usr/libexec/PlistBuddy -c "Delete :OAProfile" "$app/Contents/Info.plist" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :OAProfile string $build_profile" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Delete :OABuildDate" "$app/Contents/Info.plist" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :OABuildDate string $(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$app/Contents/Info.plist"
if build_sha="$(git -C "$repo" rev-parse HEAD 2>/dev/null)"; then
  build_rev="$(git -C "$repo" rev-list --count HEAD 2>/dev/null || echo 0)"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $((28830 + build_rev))" "$app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Delete :OACommit" "$app/Contents/Info.plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OACommit string $build_sha" "$app/Contents/Info.plist"
fi
cp "$here/resources/OpenAgents.icns" "$res/OpenAgents.icns"
coax_hash="$(shasum -a 256 "$coax" | cut -d " " -f 1)"
for p in "${worker_pids[@]}"; do wait "$p"; done
cp "$repo/coax/web/build/tools/oa-worker-$coax_hash" "$res/oa-worker"
cp "$repo/coax/web/build/tools/inky-$coax_hash" "$res/inky"
cp "$repo/coax/web/build/tools/agyworker-$coax_hash" "$res/agyworker"
cp "$repo/coax/web/build/tools/gitguard-$coax_hash" "$res/gitguard"
cp "$repo/coax/web/build/tools/host-$coax_hash" "$res/coder-host"
cp "$work/psionic-inkling/psionic-inkling-serve" "$res/psionic-inkling-serve"
chmod +x "$res/psionic-inkling-serve"
cp "$work/psionic-inkling/inkling-kernels.metal" "$res/inkling-kernels.metal"
rm -rf "$res/Components/fonts" "$res/Components/icons"
mkdir -p "$res/Components/fonts" "$res/Components/icons"
cp "$repo/components/fonts/"Geist* "$res/Components/fonts/"
cp "$repo/components/icons/"*.svg "$res/Components/icons/"
cp "$repo/components/NOTICE" "$res/Components/NOTICE"

if [[ "$minimal" != "1" ]]; then
  cp "$repo/metaverse/stage-resources/bip39-english.txt" "$res/bip39-english.txt"
  full_pids=()
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/mcworker.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/claudeworker.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/oasim.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$here/ple-run.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/lean-run.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/tbench-run.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/gbench-run.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/proof-export.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/capability-eval.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/agents-catalog.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/apps/shared/tester/cli.coax" < /dev/null > /dev/null & full_pids+=($!)
  COAX="$coax" "$repo/gamebuilder/cli/build.sh" < /dev/null > /dev/null & full_pids+=($!)
  # Bundle Tier 2 + Tier 4 coding eval specs and fixtures for Gym Benchmarks so it runs from any workspace.
  if [[ -d "$repo/evals/openagents-desktop/tier2/tasks" && -d "$repo/evals/openagents-desktop/tier4/tasks" ]]; then
    rm -rf "$res/gym-evals"
    mkdir -p "$res/gym-evals/tier2" "$res/gym-evals/tier4"
    cp -R "$repo/evals/openagents-desktop/tier2/tasks" "$repo/evals/openagents-desktop/tier2/fixtures" "$res/gym-evals/tier2/"
    cp -R "$repo/evals/openagents-desktop/tier4/tasks" "$repo/evals/openagents-desktop/tier4/fixtures" "$res/gym-evals/tier4/"
    if [[ -d "$repo/evals/openagents-desktop/lean/lean-core-smoke/tasks" ]]; then
      mkdir -p "$res/gym-evals/lean/lean-core-smoke" "$res/gym-evals/lean/lean-algebra-logic"
      cp -R "$repo/evals/openagents-desktop/lean/lean-core-smoke/tasks" "$repo/evals/openagents-desktop/lean/lean-core-smoke/fixtures" "$res/gym-evals/lean/lean-core-smoke/"
      cp -R "$repo/evals/openagents-desktop/lean/lean-algebra-logic/tasks" "$repo/evals/openagents-desktop/lean/lean-algebra-logic/fixtures" "$res/gym-evals/lean/lean-algebra-logic/"
    fi
  fi
  if [[ -d "$repo/evals/terminal-bench/harbor_adapter" ]]; then
    rm -rf "$res/tbench-harbor" "$res/tbench-policies"
    mkdir -p "$res/tbench-harbor" "$res/tbench-policies"
    cp -R "$repo/evals/terminal-bench/harbor_adapter" "$res/tbench-harbor/"
    cp -R "$repo/evals/terminal-bench/policies/"*.json "$res/tbench-policies/"
  fi
  mkdir -p "$res/lean-checker" "$res/fixtures/lean-samples"
  [[ -f "$repo/resources/lean-checker/OpenAgentsVerify.lean" ]] && cp "$repo/resources/lean-checker/OpenAgentsVerify.lean" "$res/lean-checker/OpenAgentsVerify.lean"
  [[ -d "$repo/evals/openagents-desktop/lean/samples/add_zero_right" ]] && cp -R "$repo/evals/openagents-desktop/lean/samples/add_zero_right" "$res/fixtures/lean-samples/"
  for p in "${full_pids[@]}"; do wait "$p"; done
  cp "$repo/coax/web/build/tools/mcworker-$coax_hash" "$res/mcworker"
  cp "$repo/coax/web/build/tools/claudeworker-$coax_hash" "$res/claudeworker"
  cp "$repo/coax/web/build/tools/oasim-$coax_hash" "$res/oasim"
  cp "$repo/coax/web/build/tools/ple-run-$coax_hash" "$res/ple-run"
  cp "$repo/coax/web/build/tools/lean-run-$coax_hash" "$res/lean-run"
  cp "$repo/coax/web/build/tools/tbench-run-$coax_hash" "$res/tbench-run"
  cp "$repo/coax/web/build/tools/gbench-run-$coax_hash" "$res/gbench-run"
  cp "$repo/coax/web/build/tools/proof-export-$coax_hash" "$res/proof-export"
  cp "$repo/coax/web/build/tools/capability-eval-$coax_hash" "$res/capability-eval"
  cp "$repo/coax/web/build/tools/agents-catalog-$coax_hash" "$res/agents-catalog"
  cp "$repo/coax/web/build/tools/cli-$coax_hash" "$res/tester-cli"
  if [[ -x "$repo/gamebuilder/build/cli/gamebuilder" ]]; then
    cp "$repo/gamebuilder/build/cli/gamebuilder" "$res/gamebuilder"
    chmod +x "$res/gamebuilder"
  fi
  cp "$here/foreman-ctl" "$res/foreman-ctl"
  chmod +x "$res/foreman-ctl"
  mkdir -p "$res/fixtures"
  [[ -f "$repo/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json" ]] && cp "$repo/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json" "$res/fixtures/ple-data-hall-sample.ple-scene.json"
fi

sign_bundle "$app" "$work"
echo "built $app"
[[ "$open_app" == "1" ]] && open "$app"
exit 0
