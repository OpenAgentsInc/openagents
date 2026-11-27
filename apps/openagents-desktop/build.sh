#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, swiftc and codesign; holds no logic)
# Builds OpenAgents.app (v1.0.0) for this Mac: the page (app.coax) compiled to C on the shared
# component carrier (carrier.c, host.c) and Mac host (apps/shared/host/apple/ui-host.m), with the
# component shader emitted as Metal, the v1 workers it talks to, and the daemon (openagents/daemon/host.coax) that runs it.
#   apps/openagents-desktop/build.sh [--fast] [--open]
#   CODER_BUILD=/tmp/coder apps/openagents-desktop/build.sh        builds into another folder
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
swift_opt="-O"
open_app=0
for arg in "$@"; do
  case "$arg" in
    --minimal|--full) ;;  # v1 build profile (#14137); kept as compatibility aliases
    --fast) swift_opt="-Onone" ;;
    --open) open_app=1 ;;
    *) echo "build.sh: unknown argument: $arg (usage: build.sh [--fast] [--open])" >&2; exit 2 ;;
  esac
done
work="${CODER_BUILD:-$here/build}"   # CODER_BUILD=DIR builds a second copy and leaves a running OpenAgents's files alone
app="$work/OpenAgents.app"
stage_app="$work/OpenAgents.app.staging"
res="$stage_app/Contents/Resources"
# Always assemble into a fresh .app bundle so removed/renamed workers, shaders, or frameworks
# from older commits (e.g. breez_sdk_sparkFFI, agyworker, gamebuilder, ple-run) can never leak into a new build.
rm -rf "$stage_app"
mkdir -p "$work/c" "$stage_app/Contents/MacOS" "$res/Components"
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
build_sig="v1|$swift_opt|$(/usr/bin/clang --version | head -n 1)"
if [[ "${OA_CLEAN_BUILD:-0}" == "1" || "$(cat "$work/c/.build-sig" 2>/dev/null || true)" != "$build_sig" ]]; then
  rm -rf "$work/c"
  mkdir -p "$work/c"
  printf '%s\n' "$build_sig" > "$work/c/.build-sig"
fi
source "$here/sign.sh"
sign_prepare "$work"
app_coax="$here/app.coax"

# Build independent Coax v1 workers & Psionic Metal runtime in parallel with page C/Swift compiles.
# COAX_TOOL_BUILD_ONLY=1 compiles without executing workers (prevents 0-byte .ajr leaks).
worker_pids=()
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/openagents/runtime/oa-worker.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/openagents/workers/gitguard.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/openagents/workers/claudeworker.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/openagents/daemon/host.coax" < /dev/null > /dev/null & worker_pids+=($!)
COAX="$coax" "$repo/psionic/platform/inkling-build.sh" "$work/psionic-inkling" > /dev/null & worker_pids+=($!)

gen_pids=()
"$coax" c "$app_coax" --library --header "$work/c/gallery.h.tmp" > "$work/c/gallery.c.tmp" & gen_pids+=($!)
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$res/Components/components.metal" 2> "$work/c/components.metal.bind" & gen_pids+=($!)
"$coax" metal "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs -o "$res/Components/composition.metal" 2> "$work/c/composition.metal.bind" & gen_pids+=($!)
"$coax" c "$here/desktop-core.coax" --library --header "$work/c/desktop-core.h.tmp" > "$work/c/desktop-core.generated.c.tmp" & gen_pids+=($!)
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h.tmp" > "$work/c/entropy-policy.c.tmp" & gen_pids+=($!)
"$coax" c "$repo/coax/web/api/account-chat-cloud-wire.coax" --run-steps 250000000 --library --header "$work/c/cloud-chat-wire.h.tmp" > "$work/c/cloud-chat-wire.c.tmp" & gen_pids+=($!)
for p in "${gen_pids[@]}"; do wait "$p"; done
update_if_changed "$work/c/gallery.h.tmp" "$work/c/gallery.h"
update_if_changed "$work/c/gallery.c.tmp" "$work/c/gallery.c"
update_if_changed "$work/c/desktop-core.h.tmp" "$work/c/desktop-core.h"
update_if_changed "$work/c/desktop-core.generated.c.tmp" "$work/c/desktop-core.generated.c"
update_if_changed "$work/c/entropy-policy.h.tmp" "$work/c/entropy-policy.h"
update_if_changed "$work/c/entropy-policy.c.tmp" "$work/c/entropy-policy.c"
update_if_changed "$work/c/cloud-chat-wire.h.tmp" "$work/c/cloud-chat-wire.h"
update_if_changed "$work/c/cloud-chat-wire.c.tmp" "$work/c/cloud-chat-wire.c"
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
if obj_stale "$work/c/cloud-chat-wire-all.o" "$work/c/cloud-chat-wire.c" "$work/c/cloud-chat-wire.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/cloud-chat-wire.c" -o "$work/c/cloud-chat-wire-all.o" & cc_pids+=($!)
fi
if obj_stale "$work/c/br_host.o" "$here/br_host.c" "$here/br_host.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$here" -c "$here/br_host.c" -o "$work/c/br_host.o" & cc_pids+=($!)
fi
if obj_stale "$work/c/br_attach.o" "$here/br_attach.c" "$here/br_attach.h"; then
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -I"$here" -c "$here/br_attach.c" -o "$work/c/br_attach.o" & cc_pids+=($!)
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
if obj_stale "$work/c/cloud-chat-wire.o" "$work/c/cloud-chat-wire-all.o"; then
  ld -r "$work/c/cloud-chat-wire-all.o" -exported_symbol '_cc_*' -o "$work/c/cloud-chat-wire.o"
