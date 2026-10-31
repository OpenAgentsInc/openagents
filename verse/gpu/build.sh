#!/usr/bin/env bash
# Build vk-check and vk-kernel (docs/verse/native-gpu.md): Coax to C, then
# cc with the Vulkan loader.
#   verse/gpu/build.sh [OUT_DIR]                    generate C (if coax is here) and build
#   VERSE_GPU_GEN_ONLY=1 verse/gpu/build.sh OUT_DIR  generate C only, for a box without coax
# On NixOS build and run inside
#   nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers
# with VK_DRIVER_FILES naming the driver's ICD (the NVIDIA one is
# /run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json). To validate:
#   VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation OUT_DIR/vk-check
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
out="${1:-$here/build}"
mkdir -p "$out"
if [[ -x "$root/coax/bin/coax" ]]; then
  "$root/coax/bin/coax" test "$here/spirv.coax" > /dev/null
  "$root/coax/bin/coax" c "$here/spirv.coax" --library --header "$out/spirv.h" > "$out/spirv.c"
  cp "$here"/vk.c "$here"/vk-kernel.c "$here"/vkc.c "$here"/vkc.h "$out/"
fi
[[ -n "${VERSE_GPU_GEN_ONLY:-}" ]] && exit 0
"${CC:-cc}" -std=c11 -O2 -g -I "$out" "$out/vk.c" "$out/vkc.c" "$out/spirv.c" -lvulkan -lm -o "$out/vk-check"
"${CC:-cc}" -std=c11 -O2 -g -I "$out" "$out/vk-kernel.c" "$out/vkc.c" -lvulkan -o "$out/vk-kernel"
echo "$out/vk-check $out/vk-kernel"
