# Android release checks

This folder holds local release checks for the OpenAgents Android app. These
steps inspect artifacts; they do not sign, upload, or publish them. The Android
Gradle shell and signing configuration are still being integrated.

## Check signed artifacts

After the Android app build is available, produce the APK and AAB from the same
version metadata. Set `versionCode` above the retired app's version code 16.
Keep the signing configuration in a protected local environment. Do not put
keystore files, passwords, service credentials, or `google-services.json` in
the repository or command output.

Run the checker with the APK, AAB, R8 mapping, R8 rules, and the public asset
links file. Supply the approved APK signer, AAB signer, and Play app-signing
certificate SHA-256 fingerprints from the release owner. Fingerprints are
public identifiers; signing credentials are not.

```sh
python3 scripts/android/verify-release-artifacts.py \
  --apk "$APK" \
  --aab "$AAB" \
  --mapping "$MAPPING" \
  --r8-rules apps/android/app/proguard-rules.pro \
  --assetlinks "$ASSETLINKS" \
  --apk-cert-sha256 "$APK_CERT_SHA256" \
  --aab-cert-sha256 "$AAB_CERT_SHA256" \
  --app-signing-cert-sha256 "$PLAY_APP_SIGNING_CERT_SHA256"
```

The checker confirms the package name, version code, artifact signer
fingerprints, asset links association, both native ABIs, 16 KiB ELF load
alignment, APK zip alignment, absence of common test/debug paths and debug
symbols, and nonempty R8 rules and mapping output. It prints artifact hashes
and public certificate fingerprints so the handoff can identify the exact
files. Keep the mapping file with the release artifact in the approved private
release storage; do not commit it.

This local check does not establish Play App Signing continuity, a signed
in-place upgrade, or store readiness. Do those checks with the release owner
and a dedicated test device after B1 integrates the Android shell.

## Internal-testing handoff

Before an internal test, confirm the existing `com.openagents.app` entry and
its Play App Signing configuration. Compare the certificate that Play will
install with the certificate in `assetlinks.json`. Keep the signing key and
Firebase credentials in protected storage, outside Git.

Use a dedicated test device and synthetic account, wallet seed, grants, and
settings. Install the signed candidate over the retired app without clearing
its data. Confirm the account, wallet seed, grants, and settings remain, then
record the old and new version codes, artifact hashes, device/API/ABI, and
result. Never use an owner's account or real recovery words.

Before inviting internal testers, confirm the privacy policy, Data safety
answers, content rating, Firebase configuration for `com.openagents.app`,
tester list, and rollback instructions in Play Console. Stop before upload or
publication until the release owner approves those actions. If a rollout has
started, use Play Console's supported halt or rollback action and record the
result; do not assume an older version can be installed over a newer one.
