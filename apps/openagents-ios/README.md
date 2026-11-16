# OpenAgents iOS

Build **1095** adds articulated inspection to **Home → Microduck**: named
parts/joints, bounded poses, an axis indicator and reversible explosion,
through the same native Metal viewer and streamed asset cache. The upload is
accepted; Apple processing/group assignment is recorded separately in the
[workbench verification record](../../docs/verification/microduck-workbench-13639/README.md).

Build **1094** adds **Home → Microduck**, the existing Coax model viewer on
native Metal. Drag orbits; pinch zooms. The model streams on first use and
reopens from its verified cache. Its floating menu has camera views, Normals,
Reset and License. It is **VALID / IN_BETA_TESTING** in **Owner**.
[Pictures, checks and delivery](../../docs/verification/microduck-native-2026-11-15/README.md).

Build **1093** contains the installed Nostr custody/journal/relay foundation.
It is **VALID / IN_BETA_TESTING** in **Owner**; its matching production API is
deployed. [Foundation evidence](../../docs/verification/2026-11-15-nostr-foundation.md).

Build **1092** is **VALID / IN_BETA_TESTING** in **Owner**. **Feed** in the drawer
reads live public Nostr notes from your saved relays, with relay, author, time
and hashtag filters. It uses shared Coax state/components and a read-only native
relay service; `/feed` is the web consumer. Its matching signed generation-1
package is bundled, so a fresh installation needs no identical update.
[Release and device evidence](../../docs/verification/2026-11-15-nip01-feed-1092.md).

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

## Private Projects and routing, build 1091

1091 is **VALID / IN_BETA_TESTING**, verified in the existing **Owner** group.
It packages the private Projects client and the latest live-computer-report
routing preview. Create a Project, edit its instructions, add knowledge and
continue saved chats in the same account used by the CLI. Account chat shows
the selected computer, agent, model and effort; this is a routing preview and
does not start work on that computer yet. Its signed generation-1 feed matches
the archive. [Release and focused checks](../../docs/verification/2026-11-15-native-projects-router-1091.md).

## Private Projects

The Projects drawer now creates account-owned private Projects, edits instructions,
adds and reads text/Markdown knowledge, and starts or reopens saved personal
Project chats using the shared account composer. **Publish a public project**
retains the existing drop/draft flow. [The API contract](../../docs/protocol/projects-api-v1alpha1.md)
records the first consumer, bounds and remaining phases. No new installed native
effect is needed.

## Native Projects, build 1085

The drawer’s **Projects** opens native UIKit screens composed in Coax. Create a
project from pasted notes or the system document picker, resume saved drops,
read the agent’s draft, edit its complete fields, confirm publication, and add
work to an existing published project. These are the web’s existing drop,
review, publication and grow contracts. Private member projects open their
existing shared agent conversation, with compact tool rows, Stop, Undo and
goal agreement. Public projects have account-owned saved project conversations;
opening or returning to them stays in the app.

Email-only accounts receive an opaque ownership handle, separate from Nostr
signing keys. The existing creator grant controls project creation; private
member access and stronger-login requirements retain their existing rules.
The [protocol and bounds](../../docs/protocol/native-projects.md) describe the
versioned associations and document leases. The document effects require 1084
or later; 1085 includes recovery and encoded-input fixes. Compatible
presentation and application logic can use signed Updates.

The project test scheme is `CoaxNativeProjects`. The release record is
[here](../../docs/verification/2026-11-14-native-projects-1085.json).

## Streamed worlds, build 1075

**Worlds** opens Splice Bots and the current Everglade (the Everglade II forest
and winding path), using their existing Coax simulation and shaders on Metal.
The earlier town producer is legacy and is not a public catalogue choice.
The saved-world template is also installed. There is no WebView.

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
the realm's normal batch and ownership path. Inference uses the existing
metered Gemini 4 Hydrogen world route; the answer is saved before its edit
is handed to the editor.

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

1075 is **VALID / IN_BETA_TESTING**, verified in the existing **Owner** group.
The release is frozen at tag `openagents-ios-1075`; subsequent game commits on
main are separate from this binary. Its 18.7 MB IPA contains no world packs.
The generation-1 feed has all 20 released portable resources and inventories,
verified against the archive through both public update endpoints. A fresh
installed-app check and relaunch stored only its signed generation high-water
mark, without a duplicate download or update prompt. See the
[release evidence](../../docs/verification/2026-11-14-native-worlds-1075.json)
for exact runtime, source, artifact, tests and current bounds.

## Build and check

