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

**Release gate (2026-10-09).** A release or normal simulator build has three
tabs, **Chat**, **Wallet**, and **Account**. The Verse tab, the Gym in chat
(Train Coder, Profile, its intro, menu, cards, and Gym starter chips),
**Trainer**, **Playtest** and **My reports**, **Tailnet**, and the display
name are preview features, shown only when the Rust library is built with
`OPENAGENTS_MOBILE_PREVIEW=on` (`OPENAGENTS_MOBILE_PREVIEW=on build.sh sim`);
their developer launch arguments do nothing otherwise. See the
[mobile 1.0 audit](../../docs/mobile/1.0-audit.md). The rest of this page
describes a preview build.

The app has four tabs, shown as icons:

- **Chat** (the message icon) opens on the main menu (wireframe revision
  3, `SCR-01`): the trainer's name, level, and XP from the phone's own
  ledger (a gray bar until it is read, never a 0), a **Next:** line from
  the phone's state, **CHAT WITH OPENAGENTS**, the starter chips **Test a
  plugin**, **What's new**, and **Check a result** (each opens a chat with
  that question sent), **PROFILE** (level, your results, and what you
  made), and **THE GYM IN THE VERSE**. A new install first walks **Choose
  your agent**, an end card with **LET'S GO**, and a first-run chat that
  asks for Project map's test, so a test starts on the third tap; the step
  reached survives a relaunch. In chat, the Gym's replies carry cards the
  app draws with its own words (a plugin with **Start the test**, a test-set
  draft with **Looks good** and **Change it**, a run, a result with **Add
  to the Gym**, news, a result to check, and your credit), and sheets for a
  result's detail, **Add to the Gym** (exactly what becomes public, before
  anything does), and a test set. Rust builds each card and sheet and mints
  each button's ID (`crates/openagents-mobile/src/gym.rs`); this host
  draws them (`GymViews.swift`) and sends back only the ID tapped.
  `--gym-first-run choose|end_card|chat|done` starts the first run at a
  step and `--gym-script "tap:ID|send:TEXT|sleep:N"` walks a flow, for
  simulator checks. The app has no tab bar since #11126: a top bar with
  the menu button, a **Coder** / **Verse** switch on a new chat and in the
  Verse, and **New chat** in a conversation; the menu opens a drawer with
  **Coder**, **Computers**, **Wallet**, **Verse**, **Settings**, the
  recent chats with search and **See all…**, a **Chat** pill for a new
  chat, and the account button. Rust owns the shell's state
  (`openagents-chat-app` `coder_tab/shell.rs`); `Shell.swift` draws it.
  `--drawer` opens on the drawer, `--tab verse` on the Grid, and
  `--appearance light|dark|system` picks the theme, for screenshots. The
  chat itself opens on a new chat with OpenAgents, ready to type: the
  composer (**Ask OpenAgents**) under four feature cards to swipe through
  (the list in `openagents-chat` `home_cards.rs`, shared with the
  website), each with **Try it**, which starts a chat with its question
  (**Explore the Verse**'s opens the Grid). The cards open on a different
  card each time and move on by themselves (`openagents-chat-app`
  `carousel.rs`), pausing under a finger and never with Reduce Motion.
  Every chat goes to OpenAgents, even while a computer is ready; there is
  no chat or code mode. **Verse** is the plain Grid with your avatar and
  the other players (no Gym or Everglade outside preview builds); a reply
  that offers it shows an **Enter the Grid** card. A reply that took time starts with **Worked for 6s**, which
  opens the steps. Coder on a computer otherwise comes only from an offer
  under a reply. The chat worker's `rank` job may order the questions once
  each time the tab shows; the phone's own order stands
  when it does not answer. Chat with OpenAgents needs no computer: each message
  is a NIP-CJ conversation job signed by the device key and sent, NIP-44
  encrypted, through `relay.openagents.com` to the OpenAgents chat worker,
  and the reply streams back as partials drawn with incremental Markdown.
  The job asks for the worker's first response (`opener`), so a short
  opener shows about half a second after sending while the model starts,
  and while the tab shows the phone keeps one signed-in relay connection
  with its subscription placed, so a message only publishes its request;
  the connection closes in the background. The app holds no model key; the
  worker has no usage limits and records every job in its usage log (see
  [chat worker admission](../../docs/deployment/chat-worker.md#admission-no-usage-limits)
  and `INVARIANTS.md`). Until a reply's result arrives, a spinner and
  **Working…** sit under it, before its first
  words and under an opener or any part that shows first. When the worker's
  judgment places a message on a computer, or the router offers to run
  Coder, **Run Coder on** a computer shows as a chip under that reply once
  it is complete, and starts Coder there with the conversation so far,
  asking the computer for the engine the reply's offer named when its
  presence advertises `task-engine` (#10081); with
  no computer the chip is **Connect a computer**. No Run Coder or Open Coder
  button stands above the field; a task a conversation started is in the
  previous chats. Each job also asks for the chat router (`router`) with a bounded
  `context` (the surface, whether a computer is ready, the paired
  computer's label even while it is offline, so the chat never asks to
  connect one (#10077), and the build; never a workspace path). The router's offers show as the phone's own controls,
  acting only on a tap: Run Coder or **Connect a computer**, a screen
  (Wallet, Account > Computers, Identity keys, Playtest, Report a problem),
  or a read-only `openagents` command as a card with a **Run** button. A
  prepared answer carries follow-up chips; Report a problem offers **Share
  this chat**, off by default.
  The menu button at the top left opens the previous chats, newest first:
  basic conversations and Coder's tasks on your computers, painted from
  what the phone kept while the computers are read again, and each paired
  computer's own chats (the ones its desktop app or `openagents chat`
  started), labelled with the computer (NIP-HOST `thread.*`, #10035). One
  of those opens with its turns; a message sent there goes through the
  computer, which answers it, so the desktop and `openagents chat read`
  show it too, and the reply streams in. The stop button stops receiving
  that reply through the computer (`thread.stop`, #10039): what streamed
  stays, marked stopped, and if the chat started Coder work that is still
  running, **Stop Coder too** stops the task as well. A computer too old to
  stop a reply, or a phone that may only read it, shows no stop button at
  all. They are read again while the
  phone reaches the computer; the phone's own chats need no computer.
  The phone keeps each computer's chat list and the turns it last read in
  its encrypted store (#10041), so after a relaunch with the computer off
  they still list, marked **Last read …**, and open, marked **Saved on
  this phone**. A message typed while the computer is off waits on the
  phone under the send ID it was given and goes once the computer answers
  again, even after a relaunch. Only Coder's
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
  to rest (see [the ball](../../docs/verse/mobile.md#the-ball)). Walking
  through the arch lettered **EVERGLADE**, ahead and to the right of the
  spawn, loads [Everglade](../../docs/verse/everglade.md) into the app's
  cache with a progress panel (**Cancel**, and **Retry** after a failure)
  and enters it; the zone's arch lettered **THE GRID**, or the panel's
  **The Grid** button, comes back (see
  [the Grid's portal to Everglade](../../docs/verse/mobile.md#the-grids-portal-to-everglade)).
  The arch to [Lagrange 1](../../docs/verse/lagrange-1.md) is hidden for
  now, in every build (see
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
  (`host/App/WalletTab.swift` over `crates/openagents-mobile/src/wallet.rs`).
  The main screen is plain: one big balance (no second unit and no network
  label; a quiet "Updated …" line only when the balance is old or failed
  to update; pull down to refresh), a **Back up your wallet** card until
  the recovery words are written down, two big buttons, **Receive** (a
  payment request for any amount with a large QR code, Copy, and Share,
  and an optional amount) and **Send** (one **Paste or scan** field that
  Rust reads to tell what it is: an invoice, Lightning address, LNURL code,
  npub, Spark or Bitcoin address; an amount only when one is needed; then
  one confirm screen in plain words), **Recent activity** (the newest five,
  with **See all**), and **Advanced**, closed by default and remembered on
  the phone. Advanced holds the balance in the other unit, the network and
  refresh, **Other ways to receive** (Lightning, Spark, Bitcoin, and Nostr,
  with the switch that publishes the Spark address), **Buy bitcoin**
  (MoonPay or Cash App), deposits, people, **Agent payments**, **Show
  amounts as**, **Recovery**, and the exit backup; while it's closed it
  notes a deposit on its way or one that needs you. Its seed is in Keychain
  (`com.openagents.app.spark`). The **i** button opens the trust note, a
  plain paragraph first and the details below.
  Simulator arguments: `--wallet-section receive|send|buy` (`buy` opens
  Advanced), `--wallet-advanced 1`,
  `--wallet-method lightning|spark|bitcoin` (in Advanced), `--wallet-invoice AMOUNT`,
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
or debug build with `--rust-native-fixture` to see the sample conversation
from `crates/rust-native/fixtures/conversation.json`. A release build for a
device compiles none of the fixture screens and bundles no fixture: the
file is copied only into debug, simulator, and bench builds, and the Rust
library compiles its chat, Gym, wallet, and Computers fixtures only in
debug builds.
`UITests/TranscriptScrollUITests.swift` opens that fixture with 200 extra
rows and drags the transcript, so a chat that cannot scroll fails a test.
Run it on a simulator with `xcodebuild test -project
bins/openagents-ios/host/OpenAgents.xcodeproj -scheme OpenAgents
-destination id=UDID OPENAGENTS_RUST_LIBRARY_DIR=<target>/aarch64-apple-ios-sim/debug`
after `build.sh sim` has built the Rust library and generated the project.

## Your openagents.com account (#11107, #11165)

**Settings > Log in** signs the phone in to your openagents.com account
with a code: the phone shows the code and a QR code of
`openagents.com/device?code=…`; approve it there (scan the QR with a
signed-in phone or computer, tap **Approve on this phone**, or type the code
at openagents.com/device). Only accounts the site lets sign in can approve.
Rust keeps the session in memory and hands it to the host once; the host
keeps it in Keychain (this device only, readable after first unlock) and
hands it back at launch (`AccountLink.swift`). **Sign out** ends it on the
site and removes it.

Signed in, the drawer lists **Running** and the account's newest chats
(web chats, and each computer's synced terminal chats); **All account
chats…** lists them by computer. A chat opens read and reply: a reply to a
terminal chat waits until Coder on that computer takes it. The phone asks
once where its own chats live (**Sync all my chats** or **Keep chats on
this phone**); with sync on, each phone chat uploads, screened for keys.
**Running** shows what Coder runs on each computer (status, time, cost) with
**Approve**, **Deny**, **Stop**, and **Message**. A change to finished,
failed, or asking shows as a local notification, with **Approve** and
**Deny** on a question; the app also reads in the background
(`BGAppRefreshTask`, `com.openagents.app.link-refresh`) about every 15
minutes. Remote push for these notices is not wired yet: the push gateway
(`docs/deployment/push-gateway.md`) wakes the phone only for payment
requests today.

Release builds sign in to openagents.com. A debug build takes
`--account-origin https://staging.openagents.com` to sign in to staging.
Rust owns all of it (`crates/openagents-mobile/src/account_link.rs`,
`link_view.rs`).

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
switch in the app: it is on only in a preview build or one made with
`OPENAGENTS_PLAYTEST_LOGGING=on`, and off in release builds
([playtesting](../../docs/game/playtesting.md#playtest-logging-in-a-release)). `--report` opens the report form at launch in simulator builds.

In simulator builds, `--tab account --account-route
computers|tailnet|identity|device|changelog|playtest|reports` opens a screen directly, and
`--identity-script warn|reveal` shows the reveal warning or reveals the nsec
without taps. Launching with
`SIMCTL_CHILD_OPENAGENTS_COMPUTERS_FIXTURE=1` draws Computers from Coder's
offline fixture (`coder_computers::synthetic`), which contacts no host, and
`--computers-script open|add|activity` opens its first computer, adding a
computer, or activity. `--chat-fixture 1`
answers chats from an offline worker that sends the chat router's fields in
turn (a prepared answer with follow-ups, a dispatch offer, a read-only
command, and a Wallet offer), for screenshots, and `--chat-script
"Who are you?|!wrong"` plays messages in a new chat one reply at a time, with
`!run` (run the first offered command) and `!wrong` (Wrong answer) steps;
debug builds only.

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
appear without a separate chat pairing. The phone must be on the tailnet, through the
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
| Marketing version and build | `1.0.0` / `45` |
| Minimum OS and device family | iOS 17 / iPhone |
| Archive signing | Manual, Apple Distribution, `OpenAgents App Store` profile |

The App Store Connect record also holds `0.x` builds from an earlier app on
this bundle identifier. Build numbers only need to be unique within one
version, so `1.0.0` started at build `1`. Builds `1` to `45` are on TestFlight. Raise the build number for every
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
