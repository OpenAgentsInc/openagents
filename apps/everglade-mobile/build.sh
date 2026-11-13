#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, metal, swiftc and simctl; holds no logic)
# Builds Everglade for the iOS simulator (apps/everglade-mobile/README.md): the same Coax as the
# desktop app (the scene, your model, the temple's parts and Metaverse Engine shaders) with the iOS host, and
# the web's realms run natively as the Mac app runs them (Everglade itself and the Archery Range,
# everglade2; Experimental, fivelab) through the Metaverse Engine's page host (metaverse-engine/host/apple/).
#   apps/everglade-mobile/build.sh           builds build/ios/Everglade.app
#   apps/everglade-mobile/build.sh --open    builds, then installs and opens it in a simulator
#   apps/everglade-mobile/build.sh --device  builds build/ios-device/Everglade.app for a phone, unsigned
#                                            (ios/release.sh signs it and sends it to TestFlight)
#                                            (making an iPhone 17 Pro named "Everglade iPhone" if there is none)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
desk="$repo/apps/everglade-desktop"
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/build/ios"
target="arm64-apple-ios17.0-simulator"
sdk=iphonesimulator
min=-mios-simulator-version-min=17.0
if [[ "${1:-}" == --device ]]; then
  work="$here/build/ios-device"; target="arm64-apple-ios17.0"; sdk=iphoneos; min=-mios-version-min=17.0
