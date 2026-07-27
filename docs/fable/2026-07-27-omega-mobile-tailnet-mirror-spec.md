# Omega chat on mobile: the tailnet mirror and the zero-based home screen

- Date: 2026-07-27
- Lane: Fable design specification
- Status: owner-directed design, 2026-07-27. Strategic evidence, not
  dispatch authority. Implementation needs claimed issues under the
  normal omega and monorepo discipline.
- Omega source pin: `OpenAgentsInc/omega` `beb0e870b2`
- Monorepo pin: `OpenAgentsInc/openagents` `28b1b03967`
- Labels: claims carry `[EXISTS]`, `[NEEDS BUILD]`, or `[SPECULATION]`

## Synopsis

The owner wants Omega desktop chat visible on the OpenAgents mobile app
now.
The mobile home screen gets a zero base: one screen, the minimum needed
to mirror the desktop.
Both devices sit on the owner's tailnet, so the mobile app dials the
desktop directly and shows what the desktop is doing, live.

The design adds one component and one screen.
The component is a device bridge: a small authenticated WebSocket server
inside `omega-effectd`, bound to loopback and the tailnet interface.
The screen is the zero-based home: a paired-desktop header, a live
activity feed, and a read-only thread view.
The existing Nostr workroom lane stays the durable record and the
discovery rendezvous.
The tailnet link is a live mirror pipe, never a second truth.

## 1. The owner direction

1. Sync Omega chat with the OpenAgents mobile app as soon as possible.
2. Zero-base the mobile home screen. Keep the minimum necessary to sync
   with the desktop.
3. Assume both devices are on the owner's tailnet.
4. Discover the desktop from the mobile app on that tailnet.
5. Auto-show anything going on in the desktop, also in the mobile app.
6. This document is the specification. Implementation follows later.

## 2. What exists

### 2.1 Desktop side `[EXISTS]`

- Omega Agent threads run in the native loop with local persistence,
  executor disclosure records, and send-queue semantics.
- `omega-effectd` is the packaged engine. It owns Full Auto runs,
  receipts, and the Sync and mobile projections. The framed
  `openagents.omega.effectd.v1` protocol carries 64 KiB frames with
  generation fencing.
- The workroom law stands: GPUI never opens its own socket. Engine
  capabilities own every server.
- The issue31 lane already defines host discovery, pairing, owner
  projection, and command records over Nostr relays
  (`crates/omega_effectd/src/issue31_nostr.rs`).
  `Issue31HostDiscoveryV2` carries host ref, host public key, display
  name, protocols, relay urls, a generation, and an expiry.
  Records travel NIP-59 wrapped and identity-signed.
