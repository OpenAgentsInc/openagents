# Connect a computer by scanning a QR code

Status: design, 2026-09-29. Nothing here is implemented yet; the epic is
[#9965](https://github.com/OpenAgentsInc/openagents/issues/9965). It replaces the
first draft of this file (`13935bde84`), which made a command on the
computer the primary path. The [Plan](#plan) lists the issues that build it.

## The decision

The owner's direction (2026-09-29), binding on this design: pairing is done
primarily by a companion desktop app, so a person simply scans a QR code
shown by the desktop app with the mobile app. It must be idiot proof: no
Tailscale, and no "run this command" steps.

So the primary path is:

1. The person downloads **OpenAgents** for Mac (a signed, notarized `.dmg`),
   drags it to Applications, and opens it.
2. The window shows one large QR code and one sentence: **Scan with the
   OpenAgents app on your phone.**
3. On the phone they tap **Connect a computer** (a chip in chat, or Account >
   Computers) and point the camera at the screen.
4. Both screens say the computer is connected. A **Run Coder** offer in chat
   can now send work to that Mac.

No terminal, no Tailscale, no key to create or copy, no command. The
`openagents connect` command stays, for people with a headless box or who
want to script it; it is never the path the app or the docs lead with.

Secondary, after the QR path ships: a phone on the same Wi-Fi sees the
computer by itself, and the computer asks "Kai's iPhone wants to connect"
with a six-digit code shown on both screens. The owner reports that the
OpenAgents iOS app already holds Apple's multicast entitlement
(`com.apple.developer.networking.multicast`), which that path needs.

What does not change: the host is the only issuer of access; a device holds a
host-signed grant with typed rights ([NIP-HOST](../../../nips/openagents/NIP-HOST.md));
a route, a network, or a scan never grants anything by itself
([NIP-REACH](../../../nips/openagents/NIP-REACH.md)); terminals are
[NIP-TERM](../../../nips/openagents/NIP-TERM.md) sessions under the
`terminal` right.

## Today

What a person does now, from [Link your devices](../guides/link-devices.md):
sign in to Tailscale on every device, turn on HTTPS certificates in the
Tailscale admin console, check out this repository and the pinned Rust
toolchain, run `coder link owner init`, run `scripts/link-device.sh` on each
computer, run `coder link invite`, and scan within five minutes. Each of
those is a place a normal person stops.

| Piece | Today, with source | Kept or changed |
| --- | --- | --- |
| Transport | A NIP-REACH direct channel over TCP or WebSocket, recorded as a `tailnet` hint on port 47101 (`crates/coder-host/src/settings.rs`); private `3188` artifacts over `wss://relay.openagents.com/` for everything else, including terminals ([NIP-TERM, Transport](../../../nips/openagents/NIP-TERM.md#transport)). | The direct channel gains an iroh transport. The relay transports stay as the fallback. |
| Channel security | The NIP-REACH handshake proves the host and device Nostr keys and encrypts every frame, "over any ordered byte stream" ([NIP-REACH, Implementation status](../../../nips/openagents/NIP-REACH.md#implementation-status)). | Kept unchanged, carried inside an iroh stream. |
| Identity | Owner key in `~/.openagents/coder-owner/owner.key`; host key and grants in `~/.openagents/coder-access/`; the phone's device key in Keychain `com.openagents.app.device` ([INVARIANTS, Device identity key](../../../INVARIANTS.md#device-identity-key)). | The desktop app keeps the owner and host secrets in the OS keychain. |
| Enrollment | A `coder-host:` invitation (host key, invitation ID, 32-byte capability, 300-second life, relay URL) redeemed as `enroll.redeem` over the relay ([NIP-HOST, Host invitations](../../../nips/openagents/NIP-HOST.md#host-invitations)). | Same redemption, also carried over iroh; a new QR payload adds the computer's iroh address. |
| Clock | `issued_at` may sit up to 60 s ahead of the reader (`CLOCK_SKEW`, `crates/coder-connect/src/protocol.rs:359`, used for host access by `crates/coder-access/src/protocol.rs:843`); the channel hello allows 120 s (`MAX_CLOCK_SKEW`, `crates/coder-reach/src/channel.rs:42`). | Kept. |
| Same-network shortcut | [Tailnet admission](../../../nips/openagents/NIP-HOST.md#tailnet-admission): an invitation for a caller whose Tailscale user owns the host, port 47109 (`crates/coder-host/src/tailnet.rs:41`). | Kept as an optional path for owners who use Tailscale. Nearby pairing with a confirmation code is the default same-network path. |
| Local operator | CLI commands (`coder host revoke`, `coder host autostart on`) open the host's on-disk store directly (`crates/coder-host/src/cli.rs`). No host process exposes a local socket today. | A local control socket, same-user only, used by the desktop app and `openagents connect`. |
| Phone scanner | `bins/coder-ios/host/App/QRScanner.swift` (compiled into the OpenAgents iOS host, `bins/openagents-ios/host/project.yml`) and `bins/openagents-android/host/app/src/main/java/com/openagents/app/QRScanner.kt`. Camera permission is already declared on both. | Reused. |
| Chat tie-in | **Connect a computer** is the chat chip shown when a message needs a computer and none is ready; it opens Account > Computers ([wireframe](../../product/2026-09-28-app-wireframe.md) `SCR-17.E05`, `SCR-17.E06`, `SCR-14.E05`). | The chip opens the scanner directly. |

One consequence matters for ordering the work: pairing, **Run Coder**, and
terminals already work over the Nostr relay with no Tailscale. What Tailscale
buys today is the fast direct channel and the ceremony around the owner key.
So the first milestone needs the ceremony gone and a direct path that is not
Tailscale; it does not need every packet off the relay.

## iroh, verified

Checked on 2026-09-29 against crates.io, the published crate sources, and
[docs.rs/iroh/1.3.0](https://docs.rs/iroh/1.3.0/iroh/).

- **Versions.** `iroh` 1.3.0 (2026-09-28), 1.2.0 (2026-09-09), 1.1.0
  (2026-08-25); license `MIT OR Apache-2.0`; MSRV 1.91
  ([crates.io API](https://crates.io/api/v1/crates/iroh)). `iroh-relay`,
  `iroh-base`, and `iroh-dns-server` are also 1.3.0. QUIC is `noq` 1.3
  (iroh's fork; `iroh-quinn` stopped at 0.16.1).
- **API (1.x).** `Endpoint::builder(preset)` then `.secret_key(SecretKey)`,
  `.alpns(Vec<Vec<u8>>)`, `.relay_mode(RelayMode)`,
  `.address_lookup(..)`, `.bind().await`;
  `Endpoint::connect(impl Into<EndpointAddr>, alpn: &[u8]) -> Result<Connection, ConnectError>`;
  `Connection::remote_id() -> EndpointId`; `Router::builder(endpoint).accept(alpn, handler).spawn()`
  with a `ProtocolHandler`. `EndpointId` is `iroh_base::PublicKey`
  (Ed25519); `EndpointAddr { id, addrs: BTreeSet<TransportAddr> }` carries IP
  and relay addresses. `NodeId` and `NodeAddr` no longer exist.
  `RelayMode` is `Disabled | Default | Staging | Custom(RelayMap)`. Presets in
  `iroh::endpoint::presets`: `Empty`, `Minimal` (crypto provider only), `N0`
  (n0's DNS/PKARR lookup and n0's relays), `N0DisableRelay`
  (`src/endpoint.rs:960`, `src/endpoint/presets.rs`).
- **Address lookup.** The `AddressLookup` trait resolves an `EndpointId`.
  `iroh` ships `MemoryLookup`, `DnsAddressLookup`, and PKARR publishing
  (`src/address_lookup/`). mDNS is the separate crate
  [`iroh-mdns-address-lookup`](https://crates.io/crates/iroh-mdns-address-lookup)
  0.6.0 (`MdnsAddressLookup`, built on `swarm-discovery` 0.6, which opens its
  own UDP multicast socket on 224.0.0.251:5353; default service name
  `irohv1`, changeable with `MdnsAddressLookupBuilder::service_name`). The
  DHT is `iroh-mainline-address-lookup`. There is no lookup by a prefix of an
  `EndpointId`: resolution is by the full key.
- **Tickets.** `iroh-tickets` 1.0.0 has `EndpointTicket` (`endpoint` prefix,
  base32). We do not use it: our QR payload also carries the host's Nostr key
  and a one-time capability.
- **Relays and hole punching.** An endpoint keeps a connection to its home
  relay; connections start through the relay and move to a direct path when
  hole punching succeeds, else stay relayed. Relayed traffic is QUIC,
  encrypted end to end; the relay routes by `EndpointId` and learns who talks
  to whom. Relays also offer QUIC Address Discovery (QAD) on UDP 7842, off by
  default in `iroh-relay` (`enable_quic_addr_discovery`), needing TLS.
- **Default relays.** The `N0` preset uses n0's four public relays
  (`*.relay.n0.iroh.link`, `src/defaults.rs`), which n0 describes as
  rate-limited and meant for development
  ([iroh FAQ](https://docs.iroh.computer/about/faq)). We use `Minimal` plus
  `RelayMode::Custom` with our relay only, and no n0 DNS.
- **Relay server.** `iroh-relay` built with `--features server`, TOML config
  (`enable_relay`, `http_bind_addr`, `tls` with `cert_mode` `Manual`,
  `LetsEncrypt`, or `Reloading`, `enable_quic_addr_discovery`, `limits`,
  `access` = everyone, allow/deny lists, shared token, or HTTP callout,
  `enable_metrics`); ports TCP 443 (relay over HTTPS, WebSocket upgrade),
  TCP 80, UDP 7842 (QAD), metrics 9090; `GET /healthz`
  ([self-hosting](https://docs.iroh.computer/iroh-services/relays/self-hosted)).
  Clients hold long-lived upgraded connections.
- **Platforms.** iroh's CI builds and tests Linux, macOS, Windows, FreeBSD,
  and Android targets (`aarch64-linux-android`, `armv7`, `x86_64`, emulator
  tests) and wasm32. iOS is not in iroh's own CI; n0 ships iOS through
  [iroh-ffi](https://github.com/n0-computer/iroh-ffi) (an xcframework that
  links `SystemConfiguration` and `CoreWLAN`). On Android the app must call
  `iroh::dns::install_android_jni_context` before binding
  (`src/endpoint.rs:885`). We checked here: a scratch crate with `iroh` 1.2.0
  and `iroh-mdns-address-lookup` 0.5.0 passes `cargo check` for
  `aarch64-apple-darwin` and `aarch64-apple-ios` with Rust 1.97.1; the
  Android check needs the NDK `clang` that `ring` already needs for
  `crates/openagents-mobile`, and was not run.

How this design uses it, and what stays ours:

- iroh is **transport**. It finds a path (same network, hole-punched, or our
  relay) and proves the peer holds the `EndpointId` it dialed.
- Authority stays in our protocols. On the `openagents/reach/1` ALPN the
  existing NIP-REACH handshake runs unchanged inside one QUIC bidirectional
  stream, proving the host and device **Nostr** keys and binding the grant,
  epoch, and generation. The grant check is where access is decided, exactly
  as over TCP today. An `EndpointId` never authorizes anything.
- So there is no separate "key binding" artifact (the first draft had one).
  The phone learns the host's `EndpointId` from the QR, which also names the
  host's Nostr key, and later from host-signed NIP-REACH hints. A fake
  endpoint cannot pass the NIP-REACH host proof, and a stolen Nostr key
  alone cannot answer at the `EndpointId`.
- The channel is encrypted twice (QUIC TLS and NIP-REACH's NIP-44 frames).
  That costs a little CPU and saves a protocol rewrite; terminals move at
  most 8 KiB per frame.

## T3 Code, verified

Checked against [`pingdotgg/t3code`](https://github.com/pingdotgg/t3code) at
`ff1db030b1`:

- `t3 pair` finds the running server and prints a QR code, a URL, and a
  one-time token (`apps/server/src/cli/pair.ts`); `t3 serve` prints the same
  on start (`apps/server/src/startupAccess.ts:122-148`).
- Tokens are single-use, 12 characters, and live 5 minutes by default
  (`apps/server/src/auth/PairingGrantStore.ts:239,258,512-548`).
- A default pairing grants `orchestration:read`, `orchestration:operate`,
  `terminal:operate`, `review:write`, `relay:read`; `access:read`,
  `access:write`, and `relay:write` are administrative
  (`packages/contracts/src/auth.ts:81-115`).
- The pairing URL is the server's own origin with the token in the
  fragment, `…/pair#token=CODE`; a hosted `app.t3.codes/pair?host=…#token=`
  form exists for HTTPS endpoints. The client strips the fragment from
  history (`apps/web/src/environments/primary/auth.ts:150-166`).
- The bearer token buys a 5-minute WebSocket ticket; the socket never sees
  the long-lived token (`packages/client-runtime/src/authorization/remote.ts`).
- Their Electron desktop app shows the pairing QR in its Connections
  settings, but binds only loopback by default; a phone needs
  "network-accessible" mode (binding `0.0.0.0`), Tailscale, or their relay
  first (`apps/desktop/src/settings/DesktopAppSettings.ts:81`).

What we take: one visible code, single-use and short-lived; the server as the
boundary; a standard scope set that excludes administration. What we do
better: our QR carries an iroh address, so a phone on mobile data reaches a
Mac behind NAT with no mode switch, no Tailscale, and no port forward.

## Design

### The desktop app

**Name.** **OpenAgents** (`OpenAgents.app`, bundle ID
`com.openagents.desktop`). It is the same name as the phone app, so the
sentence "Scan with the OpenAgents app on your phone" names exactly one
thing. "OpenAgents Connect" was considered and rejected: a second product
name for one step.

**Technology.** Rust, in a new crate `crates/openagents-desktop`:

- The window is the pattern `crates/openagents-deck` already uses: a frame
  painted in software and copied into a `winit` 0.30 window's `wgpu` 29
  surface, with glyphs from `rust-native`'s bundled font. It adds no window
  dependency the workspace does not already have.
- The QR code is drawn with `qrcodegen` 1.8, already in the workspace lock.
- The menu-bar icon uses `objc2-app-kit`'s `NSStatusItem` (0.2.2 is already
  in the lock through `winit`). `tray-icon` 0.25.1 was checked and fails
  `deny.toml` on Linux (`gtk` 0.18's unsound and unmaintained advisories,
  RUSTSEC-2024-0429 and RUSTSEC-2024-0370, and `option-ext`'s MPL-2.0).
- The login item uses `SMAppService` through `objc2-service-management`
  0.3.2; the keychain uses `keyring` 4.2.0 (macOS Keychain, Windows
  Credential Manager, Secret Service on Linux). Both pass `deny.toml`.
- No web view, no Electron, no Tauri: product code here is Rust
  ([AGENTS.md](../../../AGENTS.md)).

**Processes.** Installing the app is running a host:

- `OpenAgents.app/Contents/MacOS/OpenAgents` is the window and menu-bar
  process. It holds no secret key.
- `OpenAgents.app/Contents/MacOS/coder` is the existing Coder binary. It runs
  `coder host serve` as a launchd agent that the app registers on first
  launch with `SMAppService.agent(plistName:)`, from
  `Contents/Library/LaunchAgents/com.openagents.desktop.host.plist`. It
  starts at login, keeps running when the window closes, and is listed in
  System Settings > General > Login Items. Registering through
  `SMAppService` also makes macOS attribute the agent's local-network use to
  the app ([TN3179](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)).
- `microcoder` and the other binaries a task needs ship in the same
  `Contents/MacOS`.
- The window talks to the host only through the local control socket below.

**Keys.** On first run the host creates its host Nostr key, its iroh
`SecretKey`, and an owner Nostr key, and stores them in the login keychain
(service `com.openagents.desktop`; on Linux Secret Service, on Windows
Credential Manager). Only the host process reads them. The host establishes
that owner key as its owner, so there is no owner step. Each computer's
desktop app has its own owner unless the person imports one
(`openagents connect owner import`, for someone who wants several computers
in one owner directory). Losing the computer loses nothing a phone cannot
redo: remove the computer on the phone and scan again.

**Screens** (IDs for the [wireframe](../../product/2026-09-28-app-wireframe.md)):

| ID | Screen | What it shows |
| --- | --- | --- |
| `DSK-01` | Connect a phone (first run, and from **Connect another phone**) | The QR code, large and centered, and under it **Scan with the OpenAgents app on your phone.** A checkbox **Let this phone open a terminal on this Mac** (off). A small **Can't scan? Copy a code instead**, which copies the same text once. The code changes quietly every minute. |
| `DSK-02` | Connected | **Kai's iPhone is connected.** Then one setup row: **Pick a project for Coder** with **Choose folder…** (a Git checkout), and whether Coder can run here: **Coder uses Codex or Claude Code on this Mac** with a check for each that is signed in. A switch **Let my phone start Coder here** (on once a project is picked). |
| `DSK-03` | Home | Status (**Online. Your phone can reach this Mac.** or **Offline.**), **Phones** (name, last seen, terminal allowed, **Remove**), **Coder** (running and recent tasks with their titles), **Connect another phone**. |
| `DSK-04` | A phone nearby wants to connect (after the milestone) | **Kai's iPhone wants to connect. Check that your phone shows 482 913.** **Connect** / **Don't connect**, and the terminal checkbox. |
| `DSK-05` | Menu bar | The status line, **Open OpenAgents**, **Connect a phone…**, **Pause Coder** (no new tasks start), and **Quit OpenAgents** (closes the window; Coder keeps running) beside **Stop Coder on this Mac** (unregisters the agent). |

Words follow the wireframe's [Words on screen](../../product/2026-09-28-app-wireframe.md#words-on-screen):
no key, host, relay, grant, workspace, tailnet, Tailscale, npub, or nsec on
these screens. "Project" names a workspace; "phone" names a device.

### The QR code

The payload is the text `openagents-connect:` followed by unpadded base64url
of these bytes: version `1`; host Nostr x-only key (32); host `EndpointId`
(32); invitation ID (32); capability (32, independently random); issue time
and expiry (8 each, big-endian seconds); iroh relay URL length (1) and UTF-8
bytes (0–128; empty means none); count of direct addresses (1, at most 8),
each a family byte (4 or 6), the address (4 or 16), and a port (2); label
length (1) and UTF-8 bytes (0–48, the computer's name for display, never an
identity). It is at most 654 characters (476 bytes before encoding), well
inside a QR code a phone reads from a laptop screen. The invitation ID, capability, times, and rights
record are exactly NIP-HOST's host invitation; only the carriage is new, and
the Nostr relay is not in the payload because the grant names it.

**One-time, short, and only on an unlocked screen.**

- The code exists only inside the desktop app's window, only while that
  window is visible and the screen is unlocked. Hiding the window, locking
  the screen, or ten idle minutes cancels every outstanding code
  (NIP-HOST's existing `invite.cancel`).
- The app shows a new code every 60 seconds and cancels the one it replaced
  60 seconds later, so a scan in flight still lands, and no code is
  redeemable more than two minutes after it left the screen. The NIP-HOST
  life of 300 seconds is unchanged; the app only ever shortens it. A
  successful pairing cancels all outstanding codes.
- Redemption is single-use, as NIP-HOST already requires (first valid
  redemption binds the device; any other device gets `forbidden`).
- The copied text form carries the same secret; it is copied only on a tap,
  and the clipboard entry is marked to expire after 60 seconds where the OS
  allows.

**What the scan authenticates.** The QR is read off the computer's own
screen, so it is the trusted channel. It gives the phone both of the
computer's public keys and a secret only the computer knows:

1. The phone dials the `EndpointId`; iroh's TLS proves the far end holds
   that key.
2. The phone sends `enroll.redeem` (invitation ID and capability, signed by
   its device key) on the `openagents/enroll/1` ALPN.
3. The host answers with the grant, signed by the host Nostr key from the
   QR. The phone accepts it only if both keys match the QR.

A fake computer on the path would need the host's iroh secret, its Nostr
secret, and the capability. A person photographing the screen gets at most
one redemption race inside two minutes, and the real phone then sees
`forbidden` and says so.

**Rights.** A QR pairing grants `observe` and `operate`. The checkbox on
`DSK-01` adds `terminal`, and it is part of the invitation the code carries,
so the choice is made on the computer before the scan. `review`,
`access_read`, and `access_admin` never come from the desktop QR. Reasons:
T3 grants `terminal:operate` by default, but a terminal on a phone is full
shell access to the Mac, and a person pairing to "send work to Coder" does
not expect that. The read-only command cards in chat run through
`terminal.open` (`crates/openagents-mobile/src/cli_run.rs`), so they appear
only for a phone that was allowed a terminal; the phone says why otherwise,
as it does today. Changing rights later means **Remove** and scan again with
the checkbox set; an in-place rights change is an [open question](#open-questions).

**No typed short code.** The first draft let a person type the first eight
characters of the `EndpointId`. That is not an iroh feature (lookup is by the
full key) and it is enumerable, so it is dropped. The fallback for a phone
that cannot scan is the copied text. A typed-words rendezvous through a relay
is not specified; nobody has asked for it. NIP-HOST's reverse enrollment with
its 40-bit, five-attempt code stays for headless hosts reached over SSH.

### Nearby pairing (after the milestone)

1. The host publishes itself with `iroh-mdns-address-lookup` under service
   name `openagents` (so `_openagents._udp`). The record carries only the
   `EndpointId` and addresses.
2. **Connect a computer** on the phone lists **Nearby** computers above the
   scanner. Tapping one dials it on `openagents/enroll/1` and sends an
   enrollment request: the device's Nostr key, a label, and a commitment
   `SHA-256(nonce_d)`.
3. The host replies with its Nostr key and `nonce_h`; the phone reveals
   `nonce_d`. Both compute a six-digit code from SHA-256 of
   `openagents.connect-sas.v1`, a zero byte, and both `EndpointId`s, both
   Nostr keys, and both nonces. The commitment keeps either side from
   choosing its nonce after seeing the other's, so a device in the middle,
   which holds its own keys on each side, gets one one-in-a-million guess per
   attempt.
4. `DSK-04` shows the phone's label, the code, and the terminal checkbox;
   the phone shows the same code. The person compares and clicks
   **Connect**. Nothing is approved without that click. The host keeps one
   nearby request pending at a time and at most five per ten minutes.
5. The host signs an ordinary grant with origin `approval`, as in NIP-HOST's
   approval path, and returns it on the same connection.

Platform permissions:

- **iOS.** Any traffic to a local-network address, unicast included
  (an outgoing TCP connection, a UDP unicast, any multicast), triggers the
  Local Network prompt on iOS 14 and later
  ([TN3179](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)).
  So the **QR path** needs `NSLocalNetworkUsageDescription` in
  `bins/openagents-ios/host/App/Info.plist` too, because the phone dials the
  Mac's LAN address from the code. Suggested text: "Find and connect to your
  computers on this Wi-Fi." Nearby pairing uses raw multicast sockets
  (`swarm-discovery`), which needs the multicast entitlement in the app's
  entitlements file (today it holds only `aps-environment`) and a
  regenerated provisioning profile. List `_openagents._udp` in
  `NSBonjourServices` as well. The prompt appears once; a denial means the
  QR path falls back to the relay and the phone says "Allow Local Network
  for OpenAgents in Settings to connect faster on Wi-Fi."
- **macOS.** Local network privacy applies from macOS 15. The desktop app's
  `Info.plist` carries `NSLocalNetworkUsageDescription`; the multicast
  entitlement is not required on macOS (TN3179).
- **Android.** `INTERNET` and `CAMERA` are already declared
  (`bins/openagents-android/host/app/src/main/AndroidManifest.xml`). Receiving
  multicast needs `CHANGE_WIFI_MULTICAST_STATE` and a held
  `WifiManager.MulticastLock` while the nearby list is open. Local network
  access becomes a runtime permission, `ACCESS_LOCAL_NETWORK` in the
  `NEARBY_DEVICES` group, for apps targeting Android 17 (API 37); on Android
  16 it is opt-in and uses `NEARBY_WIFI_DEVICES`
  ([Android docs](https://developer.android.com/privacy-and-security/local-network-permission)).
  The app targets SDK 35 today, so nothing is required yet; the permission
  goes in with the target bump.

### The local owner surface

The desktop app is the owner's surface on that computer. The host process
serves a **local control socket** at
`~/Library/Application Support/OpenAgents/control.sock` (on Linux
`$XDG_RUNTIME_DIR/openagents/control.sock`), in a directory of mode `0700`,
the socket `0600`. On every accepted connection the host reads the peer's
user ID (`getpeereid` on macOS, `SO_PEERCRED` on Linux) and refuses any peer
whose user ID differs from its own. Windows uses a named pipe with a
security descriptor for the current user only.

A caller that passes is the local operator, which NIP-HOST already treats as
the owner acting "with an operator command on that machine". Over the socket
it can create and cancel invitations, list and revoke devices, set the
auto-start policy and projects, and read status. The window and
`openagents connect` both use it, so there is one code path for the owner's
local actions. The existing direct-store CLI commands keep working for
CLI-only installs until the migration step retires them.

### Connecting after pairing

The phone stores the host's `EndpointId`, relay URL, and last direct
addresses beside its NIP-HOST access record. The host adds an `iroh` hint to
its signed NIP-REACH hints (transport `iroh`, address the `EndpointId`, with
the relay URL and current direct addresses), so the phone learns new
addresses without pairing again. The endpoint uses `presets::Minimal`,
`RelayMode::Custom` with our relay, and `MemoryLookup` fed from those two
sources; no n0 DNS, no n0 relays, no DHT.

Order of routes: iroh (iroh itself prefers a direct path and falls back to
our relay), then the existing Nostr relay transports. **Run Coder**
(`task.create`) and terminals therefore work even when iroh cannot connect;
they are only slower.

### The relay

`iroh-relay` does not fit Cloud Run: Cloud Run accepts only HTTP(S) and gRPC
inbound ([container contract](https://cloud.google.com/run/docs/container-contract)),
so UDP 7842 is unreachable, and it terminates TLS itself. It runs on a small
GCE VM in `openagentsgemini` (an `e2-small` in `us-central1` is enough to
start), with a static IP and the name `iroh.openagents.com`, a systemd unit,
`cert_mode = "LetsEncrypt"`, QAD on, metrics bound to localhost, firewall
TCP 80 and 443 and UDP 7842. Access starts as `everyone` with the server's
rate limits; it relays only encrypted QUIC and holds no account data. n0's
relays are never configured. `relay.openagents.com` stays where it is, the
Nostr relay on Cloud Run.

### Time

Unchanged: readers accept an `issued_at` up to 60 s ahead and hold
`expires_at` strictly; the host decides expiry. The enrollment reply carries
the host's current time, and the phone shows **Your phone's clock is off by
N minutes** when the difference is over 60 s, so a clock problem names itself
instead of reading as "expired".

### Revocation and recovery

- **Remove** on `DSK-03` is `device.revoke`: the epoch advances and open
  channels close before the next message.
- Removing the computer on the phone deletes its access record there; the
  computer still lists the phone until someone removes it.
- A reinstalled desktop app on the same Mac finds its keys in the keychain,
  so paired phones reconnect. Deleting the keychain items makes a new host;
  phones show it offline and the person scans again.

### Upgrading a computer set up the old way

On first launch, the app looks for an existing Coder host
(`~/.openagents/coder-access/` and the launchd agent that
`coder-service service install` wrote). If it finds one, it asks **Use this
Mac's existing Coder setup?** and on yes: moves the host key (and the owner
key, when `~/.openagents/coder-owner/owner.key` belongs to this host's owner)
into the keychain, verifies them by reading back, deletes the files,
uninstalls the old agent, and registers its own agent on the same
`~/.openagents` state. Grants, projects, the auto-start policy, and tasks
stay, and phones already paired keep working because the host key is the
same.

### Names: one surface

| Name | Fate |
| --- | --- |
| **OpenAgents** desktop app, and **Connect a computer** on the phone | The one path the app and the docs show. |
| `openagents connect` (`invite`, `devices`, `remove`, `status`, `owner import`) | The power-user and headless path; talks to the same local socket. |
| `coder link *`, `scripts/link-device.sh` | Deprecated when the migration step landed, kept for the release that shipped the desktop app and phone scanner ([#9974](https://github.com/OpenAgentsInc/openagents/issues/9974)), then removed ([#9978](https://github.com/OpenAgentsInc/openagents/issues/9978)). |
| `coder pair` and `./pair` (the chat-history observer's pairing) | Removed the same way; a QR-paired phone reads chats through its NIP-HOST grant's `observe`. `openagents pair` keeps the read-only observer. |
| Tailnet admission (`--tailnet-admission`, port 47109) and the phone's Tailscale device list (`crates/openagents-mobile/src/tailnet.rs`) | Kept as an optional path, not deprecated, alongside QR pairing and iroh. |
| Phone: **Add a computer > Scan invitation / Paste invitation**, **Enter owner key** | Replaced by **Connect a computer** (scanner, nearby list, **Paste a code**) as the path the app shows. **Add a computer** stays for `coder-host:` invitations from `coder host invite`, which still redeem, and **Enter owner key** for the owner directory, under Advanced. |

### Chat on the phone

- **Connect a computer** (the chip under a reply, `SCR-17.E05` and
  `SCR-17.E06`, and the row in Account > Computers) opens a new screen
  `SCR-22` **Connect a computer**: the camera with the line **Point at the
  code on your computer**, **Nearby** above it once nearby pairing ships,
  and **Paste a code** below. No computer to connect? A line: **Get
  OpenAgents for Mac at openagents.com/desktop.**
- After a scan, `SCR-23` **Connected**: the computer's name, a check, and
  **Done**, which returns to the chat the chip came from. The reply's chip
  now reads **Run Coder on Studio Mac** and dispatches through the existing
  Run Coder path (`task.create`) to that computer.
- The `SCR-11.E07` Advanced row keeps Computers for listing and removing
  computers.

## Dependency review

A scratch crate at the repository's `deny.toml` (cargo-deny 0.20.2, Rust
1.97.1) with `iroh = "=1.2.0"` and `iroh-mdns-address-lookup = "=0.5.0"`:

- With iroh's default features: 374 packages; **advisories ok, bans ok,
  sources ok, licenses failed** on five crates: `attohttpc` 0.30.1
  (MPL-2.0, through `portmapper` → `igd-next`), `spez` 0.1.2 (BSD-2-Clause,
  a proc macro under `n0-error`), and `ws_stream_wasm` 0.7.5,
  `async_io_stream` 0.3.3, `pharos` 0.5.3 (Unlicense, wasm32-only, under
  `iroh-relay`; they appear because `deny.toml` inspects every target).
- With `default-features = false, features = ["tls-ring",
  "fast-apple-datapath"]` (no `portmapper`, no metrics): 367 packages;
  `attohttpc` is gone and the other four remain. The resolved graph adds 105
  crate names the workspace lock does not have (among them `noq*`,
  `netwatch`, `hickory-proto`, `swarm-discovery`, `ed25519-dalek` 3,
  `curve25519-dalek` 5, `rustls-platform-verifier`). TLS is `rustls` with
  `ring`, as `crates/openagents-mobile` already uses; no `aws-lc-rs`.

So step 1 disables iroh's default features (losing UPnP/NAT-PMP port
mapping, which hole punching and the relay make up for), and adds two
per-crate license exceptions to `deny.toml` with reasons in
`docs/dependencies.md`: `BSD-2-Clause` for `spez`, and `Unlicense` for the
three wasm-only crates. Pin with `=`: `iroh` and `iroh-relay` at the newest
1.x release at least seven days old when the step lands (1.3.0 qualifies
from 2026-10-05). `crates/openagents-mobile` is its own Cargo workspace and
lockfile, so the phone's copy is reviewed there too.

The desktop crates (`keyring` 4.2.0, `objc2-service-management` 0.3.2,
`objc2-app-kit` with `NSStatusItem`) pass licenses, sources, and advisories
in a second scratch crate; `tray-icon` does not (above).

## Invariant changes

Each lands in `INVARIANTS.md` in the same PR as the code it describes.

| Row | Change | Step |
| --- | --- | --- |
| Linking devices: "Tailscale, SSH, and a relay only introduce devices" | Add an iroh connection, a QR scan, and nearby discovery to the list of things that only introduce. Also: an `EndpointId` never admits; the NIP-REACH handshake and the grant do. | 4 |
| New, Linking devices | The desktop app shows a code only in its visible window on an unlocked screen, replaces it every 60 s, cancels a replaced code 60 s later and all codes when hidden, locked, idle ten minutes, or after a pairing; one redemption per code. | 5 |
| New, Linking devices | A QR pairing grants `observe,operate`, plus `terminal` only when the checkbox was set before the code was shown; never `review` or an access right. | 5 |
| New, Linking devices | The local control socket is `0600` in a `0700` directory, and the host serves a peer only when its user ID equals the host's. It is the only local path that changes access or auto-start once the desktop app manages the host. | 4 |
| Linking devices: "The owner secret key stays in one private file" | Under the desktop app, the owner, host, and iroh secret keys live in the OS keychain, read only by the host process, never in a file, argument, or log line; CLI-only installs keep the file. | 4, 5 |
| "Only the host's owner, with a command on the host, turns auto-start on or widens it" | Reinterpreted: a request over the local control socket (the desktop app's switch or `openagents connect`) is a command on the host; a device still cannot. | 4 |
| New, Linking devices | Nearby pairing admits a device only after the person clicks **Connect** on the computer while both screens show the same six-digit code; one pending request, five per ten minutes. | 10 |
| Device identity key | The phone's iroh secret key is kept in the same this-device-only store as its device key. | 6 |

## Plan

The epic is [#9965](https://github.com/OpenAgentsInc/openagents/issues/9965); each step below links its issue. Waves are ordered so
the milestone (step 9) comes first; steps 10–14 follow it. The relay
(step 3) is in the milestone, because the milestone must work with the
phone on mobile data; if iroh cannot connect, the Nostr relay transports
still carry pairing, Run Coder, and terminals, only more slowly. Adopting
an old-style host (step 8) is in the milestone because the owner's own Mac
runs one. Within a wave,
steps own disjoint files; the shared `Cargo.toml` members list and
`Cargo.lock` are the only common files, resolved by rebasing.

**Wave 1** (no dependencies)

1. **`crates/openagents-connect`: endpoint, ALPNs, QR payload, control
   protocol.** ([#9966](https://github.com/OpenAgentsInc/openagents/issues/9966)) iroh pinned as above with default features off; the
   `openagents/enroll/1` and `openagents/reach/1` ALPNs; NIP-REACH channel
   over a QUIC bidirectional stream (the `coder-reach` channel over the
   stream's reader and writer); the `openagents-connect:` payload with
   fixtures; the local control protocol types; a key-source trait with a file
   implementation. `deny.toml` exceptions and `docs/dependencies.md`. Tests:
   two endpoints on loopback with relays disabled complete the handshake and
   exchange frames; payload round-trip and every malformed case.
2. **NIP updates.** ([#9967](https://github.com/OpenAgentsInc/openagents/issues/9967)) NIP-HOST: the `openagents-connect:` carriage of a host
   invitation, redemption on the enroll ALPN (every check but the relay
   binding), the local control socket as the operator, and approval with a
   confirmation code. NIP-REACH: the `iroh` hint transport and the channel
   over an iroh stream.
3. **Relay.** ([#9968](https://github.com/OpenAgentsInc/openagents/issues/9968)) `deploy/iroh-relay/` (config, systemd unit, firewall and
   VM commands) and `docs/deployment/iroh-relay.md`; the VM running at
   `iroh.openagents.com` with QAD on. Test: two endpoints on different
   networks with direct paths blocked exchange data through it.

**Wave 2** (after 1)

4. **Host listener and local control socket.** ([#9969](https://github.com/OpenAgentsInc/openagents/issues/9969)) `coder host serve` binds the
   iroh endpoint with its key from the key source, serves both ALPNs through
   the existing redemption and direct-channel dispatch (NIP-TERM included),
   publishes the `iroh` hint, and serves the control socket with the peer
   user check. `openagents connect invite|devices|remove|status|owner
   import` in `crates/openagents-cli`. Tests: a device redeems over iroh and
   opens a terminal; a peer with another user ID is refused; revocation
   closes an iroh channel.
5. **Desktop app shell.** ([#9970](https://github.com/OpenAgentsInc/openagents/issues/9970)) `crates/openagents-desktop`: `DSK-01` to `DSK-03`,
   the rotating code, the terminal checkbox, keychain key source, agent
   registration with `SMAppService`, project picker and auto-start switch
   over the control socket, a bundle layout script. Tests: the code screen's
   rotation and cancellation against a fake socket; snapshot tests of each
   screen's cells.
6. **Phone: scanner and pairing.** ([#9971](https://github.com/OpenAgentsInc/openagents/issues/9971)) The chip and Computers open `SCR-22`;
   parse the payload; an iroh endpoint in `crates/openagents-mobile` with its
   key beside the device key; redeem over iroh with relay fallback; `SCR-23`;
   route the channel over iroh; `NSLocalNetworkUsageDescription`; the
   Android JNI context. Tests: a phone model pairs with a test host over
   loopback iroh; a mismatched host key in the reply is refused.

**Wave 3** (after 4, 5, 6)

7. **Signed, notarized macOS package.** ([#9972](https://github.com/OpenAgentsInc/openagents/issues/9972)) `scripts/desktop/package-macos.sh`:
   build universal binaries, assemble the bundle, sign with the Developer ID
   and hardened runtime, notarize with `notarytool`, staple, and produce the
   `.dmg`; `docs/desktop/release.md`. Manual, no GitHub automation. The
   Developer ID certificate is an owner step.
8. **Adopt an existing host.** ([#9973](https://github.com/OpenAgentsInc/openagents/issues/9973)) The first-launch migration above, so the
   owner's own Mac and `coderos-4080` move over without re-pairing their
   phones.
9. **Milestone: TestFlight build.** ([#9974](https://github.com/OpenAgentsInc/openagents/issues/9974)) A person with a Mac and an iPhone, no
   Tailscale, installs the app from the `.dmg`, scans, and sends a Coder task
   from chat in under two minutes with no terminal.

**Wave 4** (after the milestone)

10. **Nearby pairing with a confirmation code** ([#9975](https://github.com/OpenAgentsInc/openagents/issues/9975)), including the iOS multicast
    entitlement and Android permissions.
11. **Auto-update and menu bar.** ([#9976](https://github.com/OpenAgentsInc/openagents/issues/9976)) A Rust updater that checks a signed
    manifest, downloads the new notarized build, verifies its signature and
    code signature, swaps the bundle, and restarts the agent; `DSK-05`.
12. **Linux and Windows desktop builds.** ([#9977](https://github.com/OpenAgentsInc/openagents/issues/9977))
13. **Deprecate and remove the old path.** ([#9978](https://github.com/OpenAgentsInc/openagents/issues/9978)) Aliases and notices for the names
    above, the rewritten [Link your devices](../guides/link-devices.md) with
    Tailscale under "Using Tailscale (optional)", then removal of the old
    setup commands. Tailnet admission and the chat-over-tailnet listener are
    out of scope: they stay as an option.
14. **`openagents connect --ssh`** ([#9979](https://github.com/OpenAgentsInc/openagents/issues/9979)) for a headless box: install, start the
    host, and redeem over the SSH channel.

## Acceptance

The milestone (step 9) is accepted when, on a real Mac with Tailscale not
installed and an iPhone on the TestFlight build:

- The app installs from the `.dmg` with no warning from Gatekeeper and shows
  the QR code on first launch.
- The phone scans it from **Connect a computer**, and the computer shows as
  connected on both screens, with the phone on the same Wi-Fi and again with
  the phone on mobile data.
- From a chat, **Run Coder** sends a task that runs on that Mac and streams
  its reply into the chat.
- With **Let this phone open a terminal** set, the phone opens a terminal on
  the Mac and runs a read-only command card; without it, the phone says the
  computer has not allowed a terminal and the host refuses the request.
- **Remove** on the Mac cuts the phone off: its next request is refused and
  an open terminal closes.
- Start to first task in under two minutes, with no terminal on the Mac.

Beyond the milestone: a phone with its clock 30 s behind pairs, and a code
older than its window is refused with `expired`; a phone paired the old way
reconnects over iroh after the host upgrades, with no new pairing; nearby
pairing needs the click on the computer and refuses a wrong code.

## Open questions

Each has the default this plan assumes.

- **Which model runs a repository task on a fresh Mac?** Resolved by the
  owner on 2026-09-29: "You can assume the user has already authed at the
  terminal (Codex/Claude Code signed in), but I don't want them to have to do
  terminal tasks to do pairing." So a local coding agent, Codex or Claude
  Code, already signed in on the Mac is a prerequisite of the milestone, and
  repository runs do not fall back to the OpenAgents cloud route
  (`docs/coder/runtime/host-autostart.md`). Pairing itself (install, QR,
  scan, connected, **Run Coder**) needs no terminal step. `DSK-02` shows
  which agents are signed in, and says in one line when neither is.
- **Rights change without re-pairing.** Default: **Remove** and scan again.
  A local "allow terminal" that issues a replacement grant needs a NIP-HOST
  origin for it; decide after the milestone.
- **A web page for a QR scanned by the camera app.** Default: the in-app
  scanner only. A later `https://openagents.com/connect#…` form with the
  payload in the fragment would open the app from the system camera.
- **Idle and background behavior on iOS.** iOS suspends the app's UDP
  socket in the background. Default: reconnect on foreground, as the
  WebSocket channel does; confirm with QUIC in step 6.
- **Relay access control.** Default: `everyone` with rate limits; move to a
  callout that admits only endpoints with a current grant if abuse appears.
- **One owner for several computers.** Default: one owner per desktop app,
  with `openagents connect owner import` for people who want one directory.
