set -euo pipefail
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json
timeout 300 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run '
set -e
for file in filtered.spv invalid.spv fault.spv; do spirv-val --target-env vulkan1.1 "$file"; done
gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas -I. render-sampling-gpu.c vks.c vkc.c RenderCaps.c -lvulkan -lm -o render-sampling-gpu
gcc --version >gcc-version.txt
command -v gcc >gcc-path.txt
sha256sum "$(command -v gcc)" render-sampling-gpu >native-hashes.txt
export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path "*validation-layers*" | head -1))
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
./render-sampling-gpu filtered.spv invalid.spv >positive.log 2>&1
if ./render-sampling-gpu filtered.spv fault.spv >fault.log 2>&1; then echo "missing-demotion control escaped"; exit 1; fi
grep -q "wrong=16 validation_errors=0 validation_warnings=0" fault.log
'
