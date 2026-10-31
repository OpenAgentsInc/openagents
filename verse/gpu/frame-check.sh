#!/usr/bin/env bash
# The G2 exit check (docs/verse/native-gpu.md): a Star Catcher frame drawn on
# the GPU (a full-screen pass for the sky and a stroke pass, both from
# verse/kms/starcatch-light.coax through `coax spirv`) against the CPU frame
# of verse/kms --headless, within 1/255 a channel, validation clean.
#   verse/gpu/frame-check.sh HOST [WxH] [FRAMES]
# Generates here, builds and runs on HOST over ssh, headless (vkc.h): no
# screen, VT or DRM master. HOST needs nix and the GPU's Vulkan driver.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: frame-check.sh HOST [WxH] [FRAMES]}"
size="${2:-1920x1080}"
frames="${3:-120}"
coax="${COAX:-$root/coax/bin/coax}"
work="$(mktemp -d "${TMPDIR:-/tmp}/frame-check.XXXXXX")"
trap 'rm -rf "$work"' EXIT

VERSE_KMS_GEN_ONLY=1 VERSE_KMS_REGEN=1 "$root/verse/kms/build.sh" "$work" > /dev/null
light="$root/verse/kms/starcatch-light.coax"
"$coax" spirv "$light" --fill starcatch_sky_r,starcatch_sky_g,starcatch_sky_b -o "$work/sky.spv"
"$coax" spirv "$light" --stroke starcatch_fx -o "$work/fx.spv"
cp "$here"/vk-frame.c "$here"/vkc.c "$here"/vkc.h "$work/"
# The uniforms each pass takes, as starcatch-native.c passes them: the sky
# takes w, h, time, ground top and flash; a stroke takes fade.
echo "frame-check: star-catcher $size, $frames frames, SPIR-V by coax spirv; running on $host"

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
export VK_DRIVER_FILES=\${VK_DRIVER_FILES:-/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json}
nix-shell -p gcc pkg-config libdrm vulkan-headers vulkan-loader vulkan-validation-layers --run '
  set -e
  realm=\"starcatch-native.c starcatch.c starcatch-light.c\"
  gcc -std=c11 -O2 -pthread -Wno-unknown-pragmas -I. \$(pkg-config --cflags libdrm) kms.c \$realm \$(pkg-config --libs libdrm) -lm -o verse-kms
  gcc -std=c11 -O2 -Wno-unknown-pragmas -I. vk-frame.c vkc.c \$realm -lvulkan -lm -o vk-frame
  ./verse-kms --headless $size --frames $frames --shot cpu.ppm
  export VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))
  export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
  ./vk-frame $size $frames 2 sky.spv 0,1,2,4,5 fx.spv 3 cpu.ppm ${VK_FRAME_KEEP:+--ppm gpu.ppm} || status=1
  [ -n \"${VK_FRAME_KEEP:-}\" ] && mkdir -p ${VK_FRAME_KEEP:-x} && cp cpu.ppm gpu.ppm ${VK_FRAME_KEEP:-x}/
  exit \${status:-0}
'"
