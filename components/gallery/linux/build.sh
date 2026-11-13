#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, cc, wayland-scanner and pkg-config; holds no logic)
# Builds the component library as a Linux program (components/spec.md, GitHub #13153): the
# page openagents.com/components shows (components/gallery/page.coax), the atlas built in
# Coax, the component shader emitted from Coax as SPIR-V, and the shared Wayland and Vulkan
# host (apps/shared/host/linux/). Nix supplies the C headers and libraries only.
#   components/gallery/linux/build.sh
# The program is components/gallery/build/linux/Components, with its fonts, icons and shader
# beside it. Components --shot OUT.png writes a picture and opens no window.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
if [[ -z "${OA_COMPONENTS_BUILD:-}" ]]; then
  export OA_COMPONENTS_BUILD=1 OA_COMPONENTS_BUILD_ENTRY="$here/build.sh"
  exec nix-shell -p gcc pkg-config vulkan-headers vulkan-loader wayland wayland-scanner wayland-protocols libxkbcommon \
    --run 'exec "$OA_COMPONENTS_BUILD_ENTRY"'
fi
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/../build/linux"
mkdir -p "$work/c" "$work/wl"
"$coax" c "$here/../linux.coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
# The seat's keys and text (metaverse-engine/host/): what metaverse/gpu/vkw.h's XKB and text-input glue calls.
"$coax" c "$repo/metaverse-engine/host/wayland-input.coax" --library --header "$work/c/wayland-input.h" > "$work/c/wayland-input.c"
"$coax" c "$repo/metaverse-engine/host/wayland-text.coax" --library --header "$work/c/wayland-text.h" > "$work/c/wayland-text.c"
"$coax" spirv "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/components.spv" 2> "$work/components.spv.bindings"
protocols="$(pkg-config --variable=pkgdatadir wayland-protocols)"
for spec in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1 unstable/text-input/text-input-unstable-v3; do
  name="$(basename "$spec")"
  wayland-scanner client-header "$protocols/$spec.xml" "$work/wl/$name-client-protocol.h"
  wayland-scanner private-code "$protocols/$spec.xml" "$work/wl/$name-protocol.c"
done
cflags=(-std=gnu11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$work/wl" -I"$repo/apps/shared/host/linux" -I"$repo/metaverse/gpu" -I"$repo/metaverse/kms" -I"$repo/coax/platform")
# shellcheck disable=SC2046
cc "${cflags[@]}" -DENGINE_WAYLAND_INPUT_V1 -DENGINE_WAYLAND_TEXT_V1 \
  "$here/main.c" "$repo/apps/shared/host/linux/ui-host.c" "$work/c/gallery.c" "$work/c/wayland-input.c" "$work/c/wayland-text.c" \
  "$repo/metaverse/gpu/vkc.c" "$repo/metaverse/gpu/vks.c" "$repo/metaverse/gpu/vkp.c" \
  "$work/wl/xdg-shell-protocol.c" "$work/wl/xdg-decoration-unstable-v1-protocol.c" "$work/wl/text-input-unstable-v3-protocol.c" \
  $(pkg-config --cflags --libs wayland-client wayland-cursor xkbcommon) -lvulkan -lpthread -lm -o "$work/Components.new"
rm -rf "$work/fonts" "$work/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$work/"
cp "$repo/components/zeron/NOTICE" "$work/NOTICE"
mv "$work/Components.new" "$work/Components"
echo "built $work/Components"
