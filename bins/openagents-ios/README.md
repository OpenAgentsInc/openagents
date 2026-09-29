# OpenAgents for iOS

OpenAgents is an iPhone app for commanding your computers from your phone.
It reuses Coder's Rust Native approach under its own identity: Rust builds
each screen in the `openagents-mobile` crate, and a thin SwiftUI host
decodes and renders it. The host shares Coder's renderer and native glue
([`NativeView.swift`](../coder-ios/host/App/NativeView.swift), the QR
scanner, the secret field, and the terminal keyboard) instead of copying
them.

The Android build, with the same tabs and the same Rust library, is in
[`bins/openagents-android`](../openagents-android/README.md).

The app has four tabs, shown as icons:

- **Coder** (the code icon) opens on a new chat, ready to type: the
  composer has the cursor, a selector beside the **Coder** title says
  where the message goes, and suggested actions sit above the field as
  chips. With a computer this phone may operate ready, a new chat starts
  Coder there: a NIP-HOST `task.create` in the workspace the selector
  names (**Studio Mac · openagents**), the same operation as Order work;
  with the host's auto-start policy on, the host runs Coder's engine right
  away. Tapping the selector offers each computer, **Cloud** (the basic
  Coder), and **Connect a computer**, with a check on the current one. The
  chips continue the newest chats (a clock glyph), pick another of the
  computer's workspaces (a folder glyph; the one this phone used last
  comes first), and, with no computer added, **Connect a computer**,
  which opens Account > Computers. The chat worker's `rank` job may order
  the chips once each time the tab shows; the phone's own order stands
  when it does not answer. The basic Coder needs no computer: each message
  is a NIP-CJ conversation job signed by the device key and sent, NIP-44
  encrypted, through `relay.openagents.com` to the OpenAgents chat worker,
  and the reply streams back as partials drawn with incremental Markdown.
  The job asks for the worker's first response (`opener`), so a short
  opener shows about half a second after sending while the model starts,
  and while the tab shows the phone keeps one signed-in relay connection
  with its subscription placed, so a message only publishes its request;
  the connection closes in the background. The app holds no model key; the
  worker meters each caller key (see `INVARIANTS.md`). From a
  conversation, **Run Coder on** a computer starts Coder on it with the
  conversation so far; when the worker's judgment places the message on a
  computer, it shows as a chip.
  The menu button at the top left opens the previous chats, newest first:
  basic conversations and Coder's tasks on your computers, painted from
  what the phone kept while the computers are read again. Only Coder's
  chats show; the phone does not list Claude Code, Codex, OpenCode, or
  Devin sessions. A session a Coder task delegated to OpenCode or Devin
  shows inside its chat, where the task's transcript notes it, as a
  **Delegated to OpenCode** (or Devin) row that opens to the session's
  messages, read through the same observer. An open Coder chat reads the task's ATIF transcript
  through the computer's read-only history observer (its `coder` source)
  and follows it while the task runs: the model's replies, the commands it
  ran with their output, and how the run ended. A message in a finished
  chat continues the same task, and a message while Coder works queues for
  its next turn; a long press on send offers the other ways to send,
  **Edit queue** edits what waits, and a question or approval request from
  Coder is answered in the chat. Every message is a durable NIP-HOST
  `task.command` (see [what comes later](docs/chat-later.md#built-since-build-6)).
  When the app comes to the foreground or the tab shows, Rust opens each
  computer's history connections, so the first read or send waits for no
  connection. An earlier chat opens at its newest page that has a row to
  show and fills the rows before it in the background. The screen changes
  as soon as Rust has something new: a thread of the host's own waits in
  `openagents_mobile_wait` and asks for the packet (`changed`) when a
  transcript page, a streamed reply, a chat list, or a task's status
  arrives, and while the tab shows a live chat Rust also answers every
  second. A 3-second refresh remains as a fallback.
- **Verse** (globe) is Verse's bare world: the plaza's ground grid in white
  and gray on a dark field, with your character in the center, the other
  players in the same world, and one large ball ahead of you. Walk into the
  ball to push it; it rolls with real physics under a studio light and comes
  to rest (see [the ball](../../docs/verse/mobile.md#the-ball)). The arch
  to [Lagrange 1](../../docs/verse/lagrange-1.md) is hidden for now, in
  every build (see
  [the Grid's portal](../../docs/verse/mobile.md#the-grids-portal-to-lagrange-1)). Everyone in
  the world shares the ball and the blocks, and finds them where they were
  left; walking into the pillar to the right of the spawn puts them all back
  (see [sharing the ball](../../docs/verse/mobile.md#sharing-the-ball)). It fills the screen behind the status bar and the tab bar. The
  controls are Coder's, with a second stick: push the faint stick at the
  bottom left to walk, and in touch look push the faint stick at the bottom
  right (or drag anywhere) to look, with both thumbs at once if you like;
  double-tap to jump, and pinch to zoom. Between the sticks, the
  hand/gyroscope icon switches touch and motion look (motion look hides the
  look stick), and the crosshair recenters the camera. While the tab shows, it joins the bare world (`verse-bare`) on
  `wss://relay.openagents.com` for avatar presence alone and draws other
  players' avatars in white and gray; it has no chat and reads no computers
  or chats. Presence signs with a separate world key in Keychain
  (`com.openagents.app.verse`), never the device key. Switching tabs or
  backgrounding the app closes the connection. See
  [Verse presence](../../docs/verse/mobile.md#presence-in-the-openagents-app).
  Rust draws the
  world through Coder's mobile Verse surface in its bare mode
  (`coder_mobile::VerseHandle::create_bare`, carried by
  `openagents_verse_create`, `openagents_verse_call`, and
  `openagents_verse_destroy`); the host (`host/App/VerseTab.swift`) mounts
  the Metal layer and forwards touches and motion samples, and reuses
  Coder's `PinchAdmission.swift` and `DeviceMotion.swift`. In simulator
  builds, `--verse-script look,walk,jump,zoom,recenter` drives those
  controls through the same touch path without touching the screen, and the
  log line `verse-world` reports the connection, live players, and the world
  public key. `push` holds the stick forward for a whole step, into the
  ball, `closer` pinches in past the nearest orbit into first person,
  `walkpinch` holds the stick while pinching in, `board` taps the Gym's
  board, and `wait` pauses a step; the log line also reports the ball's
  position, speed, and physics time. The Grid's
  [Gym](../../docs/verse/mobile.md#the-grids-gym) stands straight ahead of
  the spawn; its board opens in a native white-on-black panel
  (`host/App/VerseGym.swift`, over the packet types it shares with Coder in
  `GymBoard.swift`), and its connection is kept in Keychain
  (`com.openagents.app.gym`). `--gym-preview` shows the labeled synthetic
  board offline.
- **Wallet** is a Bitcoin wallet on mainnet through Breez's Spark SDK
  (`host/App/WalletTab.swift` over `crates/openagents-mobile/src/wallet.rs`):
  balance, receive (Lightning invoice, Spark address, Bitcoin deposit
  address, and this device's npub, each with a QR code, and a switch that
  publishes the Spark address in the Nostr profile), send (paste or scan an invoice, Lightning
  address, LNURL code, npub, Spark or Bitcoin address; an npub pays its
  published Spark address or its profile's Lightning address; a Lightning address shows
  its range and takes a comment; then a confirm screen with amount and fee),
  buy with dollars (MoonPay or Cash App), deposit
  claims, history, and recovery words. Its seed is in Keychain
  (`com.openagents.app.spark`). The **i** button opens the trust note.
  Simulator arguments: `--wallet-section receive|send|buy`,
  `--wallet-method lightning|spark|bitcoin`, `--wallet-invoice AMOUNT`,
  `--wallet-send TEXT` (with `--wallet-amount AMOUNT`, typed in the amount
  format), `--wallet-info 1`, `--amount-format bip177|btc`, and,
  on the offline fixture wallet `--wallet-fixture 1` (debug builds, no
  money), `--wallet-refund ADDRESS` with `--wallet-refund-review 1` and
  `--wallet-backup 1`. On-chain withdrawals show three speeds; deposits that
  need attention offer a quoted claim or an on-chain refund; the exit backup
  exports to Files. **Agent payments** lists what agents on the owner's
  computers asked the phone to pay, and which
  computers may ask. A request shows as a **Payment request** sheet over any
  tab (`host/App/AgentPayments.swift` over
  `crates/openagents-mobile/src/spend.rs`): the computer, task, purpose, the
  payee and amount decoded from the invoice, the fee, and what the
  computer's grant has left. Nothing pays until the owner taps Approve (with
  Face ID or the passcode above ₿1,000); Deny refuses it, and **Stop
  payment requests** revokes the computer. See
  [the spend protocol](../../docs/breez/spend-protocol.md).
- **Account** holds **Computers**, **Tailnet**, **Identity keys**, **About
  this device**, and **Changelog**, and links to the source code and to
  OpenAgents on X. See [Account](#account).

The Coder chat screen draws Rust Native's conversation elements: a
bottom-anchored transcript with a jump to the bottom and **Load earlier**,
messages by role, Markdown parsed in Rust, collapsible tool rows, a working
row, and a composer with send and stop. The shared iOS renderer
(`bins/coder-ios/host/App/NativeChat.swift`) reimplements the design of the
t3code iOS chat (pingdotgg/t3code, MIT) in UIKit and SwiftUI. What is left
for later is in [docs/chat-later.md](docs/chat-later.md). Launch a simulator
build with `--rust-native-fixture` to see the sample conversation from
`crates/rust-native/fixtures/conversation.json`.

## Account

**Trainer** is your trainer card: your level, your XP, the XP to the next
level, the curve (`trainer-curve-v1`), your titles, and the counted awards
behind the level, each linked to its signed award. Rust derives it on the
phone from NIP-XP events on `wss://relay.openagents.com`, trusting the
OpenAgents referee alone, with the same reader that puts
`<prefix> · lv <n>` over players' heads in the Grid. Other players see
your level there only after you tap **Show my level** and confirm, which
publishes a trainer profile (NIP-XP `13193`) signed by your trainer key;
**Hide my level** replaces it. **Link a key** lists a computer's key in
that profile; once the computer runs `microcoder xp link --trainer <your
npub>`, its XP counts toward your level without moving either secret key.
**Export card** signs your trainer card, publishes it after a confirmation,
and offers its JSON file and a public link; `openagents xp verify-card`
checks it.
Your trainer key is the
Verse world key, the one over your head; **Reveal nsec** exports it, behind
a warning, to sign a reproduction on a computer
([tutorial quests](../../docs/verse/tutorial-quests.md)). `--xp-preview`
(debug or simulator) shows a labeled fixture instead of the relay
([captures](verification/2026-09-28-trainer-levels/)).

**Computers** lists your computers as a native list: each row names the
computer and a one-word status with its route, and its menu switches it off
or on, tries it now, opens its access, or forgets it. Tapping a row opens
Coder's shared Computers screens for that computer (status, order work,
terminal, access, recent work); **Add a computer** and **Activity** open the
same screens. Rust builds the list from the Computers snapshot and runs
every choice through the same authority check as the shared screens.

**Identity keys** shows this device's Nostr key. The public key comes first
in NIP-19 form (`npub1…`), with its hex beside it, each with **Copy**. The
secret key (`nsec1…`) stays hidden until you tap **Reveal nsec** and confirm
a warning; the screen hides it again when you leave it or the app goes to
the background, and **Copy nsec** puts it on this device's pasteboard only,
for 60 seconds. The key is random, made on this device and kept in its
Keychain, and not derived from a NIP-06 seed phrase, so the nsec is its only
backup. Anyone with the nsec can act as this device on your computers.
**About this device** shows the same public key in both forms and the app's
version.

**Changelog** lists what each TestFlight build brought and a **What to
test** line for it, from Rust (`crates/openagents-mobile/src/account.rs`).
Every build gets an entry: when you raise `CURRENT_PROJECT_VERSION` in
`host/project.yml`, add the entry for that build first; the test
`every_build_has_a_changelog_entry_with_what_to_test` fails until you do.

**Playtest** (Account) holds the playtest card, **Playtest logging** (one
line saying whether it is on in this build, the log, and **Delete the
log**), **Report a problem**, and **My reports**; a long press on the tab
bar also opens **Report a problem** for the screen on view
(`crates/openagents-mobile/src/playtest.rs`, `host/App/Playtest.swift`,
[playtesting](../../docs/game/playtesting.md)). Playtest logging has no
switch in the app: it is on in every build, TestFlight archives included,
unless `build.sh` runs with `OPENAGENTS_PLAYTEST_LOGGING=off` (release
mode). `--report` opens the report form at launch in simulator builds.

In simulator builds, `--tab account --account-route
computers|tailnet|identity|device|changelog|playtest|reports` opens a screen directly, and
`--identity-script warn|reveal` shows the reveal warning or reveals the nsec
without taps. Launching with
`SIMCTL_CHILD_OPENAGENTS_COMPUTERS_FIXTURE=1` draws Computers from Coder's
offline fixture (`coder_computers::synthetic`), which contacts no host, and
`--computers-script open|add|activity` opens its first computer, adding a
computer, or activity.

## Automatic setup over the tailnet

A computer connects itself when it runs the host with
[tailnet admission](../../nips/openagents/NIP-HOST.md#tailnet-admission):

```sh
coder host serve --tailnet-admission standard
```

After you sign in on the Tailnet tab, the app asks every device on the
tailnet for an invitation. A host answers only a device that `tailscale
whois` names as its own Tailscale user, so your phone gets one and nobody
else's does. The app redeems it through the normal NIP-HOST enrollment, and
the host signs the grant, so revocation and the device list work as usual.
The same answer carries a chat invitation, so the computer's Coder chats
appear without running `coder pair`. The phone must be on the tailnet, through the
Tailscale app, to reach the computer.

A computer without tailnet admission can still be added from **Computers >
Add a computer** with a `coder-host:` invitation or an 8-character code. The device's
Nostr key stays in Keychain, and grants and pairings stay in encrypted
stores keyed by it.

## Tailnet tab

iOS does not let one app read another app's Tailscale state, and a tailnet
has no discovery broadcast, so the phone's own Tailscale connection cannot
list its peers. Instead, the app uses Tailscale's Rust control client,
[tailscale-rs](https://github.com/tailscale/tailscale-rs) (`ts_control`
0.6.1), to register as its own tailnet node named `openagents-ios`, then
reads one netmap from Tailscale's control server. It never joins the data
plane or carries traffic. This sign-in only lists devices: Computers never
uses it, and it grants no access to any host.

1. On first launch, the control server returns a sign-in URL. The app shows
   **Connect to a tailnet** and **Sign in with Tailscale**.
2. The button opens the URL in Safari. After you approve the device, the app
   reads the netmap and shows the device list.
3. The node keys stay in the app's Application Support directory, so later
   launches skip the sign-in until the node key expires.

If the tailnet has no other devices, the app shows **Connect to a tailnet**
with a **Refresh** button. tailscale-rs is pre-1.0 and unaudited; the app
uses only its control-plane client.

## App identity

| Setting | Value |
| --- | --- |
| Project, scheme, target, product | `OpenAgents` |
| Bundle identifier | `com.openagents.app` |
| App Store Connect app | `6748620735` (**OpenAgents**) |
| Development team | `HQWSG26L43` |
| Marketing version and build | `1.0.0` / `17` |
| Minimum OS and device family | iOS 17 / iPhone |
| Archive signing | Manual, Apple Distribution, `OpenAgents App Store` profile |

The App Store Connect record also holds `0.x` builds from an earlier app on
this bundle identifier. Build numbers only need to be unique within one
version, so `1.0.0` started at build `1`. Builds `1` to `17` are on TestFlight. Raise the build number for every
upload; set it in `host/project.yml` or with `OPENAGENTS_IOS_BUILD_NUMBER`.

The `OpenAgents App Store` profile uses the same Apple Distribution
certificate as Coder's profile. The older `com.openagents.app AppStore`
profile names a different certificate, and Xcode rejects it when both
certificates are in the keychain.

The icon is the black-and-white power symbol from the earlier Khala app
(commit `8a54389bd1`).

## Build

Prerequisites: the pinned Rust toolchain with the `aarch64-apple-ios` and
`aarch64-apple-ios-sim` targets, Xcode with the iOS SDK, and `xcodegen`.

```sh
# Build, install, and launch on a simulator (default: the booted one).
OPENAGENTS_IOS_DEVICE=<simulator-udid> bins/openagents-ios/build.sh sim

# Signed App Store archive. Does not upload.
bins/openagents-ios/build.sh archive

# Upload the archive to TestFlight with an App Store Connect API key.
ASC_API_KEY_ID=... ASC_API_ISSUER_ID=... ASC_API_PRIVATE_KEY_PATH=... \
  bins/openagents-ios/build.sh upload
```

Build products go to `$CARGO_TARGET_DIR/openagents-ios`. The archive command
records the source commit and workspace status beside the archive.

## Push wakes for payment requests

When an agent on one of your computers asks for a payment, the computer
publishes a spend wake (`openagents.spend-wake.v1`, see
[the spend protocol](../../docs/breez/spend-protocol.md#wakes)) to this phone.
A relay whose push executor holds this phone's lease turns it into a
notification with the fixed wake text; it never carries the amount, payee,
or task. Opening the app reads the request and shows the approval sheet.

Push is off by default. A default build asks for no notification permission,
never calls `registerForRemoteNotifications`, and signs without the
`aps-environment` entitlement, so it keeps signing with the existing
`OpenAgents App Store` profile. A push build needs two switches, both
environment variables for `build.sh`:

- **Push settings.** `OPENAGENTS_PUSH_RELAY_URL` (`wss://`),
  `OPENAGENTS_PUSH_GATEWAY_URL` (`https://`), and
  `OPENAGENTS_PUSH_APP_PROFILE` reach `Info.plist`. When all three are set,
  the app passes them to Rust, asks for notification permission, registers
  for remote notifications, and hands Rust the APNs token (`push_token`),
  which registers it with the gateway and publishes the phone's push lease.
  The Wallet tab's **Agent payments** section shows the wake status.
- **Entitlement.** `OPENAGENTS_IOS_PUSH=development` or `production` signs
  with `host/Push/OpenAgents-Push.entitlements`. An archive needs
  `production`, after **Push Notifications** is turned on for
  `com.openagents.app` and the `OpenAgents App Store` profile is regenerated.
  The owner steps are in the workspace's `NEEDS_OWNER.md`.
