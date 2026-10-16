# Prepare Coder for TestFlight

Since the 2026-10-16 rename the app is **OpenAgents** on bundle
`com.openagents.app` (the OpenAgents App Store Connect record; its next build must
be above both 59 there and 1002 here). The history below describes 1.0.0 (1002) on
the retired `com.openagents.coder` record.
Before signing, the owner checks that this version and build are available.
If they are already used, change every app/extension version in
`Zeron.xcodeproj/project.pbxproj` and `version.json` together, commit, and push.

## Build locally

From a clean fork checkout on current `origin/main`:

```sh
scratch=$(openagents scratch)
python3 scripts/ios/archive.py \
  --target-dir "$HOME/work/coder-desktop-target-agent1" \
  --output "$scratch/Coder-1.0.0-1002.xcarchive"
```

The default is unsigned. The script builds the normal iPhone Rust core in
`mobile-dist`, archives the UIKit app and Live Activity extension, and checks
versions, identity, privacy data, endpoints, and absence of the simulator
credential fixture and push prompt. It writes a neighboring JSON proof.
It refuses dirty source, existing output, and less than 9 GiB free. It stops
its own build if space drops below that threshold. It never exports or uploads.
Use `--check-only` to check source metadata without building.

The signing team is deliberately empty in source. For an owner with existing
local signing profiles, `--team YOUR_TEAM_ID` creates a locally signed archive
and verifies its signature. It does not log in, fetch profiles, or open a browser.
For a distribution-signed archive, also pass
`--distribution-certificate CERTIFICATE_SHA1`. Install App Store provisioning
profiles whose names exactly match `com.openagents.coder` and
`com.openagents.coder.LiveActivity`. The script selects each target’s matching
profile and rejects a development entitlement in the resulting app.
Xcode Organizer or an authorized `xcodebuild -exportArchive` run performs final
distribution validation and upload.

## App Store Connect fields

| Field | Value |
| --- | --- |
| Name | OpenAgents Coder |
| Bundle ID | com.openagents.coder |
| Version/build candidate | 1.0.0 / 1002 |
| Subtitle | Your coding sessions, anywhere |
| Category | Developer Tools |
| Privacy policy | https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents/apps/ios/release/privacy.md (the old `coder-desktop` address still resolves from the archived repository) |
| Support | https://github.com/OpenAgentsInc/openagents/issues |
| What to test | Copy `test-notes.md` |
| Review sign-in | OpenAgents device approval on openagents.com; use a dedicated test account |
| Notifications | No push notifications in this build |

Review contact details stay with the existing record. Do not put a credential
in this repository or in public notes. If Apple requests a review account,
provide a dedicated account through its private review fields.

## Privacy and encryption

The manifest declares account name/email, user and device IDs, messages,
photos, other submitted content, and optional voice audio, linked to the
account for app functionality. No tracking domains or advertising use are
declared. App Store Connect's privacy answers must describe the same behavior,
including the OpenAgents account service and any model provider a connected
computer uses. Voice remains an unverified first-release feature.

UserDefaults, file timestamps, and disk-space access keep their existing
required-reason declarations. Review Xcode's generated privacy report before
upload. See Apple's [privacy manifest data types](https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacycollecteddatatypes/nsprivacycollecteddatatype).

The Rust transports use rustls for HTTPS and secure WebSockets, including
standard cryptography outside Apple's OS implementation. The app does not
promise end-to-end encrypted chat storage. The owner selected the export
exemption for this release on 2026-10-16.
Both app and extension declare `ITSAppUsesNonExemptEncryption=false`; archive
verification requires the app's declaration. This means the app uses no
non-exempt encryption, not that it uses no cryptography. See Apple's
[export compliance process](https://developer.apple.com/help/app-store-connect/manage-app-information/overview-of-export-compliance).

## Owner upload and device check

1. Confirm `sync.openagents.com` resolves and has a valid certificate. In the
   existing App Store Connect record, confirm the candidate version/build.
2. Open `apps/ios/Zeron.xcodeproj`. Select **Zeron > Signing & Capabilities**
   and the OpenAgents team; do the same for **ZeronLiveActivity**. Keep Push
   Notifications absent. Select **Any iOS Device (arm64)**.
3. Choose **Product > Archive**. In **Window > Organizer > Archives**, select
   OpenAgents Coder, then **Validate App**. Review privacy and encryption
   answers. Resolve signing or validation errors before continuing.
4. When the owner authorizes an upload, choose **Distribute App > App Store
   Connect > Upload**, review bundle/version/team, and submit the upload.
5. In **App Store Connect > Apps > OpenAgents Coder > TestFlight**, wait for
   processing, resolve **Missing Compliance** if shown, select the build,
   and paste `test-notes.md` into **What to Test**. Add it to an internal group.
6. On a dedicated test iPhone, install from **TestFlight**, sign in with a
   dedicated account, approve the matching code, and repeat the desktop/phone
   scenario in the monorepo's `docs/mobile/coder-desktop-phone-e2e.md`.
   Check background/foreground, network loss, attachments, and **Settings >
   Sign Out**. Confirm no notification permission prompt appears.

Record the processed build number and device result in workspace
`NEEDS_OWNER.md`. If the device check finds a defect, file a new issue.

## Release simulator launch check

Run the optimized app's first-launch, demo, scene-restoration, and send/reply
checks before archiving. This scheme excludes the Debug unit-test host:

```sh
ZERON_SKIP_CORE=1 CARGO_TARGET_DIR="$HOME/work/coder-desktop-target-agent1" \
  xcodebuild -project apps/ios/Zeron.xcodeproj -scheme ZeronReleaseSmoke \
  -destination 'platform=iOS Simulator,id=8010197D-517D-43DA-9559-46742EFABC65' \
  ARCHS=arm64 ONLY_ACTIVE_ARCH=YES CODE_SIGN_IDENTITY=- CODE_SIGNING_ALLOWED=YES \
  -parallel-testing-enabled NO \
  -only-testing:ZeronUITests/LaunchLifecycleTests \
  -only-testing:ZeronUITests/SessionFlowTests/testSendEchoesAndStreamsReply test
```

Build the Release simulator Rust core before using `ZERON_SKIP_CORE=1`:
`CONFIGURATION=Release scripts/ios/build-core.sh iphonesimulator`.
The native `CoderIdentityTests` also seeds a saved legacy delegate and checks
that connection uses the current UIKit delegate.
