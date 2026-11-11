#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang and xcrun; holds no logic)
# Builds the component library as a Mac program on the shared Mac host
# (apps/shared/host/apple/ui-host.m, the ui-host.h contract the Linux host keeps): the same
# main.c and Coax as components/gallery/linux/, the atlas built in Coax, and the component
# shader emitted from Coax as Metal.
#   components/gallery/apple/build.sh
# The program is components/gallery/build/apple/Components. Components --shot OUT.png writes a
# picture and opens no window, no Dock icon and no menu.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/../build/apple"
mkdir -p "$work/c"
"$coax" c "$here/../linux.coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/components.metal" 2> "$work/components.metal.bind"
cflags=(-O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux")
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$here/../linux/main.c" -o "$work/c/main.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/gallery.c" -o "$work/c/gallery.o"
/usr/bin/clang -fobjc-arc "${cflags[@]}" -c "$repo/apps/shared/host/apple/ui-host.m" -o "$work/c/ui-host.o"
/usr/bin/clang "$work/c/main.o" "$work/c/gallery.o" "$work/c/ui-host.o" \
  -framework AppKit -framework Metal -framework QuartzCore -framework ImageIO -framework UniformTypeIdentifiers -framework CoreGraphics -lm -o "$work/Components.new"
rm -rf "$work/fonts" "$work/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$work/"
cp "$repo/components/zeron/NOTICE" "$work/NOTICE"
mv "$work/Components.new" "$work/Components"
echo "built $work/Components"
