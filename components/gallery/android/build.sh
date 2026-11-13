#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, the NDK's clang and adb; holds no logic)
# Builds the component library as an Android program (arm64) on the shared Android host
# (apps/shared/host/android/ui-host.c, the ui-host.h contract): the same main.c and Coax as
# components/gallery/linux/, the atlas built in Coax, the component shader emitted as SPIR-V.
#   components/gallery/android/build.sh            build components/gallery/build/android/
#   components/gallery/android/build.sh --shot OUT.png [ARGS...]
#                                                  build, run it on the device or emulator adb
#                                                  sees (in /data/local/tmp) and bring the picture back
#   components/gallery/android/build.sh --apk      also build Components.apk: the same program as a
#                                                  NativeActivity (activity.c) drawing in its window
#   components/gallery/android/build.sh --install  build the APK, install it on the device or emulator
#                                                  adb sees and start it
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
"$llvm/bin/aarch64-linux-android30-clang" -std=gnu11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$repo/apps/shared/host/linux" -I"$repo/metaverse/gpu" \
  "$here/../linux/main.c" "$work/c/gallery.c" "$repo/apps/shared/host/android/ui-host.c" "$repo/metaverse/gpu/vkc.c" "$repo/metaverse/gpu/vks.c" \
  -lvulkan -lm -o "$work/Components.new"
rm -rf "$work/fonts" "$work/icons"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$work/"
mv "$work/Components.new" "$work/Components"
echo "built $work/Components"
if [[ "${1:-}" == --apk || "${1:-}" == --install ]]; then
  tools="$sdk/build-tools/35.0.0"; platform="$sdk/platforms/android-35/android.jar"
  glue="$ndk/sources/android/native_app_glue"
  mkdir -p "$work/apk/lib/arm64-v8a" "$work/apk/assets"
  "$llvm/bin/aarch64-linux-android30-clang" -std=gnu11 -O2 -fPIC -shared -ffp-contract=off -fno-fast-math -w -DUI_ANDROID_WINDOW -Dmain=gallery_main \
    -I"$work/c" -I"$repo/apps/shared/host/linux" -I"$repo/apps/shared/host/android" -I"$repo/metaverse/gpu" -I"$glue" \
    "$here/activity.c" "$here/../linux/main.c" "$work/c/gallery.c" "$repo/apps/shared/host/android/ui-host.c" \
    "$repo/metaverse/gpu/vkc.c" "$repo/metaverse/gpu/vks.c" "$repo/metaverse/gpu/vkp.c" "$glue/android_native_app_glue.c" \
    -u ANativeActivity_onCreate -lvulkan -landroid -llog -lm -o "$work/apk/lib/arm64-v8a/libcomponents.so"
  rm -rf "$work/apk/assets/fonts" "$work/apk/assets/icons"
  cp -R "$work/fonts" "$work/icons" "$work/apk/assets/"; cp "$work/components.spv" "$work/apk/assets/"
  "$tools/aapt2" link -o "$work/unsigned.apk" -I "$platform" --manifest "$here/AndroidManifest.xml" -A "$work/apk/assets" --min-sdk-version 30 --target-sdk-version 35
  (cd "$work/apk" && zip -q -r "$work/unsigned.apk" lib)
  "$tools/zipalign" -f -p 4 "$work/unsigned.apk" "$work/aligned.apk"
  key="$HOME/.android/debug.keystore"
  [[ -f "$key" ]] || { echo "build.sh: no debug keystore at $key (apps/everglade-mobile/android/build.sh makes one)" >&2; exit 1; }
  "$tools/apksigner" sign --ks "$key" --ks-pass pass:android --key-pass pass:android --out "$work/Components.apk" "$work/aligned.apk" 2>/dev/null
  echo "built $work/Components.apk"
  if [[ "$1" == --install ]]; then
    adb="$sdk/platform-tools/adb"
    "$adb" install -r "$work/Components.apk" >/dev/null
    "$adb" shell am start -n com.openagents.components/android.app.NativeActivity >/dev/null
    echo "started on $("$adb" devices | awk 'NR > 1 && $2 == "device" { print $1; exit }')"
  fi
fi
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
