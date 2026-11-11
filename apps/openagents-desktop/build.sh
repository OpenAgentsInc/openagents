#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang and xcrun; holds no logic)
# Builds OpenAgents.app for this Mac (docs/desktop/coax-desktop.md, GitHub #13156): the
# Zeron-fork chat screen (zeron/screen.coax, zeron/desk.coax) compiled to C behind zeron-face.c,
# on the shared Mac host (apps/shared/host/apple/ui-host.m, the ui-host.h contract the Linux
# app keeps) with the same linux/main.c and the chat component it fills the window with (linux/chat.c), CoreText's atlas (apps/shared/host/apple/ct-atlas.m),
# the component shader (components/backend/gpu-shader.coax) emitted as Metal, and dictation
# (zeron/dictation-linux.coax's rules, linux/dictation-host.c, CoreAudio through
# apps/openagents-cli/dictate-mac.m and Psionic's resident Parakeet on Metal).
#   apps/openagents-desktop/build.sh [--open]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/build"
app="$work/OpenAgents.app"
mkdir -p "$work/c" "$app/Contents/MacOS" "$app/Contents/Resources"
cflags=(-O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$here" -I"$repo/apps/shared/host/linux" -I"$repo/apps/shared/host/apple" -I"$repo/coax/platform" -I"$repo/psionic/platform")
# The app's Coax (the Linux app's unit: settings.coax and what it loads, and the Coax atlas, which the Mac does not use).
"$coax" c "$here/zeron/linux.coax" --library --header "$work/c/zeron.h" > "$work/c/zeron.c"
# The app's unit and Parakeet's both export Coax's SHA-256 (S_hash, S_hex): the app's copy gets names of its own.
/usr/bin/clang -std=gnu11 "${cflags[@]}" -DS_hash=oa_zeron_S_hash -DS_hex=oa_zeron_S_hex -c "$work/c/zeron.c" -o "$work/c/zeron.o"
# The Lab's NIP-98 signer for our inference route, a unit of its own (lab-sign.coax); its crypto's checks need the larger step budget.
"$coax" c "$here/lab-sign.coax" --run-steps 250000000 --library --header "$work/c/lab-sign.h" > "$work/c/lab-sign.c"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -DS_hash=oa_lab_S_hash -DS_hex=oa_lab_S_hex -c "$work/c/lab-sign.c" -o "$work/c/lab-sign-all.o"
# It carries its own copy of the hashes the app's unit also exports: only its one entry point stays visible (objcopy on Linux).
ld -r "$work/c/lab-sign-all.o" -exported_symbol _coaxial_lab_nip98 -o "$work/c/lab-sign.o"
# Dictation: Psionic's resident Parakeet (built when its inputs changed), and the shared controller with the host's rules as on Linux.
COAX="$coax" "$repo/coax/web/tool.sh" "$here/dictation-build.coax" "$repo" "$work/c/parakeet" > /dev/null
"$coax" c "$here/zeron/dictation-linux.coax" --release --library --header "$work/c/dictation.h" --run-steps 250000000 > "$work/c/dictation.c"
# The component shader (components/backend/gpu-shader.coax) emitted as Metal, the one shader /chat (WGSL) and Linux (SPIR-V) draw with.
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/c/gpu-shader.metal" 2> "$work/c/gpu-shader.bind"
xcrun -sdk macosx metal "$work/c/gpu-shader.metal" -o "$app/Contents/Resources/zeron.metallib"
objc=()
for m in "$repo/apps/shared/host/apple/ui-host.m" "$repo/apps/shared/host/apple/ct-atlas.m" "$here/dictation-mac.m" "$repo/apps/openagents-cli/dictate-mac.m"; do
  /usr/bin/clang -fobjc-arc "${cflags[@]}" -c "$m" -o "$work/c/$(basename "$m" .m).o"; objc+=("$work/c/$(basename "$m" .m).o")
done
/usr/bin/clang -std=gnu11 "${cflags[@]}" \
  "$here/linux/main.c" "$here/linux/chat.c" "$here/linux/dictation-host.c" "$here/dictation-face.c" "$here/zeron-face.c" "$work/c/dictation.c" \
  "$repo/apps/openagents-cli/dictate-native.c" "$repo/coax/platform/process-native.c" "$repo/coax/platform/store-file-native.c" \
  "$work/c/zeron.o" "$work/c/lab-sign.o" "${objc[@]}" "$repo/psionic/build/parakeet/libpsionic-parakeet.a" \
  -framework AppKit -framework Metal -framework QuartzCore -framework CoreText -framework ImageIO -framework UniformTypeIdentifiers \
  -framework AVFoundation -framework CoreAudio -framework AudioToolbox -framework Foundation -lpthread -lm -o "$app/Contents/MacOS/OpenAgents.new"
mv "$app/Contents/MacOS/OpenAgents.new" "$app/Contents/MacOS/OpenAgents"
cp "$here/Info.plist" "$app/Contents/Info.plist"
rm -rf "$app/Contents/Resources/fonts" "$app/Contents/Resources/icons" "$app/Contents/Resources/default.metallib"
# Geist, its OFL, Zeron's icons and the NOTICE live beside the shared components (components/zeron/).
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$app/Contents/Resources/"
cp "$repo/components/zeron/NOTICE" "$app/Contents/Resources/NOTICE"
codesign --force --sign - "$app" >/dev/null
echo "built $app"
[[ "${1:-}" == --open ]] && open "$app"
exit 0
