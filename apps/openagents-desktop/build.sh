#!/usr/bin/env bash
# Builds OpenAgents.app for this Mac (docs/desktop/coax-desktop.md): the
# Zeron-fork chat screen (zeron/screen.coax, GPUI's shaders in Zeron.metal,
# Geist and Zeron's icons) it opens on, and, still in the tree, the HUD chat
# (chat-hud.coax, the old HUD chat /chat drew before the Zeron one) and the Lab's router (router.coax,
# Mammoth), all compiled to C behind their C faces, with the AppKit host in Swift.
#   apps/openagents-desktop/build.sh [--open]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
work="$here/build"
app="$work/OpenAgents.app"
mkdir -p "$work/c" "$app/Contents/MacOS" "$app/Contents/Resources"
"$repo/coax/bin/coax" c "$repo/apps/openagents-web/chat-hud.coax" --library --header "$work/c/chat.h" > "$work/c/chat.c"
cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -I"$work/c" -I"$here")
/usr/bin/clang "${cflags[@]}" -c "$work/c/chat.c" -o "$work/c/chat.o"
/usr/bin/clang "${cflags[@]}" -c "$here/face.c" -o "$work/c/face.o"
"$repo/coax/bin/coax" c "$here/router.coax" --library --header "$work/c/router.h" > "$work/c/router.c"
/usr/bin/clang "${cflags[@]}" -c "$work/c/router.c" -o "$work/c/router.o"
/usr/bin/clang "${cflags[@]}" -c "$here/route-face.c" -o "$work/c/route-face.o"
/usr/bin/clang "${cflags[@]}" -c "$repo/coax/platform/process-native.c" -o "$work/c/process.o"
"$repo/coax/bin/coax" c "$here/zeron/settings.coax" --library --header "$work/c/zeron.h" > "$work/c/zeron.c"
/usr/bin/clang "${cflags[@]}" -c "$work/c/zeron.c" -o "$work/c/zeron.o"
/usr/bin/clang "${cflags[@]}" -c "$here/zeron-face.c" -o "$work/c/zeron-face.o"
# The Lab's NIP-98 signer for our inference route, a unit of its own (lab-sign.coax); its crypto's checks need the larger step budget.
"$repo/coax/bin/coax" c "$here/lab-sign.coax" --run-steps 250000000 --library --header "$work/c/lab-sign.h" > "$work/c/lab-sign.c"
/usr/bin/clang "${cflags[@]}" -c "$work/c/lab-sign.c" -o "$work/c/lab-sign.o"
xcrun -sdk macosx metal "$here/Shaders.metal" "$here/Zeron.metal" -o "$app/Contents/Resources/default.metallib"
xcrun -sdk macosx swiftc -O -import-objc-header "$here/face.h" "$here"/Sources/*.swift "$work/c/chat.o" "$work/c/face.o" "$work/c/router.o" "$work/c/route-face.o" "$work/c/process.o" "$work/c/zeron.o" "$work/c/zeron-face.o" "$work/c/lab-sign.o" \
  -framework AppKit -framework Metal -framework MetalKit -framework CoreText -o "$app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$app/Contents/Info.plist"
rm -rf "$app/Contents/Resources/fonts" "$app/Contents/Resources/icons"
# Geist, its OFL, Zeron's icons and the NOTICE live beside the shared components (apps/shared/zeron/).
cp -R "$repo/apps/shared/zeron/fonts" "$repo/apps/shared/zeron/icons" "$app/Contents/Resources/"
cp "$repo/apps/shared/zeron/NOTICE" "$app/Contents/Resources/NOTICE"
codesign --force --sign - "$app" >/dev/null
echo "built $app"
[[ "${1:-}" == --open ]] && open "$app"
exit 0
