#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, metal and swiftc; holds no logic)
# Builds Everglade.app for this Mac (everglade/docs/desktop-app-spec.md): the
# Workshop scene (workshop.coax) compiled to C behind face.h, its shader, and
# the AppKit and Metal host in Swift.
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
xcrun -sdk macosx metal "$here/Workshop.metal" -o "$app/Contents/Resources/default.metallib"
xcrun -sdk macosx swiftc -O -import-objc-header "$here/face.h" "$here"/Sources/*.swift "$work/c/workshop.o" "$work/c/face.o" \
  -framework AppKit -framework Metal -framework MetalKit -framework QuartzCore -lm -o "$app/Contents/MacOS/Everglade"
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
