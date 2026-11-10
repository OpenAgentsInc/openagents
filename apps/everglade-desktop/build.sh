#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, metal and swiftc; holds no logic)
# Builds Everglade.app for this Mac (everglade/docs/desktop-app-spec.md): the
# Workshop scene (workshop.coax) and your model (model.coax) compiled to C
# behind face.h, their shaders, Everglade II's pack, and the AppKit and Metal
# host in Swift.
#   apps/everglade-desktop/build.sh [--open]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/build"
app="$work/Everglade.app"
mkdir -p "$work/c" "$app/Contents/MacOS" "$app/Contents/Resources"
"$coax" c "$here/workshop.coax" --library --header "$work/c/workshop.h" > "$work/c/workshop.c"
cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -I"$work/c" -I"$here")
/usr/bin/clang "${cflags[@]}" -c "$work/c/workshop.c" -o "$work/c/workshop.o"
/usr/bin/clang "${cflags[@]}" -c "$here/face.c" -o "$work/c/face.o"
"$coax" c "$here/model.coax" --library --header "$work/c/model.h" > "$work/c/model.c"
/usr/bin/clang "${cflags[@]}" -w -c "$work/c/model.c" -o "$work/c/model.o"
/usr/bin/clang "${cflags[@]}" -c "$here/model-face.c" -o "$work/c/model-face.o"
xcrun -sdk macosx metal "$here/Workshop.metal" "$here/Model.metal" -o "$app/Contents/Resources/default.metallib"
cp "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$app/Contents/Resources/everglade2.pack"
# The web's character code passes its Creator and Bodies by value, megabytes a call, so the main
# thread, which runs every Coax call, gets a 512 MB stack (reserved, not touched until used).
xcrun -sdk macosx swiftc -O -import-objc-header "$here/face.h" "$here"/Sources/*.swift "$work/c/workshop.o" "$work/c/face.o" "$work/c/model.o" "$work/c/model-face.o" \
  -framework AppKit -framework Metal -framework MetalKit -framework QuartzCore -framework ImageIO -framework CoreGraphics -lm \
  -Xlinker -stack_size -Xlinker 0x20000000 -o "$app/Contents/MacOS/Everglade"
cp "$here/Info.plist" "$app/Contents/Info.plist"
codesign --force --sign - "$app" >/dev/null
echo "built $app"
# --open runs a copy outside the checkout, so the app outlives a worktree.
if [[ "${1:-}" == --open ]]; then
  home="$HOME/.openagents/everglade"
  mkdir -p "$home"
  rm -rf "$home/Everglade.app"
  ditto "$app" "$home/Everglade.app"
  open "$home/Everglade.app"
fi
exit 0
