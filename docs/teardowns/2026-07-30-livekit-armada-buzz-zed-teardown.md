# LiveKit across Armada, Buzz, Zed, and Sarah — 2026-07-30

Commit-pinned, read-only synthesis of the LiveKit and realtime-media paths in
the existing [Armada](./2026-07-30-armada-teardown.md),
[Buzz](./2026-07-21-buzz-teardown.md), and
[Zed](./2026-07-18-zed-teardown.md) teardowns, extended with source tours of
LiveKit's JavaScript Agents framework and Rust SDKs and a migration analysis
for Sarah's current OpenAI Realtime voice path. This review traced the source
trees more narrowly than the original product audits. It did not install
dependencies, run the products, connect to LiveKit or OpenAI, join a Buzz
huddle, or exercise media devices. [limitation]

## Summary

These systems do not present variations of the same integration:

- **Armada embeds LiveKit as its cross-platform room-media product.** React
  owns the room lifecycle and UI. NIP-29 relays or a Concord blind broker mint
  connection credentials. NIP-29 and DM calls trust the relay-operated SFU
  with plaintext media; Concord derives per-sender media keys and enables
  LiveKit frame E2EE while keeping community identity out of the broker.
- **Zed uses LiveKit as a bounded native media plane beneath its own
  collaboration authority.** The Zed collab service owns invitations,
  membership, roles, channels, shared projects, and room persistence. It mints
  LiveKit JWTs and changes LiveKit publish permissions. The Rust client uses a
  pinned LiveKit SDK fork for microphone audio and screen-share video only.
  Project and editor state never travels through LiveKit.
- **Buzz does not integrate LiveKit.** It captures microphone audio directly,
  encodes Opus in its Tauri process, authenticates an audio WebSocket with
  NIP-42, and forwards opaque frames through `buzz-relay`. Its optional
  Redis/QUIC mesh extends that custom transport across relay pods. One stale
  TypeScript comment mentions a “Mic track from LiveKit”; no dependency,
  token endpoint, room, credential, SDK call, or deployment setting supports
  that comment.
- **Sarah does not currently integrate LiveKit, but LiveKit Agents explicitly
  supports its OpenAI Realtime model family.** Omega connects to an
  OpenAgents-owned WebSocket gateway, and that gateway connects server-side to
  OpenAI Realtime. A LiveKit design would put Omega in a room and add an
  Agents worker that joins the room and opens its own server-side OpenAI
  Realtime WebSocket. It is compatible, but it is an architectural migration,
  not a URL or SDK substitution.

