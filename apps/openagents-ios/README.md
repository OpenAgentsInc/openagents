# OpenAgents iOS

The replacement app is the [Coax Native starter](../../coax/native/README.md):
Coax-generated portable modules, native UIKit controls, native account
services, and optional signed Updates. The OpenAgents book starts here and
adds login and agent chat as Coax application functionality.

Bundle `com.openagents.app`, team HQWSG26L43, version 1.0.0, existing icons
and entitlements are preserved. Build 1068 introduces the replacement. The
earlier Metal world, wallet and voice screens are excluded from this target;
their Keychain namespaces are retained. Their implementation notes are in
[README-legacy.md](README-legacy.md).

Build 1069 restores only the existing Spark Bitcoin balance under **More →
Bitcoin balance**. Its separate `extras/` Coax module is not part of the
book. It reads the same Keychain entropy, English BIP-39 mapping, default
mainnet account and `Application Support/spark-mainnet` SDK directory as
the former wallet. The pinned Swift SDK is 0.27.2. Its server configuration
disables background services/optimizers and automatic deposit claims;
the adapter explicitly syncs and reads `balanceSats`, then disconnects.
There are no payment, receive, restore or seed-writing bridge operations.
Missing, locked, corrupt and unavailable are distinct from a zero balance.

The independent extras module and starter reuse the same components and
browser ABI. A future app update may remove the extras surface without a
native rebuild. The seed and recovery words stay in Swift/SDK memory.

The production account surface is now [`home/`](home/README.md): email-code
login and a read-only list of account-saved chats, delivered over the air to
build 1069 and rendered with UIKit from build 1070. The original starter remains reusable.

Build 1070 replaces the account WebView with UIKit fields, buttons, labels
and scrolling. JavaScriptCore executes generated Coax code; signed OTA can
still replace application logic and composition. The runtime fingerprint
changes, isolating its update feed from 1069. Native SDKs, wallet keys and
auth storage retain their identity. Code fields use `.oneTimeCode`; actual
email suggestions depend on iOS and the mail/AutoFill integration.

## Build and check

`apps/openagents-ios/build.sh` generates on a remote build host and builds
with Xcode on this Mac's slots. XcodeGen is required. No simulator window
needs to be opened.

```sh
coax/remote/remote.sh run -- bash coax/native/check.sh
apps/openagents-ios/build.sh
coax/remote/remote.sh run --local -- xcodebuild \
  -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -destination 'platform=iOS Simulator,name=Coax Native Starter' \
  -derivedDataPath /tmp/coax-native-sim CODE_SIGNING_ALLOWED=NO test
```

Checks cover signature tampering, runtime/app mismatch, generation replay,
artifact corruption, traversal, activation, interrupted-trial recovery and
the rendered starter. The installed authority is Coax; generated C is not
committed. See the starter guide for optional offline-only builds, the
installed service inventory, OTA publication and server configuration.

## TestFlight

Keep build numbers above App Store Connect's latest build. Credentials come
from `~/work/.secrets/appstoreconnect.env` or ASC_API_* variables. Archives,
profiles, private keys and signing material remain outside the repository.

```sh
apps/openagents-ios/release.sh /absolute/fresh/output
# With ASC_API_* loaded, after Apple reports VALID:
python3 apps/openagents-ios/tools/app_store.py status --build 1068
python3 apps/openagents-ios/tools/app_store.py testflight --build 1068 \
  --group Owner --notes-file apps/openagents-ios/release-notes/1068.txt
```

The release script generates resources, signs, archives, exports and uploads.
Group assignment remains explicit. It never creates testers or submits a
public App Store release. Keep the released `Generated/runtime.txt` and
`Native.plist` with its archive: updates target that installed runtime.