fi

c_objs=(
  "$work/c/carrier.o" "$work/c/gallery.o" "$work/c/ui-host.o" "$work/c/desktop-core.o"
  "$work/c/cloud-chat-wire.o"
  "$work/c/br_host.o" "$work/c/br_attach.o"
)
link_flags=(
  -framework AppKit -framework Metal -framework QuartzCore -framework CoreText -framework ImageIO
  -framework UniformTypeIdentifiers -framework CoreGraphics -framework CryptoKit -framework Security
  -framework AuthenticationServices -framework UserNotifications -framework IOKit -framework WebKit
)
swift_files=(
  "$here/Activation.swift" "$here/AppDelegate.swift" "$here/CloudChatSync.swift" "$here/DaemonClient.swift"
  "$here/DefaultsMigration.swift" "$here/DesktopCore.swift" "$here/LocalModels.swift"
  "$here/Notifications.swift" "$here/Store.swift" "$here/Updater.swift"
  "$here/Window.swift" "$here/Window+Attach.swift" "$here/Window+Chats.swift"
  "$here/Window+Dialogs.swift" "$here/Window+Files.swift" "$here/Window+FollowUps.swift"
  "$here/Window+Menus.swift" "$here/Window+Palette.swift" "$here/Window+Runs.swift"
  "$here/Window+Settings.swift" "$here/Window+TitleBar.swift" "$here/Window+Update.swift"
  "$here/Window+Worktrees.swift" "$here/Worker.swift" "$here/Worktree.swift" "$here/KeepAwake.swift"
  "$here/Window+GitDiff.swift" "$here/Window+GitActions.swift" "$here/Window+Checkpoints.swift"
  "$here/AgentSettings.swift" "$here/TitleGenerator.swift"
  "$here/Window+Environments.swift" "$here/Window+McpSkills.swift"
  "$here/BrowserViewportView.swift" "$here/Window+Browser.swift"
  "$here/Account.swift" "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$repo/apps/shared/native/KitPasskey.swift"
)

xcrun -sdk macosx swiftc "$swift_opt" -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 -import-objc-header "$here/Bridge.h" \
  "${swift_files[@]}" \
  "${c_objs[@]}" \
  "${link_flags[@]}" \
  -o "$stage_app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$stage_app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Delete :OAProfile" "$stage_app/Contents/Info.plist" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :OAProfile string v1" "$stage_app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Delete :OABuildDate" "$stage_app/Contents/Info.plist" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :OABuildDate string $(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$stage_app/Contents/Info.plist"
if build_sha="$(git -C "$repo" rev-parse HEAD 2>/dev/null)"; then
  build_rev="$(git -C "$repo" rev-list --count HEAD 2>/dev/null || echo 0)"
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $((28830 + build_rev))" "$stage_app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Delete :OACommit" "$stage_app/Contents/Info.plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :OACommit string $build_sha" "$stage_app/Contents/Info.plist"
fi
cp "$here/resources/OpenAgents.icns" "$res/OpenAgents.icns"
coax_hash="$(shasum -a 256 "$coax" | cut -d " " -f 1)"
for p in "${worker_pids[@]}"; do wait "$p"; done
cp "$repo/coax/web/build/tools/oa-worker-$coax_hash" "$res/oa-worker"
cp "$repo/coax/web/build/tools/gitguard-$coax_hash" "$res/gitguard"
cp "$repo/coax/web/build/tools/claudeworker-$coax_hash" "$res/claudeworker"
cp "$repo/coax/web/build/tools/host-$coax_hash" "$res/coder-host"
cp "$work/psionic-inkling/psionic-inkling-serve" "$res/psionic-inkling-serve"
chmod +x "$res/psionic-inkling-serve"
cp "$work/psionic-inkling/inkling-kernels.metal" "$res/inkling-kernels.metal"
rm -rf "$res/Components/fonts" "$res/Components/icons"
mkdir -p "$res/Components/fonts" "$res/Components/icons"
cp "$repo/components/fonts/"Geist* "$res/Components/fonts/"
cp "$repo/components/icons/"*.svg "$res/Components/icons/"
cp "$repo/components/NOTICE" "$res/Components/NOTICE"

sign_bundle "$stage_app" "$work"
rm -rf "$work/OpenAgents.app.old"
[[ -e "$app" ]] && mv "$app" "$work/OpenAgents.app.old"
mv "$stage_app" "$app"
rm -rf "$work/OpenAgents.app.old"
echo "built $app"
[[ "$open_app" == "1" ]] && open "$app"
exit 0
