#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, cc, wayland-scanner and pkg-config; holds no logic)
# Builds OpenAgents for Linux (docs/apps/2026-11-10-apps-roadmap.md Step 1, GitHub #13153):
# the same Coax the Mac app compiles (zeron/linux.coax) behind zeron-face.c, the atlas built in
# Coax, the component shader emitted from Coax as SPIR-V, and the shared Wayland and Vulkan
# host (apps/shared/host/linux/). Nix supplies the C headers and libraries only.
#   apps/openagents-desktop/linux/build.sh [--install] [--open [APP ARGS...]]
# The program is build/linux/OpenAgents, with its fonts, icons and shader beside it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
if [[ -z "${OA_LINUX_BUILD:-}" ]]; then
  export OA_LINUX_BUILD=1 OA_LINUX_BUILD_ENTRY="$here/build.sh"
  printf -v OA_LINUX_BUILD_ARGS '%q ' "$@"; export OA_LINUX_BUILD_ARGS
  exec nix-shell -p gcc pkg-config vulkan-headers vulkan-loader wayland wayland-scanner wayland-protocols libxkbcommon systemd \
    --run 'eval "exec \"$OA_LINUX_BUILD_ENTRY\" $OA_LINUX_BUILD_ARGS"'
fi
coax="${COAX:-$repo/coax/bin/coax}"
app="$here/.."
work="$app/build/linux"
mkdir -p "$work/c" "$work/wl"
"$coax" c "$app/zeron/linux.coax" --library --header "$work/c/zeron.h" > "$work/c/zeron.c"
"$coax" c "$app/lab-sign.coax" --run-steps 250000000 --library --header "$work/c/lab-sign.h" > "$work/c/lab-sign.c"
# The seat's keys and text (aethyr/host/): what verse/gpu/vkw.h's XKB and text-input glue calls.
"$coax" c "$repo/aethyr/host/wayland-input.coax" --library --header "$work/c/wayland-input.h" > "$work/c/wayland-input.c"
"$coax" c "$repo/aethyr/host/wayland-text.coax" --library --header "$work/c/wayland-text.h" > "$work/c/wayland-text.c"
# Dictation: the shared controller with the Linux host's rules (zeron/dictation-linux.coax), a unit of its
# own as on the Mac, and Psionic's resident Parakeet (CUDA, whose driver and NVRTC it opens at run time).
"$coax" c "$app/zeron/dictation-linux.coax" --release --library --header "$work/c/dictation.h" --run-steps 250000000 > "$work/c/dictation.c"
COAX="$coax" COAX_CC=cc "$repo/psionic/platform/parakeet-build.sh" "$repo/psionic/build/parakeet" "$repo" > /dev/null
"$coax" spirv "$repo/apps/openagents-web/gpu/chat.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/zeron.spv" 2> "$work/zeron.spv.bindings"
protocols="$(pkg-config --variable=pkgdatadir wayland-protocols)"
for spec in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1 unstable/text-input/text-input-unstable-v3; do
  name="$(basename "$spec")"
  wayland-scanner client-header "$protocols/$spec.xml" "$work/wl/$name-client-protocol.h"
  wayland-scanner private-code "$protocols/$spec.xml" "$work/wl/$name-protocol.c"
done
cflags=(-std=gnu11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$work/wl" -I"$app" -I"$repo/apps/shared/host/linux" -I"$repo/verse/gpu" -I"$repo/verse/kms" -I"$repo/coax/platform" -I"$repo/psionic/platform")
# the lab's signer is a unit of its own (lab-sign.coax)
cc "${cflags[@]}" -DS_hash=oa_lab_S_hash -DS_hex=oa_lab_S_hex -c "$work/c/lab-sign.c" -o "$work/c/lab-sign.o"
# and it carries its own copy of the hashes the app's unit also exports: only its one entry point stays visible
objcopy --keep-global-symbol=coaxial_lab_nip98 "$work/c/lab-sign.o"
# the app's unit and Parakeet's both export Coax's SHA-256 (S_hash, S_hex): the app's copy gets names of its own
cc "${cflags[@]}" -DS_hash=oa_zeron_S_hash -DS_hex=oa_zeron_S_hex -c "$work/c/zeron.c" -o "$work/c/zeron.o"
# shellcheck disable=SC2046
cc "${cflags[@]}" -DAETHYR_WAYLAND_INPUT_V1 -DAETHYR_WAYLAND_TEXT_V1 \
  "$here/main.c" "$here/stubs.c" "$here/dictation-host.c" "$app/dictation-face.c" "$repo/apps/openagents-cli/dictate-native.c" "$work/c/dictation.c" "$app/zeron-face.c" "$repo/apps/shared/host/linux/ui-host.c" "$repo/apps/shared/host/linux/portal.c" "$work/c/zeron.o" "$work/c/lab-sign.o" "$work/c/wayland-input.c" "$work/c/wayland-text.c" \
  "$repo/verse/gpu/vkc.c" "$repo/verse/gpu/vks.c" "$repo/verse/gpu/vkp.c" "$repo/coax/platform/process-native.c" "$repo/coax/platform/store-file-native.c" \
  "$work/wl/xdg-shell-protocol.c" "$work/wl/xdg-decoration-unstable-v1-protocol.c" "$work/wl/text-input-unstable-v3-protocol.c" \
  "$repo/psionic/build/parakeet/libpsionic-parakeet.a" \
  $(pkg-config --cflags --libs wayland-client wayland-cursor xkbcommon libsystemd) -lvulkan -ldl -lpthread -lm -o "$work/OpenAgents.new"
rm -rf "$work/fonts" "$work/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$work/"
cp "$repo/components/zeron/NOTICE" "$work/NOTICE"
mv "$work/OpenAgents.new" "$work/OpenAgents"
echo "built $work/OpenAgents"
# --install: a copy that does not need this checkout (~/.openagents/desktop/app/), the command
# ~/.openagents/bin/openagents-desktop and a launcher entry. A window that is open keeps its own copy.
if [[ "${1:-}" == --install ]]; then
  shift
  dest="$HOME/.openagents/desktop/app"
  mkdir -p "$dest.new" "$HOME/.openagents/bin" "$HOME/.local/share/applications"
  cp -R "$work/OpenAgents" "$work/zeron.spv" "$work/fonts" "$work/icons" "$work/NOTICE" "$dest.new/"
  rm -rf "$dest.old"; [[ -d "$dest" ]] && mv "$dest" "$dest.old"; mv "$dest.new" "$dest"; rm -rf "$dest.old"
  printf '#!/bin/sh\n# Firm reason: 5 (thin wrapper around the installed program)\nexec "$HOME/.openagents/desktop/app/OpenAgents" "$@"\n' > "$HOME/.openagents/bin/openagents-desktop"
  chmod 755 "$HOME/.openagents/bin/openagents-desktop"
  printf '[Desktop Entry]\nType=Application\nName=OpenAgents\nComment=Claude Code and Codex side by side\nExec=%s/.openagents/bin/openagents-desktop\nTerminal=false\nCategories=Development;\nStartupWMClass=com.openagents.desktop\n' "$HOME" > "$HOME/.local/share/applications/openagents-desktop.desktop"
  echo "installed $HOME/.openagents/bin/openagents-desktop"
fi
if [[ "${1:-}" == --open ]]; then shift; exec "$work/OpenAgents" "$@"; fi