The direct answer to the Sarah question is therefore **yes**:
[LiveKit's OpenAI Realtime plugin](https://docs.livekit.io/agents/models/realtime/plugins/openai/)
runs an OpenAI Realtime model inside an `AgentSession` in Node.js or Python.
The plugin page answers provider compatibility. The source tours below answer
the separate implementation question: `agents-js` supplies the worker and
model bridge, while `rust-sdks` supplies native room/media transport and does
not itself contain an OpenAI client or agent runtime. [source]

```text
Armada
Nostr/Concord membership ── signed token request ── broker/relay
          │                                          │
          └── React room product ── JWT + URL ── LiveKit SFU
                 audio · camera · screen share · optional frame E2EE

Zed
Zed collab service ── room/membership/roles/projects ── Zed client
          │                                               │
          └── JWT + publish permission ── LiveKit ─────────┘
                                  microphone · screen share only

Buzz
NIP-29 huddle events ── direct getUserMedia ── Tauri Opus pipeline
          │                                         │
          └── NIP-42 audio WebSocket ── buzz-relay ──┘
                      optional Redis/QUIC pod mesh; no LiveKit

Sarah today
Omega ── one-use ticket + OAA1 PCM/control WSS ── OpenAgents gateway
                                                     │
                                                     └── OpenAI Realtime WSS

Sarah with LiveKit Agents
Omega ── WebRTC room via rust-sdks ── LiveKit ── agents-js worker
                                                   │
                                                   └── OpenAI Realtime WSS
```

The useful design comparison is therefore not “which LiveKit wrapper is
best.” It is **where membership authority ends and media transport begins**.
Armada demonstrates a portable web/mobile client, blind token brokering, and
application-layer media E2EE. Zed demonstrates a mature native client plus a
server-owned role-to-media-permission bridge. Buzz demonstrates the cost and
control tradeoff of owning the media transport instead of adopting an SFU.
[source] [inferred]

## 1. Snapshot, provenance, and limits

### 1.1 Exact source identity

| Project | Audited source | Media posture | License |
| --- | --- | --- | --- |
| Armada | [`soapbox-pub/armada@5b99f88d`](https://github.com/soapbox-pub/armada/tree/5b99f88d309052abc1eeb4f0b2ef437de086e709) | LiveKit JS room product across web, Electron, Android, and iOS | AGPL-3.0 client; Electron package metadata says MIT |
| Buzz | [`block/buzz@be13b4bb`](https://github.com/block/buzz/tree/be13b4bb9ce228b21fa3682ce75d75cba5950561) | Custom Opus/WebSocket huddles; no LiveKit integration | Apache-2.0 |
| Zed | [`zed-industries/zed@f032f4d4`](https://github.com/zed-industries/zed/tree/f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4) | Native LiveKit media plane below Zed collaboration control | GPL-3.0-or-later by default; marked components may differ |
| LiveKit Agents JS | [`livekit/agents-js@d5d8d048`](https://github.com/livekit/agents-js/tree/d5d8d0487d2e99f49a1b56ab6b9e82b481491955) | Server-side agent worker, room I/O, voice orchestration, and OpenAI Realtime provider plugin | Apache-2.0 framework; bundled LiveKit model artifacts have separate terms |
| LiveKit Rust SDKs | [`livekit/rust-sdks@00258d1e`](https://github.com/livekit/rust-sdks/tree/00258d1e52e563327f3ed75807ea03a189a5c2d2) | Native LiveKit room, media, data, E2EE, token, and server APIs; no agent runtime | Apache-2.0; preserve applicable bundled third-party notices |
| Sarah | Omega [`0136fca2`](https://github.com/OpenAgentsInc/omega/tree/0136fca2d11900ddc7982665482ed8cd035391c7) and this OpenAgents snapshot | Custom authenticated PCM/control gateway to server-side OpenAI Realtime | Repository-specific; outside this SDK license comparison |

The Armada and Zed identities match the exact source snapshots in their
original teardowns. The Buzz comparison uses the later Forum follow-up commit
already pinned by the Buzz teardown because it contains the current huddle and
cross-pod mesh source discussed here. [source]

Omega is a tracked Zed fork, but its current state is a separate fact. Omega
retired Zed collaboration, then commit `b7e1c3e64c3e24322e4c07f704c996570f6b099e`
deleted the active `call`, `livekit_api`, and `livekit_client` crates. Its
remaining `libwebrtc` dependency serves Sarah voice audio processing, not a
LiveKit room. Stale dev configuration and reserved protobuf fields do not
constitute an active integration. [history]

### 1.2 Evidence labels

- **`[source]`** — directly encoded in a pinned source tree or existing teardown.
- **`[test]`** — encoded in source tests or test fixtures.
- **`[history]`** — supported by checked commit history.
- **`[inferred]`** — reasoned from multiple observations.
- **`[limitation]`** — a boundary on what this source-only review proves.

### 1.3 Limits

This review proves source shape, not deployed behavior. It does not prove:

- Armada broker JWT claims, TURN topology, reconnect behavior, or E2EE
  interoperability against a real LiveKit server. The separate
  `armada-relay` backend was not in the audited tree.
- Buzz audio quality, multi-pod behavior, or failure recovery on a live relay.
- Zed's hosted collab or LiveKit Cloud configuration, production availability,
  or current upstream behavior beyond the pinned commit.
- LiveKit Agents interoperability with Sarah's exact command, accounting,
  transcript, reconnect, and audio-processing contracts.
- LiveKit Rust SDK packaged size, device behavior, WebRTC APM interaction, or
  end-to-end latency in Omega.
- cryptographic security, traffic-analysis resistance, platform permission
  behavior, or end-to-end call quality for any project.

## 2. Comparison at the authority boundary

| Concern | Armada | Buzz | Zed |
| --- | --- | --- | --- |
| Workspace/community authority | Concord client state or NIP-29/Buzz relay | NIP-29 relay and signed event log | Zed collab service and database |
| Media transport | LiveKit SFU | Custom Opus over relay WebSocket; optional Redis/QUIC mesh | LiveKit SFU |
| Credential admission | NIP-98 request to relay, or Concord channel-key proof to blind broker | NIP-42 challenge over media WebSocket; no media JWT | Collab service mints HS256 LiveKit JWT |
| Room identity | NIP-29 group id; deterministic DM id; Concord derived voice public key | Random ephemeral NIP-29 channel UUID | Random persisted 30-character LiveKit room name |
| Publish authority | Token/broker policy; membership enforced before mint | Relay membership and parent-huddle linkage | Zed role changes update LiveKit participant permission |
| Media E2EE | Concord only: per-sender derived frame keys | No separate application media-E2EE layer found | No application media-E2EE layer found |
| Product media | Audio, camera, screen share, reactions, raise hand | Desktop audio, reactions, local STT/TTS, agent participation | Audio and screen share |
| Client substrate | LiveKit JS + React components | Browser capture + Tauri Rust audio stack | Forked LiveKit Rust SDK + native GPUI/audio stack |
| Server dependency | LiveKit plus token broker/relay | `buzz-relay` is signaling, authorization, and media fanout | Zed collab plus LiveKit |
| State sent through media service | Media, room-level metadata, random or Nostr-derived participant identity | Opaque Opus frames stay inside Buzz infrastructure | Media and LiveKit participant state; no editor/project data |

All three keep conversation or workspace membership outside the audio codec
itself. Their difference is how strongly that separation is enforced:

1. Armada asks a Nostr-aware admission service for a short-lived LiveKit
   credential.
2. Zed's own collaboration service maps its database roles into LiveKit
   grants.
3. Buzz performs membership checks inside the relay that also forwards media.

Armada's blind Concord broker has the strongest privacy goal. Zed has the
clearest server-owned permission bridge. Buzz has the fewest moving services
in a single-pod deployment, but it takes ownership of jitter, codec,
backpressure, mesh, and media-operability work that LiveKit would otherwise
provide. [inferred]

## 3. Armada: LiveKit as a portable call product

### 3.1 Dependency and application boundary

Armada declares `@livekit/components-react ^2.9.20`,
`@livekit/components-styles ^1.2.0`, and `livekit-client ^2.17.2`. The audited
lock resolves `components-react` 2.9.21 and `livekit-client` 2.19.2. Vite
places LiveKit in a dedicated vendor chunk, and `CallProvider` lazily loads the
roughly 0.5 MB SDK path only on the first call. [source]

The app-level `CallProvider` owns exactly one active call.
`PersistentVoiceRoom` stays mounted across navigation, so moving among
channels does not discard the LiveKit connection. The same `VoiceRoomShell`
renders NIP-29 group calls, direct-message calls, and Concord calls. Their
differences are isolated to credential acquisition, participant identity, and
E2EE configuration. [source]

The connected room starts microphone and camera disabled. Inside the
`LiveKitRoom`, Armada mounts:

- remote audio playback;
- active-speaker, mute, and participant reporters;
- RNNoise processing;
- call bars and participant rosters;
- camera and screen-share tiles; and
- the call stage, controls, floating stage, and mobile preview.

While connected, the LiveKit roster is the active-call truth. For NIP-29
sidebar presence outside the room, a relay-published kind `39004` participant
event is a fallback projection. [source]

Primary source paths:

- [`src/components/CallProvider.tsx`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/components/CallProvider.tsx)
- [`src/components/PersistentVoiceRoom.tsx`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/components/PersistentVoiceRoom.tsx)
- [`src/components/chat/CallStage.tsx`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/components/chat/CallStage.tsx)
- [`src/hooks/useLivekit.ts`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/hooks/useLivekit.ts)

### 3.2 NIP-29 and DM credential flow

Voice is available when a group carries the `livekit` metadata tag or its
relay answers `GET /.well-known/nip29/livekit` with HTTP 204. Armada converts
the relay WebSocket origin to HTTP and caches that capability probe for five
minutes. [source]

For a group, the credential endpoint is:

```text
GET <relay-http-origin>/.well-known/nip29/livekit/<group-id>
```

For a DM, it is:

```text
GET <relay-http-origin>/.well-known/nip29/livekit-dm/<dm-room-id>
```

Armada signs a NIP-98 kind `27235` event with the current Nostr signer. The
event binds the exact URL and `GET` method and is sent as
`Authorization: Nostr <base64-event>`. The response must supply a participant
token and server URL. A missing field or non-success response becomes a
visible call error. [source]

Group room identity is the NIP-29 group id. DM identity is deterministic:

```text
dm:<lexicographically-lower-pubkey>:<lexicographically-higher-pubkey>
```

Both participants therefore request the same room without a separate
coordination record. The token query deliberately does not refresh on focus,
reconnect, or remount: minting a new token also mints a new randomized
LiveKit identity and would force an avoidable disconnect/rejoin. The source
states that relay tokens last six hours. [source]

NIP-29 and DM media are not application-layer E2EE. Their relay-operated SFU
is inside the trust boundary. Nostr authenticates admission; it does not
encrypt the media after admission. [source]

### 3.3 Concord blind broker and frame E2EE

Concord derives a voice room key and media key from the channel and current
epoch. The room name is the derived voice public key. The client requests a
credential from:

```text
GET <broker-origin>/.well-known/concord/av/<voice-room-pubkey>
```

The request carries a fresh nonce and a kind `27235` grant signed by the
derived voice secret, not by a member's public Nostr identity:

```text
Authorization: Concord <base64-event>
```

That proves possession of current channel material while withholding the
community and named membership from the broker. Broker rendezvous is
coordinated through encrypted channel presence. Occupied brokers win before
deployment or user defaults; candidates are deterministically ordered and
tried with bounded fallthrough. `https://armada.buzz` is the default public
broker. [source]

Concord configures LiveKit's E2EE worker with:

- per-identity sender keys rather than one shared key;
- AES-256-GCM frame keys;
- no automatic ratchet; and
- a media key derived separately for every channel epoch.

Encrypted signed presence binds a broker-issued random LiveKit identity to a
member. A unique valid claim receives the derived sender key. An unclaimed or
contested identity receives random key material and cannot be decoded or
rendered. Rekey advances the room and media keys; ban, leave, vault removal,
or channel deletion hangs up the call. [source]

This design hides content and member identity from the broker more effectively
than the NIP-29 path. It does not hide room-level traffic, timing, or random
participant identifiers from LiveKit. The blind broker is an admission and
availability dependency, not a zero-observation transport. [inferred]

Primary source paths:

- [`src/concord-v2/lib/voice.ts`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/concord-v2/lib/voice.ts)
- [`src/concord-v2/lib/derive.ts`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/concord-v2/lib/derive.ts)
- [`src/concord-v2/hooks/useVoice2.ts`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/concord-v2/hooks/useVoice2.ts)
- [`src/concord-v2/hooks/useCallSync2.ts`](https://github.com/soapbox-pub/armada/blob/5b99f88d309052abc1eeb4f0b2ef437de086e709/src/concord-v2/hooks/useCallSync2.ts)

### 3.4 Media and host integration

Armada joins muted and makes microphone publication explicit. It supports
microphone, camera, screen share, best-effort tab audio, leave, per-participant
volume, camera/share focus, theater mode, multiple-share cycling, and active
speaker ordering. Concord adds raise-hand and bounded reaction state. [source]

Capture defaults include echo cancellation, noise suppression, automatic gain
control, Opus RED and DTX, 720p camera with simulcast, 1080p screen share,
adaptive stream, and dynacast. A BSD RNNoise AudioWorklet can replace browser
noise suppression; failure falls back to the raw microphone path. [source]

The same React call product runs in four hosts:

- Electron permits media and display capture only from Armada's app origin and
  provides a native screen/window picker.
- Android declares audio, camera, and audio-routing permissions.
- iOS declares camera and microphone usage strings.
- Capacitor disables LiveKit's page-leave disconnect behavior so a transient
  native background event does not tear down the call.

The main test strength is in pure Concord derivation, broker selection,
presence, identity contention, and rekey/hangup law. The audited tree has no
equivalent direct test of the actual LiveKit room, NIP-29/DM token flow, media
permissions, device capture, or E2EE worker interoperability. [test]

## 4. Buzz: a deliberate non-LiveKit architecture

### 4.1 Negative finding

At the pinned Buzz tree, a case-insensitive source search finds one `LiveKit`
occurrence: an `audioWorklet.ts` parameter comment that says “Mic track from
LiveKit.” The caller obtains the track directly from
`navigator.mediaDevices.getUserMedia`. No TypeScript or Rust manifest or lock
contains a LiveKit package, and no runtime or deployment file defines a
LiveKit server, URL, key, secret, room, token, or API call. [source]

The correct statement is not that Buzz “integrates LiveKit differently.”
**Buzz owns an alternative media stack.**

### 4.2 Huddle control and room identity

Starting a huddle creates a private ephemeral NIP-29 channel with:

- a random UUID;
- a one-hour TTL;
- a normalized display name;
- a guidelines event; and
- invited agents added as bot members.

A kind `48100` event links the huddle to its parent channel. Relay and client
events describe joined, left, and ended state. The media room identity is the
ephemeral channel UUID, not a LiveKit room name. Fallback display names use
`huddle-<first-eight-uuid-characters>`; ordinary channels and DMs derive a
human-readable huddle name capped at 80 characters. [source]

Primary source paths:

- [`desktop/src-tauri/src/huddle/mod.rs`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/huddle/mod.rs)
- [`crates/buzz-core/src/kind.rs`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/crates/buzz-core/src/kind.rs)
- [`desktop/src/features/huddle/lib/huddleChannelName.ts`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src/features/huddle/lib/huddleChannelName.ts)

### 4.3 Capture, transport, and playback

The browser captures a 48 kHz mono microphone with echo cancellation and noise
suppression. An AudioWorklet passes PCM through raw Tauri IPC. Rust encodes
Opus at 32 kbps with DTX and prepends a compact sequence, timestamp, level,
and flags header. The client connects to:

```text
WS /huddle/<channel-uuid>/audio
```

The relay validates framing and forwards Opus opaquely. It does not transcode.
Each receiving peer has its own Opus decoder, WebRTC NetEq jitter buffer, and
Rodio playback path. [source]

The WebSocket begins with a challenge. The client signs a NIP-42 kind `22242`
event that binds the relay and challenge, then claims the parent channel and
protocol version. Before forwarding media, the server:

1. derives the tenant from the HTTP host;
2. verifies NIP-42;
3. enforces relay membership;
4. enforces ephemeral-channel access and the signed parent/huddle link;
5. rejects archived channels; and
6. pins one compatible wire version for the room.

There is no separate media token endpoint. The authenticated audio socket is
the admission boundary. [source]

Primary source paths:

- [`desktop/src/features/huddle/HuddleContext.tsx`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src/features/huddle/HuddleContext.tsx)
- [`desktop/src-tauri/src/huddle/relay_api.rs`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/huddle/relay_api.rs)
- [`desktop/src-tauri/src/huddle/jitter.rs`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/huddle/jitter.rs)
- [`crates/buzz-relay/src/audio/handler.rs`](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/crates/buzz-relay/src/audio/handler.rs)

### 4.4 Product scope and operational cost

Buzz huddles are desktop, audio-only rooms. They include roster and
active-speaker UI, mute, push-to-talk or voice activity, device choice,
microphone gain, reactions, agent invite/removal, local STT transcripts, and
local TTS for agent messages with barge-in. The audited mobile code renders
huddle lifecycle events but does not join media. No camera, video, screen
share, recording, or separate application media-E2EE layer was found.
[source]

Single-pod relay audio uses an in-memory room manager. The current tree also
contains an opt-in cross-pod path with Redis owner leases and fencing,
control streams, and QUIC datagrams. `BUZZ_MESH=on` enables it. Helm defaults
huddle audio on for one replica and off for HA unless explicitly overridden.
Some chart comments still describe huddles as unavailable “until an SFU
exists,” so deployment prose lags the source. [source]

Buzz owns the failure surface that an SFU product normally absorbs:

- codec framing and version negotiation;
- jitter, decoding, mixing, and device playback;
- room capacity and per-peer queue bounds;
- challenge timeouts and heartbeats;
- reconnect backoff;
- cross-pod ownership, fencing, fanout, and teardown; and
- observability for both the Nostr lifecycle and media transport.

The benefit is one signed identity and room system with no third-party media
service. The cost is substantial bespoke realtime infrastructure and, in the
audited design, a narrower audio-only client matrix. [inferred]

Buzz has broad source tests for audio framing, room access, tenancy, version
and capacity limits, lease/fencing behavior, cross-pod roster and teardown,
jitter, preprocessing, TTS, naming, availability, UI error mapping, and Helm
rendering. This review did not run them. [test] [limitation]

## 5. Zed: LiveKit beneath native collaboration authority

### 5.1 Crate boundary and dependency pin

At the audited commit, Zed's active integration spans four layers:

| Layer | Responsibility |
| --- | --- |
| `crates/collab` | authoritative rooms, channels, invitations, roles, shared-project membership, JWT minting, LiveKit participant administration |
| `crates/proto` | typed room and `LiveKitConnectionInfo` messages |
| `crates/call` | one global active call, GPUI room state, project sharing/following, mute/deafen/share lifecycle, diagnostics |
| `crates/livekit_client` | SDK adapter, audio capture/playback, screen capture/publication, remote video rendering, event translation |

The workspace pins Zed's fork of `livekit-rust-sdks` at
`d0e27be0cdad89eadab3e36207cda0a2b6e359ee`, including `livekit` 0.7.32,
`libwebrtc` 0.3.26, and `webrtc-sys` 0.3.23. Zed also owns
`crates/livekit_api`, a small server-side JWT and LiveKit API adapter.
[source]

Primary source paths:

- [`crates/call/src/call_impl/room.rs`](https://github.com/zed-industries/zed/blob/f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4/crates/call/src/call_impl/room.rs)
- [`crates/livekit_client/src/livekit_client.rs`](https://github.com/zed-industries/zed/blob/f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4/crates/livekit_client/src/livekit_client.rs)
- [`crates/livekit_api/src/token.rs`](https://github.com/zed-industries/zed/blob/f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4/crates/livekit_api/src/token.rs)
- [`crates/proto/proto/call.proto`](https://github.com/zed-industries/zed/blob/f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4/crates/proto/proto/call.proto)

### 5.2 Control plane and credential flow

For a direct call, `crates/collab` creates a random 30-character LiveKit room
name and persists it with the Zed room. For a channel, the persistent channel
room has its own stored LiveKit name. Create and join responses can carry:

```text
LiveKitConnectionInfo {
  server_url
  token
  can_publish
}
```

The token is HS256 with the configured API key as issuer and Zed user id as
subject and participant identity. Default lifetime is six hours. Normal
members receive publish and subscribe grants. Guests may subscribe but not
publish. Administrative tokens carry room-admin or room-create grants. The
collab service reads `LIVEKIT_SERVER`, `LIVEKIT_KEY`, and `LIVEKIT_SECRET`;
local development composes a LiveKit server at port 7880. [source]

Zed room roles remain authoritative. When a role changes, collab computes
whether the role may use the microphone and updates that LiveKit participant's
`can_publish` and `can_publish_data` permissions. On leave it removes the
participant. When an ephemeral Zed room is deleted, collab deletes its
LiveKit room. LiveKit is therefore not trusted to decide who belongs to the
collaboration; it enforces a projection of Zed's decision. [source]

Shared projects, worktrees, buffers, participant locations, following, and
editor state flow through Zed's own protobuf/collab system. LiveKit carries
microphone audio, screen-share video, participant media state, and connection
quality. This is the cleanest control/media split of the three projects.
[source] [inferred]

### 5.3 Client connection and media lifecycle

The call layer serializes joins and prevents a client from joining a second
room while already active. After receiving Zed room state and optional
connection information, it connects LiveKit asynchronously and translates
SDK events into GPUI room updates. Leave clears shared and joined projects,
participants, subscriptions, media tracks, diagnostics, and connection tasks
before sending the Zed `LeaveRoom` request. [source]

On successful connection, Zed creates diagnostics and opens the microphone if
the Zed role permits it. Even with mute-on-join, it opens the track early so
Bluetooth's A2DP-to-HFP profile switch occurs during expected join
instability; actual publication remains muted until user state permits it.
Deafening disables remote audio and auto-mutes the local publication without
discarding the user's explicit mute intent. [source]

Zed owns substantial native media work around the SDK:

- CPAL input and output device selection;
- 10 ms audio frames;
- libwebrtc echo cancellation, AGC2, high-pass filtering, and noise suppression;
- resampling, remixing, remote-track mixing, and realtime-priority tasks;
- active-speaker and connection-quality projection;
- GPUI screen capture to a native WebRTC source;
- VP8 screen-share publication; and
- native remote-video conversion and GPUI rendering.

Wayland has a portal/PipeWire capture path with a first-frame timeout.
Capture failure automatically stops sharing. Screen sharing on macOS requires
Monterey because the LiveKit path depends on ScreenCaptureKit. The FreeBSD
build uses a mock facade and does not provide the same WebRTC media path.
[source]

### 5.4 Reconnect and test posture

Zed collaboration reconnect and LiveKit reconnect are distinct. Zed reissues
`RejoinRoom` to restore projects and presence. At the audited commit,
`RejoinRoomResponse` intentionally does not carry refreshed LiveKit connection
information: field 4 is reserved after a token-refresh attempt was reverted
following phantom-collaborator regressions. The LiveKit SDK still has its own
`Reconnecting` and `Reconnected` events, but a control-plane rejoin does not
mint a replacement media connection at this snapshot. [source] [history]

Zed's tests are stronger at the integration seam than Armada's:

- token tests cover deterministic time claims and normal, guest, and admin grants;
- an in-memory LiveKit server tests join, leave, tracks, mute, token
  revocation, and permission changes;
- collaboration integration tests cover multi-client channel connection,
  leave/rejoin, stale connection cleanup, and fresh-token behavior.

These are source test fixtures, not proof against an external production
LiveKit service. [test] [limitation]

### 5.5 Current Omega delta

The Zed teardown is valid evidence for upstream Zed at its pin. It is not
current Omega implementation state. Omega first retired the Zed collab server
and UI control plane, then deleted the active call and LiveKit crates because
Omega no longer rendered or operated them. [history]

Omega intentionally retains libwebrtc audio processing for Sarah realtime
voice echo cancellation. Its own voice design states that Sarah does not join
a LiveKit room or publish a LiveKit track. Remaining compose configuration,
LiveKit protobuf fields, and SDK-build support are compatibility or cleanup
residue unless a live call-site path is reintroduced. [source] [history]

## 6. What the comparison teaches

### 6.1 Separate four identities

Every realtime room has at least four identities:

1. **product room** — community, channel, DM, or Zed collaboration room;
2. **membership subject** — Nostr key, Concord channel-key holder, or Zed user;
3. **media room** — LiveKit name or Buzz ephemeral audio UUID; and
4. **media participant** — token subject, randomized identity, or NIP-42 signer.

Armada, Buzz, and Zed all map these identities differently. A robust adapter
must preserve the mapping explicitly. Treating a media participant id as
product membership would collapse admission, privacy, and reconnect semantics.
[inferred]

### 6.2 Token minting is the security center

The SDK connection call is the easy part. The material boundary is the service
that decides whether to mint or accept media authority:

- Armada NIP-29 binds an exact HTTP request to a user signer.
- Armada Concord binds it to current channel-key possession without exposing
  named membership.
- Zed binds it to authenticated collab membership and role.
- Buzz eliminates the JWT but performs the equivalent check during NIP-42
  WebSocket admission.

Any OpenAgents or Omega media design should specify credential audience,
subject, room, grants, generation/epoch, expiry, revocation, reconnect, and
broker knowledge before selecting client components. [inferred]

### 6.3 Media E2EE is independent of signed chat

Nostr signatures do not encrypt WebRTC or Opus frames. Armada makes this
distinction visible: NIP-29 admission is signed but media is SFU-trusted;
Concord separately derives and installs frame keys. Buzz's signed WebSocket
admission likewise does not create a distinct E2EE media layer. Zed's signed-in
collab membership and JWT grants do not imply frame E2EE. [source]

### 6.4 LiveKit does not eliminate native media engineering

Zed still owns capture, audio processing, device switching, screen capture,
video conversion, diagnostics, and platform build integration. Armada still
owns lazy loading, device preferences, noise processing, call layout,
cross-host permissions, identity verification, and E2EE key installation.
LiveKit removes much of SFU, signaling, congestion, and room transport work; it
does not remove the product and platform work around media. [inferred]

### 6.5 Owning the transport changes the product envelope

Buzz gains a single Nostr-aware admission path and direct control over the
audio protocol. It also accepts audio-only scope, desktop-only participation,
relay bandwidth, custom HA/mesh work, and an extensive realtime test burden.
This is a coherent trade, not evidence that a custom transport is categorically
better or worse. The decision depends on whether product differentiation
actually requires owning the media plane. [inferred]

## 7. OpenAgents and Omega disposition

This document is research evidence, not implementation admission.

The strongest reusable boundaries are:

1. Keep product room membership authoritative outside the media service.
2. Project the smallest generation-bound media grant needed for the call.
3. Keep media identity distinct from owner, agent, room, and workspace identity.
4. State content encryption, metadata visibility, and broker knowledge separately.
5. Make reconnect and token refresh explicit state machines.
6. Preserve one active-call owner so navigation and multiple render surfaces
   cannot create duplicate subscriptions.
7. Test admission, revocation, rekey, stale connection cleanup, and external
   media interoperability—not only pure key derivation or UI controls.

For future Omega work:

- **Adapt from Armada:** blind broker separation, per-sender frame-key
  derivation, one persistent call owner, and clear host permission handling.
- **Adapt from Zed:** native Rust client layering, role-to-publish permission
  projection, strict project/media separation, platform audio pipeline, and
  in-memory integration fixtures.
- **Study from Buzz:** NIP-42 admission and agent-audible room behavior, plus
  the operational cost ledger for a custom media path.
- **Reject:** treating relay membership, a LiveKit participant, media
  reachability, or a signed chat event as OpenAgents command, work, receipt,
  acceptance, or release authority.

The current Omega call removal should remain the implementation truth until a
separately admitted product and assurance contract chooses a media path and
proves it in the packaged application. Inert LiveKit configuration is not a
roadmap commitment. [inferred]

## 8. LiveKit SDK tours

### 8.1 `agents-js`: the server-side agent and model bridge

[`livekit/agents-js`](https://github.com/livekit/agents-js/tree/d5d8d0487d2e99f49a1b56ab6b9e82b481491955)
is a pnpm/Turbo monorepo centered on `@livekit/agents`. It is not a browser
room SDK. It runs programmable server-side participants and combines room
I/O, jobs, voice sessions, tools, telemetry, and provider plugins. The audited
tree has 38 provider, avatar, and VAD plugin packages, 63 TypeScript examples,
and 178 TypeScript test files. [source]

The important lifecycle is:

1. `AgentServer` authenticates to LiveKit with `LIVEKIT_URL`,
   `LIVEKIT_API_KEY`, and `LIVEKIT_API_SECRET`, registers agent capabilities,
   receives job availability and assignment messages, and launches accepted
   work in its process pool.
2. `JobContext.connect()` joins the assigned LiveKit room through
   `@livekit/rtc-node`, with selective subscription and optional E2EE.
3. `AgentSession` composes VAD, STT, LLM or realtime model, TTS, chat, tools,
   user state, room I/O, and telemetry.
4. `RoomIO` selects and subscribes to the user participant, reads room audio,
   and publishes the agent microphone track and transcriptions back into the
   room.

Primary paths:

- [`agents/src/worker.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/agents/src/worker.ts)
- [`agents/src/job.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/agents/src/job.ts)
- [`agents/src/voice/agent_session.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/agents/src/voice/agent_session.ts)
- [`agents/src/voice/room_io/room_io.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/agents/src/voice/room_io/room_io.ts)

The framework supplies production concerns that Sarah's custom bridge would
otherwise continue to own: worker registration and dispatch, draining,
participant subscription, interruptions and turn strategies, handoffs,
transcriptions, metrics, OpenTelemetry hooks, reconnection, and provider
portability. That value comes with a separate worker deployment and a second
authenticated realtime leg for every conversation. [source] [inferred]

### 8.2 The OpenAI Realtime plugin answers the compatibility question

The official
[OpenAI Realtime plugin page](https://docs.livekit.io/agents/models/realtime/plugins/openai/)
is directly dispositive. In Node.js, `@livekit/agents-plugin-openai` exposes
`openai.realtime.RealtimeModel`, which can be passed as the `llm` in a
`voice.AgentSession`. It reads a server-side `OPENAI_API_KEY`, supports text
and audio modalities, exposes semantic and server VAD settings, and allows
the model and voice to be configured. Python has the equivalent integration.
[source]

The source shows the exact topology. The agent consumes 24 kHz mono room audio,
resamples and chunks frames as needed, encodes them as OpenAI
`input_audio_buffer.append` events, and opens a server-side `ws` WebSocket
directly to OpenAI. It streams text, audio, transcript, speech-boundary,
generation, and function-call events through LiveKit's realtime model
contract. Local agent tools are converted to OpenAI function definitions and
their results are returned to the provider. LiveKit is not proxying the
inference call and the client is not connecting to OpenAI WebRTC. [source]

Primary paths and examples:

- [`plugins/openai/src/realtime/realtime_model.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/plugins/openai/src/realtime/realtime_model.ts)
- [`plugins/openai/src/realtime/api_proto.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/plugins/openai/src/realtime/api_proto.ts)
- [`agents/src/llm/realtime.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/agents/src/llm/realtime.ts)
- [`examples/src/realtime_agent.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/examples/src/realtime_agent.ts)
- [`examples/src/realtime_with_tts.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/examples/src/realtime_with_tts.ts)
- [`examples/src/realtime_turn_detector.ts`](https://github.com/livekit/agents-js/blob/d5d8d0487d2e99f49a1b56ab6b9e82b481491955/examples/src/realtime_turn_detector.ts)

This aligns with OpenAI's own transport guidance:
[WebSocket is appropriate for a server-side agent or media pipeline](https://developers.openai.com/api/docs/guides/realtime-websocket),
while WebRTC is recommended for a client directly capturing and playing
audio. OpenAI also supports a
[server-side sideband control connection](https://developers.openai.com/api/docs/guides/realtime-server-controls)
alongside a client WebRTC or SIP session, so moving tools off the client does
not by itself require LiveKit. [source]

Important plugin limits for Sarah:

- The docs currently show `gpt-realtime` as the default, while Sarah requires
  `gpt-realtime-2.1`. The plugin's model option is configurable, so a proof
  must set and verify Sarah's admitted model rather than accept the default.
- Provider turn detection and LiveKit turn detection are separate choices.
  The example that uses LiveKit turn detection explicitly sets OpenAI
  `turnDetection` to `null`; enabling both without a designed ownership rule
  risks contradictory interruption behavior.
- The plugin proactively reconnects the OpenAI socket and reconstructs
  options, tools, and text conversation state. Restored items are text-only
  and cannot be truncated like live audio. That is not Sarah's current law,
  which settles the old session before admitting a new one.
- The official page warns that loading conversation history into an audio
  model can produce a text-only response. It suggests text modality with a
  separate TTS plugin when exact history loading is required.
- The documented video-input path is Python-only, not Node.js. A Node Sarah
  worker should not assume camera or screen-share frames can be added to the
  current Realtime plugin.
- The plugin exposes raw queued OpenAI client events and received server
  events, but LiveKit owns the higher-level state machine. Sarah would still
  need to prove that every usage, transcription, interruption, and function
  event required for attribution and settlement remains observable and stable.

### 8.3 `rust-sdks`: native room and media substrate

[`livekit/rust-sdks`](https://github.com/livekit/rust-sdks/tree/00258d1e52e563327f3ed75807ea03a189a5c2d2)
is the relevant native substrate for an Omega room client. Its workspace
includes:

- `livekit`, the high-level room and participant client;
- `livekit-api`, JWT/token and server APIs;
- `livekit-protocol`, generated signaling and service types;
- `libwebrtc` and `webrtc-sys`, the WebRTC wrapper and pinned native build;
- runtime, networking, data-track, FFI, UniFFI, device, image, YUV,
  resampling, and wake-word support crates.

`Room::connect(url, token, RoomOptions)` returns a `Room` plus an asynchronous
`RoomEvent` receiver. The client can publish and subscribe to audio or video,
select native device capture or submit PCM/video frames, publish simulcast or
SVC, exchange reliable or lossy data packets, use text/byte streams and RPC,
receive transcription and active-speaker events, and close explicitly.
Options cover auto-subscribe, dynacast, E2EE, ICE servers, and retry/timeouts.
The README still marks adaptive streaming as incomplete despite an option
being present, so API shape is not proof of feature parity. [source]

Media E2EE uses frame cryptors and application-supplied shared or
per-participant keys with key rings and ratcheting. Data tracks have a
separate encryption provider. The application still owns key distribution,
rotation, participant admission, and the truth of who may decrypt. A
server-side Sarah worker must receive usable media keys if it is expected to
hear or answer; an E2EE room cannot simultaneously claim that the Sarah
service is cryptographically unable to access the audio. [source] [inferred]

`livekit-api` can mint identities, metadata, attributes, and grants and call
room, ingress, egress, SIP, agent-dispatch, and connector services. Agent
dispatch targets a separately registered agent; it does not turn the Rust
process into an Agents worker. Generated capability fields mentioning agent
sessions likewise do not expose a public Rust agent-session implementation in
this snapshot. [source]

Most importantly, this repository contains **no OpenAI Realtime client,
STT/TTS/LLM orchestration, agent worker/job runtime, or LiveKit equivalent of
the JavaScript `AgentSession`**. `Room::connect` cannot be pointed at OpenAI.
An all-Rust implementation would have to retain or rebuild Sarah's OpenAI
WebSocket bridge, event state machine, turn ownership, tools, usage, and
settlement around the room SDK. [source]

Primary paths:

- [`livekit/src/room/mod.rs`](https://github.com/livekit/rust-sdks/blob/00258d1e52e563327f3ed75807ea03a189a5c2d2/livekit/src/room/mod.rs)
- [`livekit/src/room/options.rs`](https://github.com/livekit/rust-sdks/blob/00258d1e52e563327f3ed75807ea03a189a5c2d2/livekit/src/room/options.rs)
- [`livekit/src/room/track`](https://github.com/livekit/rust-sdks/tree/00258d1e52e563327f3ed75807ea03a189a5c2d2/livekit/src/room/track)
- [`livekit/src/room/e2ee`](https://github.com/livekit/rust-sdks/tree/00258d1e52e563327f3ed75807ea03a189a5c2d2/livekit/src/room/e2ee)
- [`livekit-api/src`](https://github.com/livekit/rust-sdks/tree/00258d1e52e563327f3ed75807ea03a189a5c2d2/livekit-api/src)

The native cost is material. `webrtc-sys` downloads and statically links a
pinned libwebrtc build with platform-specific build and packaging work.
Desktop and mobile targets are represented; this is not a web SDK. Before
adoption, Omega would need packaged size/startup checks and real device tests
for capture, echo cancellation, resampling, playback, reconnection, and
latency. Distribution must also preserve applicable third-party notices.
[source] [limitation]

### 8.4 How the two SDKs compose

The repositories cover different sides of one design:

| Layer | SDK | Responsibility |
| --- | --- | --- |
| Omega client | `livekit/rust-sdks` | Join room, capture/play media, publish/subscribe tracks, optional data and E2EE |
| LiveKit service | LiveKit Server or Cloud | Room signaling, SFU routing, participant grants, worker dispatch |
| Sarah worker | `livekit/agents-js` | Join as server participant, manage room I/O and agent lifecycle |
| Model bridge | `@livekit/agents-plugin-openai` | Open server-side OpenAI Realtime WebSocket, translate audio/events/tools |
| Product authority | OpenAgents and Omega | Admission, identity, terms, economics, capabilities, confirmations, effects, receipts, settlement |

Neither SDK should absorb the final row. A LiveKit token proves only bounded
media admission. A participant identity, room attribute, RPC call, data
packet, transcript, agent tool registration, or spoken sentence is not
OpenAgents work authority or permission to mutate Omega. [inferred]

## 9. Sarah's current direct Realtime path

Sarah is already a server-side Realtime design, not a desktop client calling
OpenAI directly:

```text
Omega admission UI
  → OpenAgents admission preflight
  → reviewed Start voice action
  → credit hold + one-use gateway ticket
  → custom WSS carrying OAA1 audio/control frames
  → OpenAgents Cloud Run gateway
  → OpenAI Realtime WSS using a server-held key
```

The admitted session requires `gpt-realtime-2.1`, signed little-endian PCM16
at 24 kHz mono in both directions, a same-origin `wss://openagents.com`
gateway, short ticket and session lifetimes, and unchanged terms,
capabilities, and economics. Omega rejects a different gateway or media
contract. The gateway configures audio output, `gpt-4o-mini-transcribe`,
semantic VAD, automatic response creation and interruption, and the `marin`
voice. [source]

Omega currently owns capture and playback. It converts device input to 48 kHz
stereo for its existing echo canceller, mixes and resamples to 24 kHz mono,
sends 20 ms microphone frames, and plays 24 kHz mono through Rodio while
sharing the echo-cancellation state. A LiveKit/WebRTC path would introduce its
own capture, Opus/RTP, jitter, and WebRTC audio-processing assumptions. The
design must choose one owner for capture, echo cancellation, resampling, and
output rather than stack two APM pipelines. [source] [inferred]

The custom envelope is also an authority and accounting protocol:

- OAA1 audio frames bind identity, generation, sequence, and content digest.
- Control envelopes independently bind owner, device, thread, session,
  generation, and contiguous sequence.
- OpenAI function calls become typed proposals; the provider cannot execute
  Omega effects directly.
- Protected operations require a visible one-shot confirmation and an
  unchanged command digest before Omega revalidates workspace, path,
  document version, range, and effect locally.
- Provider response and transcription usage references become idempotent
  charge records against the session hold.
- Reconnect deliberately settles the old session, repeats admission, verifies
  the disclosed terms, and consumes a new one-use ticket. It does not resume a
  socket or provider session.
- Completed transcript text is stored locally in a bounded JSONL log; the
  service intentionally retains neither audio nor transcript text.

These are not incidental WebSocket details. A LiveKit mode must either
preserve them above room media and data transport or replace them with a
separately reviewed contract that proves the same product laws. [source]

Primary current paths:

- Omega admission and session validation:
  [`openagents_sarah_voice.rs`](https://github.com/OpenAgentsInc/omega/blob/0136fca2d11900ddc7982665482ed8cd035391c7/crates/omega_effectd/src/openagents_sarah_voice.rs)
  and
  [`openagents_session.rs`](https://github.com/OpenAgentsInc/omega/blob/0136fca2d11900ddc7982665482ed8cd035391c7/crates/omega_effectd/src/openagents_session.rs)
- Omega audio, wire protocol, tools, transcript, and reconnect:
  [`voice.rs`](https://github.com/OpenAgentsInc/omega/blob/0136fca2d11900ddc7982665482ed8cd035391c7/crates/workroom_ui/src/voice.rs)
  and
  [`panel.rs`](https://github.com/OpenAgentsInc/omega/blob/0136fca2d11900ddc7982665482ed8cd035391c7/crates/workroom_ui/src/panel.rs)
- OpenAgents admission and session issuance:
  [`sarah-realtime-voice-routes.ts`](../../apps/openagents.com/workers/api/src/sarah-realtime-voice-routes.ts)
- OpenAgents provider bridge:
  [`sarah-realtime-bridge.ts`](../../apps/openagents.com/workers/api/src/cloudrun/sarah-realtime-bridge.ts)
- OpenAgents usage and settlement store:
  [`sarah-realtime-voice-store.ts`](../../packages/khala-sync-server/src/sarah-realtime-voice-store.ts)

## 10. Should Sarah adopt LiveKit?

### 10.1 What LiveKit would add

LiveKit is compelling when Sarah needs a room-shaped product:

- browser, mobile, native, and telephony clients on one WebRTC media plane;
- multiple human or agent participants;
- SIP, ingress, egress, recording, avatars, camera, or screen sharing;
- SFU congestion control, participant routing, and room observability;
- agent dispatch, worker scaling/draining, handoffs, and provider portability;
- less custom audio transport and provider event orchestration.

It does not automatically improve Sarah's current admission, command safety,
accounting, or transcript privacy. Those are OpenAgents product properties,
not SFU features. It also adds a client-to-LiveKit connection, an
agent-to-LiveKit connection, LiveKit credentials and operations, and the
agent-to-OpenAI connection that already exists today. [inferred]

### 10.2 Viable integration shapes

| Shape | Description | Assessment |
| --- | --- | --- |
| Keep `custom_wss_v1` | Current Omega-to-OpenAgents PCM/control gateway and direct server-side OpenAI Realtime WSS | Lowest complexity for one tightly controlled desktop user; already matches OpenAI's server WebSocket guidance |
| LiveKit media, current control WSS | Rust SDK carries audio; existing WSS carries commands, authority, usage, and settlement | Lowest authority migration risk, but two coordinated connections and duplicated lifecycle |
| LiveKit media and data | Room audio plus reliable LiveKit data carries versioned Sarah control envelopes | Cleaner room topology, but sequence/digest/reconnect and DTLS-SRTP versus OAA1 integrity semantics need explicit redesign |
| LiveKit Agents worker | Omega joins via Rust SDK; `agents-js` worker joins the room and uses the OpenAI Realtime plugin | Most LiveKit value and least custom provider plumbing; greatest migration of event, tool, reconnect, and accounting behavior |
| Custom all-Rust bridge | Rust SDK room participant wrapped around Sarah's current OpenAI bridge | Possible, but gains no Rust agent runtime and retains the difficult provider/state-machine work |

The strongest incremental shape is a versioned server-selected transport
beneath the existing Sarah contract:

```text
existing OpenAgents identity, admission, terms, capabilities, and economics
  → session selects transport
      ├── custom_wss_v1
      └── livekit_room_v1
  → common command authority, usage projection, settlement, and receipts
```

For `livekit_room_v1`, session issuance would return a short-lived LiveKit URL,
room, token, and generation binding only after admission is consumed and the
hold is reserved. The server must bind those credentials to the canonical
owner, device, thread, session, and generation; participant metadata is not
proof of those values. Existing clients must not receive this shape silently:
Omega currently and correctly rejects non-OpenAgents gateway URLs and
non-PCM session contracts. [inferred]

### 10.3 Recommendation

**Keep Sarah's current direct gateway as the default for its present
one-user desktop scope, and prototype LiveKit Agents as a separately admitted
`livekit_room_v1` cohort only when a concrete room, telephony, multi-device,
multimodal, or worker-orchestration requirement justifies it.**

The current path is already the OpenAI-recommended server-side WebSocket
topology for a backend media pipeline, tightly integrated with Sarah's
authority and settlement model. The LiveKit plugin proves that the Realtime
model is supported, but compatibility alone does not make the extra media and
worker control planes free. A dual-mode proof preserves a control cohort,
measures the actual value, and avoids making LiveKit an implicit authority
rewrite. [source] [inferred]

An adoption gate should prove:

1. `gpt-realtime-2.1`, voice, instructions, transcription, semantic VAD,
   interruption, and tool behavior match the admitted session.
2. The worker exposes exact provider usage and transcription references needed
   for idempotent charging, caps, expiry, and final settlement.
3. Command proposals retain canonical identity, generation, expiry, digest,
   explicit confirmation, and local Omega effect validation.
4. Reconnect cannot bypass one-use admission or create overlapping billable
   sessions, whether LiveKit and OpenAI reconnect independently or together.
5. One audio layer owns device capture, WebRTC APM/echo cancellation,
   resampling, playback, and barge-in.
6. End-to-end latency, loss recovery, transcript alignment, CPU, memory,
   binary size, and service cost are measured against `custom_wss_v1`.
7. E2EE key distribution admits the Sarah worker explicitly, rotates on
   membership changes, and makes no contradictory privacy claim.
8. Server audio/transcript retention remains off unless a newly disclosed
   policy is admitted.
9. Node plugin limits around video and history restoration are either accepted
   or covered by a separately tested architecture.

OpenAgents identity, bearer/NIP-98 authentication, read-only admission before
microphone access, reviewed terms, one-use credentials, entitlement and hold,
server-held OpenAI credentials and safety identifier, closed capability
profiles, command confirmation, local effect authority, separate delegated
work receipts, and settlement-before-replacement remain non-negotiable under
either transport. [inferred]

## 11. Central finding

**Armada makes LiveKit part of the community product, Zed makes LiveKit a
replaceable media subsystem beneath collaboration authority, Buzz replaces
LiveKit with a custom relay-native audio system, and LiveKit Agents offers
Sarah a supported but optional room-to-OpenAI Realtime bridge.**

Armada is the most relevant reference for portable calls, blind admission, and
media E2EE. Zed is the strongest reference for a native Rust client and
server-owned permission bridge. Buzz is the strongest counterfactual: it shows
what a team must own when it declines an SFU dependency. `agents-js` is the
missing server-side agent layer; `rust-sdks` is the native room substrate, not
an agent runtime.

The common architectural law is stable across all four: **media transport is
not membership, identity, workspace state, command authority, or settlement.**
A future LiveKit Sarah mode should be chosen for room/media capabilities only
after those boundaries, credential semantics, privacy claims, reconnect laws,
accounting projections, and packaged proof are explicit.