Build 1076 corrected the device's writable world cache/save paths. Its archive
still stripped native world callbacks from the executable, so opening a world
could terminate the app. Build 1077 preserves the explicit host exports and
limits each producer to its page face. The release wrapper now checks the actual
archive's Mach-O imports and exports; Debug/testability exports cannot satisfy
that gate. This native linking correction requires a TestFlight install.

Build 1078 includes that repair and corrects the Everglade choice to the current
`everglade2` producer and `/worlds/everglade2` path, both from the library and
the account program. Its native title is **Everglade**. The library choice is
portable Coax and can also be corrected over Updates on compatible builds.
1078 is **VALID / IN_BETA_TESTING**, verified in **Owner**; its signed feed
matches all twenty archived resources. See the
[crash and release record](../../docs/verification/2026-11-14-native-world-crash-1078.json).

Build 1079 adopts T3 Code's compact work rows for computer transcripts. Tool
output starts collapsed, opens as literal selectable text with bounded
scrolling and Copy, and keeps its disclosure state across polls. The header
has a small refresh/status control; background polling stays quiet. Earlier
history moves above the transcript and remains still while being read. Main's
existing model and effort controls are retained. The shared Coax presentation
also has a compatible OTA for 1078; the native row and status styling requires
1079. It is **VALID / IN_BETA_TESTING**, verified in **Owner**. See the
[release record](../../docs/verification/2026-11-14-native-transcript-1079.json).

`CoaxNativeTranscript` covers the generated program, native tool disclosure,
copy/selection and draft preservation. `CoaxNativeTranscriptLive` signs into
an independent local HTTP oracle and opens a computer transcript in the
installed app; it makes no provider calls or real account changes.

Build 1080 removes the world's navigation and status bars. The Metal canvas
fills the screen; one 44-point floating menu retains Close world, the existing
editor and Retry after a loading failure. Opening the menu releases held
game input. Splice Bots' shared touch adapter now converts the engine's
positive-up look pitch into the game's positive-down mouse-delta convention,
correcting inverted vertical drag without changing mouse or horizontal look.
These native host/producer corrections require TestFlight. 1080 is
**VALID / IN_BETA_TESTING**, verified in **Owner**. Its matching generation-1
feed passes the fresh-install deduplication check. See the
[release record](../../docs/verification/2026-11-14-native-world-controls-1080.json).

