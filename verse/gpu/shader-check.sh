#!/usr/bin/env bash
# The G4 check (docs/verse/native-gpu.md): verse/gpu/shaders/quad.coax through
# `coax spirv --vertex/--fragment`, drawn on HOST's GPU with its bindings,
# against the same procedures built for the CPU, under the validation layers.
#   verse/gpu/shader-check.sh HOST [W H]
# Generates here, builds and runs on HOST over ssh, headless (vkc.h).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: shader-check.sh HOST [W H]}"
w="${2:-256}"; h="${3:-256}"
coax="${COAX:-$root/coax/bin/coax}"
work="$(mktemp -d "${TMPDIR:-/tmp}/shader-check.XXXXXX")"
trap 'rm -rf "$work"' EXIT

quad="$here/shaders/quad.coax"
"$coax" test "$quad" --library > /dev/null
"$coax" spirv "$quad" --vertex quad_vs --fragment quad_fs -o "$work/quad.spv"
"$coax" c "$quad" --library --header "$work/quad.h" > "$work/quad.c"
cp "$here"/vk-shader.c "$here"/vks.c "$here"/vks.h "$here"/vkc.c "$here"/vkc.h "$work/"
echo "shader-check: quad.coax at ${w}x${h}; running on $host"

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
export VK_DRIVER_FILES=\${VK_DRIVER_FILES:-/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json}
nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers --run '
  set -e
  gcc -std=c11 -O2 -ffp-contract=off -Wno-unknown-pragmas -I. vk-shader.c vks.c vkc.c quad.c -lvulkan -lm -o vk-shader
  export VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))
  export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
  ./vk-shader $w $h quad.spv
'"
