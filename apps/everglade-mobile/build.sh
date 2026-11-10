#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, metal, swiftc and simctl; holds no logic)
# Builds Everglade for the iOS simulator (apps/everglade-mobile/README.md): the same Coax as the
# desktop app (the scene, your model, the temple's parts and Aethyr shaders) with the iOS host.
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
rm -rf "$app"; mkdir -p "$work/c" "$app"
cc=(xcrun -sdk "$sdk" clang -target "$target" -std=c11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$desk")
# The scene alone (no Next issue panel: a phone has no agent), your model, and the temple's parts.
"$coax" c "$desk/workshop.coax" --library --header "$work/c/workshop.h" > "$work/c/workshop.c"
"$coax" c "$desk/model.coax" --library --header "$work/c/model.h" > "$work/c/model.c"
"$coax" c "$repo/everglade/workshop/temple.coax" --library --header "$work/c/temple.h" > "$work/c/temple.c"
for unit in workshop model temple; do "${cc[@]}" -c "$work/c/$unit.c" -o "$work/c/$unit.o"; done
for face in face model-face place-face; do "${cc[@]}" -c "$desk/$face.c" -o "$work/c/$face.o"; done
# The shaders: the strokes and your model's, and Aethyr's for the temple, emitted from Coax.
xcrun -sdk "$sdk" metal -w $min "$desk/Workshop.metal" "$desk/Model.metal" -o "$app/default.metallib"
for pair in sky scene light; do
  "$coax" metal "$repo/everglade/workshop/gpu/temple.coax" --vertex "temple_${pair}_vs" --fragment "temple_${pair}_fs" -o "$work/c/temple-$pair.metal" 2> "$work/c/temple-$pair.bind"
  xcrun -sdk "$sdk" metal -w $min "$work/c/temple-$pair.metal" -o "$app/temple-$pair.metallib"
done
xcrun -sdk "$sdk" swiftc -target "$target" -O -import-objc-header "$desk/face.h" "$here/ios/Sources/main.swift" "$desk/Sources/Engine.swift" "$desk/Sources/Faces.swift" \
  "$work/c/workshop.o" "$work/c/model.o" "$work/c/temple.o" "$work/c/face.o" "$work/c/model-face.o" "$work/c/place-face.o" \
  -framework UIKit -framework Metal -framework MetalKit -framework QuartzCore -framework CoreText -framework ImageIO -framework CoreGraphics -lm -o "$app/Everglade"
cp "$here/ios/Info.plist" "$app/Info.plist"
# The icon: actool makes the sizes iOS asks for from the one picture, and says what Info.plist needs.
xcrun actool "$here/ios/Assets.xcassets" --compile "$app" --platform "$sdk" --minimum-deployment-target 17.0 --app-icon AppIcon --output-partial-info-plist "$work/icon.plist" >/dev/null
/usr/libexec/PlistBuddy -c "Merge $work/icon.plist" "$app/Info.plist"
cp -R "$repo/apps/openagents-web/realms/everglade2/assets/fonts" "$app/fonts"
cp "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$app/everglade2.pack"
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
