#!/usr/bin/env bash
# Archive this rebuilt OpenAgents app, export and upload. Processing/group
# assignment is separate; do not spin or create new testers here.
# Usage: apps/openagents-ios/release.sh /absolute/output/directory
set -euo pipefail
cd "$(dirname "$0")"
out="${1:?usage: $0 /absolute/output/directory}"
[[ "$out" = /* ]] || { echo "Output directory must be absolute" >&2; exit 2; }
[[ ! -e "$out/OpenAgents.xcarchive" && ! -e "$out/upload" ]] || { echo "Use a fresh output directory" >&2; exit 2; }
if [[ -z "${ASC_API_KEY_ID:-}" ]]; then
  set -a; source "$HOME/work/.secrets/appstoreconnect.env"; set +a
fi
: "${ASC_API_KEY_ID:?}" "${ASC_API_ISSUER_ID:?}" "${ASC_API_PRIVATE_KEY_PATH:?}"
mkdir -p "$out"
./generate.sh
xcodegen generate
cert="${OPENAGENTS_CERT_ID:-GCN7Q59Y27}"
sha1="$(python3 tools/app_store.py cert-sha1 "$cert")"
security find-identity -v -p codesigning | /usr/bin/grep -q "$sha1" || { echo "Distribution identity unavailable" >&2; exit 1; }
python3 tools/app_store.py profile --cert "$cert" --out "$out/OpenAgents.mobileprovision"
uuid="$(security cms -D -i "$out/OpenAgents.mobileprovision" | plutil -extract UUID raw -)"
for profile_dir in "$HOME/Library/MobileDevice/Provisioning Profiles" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"; do
  mkdir -p "$profile_dir"; cp "$out/OpenAgents.mobileprovision" "$profile_dir/$uuid.mobileprovision"
done
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Release \
  -destination generic/platform=iOS -derivedDataPath "$out/dd" -archivePath "$out/OpenAgents.xcarchive" \
  DEVELOPMENT_TEAM=HQWSG26L43 CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY="$sha1" \
  PROVISIONING_PROFILE_SPECIFIER="OpenAgents App Store" archive
python3 - "$out/ExportOptions.plist" "$sha1" <<'PY'
import plistlib,sys
with open(sys.argv[1],'wb') as file:
    plistlib.dump({'method':'app-store-connect','destination':'export','signingStyle':'manual',
                  'teamID':'HQWSG26L43','signingCertificate':sys.argv[2],
                  'provisioningProfiles':{'com.openagents.app':'OpenAgents App Store'},
                  'manageAppVersionAndBuildNumber':False,'uploadSymbols':True},file)
PY
xcodebuild -exportArchive -archivePath "$out/OpenAgents.xcarchive" \
  -exportOptionsPlist "$out/ExportOptions.plist" -exportPath "$out/upload"
keys="$(mktemp -d)"
trap 'rm -rf "$keys"' EXIT
cp "$ASC_API_PRIVATE_KEY_PATH" "$keys/AuthKey_$ASC_API_KEY_ID.p8"
API_PRIVATE_KEYS_DIR="$keys" xcrun altool --upload-app -t ios -f "$out/upload/OpenAgents.ipa" \
  --apiKey "$ASC_API_KEY_ID" --apiIssuer "$ASC_API_ISSUER_ID"
echo 'OpenAgents upload accepted; query tools/app_store.py status before TestFlight group assignment.'
