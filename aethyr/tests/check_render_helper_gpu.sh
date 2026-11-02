#!/usr/bin/env bash
# Headless actual-source fine/helper and per-pass byte-view qualification.
set -euo pipefail
host="${1:?usage: check_render_helper_gpu.sh HOST OUTPUT_DIR}"
out="${2:?usage: check_render_helper_gpu.sh HOST OUTPUT_DIR}"
root="$(cd "$(dirname "$0")/../.." && pwd -P)"
cd "$root"
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
coax="${COAX:-$root/coax/bin/coax}"
"$coax" spirv aethyr/render/tests/pipeline-shader.coax --vertex rp_vertex --fragment rp_helper_fragment -o "$out/helper.spv" 2> "$out/helper.reflection"
"$coax" spirv aethyr/tests/render-binding-shader.coax --vertex render_range_vs --fragment render_range_fs -o "$out/range.spv" 2> "$out/range.reflection"
"$coax" c aethyr/tests/render-capabilities-bridge.coax --library --header "$out/RenderCaps.h" > "$out/RenderCaps.c"
cp aethyr/tests/render-helper-gpu.c verse/gpu/vks.c verse/gpu/vks.h verse/gpu/vkc.c verse/gpu/vkc.h "$out/"
cat > "$out/remote.sh" <<'REMOTE'
set -euo pipefail
export VK_DRIVER_FILES="${VK_DRIVER_FILES:-/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json}"
timeout 300 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run '
  set -e
  spirv-val --target-env vulkan1.1 helper.spv
  spirv-val --target-env vulkan1.1 range.spv
  gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas -I. render-helper-gpu.c vks.c vkc.c RenderCaps.c -lvulkan -lm -o render-helper-gpu
  export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path "*validation-layers*" | head -1))
  export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
  ./render-helper-gpu helper.spv range.spv
'
REMOTE
COPYFILE_DISABLE=1 tar --no-xattrs -C "$out" -cf - . | ssh -o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=15 -o ServerAliveCountMax=2 "$host" 'set -e; d=$(mktemp -d); trap '\''rm -rf "$d"'\'' EXIT; tar -xf - -C "$d" 2>/dev/null; cd "$d"; bash remote.sh'