Build 1081 packages the completed shared mono HUD work from
[issue 13580](https://github.com/OpenAgentsInc/openagents/issues/13580).
Its upload was started before work on navigation, without waiting for Apple.
It is **VALID / IN_BETA_TESTING**, verified in **Owner**; see the
[release record](../../docs/verification/2026-11-14-native-mono-1081.json).

Build 1082 adopts T3 Code mobile's history/detail structure with our shared
HUD controls. On a phone, the searchable history pane slides into the
conversation; on regular widths it stays beside it. Computer conversations
are grouped by host, followed by saved account chats. New chat chooses its
computer or account before opening the composer. Settings and Worlds live
in the history footer, and model/effort choices stay by the composer.
1082 is **VALID / IN_BETA_TESTING**, verified in **Owner**. Its generation-1
feed matches the archived resources and passes the live fresh-install check.
See the [release record](../../docs/verification/2026-11-14-native-navigation-1082.json).
Opening history dismisses the keyboard and pauses transcript polling.
Switching chats preserves sixteen drafts in memory; unresolved commands
cannot be replaced. Existing desktop and world-agent presentation remains
available through the optional workspace profile.

`CoaxNativeNavigation` checks the compiled Coax program and installed native
app against the independent HTTP oracle (`Tests/email_fixture.py
--environment-transcript`): Mac/CoderOS history, search, chat switching,
drafts, compact tools, model/effort choices and Settings. The navigation reuses
`components/kit/frames.coax`; its state is `apps/shared/app-navigation.coax`
and its layout is `components/layout/workspace.coax`.

The release can retain a frozen world's exact immutable asset records while
rebinding its inventory to a rebuilt producer:
`coax/web/tool.sh coax/native/worlds/package.coax --rebind INVENTORY PRODUCER_DIR OUTPUT_INVENTORY`.
The existing inventory codec writes it and the installed runtime catalogue;
other worlds' identities and asset bytes remain pinned.

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

## Durable chat staging check

`CoaxNativeDurableStaging` runs the installed generated account/Project program
through the native phone signer, device proof and HTTP adapter against staging.
It creates private fixture Projects, uses their selected knowledge in actual
inference, observes running deltas, stops and reopens a turn, completes a
follow-up and reopens an ordinary account chat. Its token store is isolated
from the installed login. The phone's staging account must already be enrolled
in the existing team beta. Receipts are retained as XCTest attachments.

```sh
TEST_RUNNER_OA_DURABLE_STAGING=1 coax/remote/remote.sh run --local -- xcodebuild \
  -project apps/openagents-ios/OpenAgents.xcodeproj -scheme CoaxNativeDurableStaging \
  -destination 'platform=iOS Simulator,name=Coax Native Starter' \
  -derivedDataPath /tmp/coax-native-durable CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- test
```

Without that opt-in the live case skips. The ordinary `CoaxNativeSettings`
selection includes the local HTTP oracle's installed UIKit chat, background,
restart, Stop, account-switch and private Project flows.

## TestFlight

Keep build numbers above App Store Connect's latest build. Credentials come
from `~/work/.secrets/appstoreconnect.env` or ASC_API_* variables. Archives,
profiles, private keys and signing material remain outside the repository.

```sh
apps/openagents-ios/release.sh /absolute/fresh/output
# With ASC_API_* loaded, after Apple reports VALID:
python3 apps/openagents-ios/tools/app_store.py status --build 1075
python3 apps/openagents-ios/tools/app_store.py testflight --build 1075 \
  --group Owner --notes-file apps/openagents-ios/release-notes/1075.txt
```

The release script generates resources, signs, archives, exports and uploads.
Group assignment remains explicit. It never creates testers or submits a
public App Store release. Keep the released `Generated/runtime.txt` and
`Native.plist` with its archive: updates target that installed runtime.

### Publishing world content

`generate.sh` writes the pinned inventories to `Starter/worlds/` and immutable
objects to `build/world-assets/objects/`. Upload objects before publishing an
inventory that names them:

```sh
gcloud storage rsync apps/openagents-ios/build/world-assets/objects \
  gs://openagentsgemini-native-world-assets/objects \
  --cache-control='public,max-age=31536000,immutable' --quiet
```

This bucket contains public game assets only. User uploads retain the existing
authenticated world-file flow. Never remove an object still referenced by an
installed or published inventory. The app checks every required object's size
and digest before opening the world; optional textures are requested by name
from the installed producer and resolved through that same inventory.

Publish compatible UI/inventory changes through the signed package tools in
[`coax/native/`](../../coax/native/README.md). The worlds-enabled package needs
both `.js` and `.tail.js` parts of all four portable programs (`app`, `extras`,
`worlds`, `editor`), their runtime and fonts, and all five `worlds/*.json`
inventories. Wasm parity artifacts stay in the archive and are omitted from
the OTA manifest so it retains the frozen 24-file limit. A new build's initial feed
must be packaged from that archive's `Starter/` directory, rather than a later
generation of the checkout. Matching bundled hashes let a fresh installation
record the feed without offering a redundant update.

Settings → **Usage** reads Claude Code's reported five-hour and weekly limits
from each connected computer. It shows remaining percentage, reset time and
stale/unavailable readings separately. **Connect a Claude account** starts
that computer's official CLI sign-in in an isolated profile; open Claude on
the phone and paste its authorization code back. **Use for new chats** changes
only subsequent managed conversations on that computer. Existing conversations
keep their account. Tokens remain in the computer's Keychain/private CLI store.
The exact environment grant must opt in locally with `accounts-enable`;
see [the provider-account contract](../../docs/protocol/provider-accounts-v1.md).
Codex accounts are a later adapter. This screen uses the installed 1082 services
and can be delivered as a signed OTA update.


Build 1083 starts on a quiet Chat home: the existing account composer stays
at the bottom, with centered Splice Bots, Everglade and world-library
shortcuts. The menu opens Chat, Projects and Worlds, followed by searchable
recent computer/account conversations and Settings. Phones use a narrow
drawer, a dismissible backdrop, edge-open/left-close gestures and VoiceOver
escape; regular-width layouts keep the sidebar column. Drafts survive place
and conversation changes, and an unresolved command must be reconciled before
leaving its conversation. Existing model/effort and Usage flows are retained.

Projects reads the same account project list as the web sidebar through
`GET /api/projects` and its existing line codec. A project's explicitly
labelled action opens its page on openagents.com. World shortcuts use the
existing native streamed-world service; Everglade opens `everglade2`.
The drawer geometry and welcome layout require installing 1083. Future
compatible screen changes continue through signed Updates.

1083 is **VALID / IN_BETA_TESTING**, verified in **Owner**.
The [1083 release record](../../docs/verification/2026-11-14-native-drawer-1083.json)
keeps the archive/runtime identities, installed UI checks and live-feed evidence.
The fresh-install feed check uses the independent email/environment oracle on
port 13493 for account calls while fetching Updates from the public endpoints.

## Component gallery (1086)

Settings → **Components** opens the shared Coax gallery in an embedded Metal
view. The web companion is [openagents.com/components-gpu](https://openagents.com/components-gpu),
linked from the HTML gallery at `/components`. Both run
`components/gallery/page.coax`, the common GPU backend/shader and atlas.
Motion, Controls, HUD and ARWES sections have interactive specimens; the
native options menu also exposes section, playback, reduced motion and skin
choices. The carrier stops drawing offscreen/backgrounded and releases its
atlas/state when dismissed. Installing 1086 is required for the native carrier.
The gallery's top icon buttons use UIKit Liquid Glass on iOS 26 and later;
older iOS versions retain the shared button frame. The shared native button
offers this appearance as an opt-in for other floating controls.
The gallery's compiled components require a new build to change; the ordinary
Coax app screens continue through Updates.

`CoaxNativeComponents` exercises section switching, touch scrolling,
background/resume and the real Settings entry against the independent local
email oracle (`Tests/email_fixture.py RELEASE --port 13567 --environment-transcript`).
This is a preview of shared visual components, not a switch of the production
chat renderer or completion of every Arcade animation. See
[the component guide](../../components/README.md) for exact implementation scope.

1086 is **VALID / IN_BETA_TESTING**, verified in **Owner**. Its signed generation-1
feed matches all twenty archived resources. See the
[gallery evidence and pictures](../../docs/verification/2026-11-14-component-gallery/README.md).

## Monochrome Arcade application components (1087)

The default mobile app now uses the shared GPU component scene for all current
app-owned chrome: buttons, fields, composer, transcript/tool decorations,
drawer/menus, Projects, Usage, Settings/extras, world library/editor/overlay,
progress, notices and Updates. `components/native/scene.coax` uses the existing
kit and GPU backend; `NativeArcadeSurface` forwards actual measured bounds,
ancestor clips, keys and control state through the shared Metal carrier.
White/grayscale replaces cyan, including alternate skins in the phone gallery.
World content keeps its authored colors.

The native text/input/accessibility adapters preserve selection, IME, OTP
AutoFill, Dynamic Type, keyboard docking, scrolling and actions. Frame entry,
press/focus, activity arcs and progress springs live in Coax and honor reduced
motion. Desktop adapters are unchanged. This is not completion of all 28 Arcade
motion cases or a measured performance claim.

`CoaxNativeArcade` runs the installed carrier, update-offer presentation,
Markdown/transcript and gallery tests. Navigation, Chat, Usage, Project,
Balance and Worlds UI suites qualify the existing flows against independent
local account fixtures and real streamed world assets. New native code needs
1087; later compatible Coax app-screen changes can continue through Updates.

1087 is **VALID / IN_BETA_TESTING**, explicitly verified in **Owner**. Its
signed generation-1 feed matches all twenty resources in the exported IPA;
the fresh-install/relaunch check confirms no redundant Apply prompt. See
[the release evidence and screenshots](../../docs/verification/2026-11-14-arcade-mobile/README.md).

## Native account/settings, build 1088

The rail’s Settings opens the Arcade-style Account hub in the shared grayscale
components. It includes profile editing/publication, WSS relays, privacy, iOS
security notifications, keys/passkeys, masked identity/login-key backups,
device sessions and resources. Email-code login and account-managed identity
provisioning remain; additional nsec/hex login keys never replace that identity.
See [the complete mapping](../../docs/arcade/07-account-settings.md),
[the versioned service](../../docs/protocol/account-settings-v1.md), and
[release evidence](../../docs/verification/2026-11-14-arcade-settings/README.md).
The focused scheme is `CoaxNativeSettings`. New SDK/key effects require 1088.

1088 is **VALID / IN_BETA_TESTING**, verified in **Owner**. Its generation-1
feed matches every archived portable resource and passes the installed
fresh-install/relaunch no-prompt check. Account/settings migration 0202 is
live in production.

## Simplified workspace, build 1090

Titles stay centered, creation uses a plus, and foreground synchronization
replaces refresh controls. The composer contains text and Send/Stop. Worlds
opens in the main workspace; history reveals at a fixed width with single-line
truncated rows. The shared project review from main is included, with an 8 MiB
app stack inside the existing 32 MiB runtime. The grayscale gallery repair
from 1089 is included. [Checks and installed pictures](../../docs/verification/2026-11-14-mobile-workspace/README.md).

1090 is **VALID / IN_BETA_TESTING**, verified in **Owner**. Its signed generation-1
feed matches all 20 IPA resources; fresh install and relaunch offer no redundant update.
