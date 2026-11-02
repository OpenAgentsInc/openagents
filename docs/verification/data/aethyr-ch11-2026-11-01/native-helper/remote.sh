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
