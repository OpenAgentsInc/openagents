#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, cc, wayland-scanner and pkg-config; holds no logic)
# Builds OpenAgents Desktop for Linux (GitHub #13781): page.coax + desktop core
# (coax/lib/agent/desktop/) + shared Wayland/Vulkan host (apps/shared/host/linux/).
#   apps/openagents-desktop/linux/build.sh [--smoke] [--flow-test] [--open] [--no-worker]
# --flow-test: the chat flow test (flow-test.sh) on the mock worker, headless.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
app_dir="$(cd "$here/.." && pwd)"
repo="$(cd "$here/../../.." && pwd)"

if [[ -z "${OA_DESKTOP_LINUX_BUILD:-}" ]] && command -v nix-shell >/dev/null 2>&1; then
  export OA_DESKTOP_LINUX_BUILD=1
  cmd="$(printf '%q ' "$here/build.sh" "$@")"
  exec nix-shell -p gcc pkg-config vulkan-headers vulkan-loader wayland wayland-scanner wayland-protocols libxkbcommon dbus systemd mesa \
    --run "exec $cmd"
fi

coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then "$repo/coax/build.sh"; fi

work="$here/build"
mkdir -p "$work/c" "$work/wl" "$work/resources"

# 1. Compile page (app.coax), desktop core bridge (core.coax), entropy policy, and Wayland input/text.
"$coax" c "$app_dir/app.coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" c "$here/core.coax" --library --header "$work/c/core.h" > "$work/c/core.c"
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h" > "$work/c/entropy.c"
"$coax" c "$repo/metaverse-engine/host/wayland-input.coax" --library --header "$work/c/wayland-input.h" > "$work/c/wayland-input.c"
"$coax" c "$repo/metaverse-engine/host/wayland-text.coax" --library --header "$work/c/wayland-text.h" > "$work/c/wayland-text.c"

# 2. Emit the shared component SPIR-V shader.
"$coax" spirv "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/resources/components.spv" 2> "$work/resources/components.spv.bindings"

# 3. Generate Wayland protocol C bindings.
protocols="$(pkg-config --variable=pkgdatadir wayland-protocols)"
for spec in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1 unstable/text-input/text-input-unstable-v3; do
  name="$(basename "$spec")"
  wayland-scanner client-header "$protocols/$spec.xml" "$work/wl/$name-client-protocol.h"
  wayland-scanner private-code "$protocols/$spec.xml" "$work/wl/$name-protocol.c"
done

# 4. Compile C units and link OpenAgents.
p="$repo/coax/platform"
cflags=(-std=gnu11 -O2 -ffp-contract=off -fno-fast-math -fno-builtin-pow -w \
  -D_DEFAULT_SOURCE -DENGINE_WAYLAND_INPUT_V1 -DENGINE_WAYLAND_TEXT_V1 \
  -I"$work/c" -I"$work/wl" -I"$app_dir" -I"$repo/apps/shared/host/linux" \
  -I"$repo/metaverse/gpu" -I"$repo/metaverse/kms" -I"$p")

# shellcheck disable=SC2046
cc "${cflags[@]}" \
  "$here/main.c" \
  "$app_dir/carrier.c" \
  "$repo/apps/shared/host/linux/ui-host.c" \
  "$repo/apps/shared/host/linux/portal.c" \
  \
  "$work/c/gallery.c" "$work/c/core.c" "$work/c/entropy.c" \
  "$work/c/wayland-input.c" "$work/c/wayland-text.c" \
  "$repo/metaverse/gpu/vkc.c" "$repo/metaverse/gpu/vks.c" "$repo/metaverse/gpu/vkp.c" \
  "$work/wl/xdg-shell-protocol.c" "$work/wl/xdg-decoration-unstable-v1-protocol.c" "$work/wl/text-input-unstable-v3-protocol.c" \
  "$p/process-native.c" "$p/store-file-native.c" "$p/read-file-native.c" \
  "$p/socket-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" \
  $(pkg-config --cflags --libs wayland-client wayland-cursor xkbcommon libsystemd) \
  -lvulkan -lpthread -lm -o "$work/OpenAgents.new"

rm -rf "$work/resources/fonts" "$work/resources/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$work/resources/"
cp "$repo/components/zeron/NOTICE" "$work/resources/NOTICE"
mv "$work/OpenAgents.new" "$work/OpenAgents"
echo "built $work/OpenAgents"

# 5. Build mcworker (Microcoder worker) unless --no-worker is passed.
want_worker=1
for arg in "$@"; do [[ "$arg" == --no-worker ]] && want_worker=0; done
if [[ "$want_worker" == 1 && -f "$repo/coder/tui/mcworker.coax" ]]; then
  "$repo/coax/web/tool.sh" "$repo/coder/tui/mcworker.coax" --build-only >/dev/null 2>&1 || true
fi

# Headless runs draw on Mesa's software Vulkan (lavapipe) when no driver is set.
software_vulkan() {
  if [[ -z "${VK_DRIVER_FILES:-}" && -z "${VK_ICD_FILENAMES:-}" ]]; then
    for d in $(echo "${NIX_PROFILES:-} ${XDG_DATA_DIRS:-} /usr/share /run/opengl-driver/share" | tr ':' ' '); do
      for icd in "$d"/vulkan/icd.d/lvp_icd*.json "$d"/share/vulkan/icd.d/lvp_icd*.json; do
        if [[ -f "$icd" ]]; then export VK_DRIVER_FILES="$icd"; return; fi
      done
    done
    if [[ -d /nix/store ]]; then
      icd="$(ls -1 /nix/store/*mesa*/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -n 1 || true)"
      if [[ -n "$icd" ]]; then export VK_DRIVER_FILES="$icd"; fi
    fi
  fi
}

for arg in "$@"; do
  if [[ "$arg" == --smoke ]]; then
    software_vulkan
    "$work/OpenAgents" --smoke --resources "$work/resources"
  elif [[ "$arg" == --flow-test ]]; then
    software_vulkan
    "$here/flow-test.sh" --no-build
  elif [[ "$arg" == --open ]]; then
    exec "$work/OpenAgents" --resources "$work/resources"
  fi
done