- The owned relay `relay.openagents.com` is live with authenticated
  round trips (omega#45 foundations).
- Grant revocation outlives a restart on the community room lane
  (omega#48 work), and hosted grants persist across restarts on current
  `main`.

### 2.2 Mobile side `[EXISTS]`

- The app is Expo React Native with Effect Native components and typed
  tokens, per the standing mobile policy.
- The issue31 mobile runtime exists:
  `apps/openagents-mobile/src/workroom/issue31-mobile-nostr-runtime.ts`
  plus a device key vault, relay cursor store, outbound event store, and
  read models for owner-private and community records.
- The home screen at
  `apps/openagents-mobile/src/screens/home-screen.tsx` is a kitchen
  sink. It loads Khala sync, Sarah, Full Auto, managed sandboxes, the
  issue31 workroom, coding composer surfaces, git, files, terminal, and
  settings ports into one surface.
- The D-track state: omega#45 landed, omega#46 renders on the
  simulator, omega#49 physical-device proof has not started.

### 2.3 The tailnet constraint `[EXISTS]`

Tailscale gives every owner device a stable address and MagicDNS name.
Tailscale does not carry multicast or mDNS between devices.
A phone cannot broadcast-scan the tailnet, and the CGNAT range is not
scannable.
So "local discovery" cannot mean LAN broadcast.
It must mean a signed announcement the phone can read, plus a direct
dial.
This specification says that honestly instead of promising broadcast
discovery that cannot work.

## 3. The design: two transports, one record

### 3.1 The rule

The Nostr workroom record stays the durable truth and the rendezvous.
The tailnet device bridge is a direct, low-latency, ephemeral mirror.
When the two disagree, the record wins for history and the bridge wins
for liveness.
The mobile app stores no second durable copy from the bridge.

### 3.2 The device bridge `[NEEDS BUILD]`

A WebSocket server inside `omega-effectd`, never inside GPUI.

1. Bind to loopback and the tailnet interface only. Never a public
   bind. Never a relay through any cloud service.
2. The protocol is typed, versioned, and generation-fenced:
   `openagents.omega.device_bridge.v1`. Frames stay within the 64 KiB
   budget of the engine protocol family.
3. Every connection authenticates at the application layer. The tailnet
   is a network boundary, not the authority boundary. The client proves
   possession of a paired device key from the existing mobile device
   key vault, and the host checks the grant against the same revocation
   state the relay lane uses.
4. A revoked device grant closes both transports. The bridge must not
   outlive a revocation the relay lane already enforces.

### 3.3 Discovery and pairing `[NEEDS BUILD]`

The ladder, in order:

1. **Last-known endpoint.** The app caches the last good bridge
   endpoint and dials it first.
2. **Relay announcement.** The host extends its discovery record with
   signed direct endpoints: MagicDNS name, port, bridge protocol id,
   generation, and expiry. A V3 of `Issue31HostDiscoveryV2` or a
   sibling record carries the field. The phone reads the announcement
   from the owned relay and dials the endpoint directly.
3. **QR pairing.** First contact, and the relay-less bootstrap. The
   desktop shows a QR code that carries the endpoint, the host public
   key, and a one-time pairing secret. The phone scans it, dials the
   bridge, and completes the pairing grant over the direct link.
4. **Manual entry.** The person types the MagicDNS name. Always
   available, never required in the normal path.

Auto-show follows from steps 1 and 2: a paired phone on the tailnet
reconnects without any tap.

### 3.4 The wire contract `[NEEDS BUILD]`

Frames, all typed:

| Frame | Direction | Content |
| --- | --- | --- |
| `hello` | phone to host | protocol version, device key proof, resume cursor |
| `grant` | host to phone | session admission, snapshot generation, or a typed refusal |
| `snapshot` | host to phone | the active mirror state: thread list and run list |
| `delta` | host to phone | one mirror change: a thread entry, a streaming text delta, a disclosure record, a turn state change, a run state change, a receipt ref |
| `heartbeat` | both | liveness and generation |
| `bye` | both | typed close reason |

Rules:

1. Deltas carry a monotonic sequence per snapshot generation. A gap
   forces a fresh `snapshot`. Resume uses the cursor from `hello`.
2. The mirror is ephemeral. Reconnect re-snapshots. Nothing on the
   phone claims durability from this pipe.
3. Version 1 is read-only. The bridge accepts no command frames. A
   command frame gets a typed refusal.
4. Redaction follows the existing owner-projection schema. No raw
   provider payloads, no credentials, no local absolute paths beyond
   what the owner projection already admits.

### 3.5 What the mirror carries `[NEEDS BUILD]`

"Anything going on in the desktop" means, at version 1:

1. Active and recent Omega Agent threads: title, executor disclosure
   record, turn state, and the live transcript stream.
2. Full Auto runs: lifecycle state, lane, and receipt refs.
3. Engine health: engine up, engine generation, and lane readiness.

The source is the existing projection path.
The host bridge already drives panel threads and publishes projections
into the engine.
The device bridge subscribes to the same engine-side projection state
the Sync lane uses, so GPUI gains no new authority and no new socket.

## 4. The zero-based home screen `[NEEDS BUILD]`

### 4.1 What the screen is

One screen, three parts, nothing else:

1. **The desktop header.** The paired desktop's name and a connection
   state with exactly three honest values: `direct` (bridge connected),
   `relay` (record-following only), `offline` (neither, showing the
   last mirror with a staleness label).
2. **The activity feed.** Threads and runs, most recent activity first,
   streaming live. Each row shows the title, the executor disclosure,
   and the current state. The feed is the auto-show surface: it updates
   without any tap.
3. **The thread view.** Tap a row, read the transcript live.
   Read-only at version 1.

### 4.2 What the zero base removes from home

The current home screen loads Khala sync, Sarah, Full Auto controls,
managed sandboxes, the coding composer stack, git, files, terminal, and
settings ports.
None of that renders on the zero-based home.
The code is not deleted. The surfaces leave the home screen and remain
reachable behind navigation, or stay dormant until their own screens
return deliberately.
This mirrors the Omega zero-base law: hide by filter, not by deletion.

### 4.3 Empty and failure states

1. Not paired: the home screen is the pairing screen. One button, scan
   the QR on the desktop.
2. Paired, desktop unreachable: the last mirror renders with an honest
   staleness label and the `offline` state. No spinner theater.
3. Tailnet off on the phone: the state names the reason when the app
   can detect it, and says `offline` when it cannot.

### 4.4 Implementation posture

1. Effect Native components with typed style objects on the shared
   tokens, per the standing mobile policy. No new styling system.
2. Behavior contracts land with the screen, per the repository mandate.
   The two owner expectations are stated now: "the home screen shows
   desktop activity automatically when paired" and "the connection
   state is always visible and honest."
3. Coordination warning: `home-screen.tsx` and the issue31 mobile
   runtime have live concurrent edits in the canonical checkout today.
   The zero-base work must claim its lane and serialize on those files.

## 5. Authority and truth laws

1. The mirror is a projection. The phone never becomes a second durable
   authority for thread or run state.
2. Version 1 is read-only end to end. A later composer stage sends
   typed commands through the same authority path the issue31 command
   records use, with the same grants, and lands as its own gated stage.
3. The bridge binds to loopback and tailnet only. No public exposure,
   no proxy through any OpenAgents cloud surface, no new cloud service.
   The Google Cloud production authority is untouched because this is
   device-to-device.
4. Device grant revocation applies to both transports at once.
5. No credential copy. The phone holds its own device key. The desktop
   holds its own identity. Pairing exchanges grants, never keys.
6. Records that later need durability go through the workroom record
   lane, signed, as today. The bridge never writes records.

## 6. Staging

| Stage | Delivers | Proof |
| --- | --- | --- |
| M0 | The device bridge server, QR pairing, and the thread-list snapshot on the zero-based home | Simulator journey: pair, see the thread list appear with no tap |
| M1 | Live deltas: streaming transcript, disclosure, turn state. Reconnect with resume cursor. Relay endpoint announcement and auto-redial | Kill the app, reopen, feed catches up. Change tailnet networks, feed recovers |
| M2 | Runs and receipts in the feed. The three-state connection header. Staleness labels | Engine restart shows honest states end to end |
| M3 | The composer: send and steer from the phone as typed commands. Owner-gated | Physical-device proof per the omega#49 protocol |

M0 through M2 are read-only and need no new owner gate beyond normal
packet admission.
M3 changes authority reach and waits for its own admission.
Physical-phone evidence follows the D-track evidence protocol, and a
simulator pass is never a packaged claim.

## 7. Risks, stated plainly

1. **iOS backgrounding.** The socket drops when the app backgrounds.
   The resume cursor and re-snapshot make that cheap. Push notification
   wake-ups are a later, separate lane.
2. **Tailscale state on the phone.** The VPN toggle changes
   reachability outside the app's control. The tri-state header exists
   for exactly this. The app never claims `direct` without a live
   heartbeat.
3. **Two-transport divergence.** The bridge and the relay can disagree
   during partitions. The rule in section 3.1 resolves it, and the
   mirror's ephemerality makes the resolution cheap.
4. **Scope creep.** The composer, Sarah, Full Auto control, and
   sandbox surfaces will all ask to return to the home screen. The
   zero base holds until the owner adds a surface back deliberately.
5. **Concurrent edits.** The mobile home screen is under live edit by
   another lane today. Claim before mutation, per the standing rule.
6. **Discovery honesty.** If the relay is down and the cache is cold,
   discovery falls to the QR or manual rungs. The app says so instead
   of spinning.

## 8. What this specification does not decide

1. It mints no issues and claims no lane. The owner asked for the
   specification first.
2. It does not choose the exact V3 discovery-record shape. The
   implementing packet freezes it with fixtures, like every issue31
   record before it.
3. It does not decide when the composer stage (M3) is admitted.
4. It does not move the D-track issues (omega#46 through omega#49).
   The zero-based home consumes the same lane and does not replace its
   physical-proof obligations.
5. It does not touch Khala routing, metering, or any cloud lane.
