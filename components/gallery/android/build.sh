#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, the NDK's clang and adb; holds no logic)
# Builds the component library as an Android program (arm64) on the shared Android host
# (apps/shared/host/android/ui-host.c, the ui-host.h contract): the same main.c and Coax as
# components/gallery/linux/, the atlas built in Coax, the component shader emitted as SPIR-V.
# It draws offscreen only (the host has no window yet).
#   components/gallery/android/build.sh            build components/gallery/build/android/
#   components/gallery/android/build.sh --shot OUT.png [ARGS...]
#                                                  build, run it on the device or emulator adb
#                                                  sees (in /data/local/tmp) and bring the picture back
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
[[ -d "$sdk/ndk" ]] || { echo "build.sh: no Android NDK under $sdk/ndk" >&2; exit 1; }
ndk="${ANDROID_NDK_HOME:-$sdk/ndk/$(ls "$sdk/ndk" | sort -V | tail -1)}"
llvm="$(echo "$ndk"/toolchains/llvm/prebuilt/*)"
work="$here/../build/android"
mkdir -p "$work/c"
"$coax" c "$here/../linux.coax" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" spirv "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/components.spv" 2> "$work/components.spv.bindings"
"$llvm/bin/aarch64-linux-android30-clang" -std=gnu11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux" -I"$repo/verse/gpu" \
  "$here/../linux/main.c" "$work/c/gallery.c" "$repo/apps/shared/host/android/ui-host.c" "$repo/verse/gpu/vkc.c" "$repo/verse/gpu/vks.c" \
  -lvulkan -lm -o "$work/Components.new"
rm -rf "$work/fonts" "$work/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$work/"
mv "$work/Components.new" "$work/Components"
echo "built $work/Components"
if [[ "${1:-}" == --shot ]]; then
  out="${2:?usage: build.sh --shot OUT.png [ARGS...]}"; shift 2
  adb="$sdk/platform-tools/adb"
  dir=/data/local/tmp/oa-components
  "$adb" shell mkdir -p "$dir"
  "$adb" push -q "$work/Components" "$work/components.spv" "$work/fonts" "$work/icons" "$dir/" >/dev/null
  "$adb" shell "cd $dir && ./Components --shot $dir/shot.png $*"
  "$adb" pull -q "$dir/shot.png" "$out" >/dev/null
  echo "wrote $out"
fi