fi
app="$work/Everglade.app"
rm -rf "$app"; mkdir -p "$work/c" "$app/Frameworks"
cc=(xcrun -sdk "$sdk" clang -target "$target" -std=c11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$desk")
# The worlds alone (scenes.coax: the list, the Workshop; no Next issue panel: a
# phone has no agent), your model, and the two places' parts.
"$coax" c "$desk/scenes.coax" --library --header "$work/c/workshop.h" > "$work/c/workshop.c"
"$coax" c "$desk/model.coax" --library --header "$work/c/model.h" > "$work/c/model.c"
"$coax" c "$repo/everglade/workshop/temple.coax" --library --header "$work/c/temple.h" > "$work/c/temple.c"
for unit in workshop model temple; do "${cc[@]}" -c "$work/c/$unit.c" -o "$work/c/$unit.o"; done
for face in face model-face place-face; do "${cc[@]}" -c "$desk/$face.c" -o "$work/c/$face.o"; done
# The shaders: the strokes and your model's, and the Metaverse Engine's for the temple, emitted from Coax.
xcrun -sdk "$sdk" metal -w $min "$desk/Workshop.metal" "$desk/Model.metal" -o "$app/default.metallib"
for pair in sky scene light; do
  "$coax" metal "$repo/everglade/workshop/gpu/temple.coax" --vertex "temple_${pair}_vs" --fragment "temple_${pair}_fs" -o "$work/c/temple-$pair.metal" 2> "$work/c/temple-$pair.bind"
  xcrun -sdk "$sdk" metal -w $min "$work/c/temple-$pair.metal" -o "$app/temple-$pair.metallib"
done
# The realms, as apps/everglade-desktop/build.sh builds them for the Mac. A realm's Coax as C is the
# same for the simulator and a phone, so it is emitted once (build/realm/NAME, over a minute each)
# and kept until its sources change; its objects are kept a platform. iOS loads code only from a
# framework, so each realm's library is Frameworks/NAME.framework. The face's imports (the socket,
# HTTP, the clock) are the page host's in the app, found when the library loads.
key="$( (shasum -a 256 "$coax" | cut -d ' ' -f 1; git -C "$repo" rev-parse HEAD:apps/openagents-web HEAD:metaverse-engine HEAD:metaverse HEAD:everglade HEAD:coax/lib; git -C "$repo" status --porcelain -- apps/openagents-web metaverse-engine metaverse everglade coax/lib ':(exclude)metaverse-engine/host/apple'; git -C "$repo" diff HEAD -- apps/openagents-web metaverse-engine metaverse everglade coax/lib ':(exclude)metaverse-engine/host/apple') | shasum -a 256 | cut -d ' ' -f 1)"
build_realm() {   # NAME: Frameworks/NAME.framework
  local name="$1" realm="$here/build/realm/$1" objects="$work/realm/$1" framework="$app/Frameworks/$1.framework"
  mkdir -p "$realm" "$objects" "$framework" "$app/$name"
  if [[ "$(cat "$realm/key" 2>/dev/null)" != "$key" || ! -f "$realm/sources" ]]; then
    rm -f "$realm/key"
    COAX="$coax" "$repo/metaverse/native/build.sh" "$name" "$realm" >/dev/null
    echo "$key" > "$realm/key"
  fi
  if [[ "$(cat "$objects/key" 2>/dev/null)" != "$key" || ! -f "$objects/$name" ]]; then
    rm -f "$objects/key"
    local realm_sources realm_objects=() realm_pids=() s pid
    read -r -a realm_sources < "$realm/sources"
    for s in "${realm_sources[@]}"; do
      xcrun -sdk "$sdk" clang -target "$target" -std=gnu11 -O2 -ffp-contract=off -fno-fast-math -DCOAXIAL_NATIVE_ADDRESS -w -I"$realm" -c "$realm/$s" -o "$objects/${s%.c}.o" & realm_pids+=($!); realm_objects+=("$objects/${s%.c}.o")
    done
    for pid in "${realm_pids[@]}"; do wait "$pid"; done
    xcrun -sdk "$sdk" clang -target "$target" -dynamiclib -install_name "@rpath/$name.framework/$name" -Wl,-undefined,dynamic_lookup -w "${realm_objects[@]}" -lm -o "$objects/$name"
    echo "$key" > "$objects/key"
  fi
  cp "$objects/$name" "$framework/$name"
  /usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string com.openagents.everglade.$name" -c "Add :CFBundleName string $name" -c "Add :CFBundleExecutable string $name" \
    -c "Add :CFBundlePackageType string FMWK" -c "Add :CFBundleShortVersionString string 1.0" -c "Add :CFBundleVersion string ${EVERGLADE_BUILD_NUMBER:-1}" \
    -c "Add :CFBundleInfoDictionaryVersion string 6.0" -c "Add :MinimumOSVersion string 17.0" \
    -c "Add :CFBundleSupportedPlatforms array" -c "Add :CFBundleSupportedPlatforms:0 string $([[ "$sdk" == iphoneos ]] && echo iPhoneOS || echo iPhoneSimulator)" "$framework/Info.plist" >/dev/null
  [[ "$sdk" == iphoneos ]] && plutil -convert binary1 "$framework/Info.plist"
  return 0
}
# A realm's shaders: the web's own Coax (apps/openagents-web/gpu/), emitted for Metal a pair at a
# time, each with the list of what it reads (.bind), in Metal's relaxed math mode as on the Mac.
emit_pair() {   # REALM ROLE SOURCE VERTEX FRAGMENT
  "$coax" metal "$("$repo/metaverse/native/build.sh" --gpu "$3")" --vertex "$4" --fragment "$5" -o "$work/c/$1-$2.metal" 2> "$app/$1/$2.bind"
  xcrun -sdk "$sdk" metal -w $min -fmetal-math-mode=relaxed -fmetal-math-fp32-functions=precise "$work/c/$1-$2.metal" -o "$app/$1/$2.metallib"
}
emit_page() {   # REALM SOURCE PREFIX FX FXPREFIX
  local pids=() pid
  emit_pair "$1" bg "$2" "$3_bg_vs" "$3_bg_fs" & pids+=($!)
  emit_pair "$1" shadow "$2" "$3_shadow_vs" "$3_shadow_fs" & pids+=($!)
  emit_pair "$1" scene "$2" "$3_scene_vs" "$3_scene_fs" & pids+=($!)
  emit_pair "$1" bloom "$2" "$3_bg_vs" "$3_bloom_fs" & pids+=($!)
  for pid in "${pids[@]}"; do wait "$pid"; done; pids=()
  emit_pair "$1" post "$2" "$3_bg_vs" "$3_post_fs" & pids+=($!)
  emit_pair "$1" fx "$2" "$3_fx_vs" "$3_fx_fs" & pids+=($!)
  emit_pair "$1" particle "$4" "$5_particle_vs" "$5_particle_fs" & pids+=($!)
  emit_pair "$1" particle_over "$4" "$5_particle_over_vs" "$5_particle_over_fs" & pids+=($!)
  for pid in "${pids[@]}"; do wait "$pid"; done
  local passes; passes="$("$repo/metaverse/native/build.sh" --gpu "$2")"
  cp "${passes%.coax}.bindings" "$app/$1/page.bindings"
  cp "$work/c/page-host.metallib" "$app/$1/page-host.metallib"
}
xcrun -sdk "$sdk" metal -w $min "$repo/metaverse-engine/host/apple/PageHost.metal" -o "$work/c/page-host.metallib"
build_realm everglade2
emit_page everglade2 everglade2 everglade2 everglade2-fx e2fx
build_realm fivelab
emit_page fivelab charactercreator charactercreator fivelab-fx fxg
cp "$repo/apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz" "$app/fivelab/glade.pack.gz"
# The Bitcoin Lab: its own module (apps/openagents-web/old/labs/bitcoin-lab-walk.coax), the same passes with its
# Grid and stations in the particle pass (bitcoin-lab-fx), Experimental's pack.
build_realm bitcoin-lab-walk
emit_page bitcoin-lab-walk charactercreator charactercreator bitcoin-lab-fx qfx
# The host core's face (metaverse-engine/host/core/core-face.coax, #13304) as C: the one pack reader, a paged
# block build read whole, a block layer's levels, the format this GPU takes and the frame plan's records.
# PageHost.m is built on the core over it (metaverse-engine/host/core/host-core.h); Engine.swift finds a pack's
# entries through the reader (pack-table.h, which includes the face under its old name).
"$coax" c "$repo/metaverse-engine/host/core/core-face.coax" --library --header "$work/c/engine-core-face.h" > "$work/c/engine-core-face.c"
# (Its SHA-256 helpers are also native-audio's: this unit keeps its own names for them.)
"${cc[@]}" -w -DS_hash=engine_core_S_hash -DS_hex=engine_core_S_hex -c "$work/c/engine-core-face.c" -o "$work/c/engine-core-face.o"
printf '#include "engine-core-face.h"\n' > "$work/c/engine-pack-table.h"
xcrun -sdk "$sdk" clang -target "$target" -fobjc-arc -fmodules -O2 -Wno-shadow-ivar -I"$work/c" -c "$repo/metaverse-engine/host/apple/PageHost.m" -o "$work/c/page-host.o"
# A phone's tier for the pack's layers is Coax's (metaverse-engine/render/texture-format.coax tf_side), which
# the core's face carries.
printf '#include "engine-core-face.h"\n' > "$work/c/texture-format.h"
# The realms' sound, as on the Mac: the Metaverse Engine's Apple audio host (metaverse-engine/host/apple/audio-host.h,
# PageAudio.swift and the desktop app's MetalAudio.swift) over the shared Coax audio authorities,
# native-audio.coax as C, and the site's sound files a page names with their PCM companions.
"$coax" c "$repo/metaverse-engine/host/native-audio.coax" --library --header "$work/c/native-audio.h" > "$work/c/native-audio.c"
"${cc[@]}" -c "$work/c/native-audio.c" -o "$work/c/native-audio.o"
printf '#include "native-audio.h"\n#include "%s"\n#include "%s"\n' "$repo/metaverse-engine/host/apple/audio-host.h" "$repo/metaverse-engine/host/apple/audio-host.c" > "$work/c/audio-host-unit.c"
"${cc[@]}" -c "$work/c/audio-host-unit.c" -o "$work/c/audio-host.o"
# The pictures the realms name (the classes' and spells' icons), where the site has them.
mkdir -p "$app/web/realms/everglade" "$app/web/realms/5e-mmo"
cp -R "$repo/apps/openagents-web/realms/everglade/hotbar" "$repo/apps/openagents-web/realms/everglade/sfx" "$repo/apps/openagents-web/realms/everglade/pcm-v1" "$app/web/realms/everglade/"
cp -R "$repo/apps/openagents-web/realms/5e-mmo/hotbar" "$app/web/realms/5e-mmo/"
printf '#include "%s"\n#import "%s"\n#include "%s"\n#include "%s"\n#include "%s"\n' "$desk/face.h" "$repo/metaverse-engine/host/apple/page-host.h" "$work/c/texture-format.h" "$repo/metaverse-engine/host/apple/audio-host.h" "$repo/metaverse-engine/host/pack-table.h" > "$work/c/bridge.h"
xcrun -sdk "$sdk" swiftc -target "$target" -O -Xcc -I"$work/c" -import-objc-header "$work/c/bridge.h" "$here"/ios/Sources/*.swift "$desk/Sources/Engine.swift" "$desk/Sources/Faces.swift" "$desk/Sources/Realms.swift" "$repo/metaverse-engine/host/apple/PageAudio.swift" "$repo/apps/metaverse-engine-desktop/Sources/MetalAudio.swift" \
  "$work/c/audio-host.o" "$work/c/native-audio.o" "$work/c/workshop.o" "$work/c/model.o" "$work/c/temple.o" "$work/c/face.o" "$work/c/model-face.o" "$work/c/place-face.o" "$work/c/page-host.o" "$work/c/engine-core-face.o" \
  -framework UIKit -framework Metal -framework MetalKit -framework QuartzCore -framework CoreText -framework ImageIO -framework CoreGraphics -framework CoreMotion -framework AVFoundation -lm -lz \
  -Xlinker -export_dynamic -Xlinker -rpath -Xlinker @executable_path/Frameworks -o "$app/Everglade"
cp "$here/ios/Info.plist" "$app/Info.plist"
# The icon: actool makes the sizes iOS asks for from the one picture, and says what Info.plist needs.
xcrun actool "$here/ios/Assets.xcassets" --compile "$app" --platform "$sdk" --minimum-deployment-target 17.0 --app-icon AppIcon --output-partial-info-plist "$work/icon.plist" >/dev/null
/usr/libexec/PlistBuddy -c "Merge $work/icon.plist" "$app/Info.plist"
cp -R "$repo/apps/openagents-web/realms/everglade2/assets/fonts" "$app/fonts"
cp "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$app/everglade2.pack"
# Everglade II's layers as GPU blocks (#13305): the pack's ASTC 4x4 block build beside it, which the
# page host loads in place of the pack and uploads with no decode. The encode is kept by the pack's hash.
blocks="$work/blocks/everglade2-$(shasum -a 256 "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" | cut -c1-16).astc4.vpk3"
mkdir -p "$work/blocks"
[[ -f "$blocks" ]] || "$repo/coax/web/tool.sh" "$repo/metaverse-engine/tools/block-pack.coax" "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$blocks" --format astc4 >/dev/null
cp "$blocks" "$app/everglade2.astc4.vpk3"
if [[ "$sdk" == iphoneos ]]; then
  # What the App Store asks every upload to say about how it was built.
  put() { /usr/libexec/PlistBuddy -c "Add :$1 string $2" "$app/Info.plist"; }
  put DTPlatformName iphoneos
  put DTPlatformVersion "$(xcrun --sdk iphoneos --show-sdk-platform-version)"
  put DTSDKName "iphoneos$(xcrun --sdk iphoneos --show-sdk-version)"
  put DTSDKBuild "$(xcrun --sdk iphoneos --show-sdk-build-version)"
  put DTPlatformBuild "$(xcrun --sdk iphoneos --show-sdk-build-version)"
  put DTXcode "$(xcodebuild -version | awk 'NR == 1 { split($2, v, "."); printf "%02d%d%d", v[1], v[2], v[3] }')"
  put DTXcodeBuild "$(xcodebuild -version | awk 'NR == 2 { print $3 }')"
  put DTCompiler com.apple.compilers.llvm.clang.1_0
  put BuildMachineOSBuild "$(sw_vers -buildVersion)"
  /usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms array" -c "Add :CFBundleSupportedPlatforms:0 string iPhoneOS" "$app/Info.plist"
  /usr/libexec/PlistBuddy -c "Add :UIRequiredDeviceCapabilities array" -c "Add :UIRequiredDeviceCapabilities:0 string arm64" -c "Add :UIRequiredDeviceCapabilities:1 string metal" "$app/Info.plist"
  [[ -n "${EVERGLADE_BUILD_NUMBER:-}" ]] && /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $EVERGLADE_BUILD_NUMBER" "$app/Info.plist"
  plutil -convert binary1 "$app/Info.plist"
else
  for f in "$app"/Frameworks/*.framework; do codesign --force --sign - "$f" >/dev/null; done
  codesign --force --sign - "$app" >/dev/null
fi
echo "built $app"
if [[ "${1:-}" == --open ]]; then
  # A simulator that is already up, else the first there is, else a new one.
  device="$(xcrun simctl list devices booted -j | python3 -I -c 'import sys,json; d=[x["udid"] for v in json.load(sys.stdin)["devices"].values() for x in v]; print(d[0] if d else "")')"
  if [[ -z "$device" ]]; then
    device="$(xcrun simctl list devices available -j | python3 -I -c 'import sys,json; d=[x["udid"] for v in json.load(sys.stdin)["devices"].values() for x in v]; print(d[0] if d else "")')"
    if [[ -z "$device" ]]; then
      runtime="$(xcrun simctl list runtimes -j | python3 -I -c 'import sys,json; r=[x["identifier"] for x in json.load(sys.stdin)["runtimes"] if x.get("isAvailable") and x.get("platform","iOS")=="iOS"]; print(r[-1] if r else "")')"
      [[ -n "$runtime" ]] || { echo "build.sh: no iOS simulator runtime is installed (Xcode > Settings > Components)" >&2; exit 1; }
      device="$(xcrun simctl create "Everglade iPhone" com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro "$runtime")"
    fi
    xcrun simctl boot "$device"
  fi
  open -a Simulator --args -CurrentDeviceUDID "$device"
  xcrun simctl bootstatus "$device" >/dev/null
  xcrun simctl install "$device" "$app"
  xcrun simctl launch "$device" com.openagents.everglade
  echo "open in simulator $device"
fi
exit 0
