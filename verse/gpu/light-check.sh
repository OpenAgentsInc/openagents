#!/usr/bin/env bash
# The G1 exit check (docs/verse/native-gpu.md): every #export_c procedure of
# verse/light.coax on the GPU, through `coax spirv --kernel`, against its C
# build over the same rows of arguments.
#   verse/gpu/light-check.sh HOST    generate here, build and run on HOST over ssh
# HOST needs nix (gcc, the Vulkan loader and the validation layers come from
# nix-shell) and the GPU's Vulkan driver; the run is headless (vkc.h).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: light-check.sh HOST}"
coax="$root/coax/bin/coax"
src="${VERSE_GPU_LIGHT:-$root/verse/light.coax}"
work="$(mktemp -d "${TMPDIR:-/tmp}/light-check.XXXXXX")"
trap 'rm -rf "$work"' EXIT

"$coax" c "$src" --library --header "$work/light.h" > "$work/light.c"
# Each export and its parameter count, from the header Coax wrote.
grep -E '^float [a-z_0-9]+\((float [a-z_0-9]+(, )?)*\);$' "$work/light.h" |
  sed -E 's/^float ([a-z_0-9]+)\((.*)\);$/\1 \2/' |
  awk '{ n = 0; for (i = 2; i <= NF; i++) if ($i == "float") n++; print $1, n }' > "$work/exports"
{
  while read -r name n; do
    args=""
    for ((k = 0; k < n; k++)); do args+="${args:+, }a[$k]"; done
    echo "static float call_$name(const float *a) { return $name($args); }"
  done < "$work/exports"
  echo "static const Entry LIGHT[] = {"
  while read -r name n; do echo "  {\"$name\", $n, call_$name},"; done < "$work/exports"
  echo "};"
} > "$work/light-table.h"
while read -r name n; do "$coax" spirv "$src" --kernel "$name" -o "$work/$name.spv"; done < "$work/exports"
cp "$here"/vk-kernel.c "$here"/vkc.c "$here"/vkc.h "$here"/light-cpu.c "$here"/light-grid.c "$work/"
echo "light-check: $(wc -l < "$work/exports" | tr -d ' ') procedures of $src, SPIR-V by coax spirv; running on $host"

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work" -cf - . | ssh -o BatchMode=yes "$host" 'set -e
d=$(mktemp -d); trap "rm -rf $d" EXIT; tar -x -C "$d" 2>/dev/null; cd "$d"
export VK_DRIVER_FILES=${VK_DRIVER_FILES:-/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json}
nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers --run "
  set -e
  gcc -std=c11 -O2 -Wno-unknown-pragmas -I. vk-kernel.c vkc.c -lvulkan -o vk-kernel
  gcc -std=c11 -O2 -Wno-unknown-pragmas -I. light-cpu.c light.c -lm -o light-cpu
  gcc -std=c11 -O2 light-grid.c -o light-grid
  export VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))
  export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
  failed=0
  while read -r name n; do
    ./light-grid \$n 65536 > in.f32
    ./vk-kernel \$name.spv \$n in.f32 gpu.f32 2> vk.log || { cat vk.log; failed=1; continue; }
    grep -q \"validation: 0 errors, 0 warnings\" vk.log || { cat vk.log; failed=1; }
    ./light-cpu \$name in.f32 gpu.f32 || failed=1
  done < exports
  [ \$failed = 0 ] && echo \"light-check: every procedure agrees, validation clean\" || { echo \"light-check: FAILED\"; exit 1; }
"'
