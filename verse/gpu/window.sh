#!/usr/bin/env bash
# Star Catcher in a Vulkan window on HOST's Wayland desktop (verse/gpu/vk-window.c),
# the G2 desktop-app check (docs/verse/native-gpu.md). It opens a window on
# that desktop, so run it when the person at that screen wants one.
#   verse/gpu/window.sh HOST [vk-window options: --fullscreen --seconds N --scale K --size WxH]
# WAYLAND_DISPLAY on HOST defaults to the compositor on the screen
# (VERSE_WAYLAND_DISPLAY overrides it). Generates here; builds and runs on
# HOST over ssh in nix-shell, under the validation layers.
# VERSE_WINDOW_BUILD_ONLY=1 builds without opening the window.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: window.sh HOST [--fullscreen] [--seconds N] [--scale K] [--size WxH]}"
shift
coax="${COAX:-$root/coax/bin/coax}"
work="$(mktemp -d "${TMPDIR:-/tmp}/window.XXXXXX")"
trap 'rm -rf "$work"' EXIT

VERSE_KMS_GEN_ONLY=1 VERSE_KMS_REGEN=1 "$root/verse/kms/build.sh" "$work" > /dev/null
light="$root/verse/kms/starcatch-light.coax"
"$coax" spirv "$light" --fill starcatch_sky_r,starcatch_sky_g,starcatch_sky_b -o "$work/sky.spv"
"$coax" spirv "$light" --stroke starcatch_fx -o "$work/fx.spv"
cp "$here"/vk-window.c "$here"/vkc.c "$here"/vkc.h "$here"/vkp.c "$here"/vkp.h "$here"/vkr.c "$here"/vkr.h "$work/"
args="$*"
display="${VERSE_WAYLAND_DISPLAY:-wayland-1}"

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
export VK_DRIVER_FILES=\${VK_DRIVER_FILES:-/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json}
export XDG_RUNTIME_DIR=/run/user/\$(id -u) WAYLAND_DISPLAY=$display
nix-shell -p gcc pkg-config wayland wayland-scanner wayland-protocols vulkan-headers vulkan-loader vulkan-validation-layers --run '
  set -e
  p=\$(pkg-config --variable=pkgdatadir wayland-protocols)
  for x in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1; do
    n=\$(basename \$x)
    wayland-scanner client-header \$p/\$x.xml \$n-client-protocol.h
    wayland-scanner private-code \$p/\$x.xml \$n-protocol.c
  done
  realm=\"starcatch-native.c starcatch.c starcatch-light.c\"
  gcc -std=c11 -O2 -Wall -Wno-unknown-pragmas -I. vk-window.c vkc.c vkp.c vkr.c xdg-shell-protocol.c xdg-decoration-unstable-v1-protocol.c \$realm \$(pkg-config --cflags --libs wayland-client) -lvulkan -lm -o vk-window
  ${VERSE_WINDOW_BUILD_ONLY:+echo built; exit 0}
  export VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))
  export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
  ./vk-window sky.spv 0,1,2,4,5 fx.spv 3 $args
'"
