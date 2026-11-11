#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh, codesign and altool; holds no logic)
# Builds Everglade for iPhone, signs it for the App Store and uploads it to TestFlight
# (App Store Connect app "Everglade", com.openagents.everglade, team HQWSG26L43).
#   apps/everglade-mobile/ios/release.sh BUILD_NUMBER
# BUILD_NUMBER must be higher than every build already uploaded for this version.
# Needs ~/work/.secrets/appstoreconnect.env (ASC_API_KEY_ID, ASC_API_ISSUER_ID,
# ASC_API_PRIVATE_KEY_PATH) and the Apple Distribution identity in the login keychain.
# After the upload Apple processes the build; `release.sh status BUILD_NUMBER` says where it is.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
tool="$repo/apps/openagents-ios/tools/app_store.py"
bundle=com.openagents.everglade
team=HQWSG26L43
if [[ -z "${ASC_API_KEY_ID:-}" ]]; then set -a; source "$HOME/work/.secrets/appstoreconnect.env"; set +a; fi
: "${ASC_API_KEY_ID:?}" "${ASC_API_ISSUER_ID:?}" "${ASC_API_PRIVATE_KEY_PATH:?}"
# The OpenAgents app's App Store tool, pointed at this app.
asc() { python3 -I -c 'import sys; sys.path.insert(0, sys.argv[1]); import app_store; app_store.BUNDLE = sys.argv[2]; sys.argv = ["app_store"] + sys.argv[3:]; app_store.main()' "$(dirname "$tool")" "$bundle" "$@"; }
version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$here/Info.plist")"
if [[ "${1:-}" == status ]]; then asc status --build "${2:?usage: release.sh status BUILD_NUMBER}" --version "$version"; exit 0; fi
number="${1:?usage: release.sh BUILD_NUMBER}"
work="$here/../build/ios-device"
EVERGLADE_BUILD_NUMBER="$number" "$here/../build.sh" --device
app="$work/Everglade.app"
cert="${OPENAGENTS_CERT_ID:-GCN7Q59Y27}"
sha1="$(asc cert-sha1 "$cert")"
security find-identity -v -p codesigning | /usr/bin/grep -q "$sha1" || { echo "release.sh: the Apple Distribution identity for certificate $cert is not in the keychain" >&2; exit 1; }
asc profile --cert "$cert" --name "Everglade App Store" --out "$work/Everglade.mobileprovision"
cp "$work/Everglade.mobileprovision" "$app/embedded.mobileprovision"
# The entitlements are the profile's own, which is what the App Store checks the signature against.
security cms -D -i "$work/Everglade.mobileprovision" > "$work/profile.plist"
plutil -extract Entitlements xml1 -o "$work/entitlements.plist" "$work/profile.plist"
# The realms' libraries are frameworks inside the app: each is signed before the app that holds it.
for f in "$app"/Frameworks/*.framework; do codesign --force --sign "$sha1" --generate-entitlement-der --timestamp "$f"; done
codesign --force --sign "$sha1" --entitlements "$work/entitlements.plist" --generate-entitlement-der --timestamp "$app"
codesign --verify --strict "$app"
rm -rf "$work/Payload" "$work/Everglade.ipa"; mkdir -p "$work/Payload"; cp -R "$app" "$work/Payload/"
(cd "$work" && zip -q -r -y Everglade.ipa Payload)
keys="$(mktemp -d)"; trap 'rm -rf "$keys"' EXIT
cp "$ASC_API_PRIVATE_KEY_PATH" "$keys/AuthKey_$ASC_API_KEY_ID.p8"
API_PRIVATE_KEYS_DIR="$keys" xcrun altool --upload-app -t ios -f "$work/Everglade.ipa" --apiKey "$ASC_API_KEY_ID" --apiIssuer "$ASC_API_ISSUER_ID"
echo "Everglade $version ($number) uploaded; apps/everglade-mobile/ios/release.sh status $number says when Apple has processed it."
