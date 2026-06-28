# Khala iOS — TestFlight release runbook (working, 2026-06-26)

How to archive + sign + upload the native SwiftUI **Khala** app
(`clients/khala-ios/Khala`, bundle `com.openagents.khala`, Apple Team `HQWSG26L43`)
to App Store Connect / TestFlight from the CLI. This is the **proven** path —
build 2 (v1.0.0) was uploaded this way (Delivery UUID
`95107d96-…`). Local Xcode only; no Expo/EAS (see repo `CLAUDE.md`).

## The one big gotcha

**Xcode "cloud signing" is BLOCKED for our App Store Connect API key.** Any
`xcodebuild -exportArchive ... -allowProvisioningUpdates` with method
`app-store-connect` fails with:

```
error: exportArchive Cloud signing permission error
error: exportArchive No signing certificate "iOS Distribution" found
```

The key **can** create certs/profiles via the App Store Connect API (fastlane
uses that API directly), it just can't do Xcode's *cloud-managed* signing. So
the working path is: **create the Apple Distribution cert + an App Store
provisioning profile with fastlane, then export with MANUAL signing, then
upload with `altool`.** Do not waste time retrying `-allowProvisioningUpdates`.

## Secrets / prerequisites

- ASC API key env: `~/work/.secrets/appstoreconnect.env` →
  `ASC_API_KEY_ID`, `ASC_API_ISSUER_ID`, `ASC_API_PRIVATE_KEY_PATH` (the `.p8`).
  The `.p8` also lives at `~/.appstoreconnect/private_keys/AuthKey_<KEYID>.p8`
  (where `altool` auto-discovers it).
- `fastlane` (`/opt/homebrew/bin/fastlane`) and Xcode `altool` are installed.
- Never print the key contents, issuer, or tokens into tracked files/commits.

## Steps

### 0. Work from clean `origin/main` in a worktree
```sh
git fetch origin main && git worktree add /tmp/oa-ship origin/main
cd /tmp/oa-ship/clients/khala-ios/Khala
set -a; . ~/work/.secrets/appstoreconnect.env; set +a
```

### 1. Bump the build number (REQUIRED — each upload needs a unique, higher build)
Version keys are **hardcoded in `Khala/Resources/Info.plist`** (the project sets
`GENERATE_INFOPLIST_FILE=NO`), so bump there AND in the pbxproj:
```sh
# CFBundleVersion = build number (bump every upload); CFBundleShortVersionString = marketing version
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion <N>" Khala/Resources/Info.plist
perl -pi -e 's/CURRENT_PROJECT_VERSION = \d+;/CURRENT_PROJECT_VERSION = <N>;/g' Khala.xcodeproj/project.pbxproj
```
Commit + push this bump to `main`.

### 2. Archive (Release, generic iOS device)
The archive itself signs fine with the Apple Development cert.
```sh
xcodebuild -project Khala.xcodeproj -scheme Khala -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath /tmp/Khala.xcarchive archive \
  -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
  -authenticationKeyID "$ASC_API_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_API_ISSUER_ID" \
  DEVELOPMENT_TEAM=HQWSG26L43
```
Sanity-check the archived app has the icon + version:
`/usr/libexec/PlistBuddy -c 'Print :CFBundleIconName' /tmp/Khala.xcarchive/Products/Applications/Khala.app/Info.plist` → `AppIcon`.

