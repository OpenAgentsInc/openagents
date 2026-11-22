#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, swiftc and codesign; holds no logic)
# Builds OpenAgents.app for this Mac: the page (app.coax / app-v1.coax) compiled to C on the shared
# component carrier (carrier.c, host.c) and Mac host (apps/shared/host/apple/ui-host.m), with the
# component shader emitted as Metal, the Microcoder worker (coder/tui/mcworker.coax) it talks to, and the daemon (coder/tui/host.coax) that runs it.
#   apps/openagents-desktop/build.sh [--full|--minimal] [--open]
#   OA_RELEASE_MINIMAL=1 apps/openagents-desktop/build.sh          builds the v1.0 minimal release app
#   CODER_BUILD=/tmp/coder apps/openagents-desktop/build.sh        builds into another folder
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
minimal="${OA_RELEASE_MINIMAL:-0}"
open_app=0
for arg in "$@"; do
  case "$arg" in
    --minimal) minimal=1 ;;
    --full) minimal=0 ;;
    --open) open_app=1 ;;
    *) echo "build.sh: unknown argument: $arg (usage: build.sh [--full|--minimal] [--open])" >&2; exit 2 ;;
  esac
done
work="${CODER_BUILD:-$here/build}"   # CODER_BUILD=DIR builds a second copy and leaves a running OpenAgents's files alone
app="$work/OpenAgents.app"
res="$app/Contents/Resources"
mkdir -p "$work/c" "$app/Contents/MacOS" "$res/Components"
if [[ "$minimal" == "1" ]]; then
  rm -rf "$app/Contents/Frameworks" \
    "$res/bip39-english.txt" "$res/claudeworker" "$res/agyworker" "$res/oasim" \
    "$res/gym-evals" "$res/ple-run" "$res/lean-run" "$res/tbench-run" \
    "$res/tbench-harbor" "$res/tbench-policies" "$res/lean-checker" "$res/fixtures" \
    "$res/proof-export" "$res/capability-eval" "$res/agents-catalog" "$res/tester-cli" \
    "$res/foreman-ctl"
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
"$coax" c "$app_coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$res/Components/components.metal" 2> "$work/c/components.metal.bind"
"$coax" metal "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs -o "$res/Components/composition.metal" 2> "$work/c/composition.metal.bind"
cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux")
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$here/carrier.c" -o "$work/c/carrier.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/gallery.c" -o "$work/c/gallery.o"
/usr/bin/clang -fobjc-arc "${cflags[@]}" -c "$repo/apps/shared/host/apple/ui-host.m" -o "$work/c/ui-host.o"
"$coax" c "$here/desktop-core.coax" --library --header "$work/c/desktop-core.h" > "$work/c/desktop-core.generated.c"
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h" > "$work/c/entropy-policy.c"
cp "$here/desktop-core-unit.c" "$work/c/desktop-core-unit.c"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$work/c/desktop-core-unit.c" -o "$work/c/desktop-core-all.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$work/c/entropy-policy.c" -o "$work/c/entropy-policy.o"
for _dc_f in process-native.c store-file-native.c read-file-native.c socket-native.c entropy-native.c entropy-foreign.c; do
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$repo/coax/platform" -c "$repo/coax/platform/$_dc_f" -o "$work/c/$_dc_f.o"
done
ld -r "$work/c/desktop-core-all.o" "$work/c/entropy-policy.o" "$work/c/process-native.c.o" "$work/c/store-file-native.c.o" "$work/c/read-file-native.c.o" "$work/c/socket-native.c.o" "$work/c/entropy-native.c.o" "$work/c/entropy-foreign.c.o" -exported_symbol '_oa_dc_*' -o "$work/c/desktop-core.o"

