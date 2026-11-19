#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, swiftc and codesign; holds no logic)
# Builds Coder.app for this Mac: the page (app.coax, page.coax) compiled to C on the shared
# component carrier (carrier.c, host.c) and Mac host (apps/shared/host/apple/ui-host.m), with the
# component shader emitted as Metal, the Microcoder worker (coder/tui/mcworker.coax) it talks to, and the daemon (coder/tui/host.coax) that runs it.
#   apps/coder-desktop/build.sh [--open]
#   CODER_BUILD=/tmp/coder apps/coder-desktop/build.sh   builds into another folder
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="${CODER_BUILD:-$here/build}"   # CODER_BUILD=DIR builds a second copy and leaves a running Coder's files alone
app="$work/Coder.app"
res="$app/Contents/Resources"
mkdir -p "$work/c" "$app/Contents/MacOS" "$res/Components"
"$coax" c "$here/app.coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$res/Components/components.metal" 2> "$work/c/components.metal.bind"
"$coax" metal "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs -o "$res/Components/composition.metal" 2> "$work/c/composition.metal.bind"
cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux")
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$here/carrier.c" -o "$work/c/carrier.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/gallery.c" -o "$work/c/gallery.o"
/usr/bin/clang -fobjc-arc "${cflags[@]}" -c "$repo/apps/shared/host/apple/ui-host.m" -o "$work/c/ui-host.o"
xcrun -sdk macosx swiftc -O -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 -import-objc-header "$here/Bridge.h" \
  "$here/Worker.swift" "$here/Store.swift" "$here/Window.swift" "$here/Window+Runs.swift" "$here/Window+FollowUps.swift" "$here/Window+Update.swift" "$here/Window+Foreman.swift" "$here/Window+Chats.swift" "$here/Window+Settings.swift" "$here/Window+Dialogs.swift" "$here/Window+Menus.swift" "$here/Window+ForemanAPI.swift" "$here/Window+ForemanChats.swift" "$here/AppDelegate.swift" \
  "$here/Foreman.swift" "$here/ForemanOperator.swift" "$here/Updater.swift" "$here/DaemonClient.swift" "$work/c/carrier.o" "$work/c/gallery.o" "$work/c/ui-host.o" \
  -framework AppKit -framework Metal -framework QuartzCore -framework CoreText -framework ImageIO -framework UniformTypeIdentifiers -framework CoreGraphics \
  -o "$app/Contents/MacOS/Coder.new"
mv "$app/Contents/MacOS/Coder.new" "$app/Contents/MacOS/Coder"
cp "$here/Info.plist" "$app/Contents/Info.plist"
cp "$repo/apps/openagents-desktop/native/OpenAgents.icns" "$res/Coder.icns"
# The Microcoder worker (coder/tui/mcworker.coax): tool.sh builds its image, named by the compiler's hash; the app runs the copy.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/mcworker.coax" < /dev/null > /dev/null
cp "$repo/coax/web/build/tools/mcworker-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/mcworker"
# The Claude Code worker (coder/tui/claudeworker.coax), for a sub-agent on a Claude model.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/claudeworker.coax" < /dev/null > /dev/null
cp "$repo/coax/web/build/tools/claudeworker-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/claudeworker"
# The unified daemon (coder/tui/host.coax, `coder host serve`): Coder starts it when none answers, and it
# owns the worker runs so they go on after Coder quits (GitHub #13694). An unknown verb only builds it.
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/host.coax" build-only < /dev/null > /dev/null 2>&1 || true
cp "$repo/coax/web/build/tools/host-$(shasum -a 256 "$coax" | cut -d " " -f 1)" "$res/coder-host"
cp "$here/foreman-ctl" "$res/foreman-ctl"
chmod +x "$res/foreman-ctl"
rm -rf "$res/Components/fonts" "$res/Components/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$res/Components/"
cp "$repo/components/zeron/NOTICE" "$res/Components/NOTICE"
codesign --force --sign - "$app"
echo "built $app"
[[ "${1:-}" == --open ]] && open "$app"
exit 0
