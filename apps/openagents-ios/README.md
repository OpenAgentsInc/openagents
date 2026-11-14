# OpenAgents iOS

The replacement app is the [Coax Native starter](../../coax/native/README.md):
Coax-generated portable modules, native UIKit controls, native account
services, and optional signed Updates. The OpenAgents book starts here and
adds login and agent chat as Coax application functionality.

Bundle `com.openagents.app`, team HQWSG26L43, version 1.0.0, existing icons
and entitlements are preserved. Build 1068 introduces the replacement. The
earlier Metal world, wallet and voice screens were excluded from that target;
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

Build 1073 fixes redundant update offers on a fresh TestFlight install: a
signed package whose files already match the running bundle only records its
generation, without downloading artifacts or prompting to apply it. Genuinely
changed resources still use the signed OTA path. See the
[regression and release record](../../docs/verification/2026-11-13-native-update-dedup.md).

Build1074 adds **Your computers**: Claude Code on the owner's Mac and CoderOS,
read-only Terminal history, managed conversations, Stop and held permissions.
The resident computer service owns execution independently of desktop windows.
New installed random-ID/timer services require this TestFlight build; subsequent
compatible presentation changes can use Updates. The
[runtime contract](../../docs/protocol/environment-runtime-v1alpha1.md) records
the implemented scope and current bounds.

## Streamed worlds, build 1075

**Worlds** opens Splice Bots and the main Everglade town using their existing
Coax simulation and shaders on Metal. The installed catalogue also includes
Everglade Forest and the saved-world template. There is no WebView.

The binary contains producers, compiled shaders, the account interface and
small signed asset inventories. Packs, world fonts, sounds and icons download
separately. Required files are verified whole before opening; icons load on
demand and optional sounds follow. Immutable SHA-256 objects live in the
public, asset-only bucket `openagentsgemini-native-world-assets`. Direct Cloud
Storage delivery avoids Cloud Run's large-response limit. Inventories pin the
installed producer identity, relative paths, sizes and digests. Redirects,
partial files and corrupt cached files cannot become ready assets. Verified
assets reopen offline; online rooms and account world records need a network.
Old cache generations currently have no eviction UI.

**Create a world** projects the web's existing Builder: setup questions,
typed/document planning, editable feature board, pending requests, revision
and creation. **Worlds you can edit** uses the existing account/player-scoped
API. The SDK attaches its guest player key to the verified email session's
game account through a closed request. Creator and admin rights are unchanged.

The shared Coax editor shell is projected into native controls in a resizable
sheet above the Metal view. It retains the inspector, tools, transform fields,
undo/redo, assets, files, actor plans, lists, revision history and publishing
where a realm exposes them. The Agent sheet uses the account composer and
component Markdown parser. Typed plans retain the turn's revision and follow
the realm's normal batch and ownership path. Inference uses the existing team
meter; the answer is saved before its edit is handed to the editor.

This SDK uses the generic Apple PageHost, not the canonical MetalRealm
renderer. Scene, shadow, particles, bloom and HUD are adopted; separate water
and grass passes and downloaded realm-script execution are not yet adopted
here. Scripts can be authored, built and assigned for web players, with an
explicit iOS notice. Code editing needs a server that exposes that capability.
Agent attachment-open requests are shown; automatic attachment read-back is
not installed. Producer/shader changes need TestFlight. Builder/editor/chat
logic, world records and compatible asset inventories can change over the air.

Focused schemes: `CoaxNativeWorlds` checks Metal readback, generated native
programs, cache integrity and cancellation. `CoaxNativeWorldsLive` opens both
games in the installed app through real public downloads. Physical acceptance
is tracked in root `NEEDS_OWNER.md`.

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