c_objs=("$work/c/carrier.o" "$work/c/gallery.o" "$work/c/ui-host.o" "$work/c/desktop-core.o")
link_flags=(
  -framework AppKit -framework Metal -framework QuartzCore -framework CoreText -framework ImageIO
  -framework UniformTypeIdentifiers -framework CoreGraphics -framework CryptoKit -framework Security
  -framework UserNotifications -framework IOKit
)
core_swift=(
  "$here/Activation.swift" "$here/AppDelegate.swift" "$here/DaemonClient.swift"
  "$here/DefaultsMigration.swift" "$here/DesktopCore.swift" "$here/LocalModels.swift"
  "$here/Notifications.swift" "$here/Store.swift" "$here/Updater.swift"
  "$here/Window.swift" "$here/Window+Attach.swift" "$here/Window+Chats.swift"
  "$here/Window+Dialogs.swift" "$here/Window+Files.swift" "$here/Window+FollowUps.swift"
  "$here/Window+Menus.swift" "$here/Window+Palette.swift" "$here/Window+Runs.swift"
  "$here/Window+Settings.swift" "$here/Window+TitleBar.swift" "$here/Window+Update.swift"
  "$here/Window+Worktrees.swift" "$here/Worker.swift" "$here/Worktree.swift"
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
  spark_sdk="$("$repo/apps/shared/host/apple/spark-sdk.sh")"
  mkdir -p "$app/Contents/Frameworks"
  rm -rf "$app/Contents/Frameworks/breez_sdk_sparkFFI.framework"
  cp -R "$spark_sdk/breez_sdk_sparkFFI.framework" "$app/Contents/Frameworks/breez_sdk_sparkFFI.framework"
  c_objs+=(
    "$work/c/lab-sign.o" "$work/c/pay-wallet.o" "$work/c/pay-wallet-backup.o"
    "$work/c/pay-wallet-rpc.o" "$work/c/rh-flythrough.o"
  )
  full_swift=(
    "$here/ScheduledTasks.swift" "$here/ThreadProof.swift" "$here/CapabilityStudio.swift" "$here/AgentsPage.swift"
    "$here/Window+Foreman.swift" "$here/Window+Customize.swift" "$here/Window+Environments.swift"
    "$here/Window+McpSkills.swift" "$here/Window+Providers.swift" "$here/Window+GitDiff.swift"
    "$here/Window+Simulator.swift" "$here/Window+Terminal.swift"
    "$here/SceneModel.swift" "$here/SceneView.swift" "$here/ScenePanelView.swift"
    "$here/GymDiscovery.swift" "$here/GymBenchmarkView.swift" "$here/GymTerminalBenchView.swift"
    "$here/RhFlythroughView.swift" "$here/GymWorkbenchView.swift" "$here/GymWorkbenchView+Plan.swift" "$here/Window+Gym.swift"
    "$here/Window+ForemanAPI.swift" "$here/Window+ForemanChats.swift"
    "$here/Window+GitActions.swift" "$here/Window+Checkpoints.swift"
    "$here/Window+Wallet.swift" "$here/Window+WalletRPC.swift"
    "$here/Window+Usage.swift" "$here/Window+Voice.swift" "$here/Window+Rich.swift"
    "$here/Window+Scheduled.swift" "$here/Window+Preview.swift" "$here/Window+Forge.swift"
    "$here/Foreman.swift" "$here/ForemanOperator.swift"
    "$here/Tailnet.swift" "$repo/apps/shared/native/AppTailnet.swift"
    "$here/Account.swift" "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$repo/apps/shared/native/KitPasskey.swift"
    "$repo/apps/shared/host/apple/BreezKey.swift" "$repo/apps/shared/host/apple/BIP39.swift" "$repo/apps/shared/host/apple/WalletKeychain.swift" "$repo/apps/shared/host/apple/SparkWallet.swift"
  )
  swift_files+=("${full_swift[@]}")
  link_flags+=(
    -I "$spark_sdk" -F "$spark_sdk"
    "$spark_sdk/libBreezSdkSpark.a" "$spark_sdk/libBigNumber.a"
    -framework breez_sdk_sparkFFI -Xlinker -rpath -Xlinker @executable_path/../Frameworks
    -framework AuthenticationServices -framework Speech -framework AVFoundation
    -framework CoreMedia -framework CoreVideo -framework WebKit
  )
fi

xcrun -sdk macosx swiftc -O -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 ${swift_defs[@]+"${swift_defs[@]}"} -import-objc-header "$here/Bridge.h" \
  "${swift_files[@]}" \
  "${c_objs[@]}" \
  "${link_flags[@]}" \
  -o "$app/Contents/MacOS/OpenAgents.new"
mv "$app/Contents/MacOS/OpenAgents.new" "$app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$app/Contents/Info.plist"
cp "$here/resources/OpenAgents.icns" "$res/OpenAgents.icns"
coax_hash="$(shasum -a 256 "$coax" | cut -d " " -f 1)"
# The Microcoder worker (coder/tui/mcworker.coax): tool.sh builds its image, named by the compiler's hash; the app runs the copy.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/mcworker.coax" < /dev/null > /dev/null
cp "$repo/coax/web/build/tools/mcworker-$coax_hash" "$res/mcworker"
# Its git guard (coder/tui/gitguard.coax), the PreToolUse hook workers find beside them (GitHub #13771).
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/gitguard.coax" < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/gitguard-$coax_hash" "$res/gitguard"
# The unified daemon (coder/tui/host.coax, `coder host serve`): Coder starts it when none answers, and it
# owns the worker runs so they go on after Coder quits (GitHub #13694). An unknown verb only builds it.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/host.coax" build-only < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/host-$coax_hash" "$res/coder-host"
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

if [[ "$minimal" != "1" ]]; then
  cp "$repo/metaverse/stage-resources/bip39-english.txt" "$res/bip39-english.txt"
  # The Claude Code worker (coder/tui/claudeworker.coax), for a sub-agent on a Claude model.
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/claudeworker.coax" < /dev/null > /dev/null
  cp "$repo/coax/web/build/tools/claudeworker-$coax_hash" "$res/claudeworker"
  # The Antigravity worker (coder/tui/agyworker.coax), for Antigravity ACP runs (GitHub #13802).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/agyworker.coax" < /dev/null > /dev/null
  cp "$repo/coax/web/build/tools/agyworker-$coax_hash" "$res/agyworker"
  # oasim, the iOS Simulator's command and its helper (coder/tui/oasim.coax, GitHub #13811).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/oasim.coax" help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/oasim-$coax_hash" "$res/oasim"
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
  # ple-run (ple-run.coax): a PLE run a chat starts and the pictures it shows inline (docs/desktop/steer-ple-from-chat.md).
  COAX="$coax" "$repo/coax/web/tool.sh" "$here/ple-run.coax" < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/ple-run-$coax_hash" "$res/ple-run"
  # lean-run (coder/tui/lean-run.coax): Lean 4 verifier, Gym steering & RH Workbench CLI (#13944, #13945).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/lean-run.coax" help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/lean-run-$coax_hash" "$res/lean-run"
  # tbench-run (coder/tui/tbench-run.coax): Terminal-Bench Coax Runner & Microcoder Hillclimber (#13951, #13952, #13953).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/tbench-run.coax" help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/tbench-run-$coax_hash" "$res/tbench-run"
  if [[ -d "$repo/evals/terminal-bench/harbor_adapter" ]]; then
    rm -rf "$res/tbench-harbor" "$res/tbench-policies"
    mkdir -p "$res/tbench-harbor" "$res/tbench-policies"
    cp -R "$repo/evals/terminal-bench/harbor_adapter" "$res/tbench-harbor/"
    cp -R "$repo/evals/terminal-bench/policies/"*.json "$res/tbench-policies/"
  fi
  mkdir -p "$res/lean-checker" "$res/fixtures/lean-samples"
  [[ -f "$repo/resources/lean-checker/OpenAgentsVerify.lean" ]] && cp "$repo/resources/lean-checker/OpenAgentsVerify.lean" "$res/lean-checker/OpenAgentsVerify.lean"
  [[ -d "$repo/evals/openagents-desktop/lean/samples/add_zero_right" ]] && cp -R "$repo/evals/openagents-desktop/lean/samples/add_zero_right" "$res/fixtures/lean-samples/"
  # proof-export (coax/lib/agent/desktop/proof-export.coax): canonical ATIF-v1.8 trace exporter & verifier (#13895).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/proof-export.coax" help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/proof-export-$coax_hash" "$res/proof-export"
  # capability-eval (coax/lib/agent/desktop/capability-eval.coax): two-arm local capability eval & #authority gate (#13896).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/capability-eval.coax" help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/capability-eval-$coax_hash" "$res/capability-eval"
  # agents-catalog & tester-cli (coax/lib/agent/desktop/agents-catalog.coax, apps/shared/tester/cli.coax; #13894).
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coax/lib/agent/desktop/agents-catalog.coax" help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/agents-catalog-$coax_hash" "$res/agents-catalog"
  COAX="$coax" "$repo/coax/web/tool.sh" "$repo/apps/shared/tester/cli.coax" --help < /dev/null > /dev/null 2>&1 || true
  cp "$repo/coax/web/build/tools/cli-$coax_hash" "$res/tester-cli"
  cp "$here/foreman-ctl" "$res/foreman-ctl"
  chmod +x "$res/foreman-ctl"
  mkdir -p "$res/fixtures"
  [[ -f "$repo/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json" ]] && cp "$repo/metaverse-engine/fixtures/ple-data-hall-sample.ple-scene.json" "$res/fixtures/ple-data-hall-sample.ple-scene.json"
fi

sign_bundle "$app" "$work"
echo "built $app"
[[ "$open_app" == "1" ]] && open "$app"
exit 0