### 3. Create the Apple Distribution cert (once; skip if already in keychain)
Build a fastlane API-key JSON, then create/install the cert:
```sh
python3 - "$ASC_API_KEY_ID" "$ASC_API_ISSUER_ID" "$ASC_API_PRIVATE_KEY_PATH" <<'PY'
import json,sys; k,i,p=sys.argv[1:4]
json.dump({"key_id":k,"issuer_id":i,"key":open(p).read(),"in_house":False,"duration":1200},
          open("/tmp/asc_key.json","w"))
PY
fastlane run get_certificates api_key_path:/tmp/asc_key.json development:false output_path:/tmp/khala-fl
# verify:
security find-identity -v -p codesigning | grep "Apple Distribution"
```
Note: this creates a NEW distribution cert if none with a local private key
exists (the account may already have one whose key we don't hold). Distribution
certs are capped (usually 2–3) — if at the limit, revoke an unused one in App
Store Connect rather than failing.

### 4. Create an App Store provisioning profile that includes THAT cert
The pre-existing App Store profile is tied to the *old* cert and will be
rejected ("profile doesn't include signing certificate ..."). Make a fresh one:
```sh
fastlane run get_provisioning_profile api_key_path:/tmp/asc_key.json \
  app_identifier:com.openagents.khala force:true output_path:/tmp/khala-fl
# install where Xcode looks (name is "com.openagents.khala AppStore"):
PROF=$(ls -t /tmp/khala-fl/*.mobileprovision | head -1)
UUID=$(security cms -D -i "$PROF" | plutil -extract UUID raw -)
cp "$PROF" ~/Library/MobileDevice/Provisioning\ Profiles/"$UUID".mobileprovision
```

### 5. Export the IPA with MANUAL signing
```sh
cat > /tmp/ExportOptions.plist <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>teamID</key><string>HQWSG26L43</string>
  <key>signingStyle</key><string>manual</string>
  <key>provisioningProfiles</key><dict>
    <key>com.openagents.khala</key><string>com.openagents.khala AppStore</string>
  </dict>
  <key>destination</key><string>export</string>
  <key>uploadSymbols</key><true/>
</dict></plist>
EOF
xcodebuild -exportArchive -archivePath /tmp/Khala.xcarchive \
  -exportPath /tmp/Khala-export -exportOptionsPlist /tmp/ExportOptions.plist
# -> /tmp/Khala-export/Khala.ipa   ("** EXPORT SUCCEEDED **")
```
No `-allowProvisioningUpdates` here — we supply the cert+profile, so the
cloud-signing path that fails is never taken.

### 6. Upload to TestFlight
```sh
xcrun altool --upload-app -f /tmp/Khala-export/Khala.ipa -t ios \
  --apiKey "$ASC_API_KEY_ID" --apiIssuer "$ASC_API_ISSUER_ID"
# -> "UPLOAD SUCCEEDED with no errors" + a Delivery UUID
```
The build appears under App Store Connect → TestFlight after Apple processes it
(a few minutes). First-ever upload may need export-compliance answered in ASC —
we set `ITSAppUsesNonExemptEncryption=false` in Info.plist so it auto-clears.

## App-icon validation (already fixed, keep it that way)

App Store rejects (`90713` missing `CFBundleIconName`, `90022` missing 120×120)
if the Info.plist lacks `CFBundleIconName`. Because `GENERATE_INFOPLIST_FILE=NO`,
Xcode does NOT inject it — `Khala/Resources/Info.plist` must keep
`CFBundleIconName = AppIcon`, and `Assets.xcassets/AppIcon.appiconset` must keep
all sizes incl. 60×60@2x (120×120) and the 1024×1024 marketing icon.

## "Black screen" in the Simulator is NOT the app

If the Simulator window is black with `IOSurfaceClientSetSurfaceNotify failed
e00002c7` in the log, that's a host GPU/Metal surface failure, not the app
(`simctl ... io screenshot` will still show the real UI). Fix:
```sh
xcrun simctl shutdown all && xcrun simctl erase "iPhone 17" && xcrun simctl boot "iPhone 17"
open -a Simulator   # then reinstall + launch
```

## Always keep the owner's local tree current

After mobile changes land on `main`, resync the owner's working copy + clear the
Xcode build cache so their Xcode/Simulator stops showing stale (or black) builds:
```sh
rm -rf clients/khala-ios/Khala && git checkout origin/main -- clients/khala-ios/Khala
rm -rf ~/Library/Developer/Xcode/DerivedData/Khala-*
```
