# OpenAgents for iOS

OpenAgents is an iPhone app for commanding your computers from your phone.
It reuses Coder's Rust Native approach under its own identity: Rust builds
each screen in the `openagents-mobile` crate, and a thin SwiftUI host
decodes and renders it. The host shares Coder's renderer and native glue
([`NativeView.swift`](../coder-ios/host/App/NativeView.swift), the QR
scanner, the secret field, and the terminal keyboard) instead of copying
them.

The app has four tabs, shown as icons:

- **Coder** (`</>`) chats with Coder on your computers. Sending a message
  starts a NIP-HOST `task.create` in the computer's `openagents` workspace,
  the same operation as Order work; with the host's auto-start policy on,
  the host runs Coder's engine right away. An open chat reads the task's
  ATIF transcript through the computer's read-only history observer (its
  `coder` source) and follows it while the task runs: the model's replies,
  the commands it ran with their output, and how the run ended. While Coder
  works, the send control stops the task (`task.cancel`). A message in a
  finished chat starts a new chat.
- **Verse** (globe) is Verse's bare world: the plaza's ground grid in white
  and gray on a dark field, with your character in the center and the other
  players in the same world. It fills the screen behind the status bar and the tab bar. The
  controls are Coder's: drag anywhere to look, push the stick at the bottom
  left to walk, double-tap to jump, and pinch to zoom. The hand/gyroscope
  icon switches touch and motion look, and the crosshair recenters the
  camera. While the tab shows, it joins the bare world (`verse-bare`) on
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
  public key.
- **Wallet** is a placeholder.
- **Account** holds **Computers** (Coder's shared Computers screens: add a
  computer, access, activity, order work, terminal), **Chats on your
  computers** (the Claude, Codex, and Coder chats saved on every connected
  computer, newest first, without subagents), **Tailnet**, and **About this
  device**.

Both chat screens draw Rust Native's conversation elements: a
bottom-anchored transcript with a jump to the bottom and **Load earlier**,
messages by role, Markdown parsed in Rust, collapsible tool rows, a working
row, and a composer with send and stop. The shared iOS renderer
(`bins/coder-ios/host/App/NativeChat.swift`) reimplements the design of the
t3code iOS chat (pingdotgg/t3code, MIT) in UIKit and SwiftUI. What is left
for later is in [docs/chat-later.md](docs/chat-later.md). Launch a simulator
build with `--rust-native-fixture` to see the sample conversation from
`crates/rust-native/fixtures/conversation.json`.

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
The same answer carries a chat invitation, so the computer's chats appear
without running `coder pair`. The phone must be on the tailnet, through the
Tailscale app, to reach the computer.

A computer without tailnet admission can still be added from **Computers >
Add a computer** with a `coder-host:` invitation or an 8-character code, and
its chats from **Chats > Add a computer** with `coder pair`. The device's
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
| Marketing version and build | `1.0.0` / `8` |
| Minimum OS and device family | iOS 17 / iPhone |
| Archive signing | Manual, Apple Distribution, `OpenAgents App Store` profile |

The App Store Connect record also holds `0.x` builds from an earlier app on
this bundle identifier. Build numbers only need to be unique within one
version, so `1.0.0` started at build `1`. Builds `1` to `7` are on TestFlight. Raise the build number for every
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
