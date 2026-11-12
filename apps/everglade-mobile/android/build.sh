#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, the NDK's clang and glslc, aapt2, apksigner and adb; holds no logic)
# Builds Everglade for Android (apps/everglade-mobile/README.md): the same Coax as the desktop and
# iOS apps (the scene, your model, the temple's parts and Aethyr shaders) with the Vulkan host.
#   apps/everglade-mobile/android/build.sh           builds build/android/everglade.apk (arm64-v8a)
#   apps/everglade-mobile/android/build.sh --open    builds, then installs and opens it on the
#                                                    device or emulator adb sees (starting an emulator
#                                                    named "everglade_api35", made if there is none)
# Needs the Android SDK (platform android-35, build-tools 35.0.0, an NDK) and, for the faces'
# atlas, a Mac: CoreText draws it at build time (bake-faces).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
desk="$repo/apps/everglade-desktop"
coax="${COAX:-$repo/coax/bin/coax}"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
[[ -d "$sdk/ndk" ]] || { echo "build.sh: no Android NDK under $sdk/ndk (set ANDROID_HOME, or install one with sdkmanager 'ndk;27.1.12297006')" >&2; exit 1; }
ndk="${ANDROID_NDK_HOME:-$sdk/ndk/$(ls "$sdk/ndk" | sort -V | tail -1)}"
tools="$sdk/build-tools/35.0.0"
platform="$sdk/platforms/android-35/android.jar"
[[ -x "$tools/aapt2" && -f "$platform" ]] || { echo "build.sh: needs build-tools 35.0.0 and platform android-35 (sdkmanager 'build-tools;35.0.0' 'platforms;android-35')" >&2; exit 1; }
llvm="$(echo "$ndk"/toolchains/llvm/prebuilt/*)"
glslc="$(echo "$ndk"/shader-tools/*/glslc)"
work="$here/../build/android"
rm -rf "$work/apk"; mkdir -p "$work/c" "$work/apk/lib/arm64-v8a" "$work/apk/assets"
cc=("$llvm/bin/aarch64-linux-android30-clang" -std=c11 -O2 -fPIC -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$desk" -I"$ndk/sources/android/native_app_glue")
# The worlds alone (scenes.coax: the list, the Workshop; no Next issue panel: a
# phone has no agent), your model, and the two places' parts.
"$coax" c "$desk/scenes.coax" --library --header "$work/c/workshop.h" > "$work/c/workshop.c"
"$coax" c "$desk/model.coax" --library --header "$work/c/model.h" > "$work/c/model.c"
"$coax" c "$repo/everglade/workshop/temple.coax" --library --header "$work/c/temple.h" > "$work/c/temple.c"
objects=()
for unit in workshop model temple; do "${cc[@]}" -c "$work/c/$unit.c" -o "$work/c/$unit.o"; objects+=("$work/c/$unit.o"); done
for face in face model-face place-face; do "${cc[@]}" -c "$desk/$face.c" -o "$work/c/$face.o"; objects+=("$work/c/$face.o"); done
"${cc[@]}" -c "$here/host.c" -o "$work/c/host.o"
# Your account (#13229): the login's flow and NIP-98 signing (app-login-unit.coax, its own unit:
# secp256k1's tables take long to build), app-login.c's HTTPS, Keystore and files, and the
# Keystore device key the login is bound to (#13216; host.c runs its self-check when
# `adb shell setprop debug.oa.devicekey 1` is set).
"$coax" c "$repo/apps/shared/host/android/app-login-unit.coax" --run-steps 250000000 --library --header "$work/c/app-login-unit.h" > "$work/c/app-login-unit.c"
"${cc[@]}" -c "$work/c/app-login-unit.c" -o "$work/c/app-login-unit.o"
# Its Coax shares library code with the scene's; only alu_* is its own, the rest stays inside it.
"$llvm/bin/llvm-objcopy" --wildcard --keep-global-symbol='alu_*' "$work/c/app-login-unit.o"; objects+=("$work/c/app-login-unit.o")
for piece in device-key app-login; do "${cc[@]}" -c "$repo/apps/shared/host/android/$piece.c" -o "$work/c/$piece.o"; objects+=("$work/c/$piece.o"); done
"${cc[@]}" -c "$ndk/sources/android/native_app_glue/android_native_app_glue.c" -o "$work/c/glue.o"
# The glue's entry point is only named in the manifest, so the linker is told to keep it.
"${cc[@]}" -shared -u ANativeActivity_onCreate "$work/c/host.o" "$work/c/glue.o" "${objects[@]}" \
  -landroid -llog -lvulkan -ljnigraphics -lm -o "$work/apk/lib/arm64-v8a/libeverglade.so"
"$llvm/bin/llvm-strip" --strip-unneeded "$work/apk/lib/arm64-v8a/libeverglade.so"
# The shaders: Aethyr's for the temple, emitted from Coax, and the strokes and your model's.
for pair in sky scene light; do
  "$coax" spirv "$repo/everglade/workshop/gpu/temple.coax" --vertex "temple_${pair}_vs" --fragment "temple_${pair}_fs" -o "$work/apk/assets/temple-$pair.spv" 2> "$work/c/temple-$pair.spv.bind"
done
for stage in strokes.vert strokes.frag model.vert model.frag; do "$glslc" -O "$here/shaders/$stage" -o "$work/apk/assets/$stage.spv"; done
# The faces' atlas, drawn here by CoreText, since the phone has none.
xcrun -sdk macosx swiftc -O "$here/bake-faces/main.swift" "$desk/Sources/Faces.swift" -o "$work/bake-faces"
"$work/bake-faces" "$repo/apps/openagents-web/realms/everglade2/assets/fonts" "$work/apk/assets/faces.bin"
cp "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$work/apk/assets/everglade2.pack"
# The package: the manifest, then the library and the files (stored, not squeezed, so the app maps them).
"$tools/aapt2" compile --dir "$here/res" -o "$work/res.zip"
"$tools/aapt2" link -o "$work/unsigned.apk" -I "$platform" "$work/res.zip" --manifest "$here/AndroidManifest.xml" --min-sdk-version 30 --target-sdk-version 35
(cd "$work/apk" && zip -q -0 -r "$work/unsigned.apk" lib assets)
"$tools/zipalign" -f -p 4 "$work/unsigned.apk" "$work/aligned.apk"
# A debug key, as Android Studio makes; a release needs the owner's.
key="$HOME/.android/debug.keystore"
[[ -f "$key" ]] || { mkdir -p "$HOME/.android"; keytool -genkeypair -keystore "$key" -storepass android -keypass android -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US" >/dev/null 2>&1; }
"$tools/apksigner" sign --ks "$key" --ks-pass pass:android --key-pass pass:android --out "$work/everglade.apk" "$work/aligned.apk" 2>/dev/null
rm -f "$work/unsigned.apk" "$work/aligned.apk" "$work/everglade.apk.idsig"
echo "built $work/everglade.apk"
if [[ "${1:-}" == --open ]]; then
  adb="$sdk/platform-tools/adb"
  # A device or emulator that is already up, else the app's own emulator.
  if ! "$adb" devices | awk 'NR > 1 && $2 == "device"' | grep -q .; then
    if ! "$sdk/emulator/emulator" -list-avds | grep -qx everglade_api35; then
      image="system-images;android-35;google_apis;arm64-v8a"
      [[ -d "$sdk/system-images/android-35/google_apis/arm64-v8a" ]] || { echo "build.sh: no Android 15 system image (sdkmanager '$image')" >&2; exit 1; }
      echo no | "$sdk/cmdline-tools/latest/bin/avdmanager" create avd -n everglade_api35 -k "$image" -d pixel_8 >/dev/null
    fi
    nohup "$sdk/emulator/emulator" -avd everglade_api35 -gpu host -no-snapshot -no-boot-anim > "$work/emulator.log" 2>&1 &
    echo "starting emulator everglade_api35 (pid $!)"
    "$adb" wait-for-device
    until [[ "$("$adb" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; do sleep 2; done
  fi
  "$adb" install -r "$work/everglade.apk" >/dev/null
  "$adb" shell am start -n com.openagents.everglade/android.app.NativeActivity >/dev/null
  # A freshly booted emulator can end an app it has just installed; open it again if so.
  sleep 4
  "$adb" shell pidof com.openagents.everglade >/dev/null || "$adb" shell am start -n com.openagents.everglade/android.app.NativeActivity >/dev/null
  echo "open on $("$adb" devices | awk 'NR > 1 && $2 == "device" { print $1; exit }')"
fi
exit 0
