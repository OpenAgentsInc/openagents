#!/usr/bin/env bash
# Builds OpenAgents.app for this Mac (docs/desktop/coax-desktop.md): chat.coax
# compiled to C with the C face, chat.wgsl's passes in Metal, and the AppKit
# host in Swift.
#   apps/openagents-desktop/build.sh [--open]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
work="$here/build"
app="$work/OpenAgents.app"
mkdir -p "$work/c" "$app/Contents/MacOS" "$app/Contents/Resources"
"$repo/coax/bin/coax" c "$repo/apps/openagents-web/chat.coax" --library --header "$work/c/chat.h" > "$work/c/chat.c"
cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -I"$work/c" -I"$here")
/usr/bin/clang "${cflags[@]}" -c "$work/c/chat.c" -o "$work/c/chat.o"
/usr/bin/clang "${cflags[@]}" -c "$here/face.c" -o "$work/c/face.o"
xcrun -sdk macosx metal "$here/Shaders.metal" -o "$app/Contents/Resources/default.metallib"
xcrun -sdk macosx swiftc -O -import-objc-header "$here/face.h" "$here"/Sources/*.swift "$work/c/chat.o" "$work/c/face.o" \
  -framework AppKit -framework Metal -framework MetalKit -framework CoreText -o "$app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$app/Contents/Info.plist"
codesign --force --sign - "$app" >/dev/null
echo "built $app"
[[ "${1:-}" == --open ]] && open "$app"
exit 0
