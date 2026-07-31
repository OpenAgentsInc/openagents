# LiveKit across Armada, Buzz, Zed, and Sarah — 2026-07-30

Commit-pinned, read-only synthesis of the LiveKit and realtime-media paths in
the existing [Armada](./2026-07-30-armada-teardown.md),
[Buzz](./2026-07-21-buzz-teardown.md), and
[Zed](./2026-07-18-zed-teardown.md) teardowns, extended with source tours of
LiveKit's JavaScript Agents framework, Rust SDKs, and React Native SDK and a
migration analysis for Sarah's current OpenAI Realtime voice path. It also
audits LiveKit's self-hosting server, deployment generator, and Helm chart and
turns them into a concrete Google Cloud deployment and Sarah cutover plan. The
final section defines how one durable Sarah identity can serve editor and
mobile 1:1 conversations and join Armada-shaped community rooms with
Buzz-shaped agent identity and participation. This review traced the source
trees more narrowly than the original product audits. It did not install
dependencies, run the products, provision Google Cloud resources, connect to
LiveKit or OpenAI, join a Buzz huddle, or exercise media devices. [limitation]

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
- **Self-hosted LiveKit fits OpenAgents' Google Cloud boundary, but not its
  current Cloud Run shape.** A disposable single-VM deployment is the fastest
  connectivity proof. A production service requires a public-node GKE
  Standard regional cluster, host networking, one SFU pod per dedicated node,
  direct UDP/TCP media reachability, a separate TURN endpoint, Redis, and
  separately operated Agents workers. Cloud Run remains the admission,
  command, accounting, and settlement authority; Cloudflare remains DNS-only.
- **A Buzz-like Sarah in Armada-like rooms is feasible, but it is not a
  transport clone.** Sarah should have one stable `principal.sarah` and Nostr
  public key while each 1:1 conversation or community room gets a separate
  presence lease, Agents job, OpenAI Realtime session, context partition, and
  usage ledger. Stock LiveKit `RoomIO` links one participant at a time, so the
  first group implementation should give Sarah an explicit, server-validated
  speaking floor rather than mix ambient speech from an entire room into one
  unattributed model stream.

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
Omega ── WebRTC room via rust-sdks ── self-hosted LiveKit ── agents-js worker
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

| Project                          | Audited source                                                                                                                                                         | Media posture                                                                                                  | License                                                                   |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Armada                           | [`soapbox-pub/armada@5b99f88d`](https://github.com/soapbox-pub/armada/tree/5b99f88d309052abc1eeb4f0b2ef437de086e709)                                                   | LiveKit JS room product across web, Electron, Android, and iOS                                                 | AGPL-3.0 client; Electron package metadata says MIT                       |
| Buzz                             | [`block/buzz@be13b4bb`](https://github.com/block/buzz/tree/be13b4bb9ce228b21fa3682ce75d75cba5950561)                                                                   | Custom Opus/WebSocket huddles; no LiveKit integration                                                          | Apache-2.0                                                                |
| Zed                              | [`zed-industries/zed@f032f4d4`](https://github.com/zed-industries/zed/tree/f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4)                                                   | Native LiveKit media plane below Zed collaboration control                                                     | GPL-3.0-or-later by default; marked components may differ                 |
| LiveKit Agents JS                | [`livekit/agents-js@d5d8d048`](https://github.com/livekit/agents-js/tree/d5d8d0487d2e99f49a1b56ab6b9e82b481491955)                                                     | Server-side agent worker, room I/O, voice orchestration, and OpenAI Realtime provider plugin                   | Apache-2.0 framework; bundled LiveKit model artifacts have separate terms |
| LiveKit Rust SDKs                | [`livekit/rust-sdks@00258d1e`](https://github.com/livekit/rust-sdks/tree/00258d1e52e563327f3ed75807ea03a189a5c2d2)                                                     | Native LiveKit room, media, data, E2EE, token, and server APIs; no agent runtime                               | Apache-2.0; preserve applicable bundled third-party notices               |
| LiveKit React Native SDK         | [`livekit/client-sdk-react-native@8ae9b600`](https://github.com/livekit/client-sdk-react-native/tree/8ae9b600f44b7f03b3dfe3d983006826ed9745f8)                         | React Native room, WebRTC, track, and mobile audio-session client used by a future OpenAgents Mobile room path | Apache-2.0                                                                |
| LiveKit React Native Expo plugin | [`livekit/client-sdk-react-native-expo-plugin@5bd7ed7d`](https://github.com/livekit/client-sdk-react-native-expo-plugin/tree/5bd7ed7d86cba726fecb21491a370a302b00e380) | Native Expo prebuild configuration for the React Native SDK and WebRTC dependency                              | Apache-2.0                                                                |
| LiveKit Server                   | [`livekit/livekit@ced94b86`](https://github.com/livekit/livekit/tree/ced94b8645829263a1a9ef6c8101936897252d6b)                                                         | Self-hosted signaling, SFU, embedded TURN, distributed routing, configuration, and metrics                     | Apache-2.0                                                                |
| LiveKit Deploy                   | [`livekit/deploy@1a7b369f`](https://github.com/livekit/deploy/tree/1a7b369f94e3a2f890d366fceeb4f273bf9fb3f6)                                                           | Production Docker Compose, Caddy, TLS, TURN, Redis, and VM startup-file generator                              | Apache-2.0                                                                |
| LiveKit Helm                     | [`livekit/livekit-helm@8f0ad080`](https://github.com/livekit/livekit-helm/tree/8f0ad0809c2be8cbed375a6f8bef10625e5e8a2b)                                               | Kubernetes deployment, GKE ingress/backend, TURN load balancer, HPA, and secret-mount templates                | Apache-2.0                                                                |
| Sarah                            | Omega [`0136fca2`](https://github.com/OpenAgentsInc/omega/tree/0136fca2d11900ddc7982665482ed8cd035391c7) and this OpenAgents snapshot                                  | Custom authenticated PCM/control gateway to server-side OpenAI Realtime                                        | Repository-specific; outside this SDK license comparison                  |

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
- Google Cloud quota, current project policy, DNS ownership, certificate
  issuance, actual regional capacity, machine-family availability, pricing,
  load-balancer behavior, or LiveKit scale under Sarah's workload.
- A self-hosted LiveKit uptime target, in-flight room survival after node
  loss, Redis failover behavior, TURN reachability from restricted networks,
  or a safe Kubernetes upgrade and drain procedure.
- cryptographic security, traffic-analysis resistance, platform permission
  behavior, or end-to-end call quality for any project.

## 2. Comparison at the authority boundary

| Concern                          | Armada                                                                   | Buzz                                                         | Zed                                                         |
| -------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------ | ----------------------------------------------------------- |
| Workspace/community authority    | Concord client state or NIP-29/Buzz relay                                | NIP-29 relay and signed event log                            | Zed collab service and database                             |
| Media transport                  | LiveKit SFU                                                              | Custom Opus over relay WebSocket; optional Redis/QUIC mesh   | LiveKit SFU                                                 |
| Credential admission             | NIP-98 request to relay, or Concord channel-key proof to blind broker    | NIP-42 challenge over media WebSocket; no media JWT          | Collab service mints HS256 LiveKit JWT                      |
| Room identity                    | NIP-29 group id; deterministic DM id; Concord derived voice public key   | Random ephemeral NIP-29 channel UUID                         | Random persisted 30-character LiveKit room name             |
| Publish authority                | Token/broker policy; membership enforced before mint                     | Relay membership and parent-huddle linkage                   | Zed role changes update LiveKit participant permission      |
| Media E2EE                       | Concord only: per-sender derived frame keys                              | No separate application media-E2EE layer found               | No application media-E2EE layer found                       |
| Product media                    | Audio, camera, screen share, reactions, raise hand                       | Desktop audio, reactions, local STT/TTS, agent participation | Audio and screen share                                      |
| Client substrate                 | LiveKit JS + React components                                            | Browser capture + Tauri Rust audio stack                     | Forked LiveKit Rust SDK + native GPUI/audio stack           |
| Server dependency                | LiveKit plus token broker/relay                                          | `buzz-relay` is signaling, authorization, and media fanout   | Zed collab plus LiveKit                                     |
| State sent through media service | Media, room-level metadata, random or Nostr-derived participant identity | Opaque Opus frames stay inside Buzz infrastructure           | Media and LiveKit participant state; no editor/project data |

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

| Layer                   | Responsibility                                                                                                                |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `crates/collab`         | authoritative rooms, channels, invitations, roles, shared-project membership, JWT minting, LiveKit participant administration |
| `crates/proto`          | typed room and `LiveKitConnectionInfo` messages                                                                               |
| `crates/call`           | one global active call, GPUI room state, project sharing/following, mute/deafen/share lifecycle, diagnostics                  |
| `crates/livekit_client` | SDK adapter, audio capture/playback, screen capture/publication, remote video rendering, event translation                    |

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

### 8.4 React Native SDK and Expo plugin: mobile room substrate

[`livekit/client-sdk-react-native`](https://github.com/livekit/client-sdk-react-native/tree/8ae9b600f44b7f03b3dfe3d983006826ed9745f8)
wraps LiveKit's JavaScript room model and native WebRTC implementation for
React Native. It depends on `@livekit/react-native-webrtc` and
`livekit-client`; applications register the WebRTC globals, start and stop a
native `AudioSession`, and join with a short-lived URL/token through
`LiveKitRoom` or the lower-level room API. It is a room/media client, not an
Agents worker or OpenAI client. [source]

The separate
[`client-sdk-react-native-expo-plugin`](https://github.com/livekit/client-sdk-react-native-expo-plugin/tree/5bd7ed7d86cba726fecb21491a370a302b00e380)
configures the native modules for Expo projects and pairs with the React
Native WebRTC config plugin. It supports development/store builds produced
through Expo prebuild; it does not make WebRTC available inside Expo Go.
Android can select communication versus media audio behavior. iOS has an
optional iOS 18+ multitasking-camera path, but Sarah's first mobile scope
needs microphone audio only and should not enable background camera or
recording. [source] [inferred]

This maps cleanly to OpenAgents Mobile's existing native-build posture, but
not to its current recorder. For `livekit_room_v1`, one component must own the
mobile audio session, microphone capture, playback, route changes, and
interruption. The existing native Realtime recorder/player and `expo-audio`
must not capture simultaneously with LiveKit. App backgrounding, Bluetooth,
phone-call interruption, permission revocation, and over-the-air compatibility
need packaged-device tests because adding or changing these native modules
requires a new native build. [source] [inferred]

Primary sources:

- [`README.md`](https://github.com/livekit/client-sdk-react-native/blob/8ae9b600f44b7f03b3dfe3d983006826ed9745f8/README.md)
- [`src/components/LiveKitRoom.tsx`](https://github.com/livekit/client-sdk-react-native/blob/8ae9b600f44b7f03b3dfe3d983006826ed9745f8/src/components/LiveKitRoom.tsx)
- [`src/audio`](https://github.com/livekit/client-sdk-react-native/tree/8ae9b600f44b7f03b3dfe3d983006826ed9745f8/src/audio)
- [Expo plugin setup](https://github.com/livekit/client-sdk-react-native-expo-plugin/blob/5bd7ed7d86cba726fecb21491a370a302b00e380/README.md)

### 8.5 How the SDKs compose

The repositories cover different sides of one design:

| Layer             | SDK                                        | Responsibility                                                                                    |
| ----------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| Omega client      | `livekit/rust-sdks`                        | Join room, capture/play media, publish/subscribe tracks, optional data and E2EE                   |
| OpenAgents Mobile | `client-sdk-react-native` plus Expo plugin | Join room, own the native mobile audio session, capture/play media, publish/subscribe tracks      |
| LiveKit service   | LiveKit Server or Cloud                    | Room signaling, SFU routing, participant grants, worker dispatch                                  |
| Sarah worker      | `livekit/agents-js`                        | Join as server participant, manage room I/O and agent lifecycle                                   |
| Model bridge      | `@livekit/agents-plugin-openai`            | Open server-side OpenAI Realtime WebSocket, translate audio/events/tools                          |
| Product authority | OpenAgents and Omega                       | Admission, identity, terms, economics, capabilities, confirmations, effects, receipts, settlement |

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

| Shape                              | Description                                                                                     | Assessment                                                                                                                   |
| ---------------------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Keep `custom_wss_v1`               | Current Omega-to-OpenAgents PCM/control gateway and direct server-side OpenAI Realtime WSS      | Lowest complexity for one tightly controlled desktop user; already matches OpenAI's server WebSocket guidance                |
| LiveKit media, current control WSS | Rust SDK carries audio; existing WSS carries commands, authority, usage, and settlement         | Lowest authority migration risk, but two coordinated connections and duplicated lifecycle                                    |
| LiveKit media and data             | Room audio plus reliable LiveKit data carries versioned Sarah control envelopes                 | Cleaner room topology, but sequence/digest/reconnect and DTLS-SRTP versus OAA1 integrity semantics need explicit redesign    |
| LiveKit Agents worker              | Omega joins via Rust SDK; `agents-js` worker joins the room and uses the OpenAI Realtime plugin | Most LiveKit value and least custom provider plumbing; greatest migration of event, tool, reconnect, and accounting behavior |
| Custom all-Rust bridge             | Rust SDK room participant wrapped around Sarah's current OpenAI bridge                          | Possible, but gains no Rust agent runtime and retains the difficult provider/state-machine work                              |

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

## 11. Self-hosting LiveKit: source tour and operational facts

The official
[self-hosting overview](https://docs.livekit.io/transport/self-hosting/)
answers the product-level question, and the three additional repositories
answer the implementation question. Self-hosting includes the LiveKit server
and Agents protocol, but not LiveKit Cloud's managed agent hosting, built-in
inference, managed observability, global network, or uptime promise. OpenAgents
must bring its own OpenAI key, worker scheduler, metrics, logs, upgrades,
capacity, and incident response. Ingress, egress, and SIP are separate
services, not features that appear merely by running the SFU. [source]

One self-hosting difference is especially important for Sarah: the comparison
table marks access-token revocation unavailable in self-hosted LiveKit. A
short-lived JWT, participant removal, room deletion, and a
generation-unique room reduce replay opportunity, but they do not turn the JWT
into Sarah's current atomic one-use ticket. A token can be replayed during its
valid join window unless an additional authority prevents it. This is a
migration gate, not a detail to bury in deployment configuration. [source]
[inferred]

### 11.1 `livekit/deploy`: a one-machine production-shaped proof

[`livekit/deploy`](https://github.com/livekit/deploy/tree/1a7b369f94e3a2f890d366fceeb4f273bf9fb3f6)
contains the generator behind the official VM path. Its production generator:

1. asks for distinct primary and TURN domains;
2. creates a LiveKit API key and secret;
3. emits LiveKit, Caddy, Docker Compose, Redis, and optional startup files;
4. enables external-IP discovery, TCP fallback, a 10,000-port UDP range, and
   embedded TURN/TLS and TURN/UDP;
5. obtains public certificates through Caddy using Let's Encrypt or ZeroSSL;
6. optionally adds the separately deployed ingress and egress services.

The generated server config is specific: signaling listens on `7880`, WebRTC
TCP on `7881`, direct ICE UDP on `50000-60000`, TURN/TLS on `5349`, and
TURN/UDP on `3478`. Caddy terminates the public HTTPS connection and forwards
signaling and TURN/TLS locally. The official
[VM guide](https://docs.livekit.io/transport/self-hosting/vm/) explicitly
includes a Google Cloud startup-script flow. [source]

That is a good staging artifact, not the production OpenAgents topology. A
single VM, local Redis, local certificates, and one SFU failure domain cannot
establish high availability. Its purpose is to prove DNS, certificates,
Omega's Rust client, OpenAI worker audio, and difficult firewall paths before
OpenAgents accepts the cost and complexity of a GKE service. [inferred]

### 11.2 `livekit/livekit`: the server contract we would operate

The pinned
[`config-sample.yaml`](https://github.com/livekit/livekit/blob/ced94b8645829263a1a9ef6c8101936897252d6b/config-sample.yaml)
is broader than the deployment page. It exposes:

- the signaling, RTC TCP, RTC UDP range, optional UDP mux, and external-IP
  settings;
- Redis single-address, Sentinel, cluster, TLS, username, and password
  settings;
- embedded TURN domains, certificates, TLS, UDP, and external-TLS behavior;
- Prometheus metrics, node selection, region, room limits, webhooks, and
  ingress/egress integration;
- room `auto_create` and `max_participants` policy;
- congestion, codec, telemetry, logging, and graceful-drain controls.

The server reads the full YAML through `LIVEKIT_CONFIG`. Its CLI also has
separate environment boundaries for `LIVEKIT_KEYS`, `LIVEKIT_REGION`,
`REDIS_PASSWORD`, TURN certificate/key paths, node IP, and UDP port. That
means OpenAgents does not need to place API secrets or the Redis password in
the Helm ConfigMap. The key-file parser rejects unsafe key-file permissions;
the Helm chart mounts a key secret at mode `0600`. [source]

Redis is not an optional cache in a multi-node deployment. LiveKit uses it as
the shared store and message bus that lets signaling instances find and proxy
to the node that owns a room. The
[distributed deployment guide](https://docs.livekit.io/transport/self-hosting/distributed/)
also sets a hard scaling boundary: one room is assigned to one node and must
fit on that node. Adding nodes increases the number of rooms and aggregate
participants; it does not shard one Sarah room across SFUs. [source]

### 11.3 `livekit-helm`: useful baseline, not a sealed production module

The pinned Helm chart defaults to:

- `hostNetwork: true`;
- one LiveKit container exposing host RTC, TURN, and optional metrics ports;
- a five-hour Kubernetes termination grace period;
- a GKE BackendConfig with a ten-hour backend timeout and 60-second
  connection draining;
- an optional HPA and `ServiceMonitor`;
- node selector, affinity, and toleration extension points;
- a separate `LoadBalancer` Service mapping public TCP `443` to the configured
  embedded TURN/TLS port.

Its GKE example uses two replicas, Redis, external-IP discovery, embedded TURN,
GKE TLS ingress, CPU HPA from one to five pods, and resources sized around an
eight-core node: `7000m` CPU requested, `7500m` limited, and 1-2 GiB memory.
The values file says only one LiveKit instance can run per physical node
because of port restrictions. [source]

The chart does **not** enforce that one-pod-per-node rule. OpenAgents must add
required pod anti-affinity by hostname, a dedicated tainted node pool, matching
tolerations and selector, and zone-spread constraints. It also does not
provide a PodDisruptionBudget, prove a safe GKE upgrade, or guarantee spare
capacity for a surge pod. Those are OpenAgents overlays. [inferred]

The chart is pinned at chart and application version `1.11.0`; the server
repository was audited at a later independent commit. Production must pin both
the chart and an explicitly reviewed server image tag or digest, then run
compatibility smoke tests. It must not inherit `latest` or assume that a
chart's `appVersion` is the desired server release. [source] [inferred]

### 11.4 Required public network surface

The official
[ports and firewall guide](https://docs.livekit.io/transport/self-hosting/ports-firewall/)
separates signaling from media:

| Surface                   | Port                                                                  | Exposure and OpenAgents treatment                                                                           |
| ------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| LiveKit API and signaling | TCP `7880` internally                                                 | Put behind the Google external Application Load Balancer and TLS; do not expose the container port directly |
| Direct WebRTC ICE         | UDP `50000-60000`                                                     | Publicly reachable on LiveKit node external IPs; firewall targets only the dedicated SFU node identity/tag  |
| WebRTC TCP fallback       | TCP `7881`                                                            | Publicly reachable on LiveKit node external IPs; same targeted firewall                                     |
| Optional WebRTC UDP mux   | UDP `7882` by convention                                              | Defer until load and compatibility tests justify replacing the port range                                   |
| TURN/TLS                  | TCP `5349` internally, public `443` through a layer-4 load balancer   | Give it a separate domain and trusted certificate; this is the broad corporate-firewall fallback            |
| TURN/UDP                  | UDP `3478`, optionally public `443` in a separately designed endpoint | Add only after the GKE forwarding path is explicitly modeled and tested                                     |
| Prometheus                | TCP `6789` when enabled                                               | Internal monitoring only; never an internet endpoint                                                        |

The
[deployment guide](https://docs.livekit.io/transport/self-hosting/deployment/)
recommends compute-optimized hosts and 10 Gbps or faster networking and says
host networking is optimal in containers. It also explains why TURN/TLS is
necessary: some enterprise networks block UDP and non-secure TCP, while TLS on
`443` resembles ordinary HTTPS. TURN is therefore a launch requirement for a
desktop agent, not a later video-call embellishment. [source]

## 12. Proposed OpenAgents Google Cloud architecture

### 12.1 Decision

Use two deliberately different stages:

1. **Connectivity canary:** one disposable GCE VM in `us-central1` generated
   from `livekit/deploy`, with no production availability claim.
2. **Production candidate:** a public-node, regional **GKE Standard** cluster
   in `us-central1`, with dedicated LiveKit SFU nodes across three zones,
   Memorystore for Redis Standard Tier, Google load balancers, and separate
   Sarah agent workers.

Do not run the SFU on Cloud Run. Cloud Run cannot provide LiveKit's direct UDP
range, node public-IP advertisement, or host network. Do not use GKE
Autopilot: Autopilot disallows `hostNetwork`, while the LiveKit Kubernetes
guide requires it. Do not use a private-node cluster for the first production
shape: LiveKit's official
[Kubernetes guide](https://docs.livekit.io/transport/self-hosting/kubernetes/)
warns that serverless and private clusters are unsupported because NAT
prevents the required WebRTC address topology. [source] [inferred]

Google recommends regional clusters for production because their control plane
is replicated across zones, and regional Standard node pools can span zones.
That improves control-plane and new-session availability. It does **not**
replicate the live media state of a room. If the SFU node owning a Sarah room
dies, that in-flight conversation must follow Sarah's failed-session and fresh
admission law; other nodes keep accepting new rooms. [source] [inferred]

```text
Cloudflare DNS-only
  livekit.openagents.com ───────────────┐
  turn.livekit.openagents.com ───────┐ │
                                      │ │
Google Cloud, project openagentsgemini, us-central1
                                      │ └─ global external Application LB
                                      │        TLS + WSS → LiveKit :7880
                                      │
                                      └── external passthrough/L4 LB
                                               TCP :443 → TURN/TLS :5349

  GKE Standard regional cluster
    dedicated public SFU node pool, three zones
      node A ─ hostNetwork LiveKit pod ─ direct UDP 50000-60000 / TCP 7881
      node B ─ hostNetwork LiveKit pod ─ direct UDP 50000-60000 / TCP 7881
      node C ─ hostNetwork LiveKit pod ─ direct UDP 50000-60000 / TCP 7881
                         │
                         └── private VPC → Memorystore Redis Standard Tier

    ordinary application node pool, no public inbound service
      Sarah Agents worker replicas ─ outbound WSS → LiveKit signaling
                                  └─ outbound WSS/TLS → OpenAI Realtime

  existing Cloud Run monolith
    identity · preflight · admission · hold · room create · JWT mint
    command proposal/decision · provider usage · settlement · room teardown
```

Cloudflare does not terminate or proxy either hostname. It publishes DNS-only
records to reserved Google IPs. Google Cloud remains the sole runtime and
certificate/load-balancer authority. [inferred]

### 12.2 Stage A: disposable GCE connectivity canary

Provision the canary in a new, separate Terraform staging state before
creating the cluster:

| Resource           | Proposed identity                              | Purpose                                              |
| ------------------ | ---------------------------------------------- | ---------------------------------------------------- |
| Static external IP | `oa-livekit-staging-ip`                        | Stable DNS and ICE candidate                         |
| VM                 | `oa-livekit-staging`                           | Compute-optimized Ubuntu VM, Docker, host networking |
| Service account    | `oa-livekit-staging`                           | Minimum logging, monitoring, and secret access       |
| Signaling DNS      | `livekit-staging.openagents.com`               | DNS-only record to the static IP                     |
| TURN DNS           | `turn-livekit-staging.openagents.com`          | DNS-only record to the same canary IP                |
| Secrets            | staging-only LiveKit key/secret and OpenAI key | Never shared with production                         |

Generate “LiveKit Server only” configuration from the pinned deploy
repository, pin the server image rather than selecting `latest`, and use Caddy
for both trusted certificates. Open only `80`/`443` for certificate and
signaling/TURN, `7881/TCP`, `3478/UDP`, and `50000-60000/UDP`; keep SSH behind
IAP or a narrowly admitted administrator path. Run local Redis only because
the VM is disposable. Do not enable ingress, egress, recording, or SIP.
[source] [inferred]

The canary exit test is not “the homepage responds.” It must cover:

- a Rust SDK room join from a packaged Omega build;
- microphone publish and Sarah audio subscribe in both directions;
- direct UDP, WebRTC TCP fallback, and TURN/TLS from a normal network, a VPN,
  and at least one restricted corporate-style network;
- certificate rotation/restart, process restart, and forced room failure;
- exact `gpt-realtime-2.1` audio, transcription, interruption, tool-event, and
  usage visibility through a throwaway worker;
- no audio, transcript, or provider secret in logs or persistent volumes.

Destroy the canary once GKE reaches equivalent connectivity. It must not become
an unreviewed singleton production service merely because it worked. [inferred]

### 12.3 Stage B: GKE Standard production candidate

Add reviewed Terraform modules to the existing `infra/prod` authority. The
current baseline owns Cloud Run, Cloud SQL, GCS, Secret Manager, and the global
application load balancer but no GKE or Memorystore resources, so this is a
real infrastructure expansion, not another Cloud Run service declaration.
The module boundary should own at least:

- a regional GKE Standard cluster in `us-central1`;
- a dedicated LiveKit SFU node pool spread across three selected zones;
- a separate ordinary application node pool for Sarah workers and cluster
  services;
- VPC/subnet/firewall rules, reserved public IPs, health checks, signaling and
  TURN load balancers, DNS output values, and certificates;
- Memorystore for Redis Standard Tier in the same VPC and region;
- Workload Identity bindings, service accounts, Secret Manager containers and
  access grants;
- dashboards, log sinks, uptime checks, and alert policies.

Start the SFU pool at three nodes, one per zone, using a currently available
compute-optimized eight-vCPU class with high network bandwidth. Treat the
machine family as a benchmark result, not a permanent textual constant.
Reserve enough external IPv4 quota for every SFU node and surge capacity
before cluster creation. GKE regional clusters can otherwise exhaust modest
default regional IP quotas. [source] [inferred]

The LiveKit workload overlay should require:

- `hostNetwork: true`;
- a node selector and taint/toleration dedicated to the SFU pool;
- required pod anti-affinity on `kubernetes.io/hostname`;
- topology spread across zones;
- three replicas minimum and a PodDisruptionBudget preserving at least two
  ready pods;
- requests close to an entire node, beginning with the chart's seven-vCPU
  request for an eight-core class;
- `RollingUpdate` with no planned unavailability and enough spare nodes for a
  surge pod;
- five-hour termination grace plus LiveKit drain before termination;
- an explicit, pinned chart version and server image digest;
- read-only filesystem and dropped capabilities where compatible with the
  pinned server, proven rather than assumed.

HPA can add pods around 50-60% sustained CPU, causing the cluster autoscaler to
add one node for each pending pod. Disable aggressive SFU scale-down at first:
a room cannot move to another node, and a five-hour grace period is a signal
that ordinary bin-packing assumptions are wrong. Scale down only after the
node is marked draining, no new rooms are assigned, existing rooms reach zero,
and the node is demonstrably safe to remove. Keep a zonal-failure capacity
margin rather than sizing three nodes to their combined peak. [source]
[inferred]

### 12.4 Signaling, TLS, media, and TURN routing

Reserve independent addresses and certificates for signaling and TURN:

- `livekit.openagents.com` terminates a trusted certificate at a global
  external Application Load Balancer and forwards HTTPS/WSS to LiveKit
  `7880`. Health checks and the container port remain load-balancer-only.
- `turn.livekit.openagents.com` resolves to a separate external layer-4
  frontend that forwards public TCP `443` to embedded TURN/TLS. The TURN
  domain must match the certificate mounted in LiveKit.
- clients receive direct node external-IP candidates for UDP
  `50000-60000` and TCP `7881`. Firewall rules allow those public media ports
  only on the dedicated SFU nodes, not on every cluster node.

Google's external Application Load Balancer supports WebSocket upgrade without
special protocol configuration. Active WebSockets have a fixed 24-hour
maximum on that load-balancer family, independent of a larger backend timeout,
so both the Rust room client and Sarah contract still need deliberate
reconnection behavior. The Helm chart's ten-hour BackendConfig is useful for
ordinary long responses but is not a promise of an immortal signaling socket.
[source]

Launch with direct ICE plus TURN/TLS. Add TURN/UDP only after a separately
reviewed GKE UDP forwarding rule or load-balancer Service proves that its
advertised address reaches every selected backend correctly. Do not advertise
UDP `443` while the signaling frontend already owns that address/port pair;
use a separate reserved address if that optimization is adopted. [inferred]

### 12.5 Redis, secrets, and identity

Use Memorystore for Redis **Standard Tier**, not Basic Tier or a Redis pod.
Google's Standard Tier replicates across zones and automatically fails over.
LiveKit clients must still reconnect after a Redis failover connection drop.
Place it on private VPC addressing in `us-central1` through Private Services
Access, enable in-transit encryption, and alert on availability, connections,
memory, evictions, latency, failover, and rejected connections. [source]
[inferred]

The implemented Terraform boundary uses `STANDARD_HA`, TLS, and
`auth_enabled=false`. The Google provider otherwise exposes the computed Redis
AUTH value in Terraform state, which violates the repository secret-state
boundary. The structured `oa-livekit-prod-redis-auth` payload therefore has
exactly `host` and `ca_cert` despite its retained name; no password is
projected into Kubernetes or the LiveKit pod. Private VPC/PSA plus TLS does not
become a claim of Redis AUTH. [source] [limitation]

Keep these values in Secret Manager with separate staging and production
instances:

- LiveKit API key and secret;
- OpenAI API key;
- Redis host and CA certificate;
- TURN certificate and private key if Google does not terminate that TLS leg;
- any Sarah worker-to-authority bearer or workload credential.

Use Workload Identity for Google API access. Materialize Kubernetes secrets
through a reviewed Secret Manager CSI or external-secret controller and point
the Helm chart at `storeKeysInSecret.existingSecret`. Do not put `livekit.keys`,
OpenAI keys, or certificate private keys into Helm values, Git, Terraform
variables, ConfigMaps, container arguments, or static service account JSON. No
Redis password exists in the admitted first-release shape. Secret payloads
remain out-of-band from Terraform state, matching the current OpenAgents Secret
Manager law. [source] [inferred]

Create distinct least-privilege identities:

| Identity              | Grants                                                                                                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| LiveKit server        | Read its LiveKit/Redis/TURN secrets and emit telemetry; no OpenAI or OpenAgents database access                                                                    |
| Sarah worker          | Read its distinct LiveKit API key/secret, OpenAI key, and worker-auth secret; connect to LiveKit and OpenAgents authority endpoints; no OpenAgents database access |
| Cloud Run authority   | Read the server-side LiveKit key needed to create/delete rooms and mint grants; retain current admission, hold, usage, and settlement access                       |
| Deployment automation | Change GKE/Helm resources and secret references, but never read provider or API secret payloads during ordinary rollout                                            |

`AgentServer` authenticates with a LiveKit API key and secret, so the worker
does hold a sensitive server credential. Give Cloud Run and the worker
different key pairs for rotation and attribution, but do not claim the worker
pair is least-privilege until the pinned server proves enforceable key-level
scopes. A stolen symmetric signing secret can mint grants. The worker
therefore belongs in the trusted service boundary even though it has no
database authority. [source] [inferred]

### 12.6 Sarah workers and optional LiveKit services

Deploy `sarah-livekit-agent` as its own GKE Deployment on the ordinary
application node pool. LiveKit Agents workers need outbound WebSocket access
to LiveKit and outbound TLS/WebSocket access to OpenAI; they do not need a
public inbound port. Begin with at least two replicas, four CPU cores and 8 GiB
per pod as an initial measurement point, a ten-minute or longer termination
grace, explicit drain, and separate staging and production agent names. Scale
on LiveKit-reported worker load, active jobs, CPU, event-loop delay, and
provider connection count rather than HTTP request concurrency. [source]
[inferred]

Do not deploy LiveKit Ingress, Egress, recording, or SIP for the first Sarah
cohort. A one-human/one-agent room needs none of them, and each adds a separate
service, permission surface, resource profile, persistence question, and
retention policy. Add one only when an admitted recording, telephony, external
stream, or import requirement exists. [source] [inferred]

### 12.7 Observability, capacity, and failure drills

Enable LiveKit Prometheus metrics on `6789` for internal scraping only and
export them to Managed Service for Prometheus and Cloud Monitoring. Combine
them with structured server/worker logs and OpenTelemetry from the Agents
worker. At minimum, dashboard and alert:

- ready/draining SFU nodes, room and participant counts, job placement, and
  room-start failures;
- CPU, memory, network packets and bytes, packet loss, jitter, retransmits,
  NACK/PLI, reconnects, and high stream-start latency;
- direct UDP versus TCP versus TURN selection and TURN allocation failures;
- signaling and TURN load-balancer health and certificate expiry;
- Redis reachability, failover, connections, memory, evictions, and latency;
- worker load, active jobs, OpenAI connection failures, first-audio latency,
  interruption latency, tool round trips, and provider usage projection;
- end-to-end admission-to-room, mouth-to-first-audio, settlement, and teardown
  latency by `transport`, without putting raw audio or transcript text in
  metrics, logs, or traces.

LiveKit's
[benchmark guide](https://docs.livekit.io/transport/self-hosting/benchmark/)
uses a 16-core GCP `c2-standard-16` reference and `lk load-test`, but its large
single-room examples are not Sarah's shape. Sarah is many concurrent rooms
with two participants, one server-side agent, a bidirectional audio track, and
an OpenAI socket per room. Build a matching harness and measure p50/p95/p99
latency, CPU and bandwidth per room, worker saturation, TURN percentage, and
Redis load. Test packaged Omega clients from outside Google Cloud; an
in-cluster load generator cannot prove last-mile ICE or TURN. [source]
[inferred]

Before a production cohort, drill:

1. SFU pod drain and replacement with active rooms;
2. abrupt SFU node loss;
3. one-zone loss;
4. Redis manual failover and transient disconnect;
5. signaling backend removal and certificate renewal;
6. TURN backend loss from a TURN-only client;
7. agent-worker drain, crash, and OpenAI disconnect;
8. HPA scale-up from no spare node and a quota-exhaustion failure;
9. a chart/server rollback;
10. provider or LiveKit outage while a credit hold exists.

The pass condition is not uninterrupted speech in every drill. Some failures
must terminate the conversation. The pass condition is bounded failure:
no overlapping provider session, no authority replay, no lost usage, no
unsettled hold, explicit user disclosure, and a new admission before any new
billable generation. [inferred]

## 13. Converting Sarah to self-hosted LiveKit

### 13.1 Target boundary

The desired topology changes Sarah's media and provider orchestration, not its
authority:

```text
Omega
  ├─ existing authenticated control WSS ───────────────┐
  └─ LiveKit Rust room client ─ audio ─┐               │
                                       ▼               ▼
                              self-hosted LiveKit   Cloud Run authority
                                       │               │
                                       ▼               │
                              Sarah Agents worker ──────┤ provider events,
                                       │               │ tools, usage
                                       ▼               │
                              OpenAI Realtime WSS       │
                                                       ▼
                                         command decision + settlement
```

For the first cohort, use LiveKit for media only and retain the current
authenticated control WebSocket for admission-bound control frames, command
proposals, digest decisions, effect outcomes, provider usage, and settlement.
That produces two coordinated connections, but it prevents an SFU migration
from simultaneously replacing Sarah's security and economic protocol.
Reliable LiveKit data or RPC can be evaluated later as a transport for the
same versioned envelopes; room metadata, participant attributes, transcript
text, or an agent function call never becomes authority. [inferred]

Responsibility after the move:

| Concern                                                                               | Owner                                                            |
| ------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Owner/device/thread identity, terms, capability profile, admission, entitlement, hold | Existing OpenAgents Cloud Run authority                          |
| Room creation/deletion and join grant minting                                         | Cloud Run authority after admission consumption                  |
| Audio capture, playback, WebRTC connection                                            | Omega using pinned LiveKit Rust SDK                              |
| SFU, ICE, congestion, TURN, room routing                                              | Self-hosted LiveKit on GKE                                       |
| Room join, audio adaptation, OpenAI Realtime socket, model-event translation          | `sarah-livekit-agent` using pinned `agents-js` and OpenAI plugin |
| Tool proposal/decision, explicit confirmation, local effect validation                | Existing OpenAgents/Omega command plane                          |
| Exact provider usage projection, charge idempotency, settlement                       | Existing Sarah usage store and Cloud Run authority               |
| Transcript recovery                                                                   | Omega's bounded local JSONL, not LiveKit recording               |

### 13.2 Versioned session and credential contract

Add an explicit server-selected transport union rather than changing the
meaning of the current response:

```text
transport: custom_wss_v1
  gateway URL + one-use ticket + OAA1 media/control contract

transport: livekit_room_v1
  livekit URL
  generation-unique room reference
  short-lived participant JWT
  participant reference
  join expiry
  existing control gateway/ticket
```

Issue `livekit_room_v1` only after preflight, reviewed start, admission
consumption, entitlement, and credit hold succeed. The authority pre-creates a
generation-unique room with automatic creation disabled, a two-participant
limit for Omega and Sarah, and a named Sarah agent dispatch. It then mints an
Omega token constrained to:

- join exactly that room;
- use a pseudonymous generation-scoped participant identity;
- publish microphone audio only;
- subscribe to Sarah audio;
- publish no camera, screen share, arbitrary tracks, or room data in the
  media-only phase;
- carry the shortest join validity that still passes initial-connect and
  expected reconnect tests, beginning with a roughly one-minute join window.

The token must not contain owner identity, email, workspace path, command
capability, account balance, or OpenAI credentials. Canonical owner, device,
thread, session, generation, and sequence stay in the control protocol and
server store. [inferred]

Because self-hosted LiveKit has no access-token revocation, this contract does
not yet equal the current one-use ticket. Mitigations are:

- consume the admission before minting;
- use a unique room and participant identity per generation;
- minimize token validity;
- pre-create the room and reject unexpected participants;
- remove the participant and delete the room at settlement;
- fence every control event and usage record by the canonical generation;
- fail closed on duplicate participant, job, or provider-session evidence.

If red-team testing can still replay the JWT into an overlapping media
participant or revive a settled generation, `livekit_room_v1` remains
staging-only until a one-use broker or equivalent join authorization is
designed. [source] [inferred]

### 13.3 Sarah agent implementation

Create a narrowly scoped `sarah-livekit-agent` package around
`@livekit/agents` and `@livekit/agents-plugin-openai`:

1. register a named `AgentServer` and accept only server-dispatched Sarah jobs;
2. validate room/session/generation attributes against a signed lookup from
   the Cloud Run authority, never against client-supplied metadata alone;
3. connect with audio-only subscription and publish only the Sarah microphone
   track and disclosed transcript delivery;
4. construct `openai.realtime.RealtimeModel` with the admitted
   `gpt-realtime-2.1`, `marin`, instructions, modalities, transcription, and
   exactly one turn-detection owner;
5. preserve the stable OpenAI safety identifier used by the current gateway;
   if the plugin cannot send the required header, wrap or patch it before
   cohort admission;
6. forward raw provider response, transcription, usage, function-call, error,
   and interruption events to a typed accounting/control adapter;
7. convert provider tool calls into current Sarah typed proposals, wait for
   the canonical Omega decision/effect outcome, then return only that result
   to OpenAI;
8. drain explicitly and reject new jobs before Kubernetes termination.

Start with OpenAI semantic VAD as the sole turn detector if exact parity with
the current gateway is the priority, and disable LiveKit turn detection.
Alternatively, test LiveKit turn detection in a separate experiment with
OpenAI turn detection set to `null`. Never enable both without a written
interruption and response-creation law. [source] [inferred]

The plugin's proactive OpenAI reconnection cannot silently apply to Sarah.
Wrap or modify its reconnect boundary so that an OpenAI socket loss ends and
settles the old Sarah generation before any replacement provider connection
is billable. Text-only conversation reconstruction is not equivalent to a
resumed audio session. LiveKit's room reconnection may repair the media leg
only while the same admitted provider generation is still alive and the
server authorizes it; otherwise both legs close and admission restarts.
[source] [inferred]

### 13.4 Omega client and audio migration

Reintroduce LiveKit to Omega as a new, isolated media adapter, not by restoring
the deleted Zed collaboration stack:

- pin the audited Rust SDK or a reviewed successor;
- map `livekit_room_v1` to `Room::connect` and reject unrecognized URLs,
  transport versions, room references, or grants;
- publish one microphone track and subscribe only to the dispatched Sarah
  participant;
- bind every room event back to the canonical session and generation;
- stop tracks, disconnect, zero buffers, and release devices before reporting
  settlement complete;
- keep the existing `custom_wss_v1` implementation as the control cohort and
  rollback path.

Choose one audio-processing owner. The long-term room-native shape should let
the LiveKit/WebRTC device path own capture, echo cancellation, jitter,
resampling, and playout and remove the duplicated current cpal/Rodio AEC path
for `livekit_room_v1`. A lower-risk experiment may inject Omega's processed PCM
through a native audio source, but then LiveKit processing that would duplicate
AEC or gain control must be disabled and proven. Never stack two echo
cancellers. [source] [inferred]

The room media clock is not Sarah's current OAA1 wire clock. LiveKit transports
Opus/WebRTC audio at its room-native rates; the Agents plugin adapts room audio
to OpenAI's 24 kHz mono input. Keep 24 kHz PCM as a
`custom_wss_v1`/provider-boundary detail rather than a global Omega device
contract. Measure resampler delay, mouth-to-ear latency, barge-in, clipping,
device switching, sleep/wake, Bluetooth, and packaged binary size on every
supported desktop platform. [source] [inferred]

### 13.5 Command, accounting, reconnect, and privacy laws

The LiveKit cohort is blocked unless all of these survive intact:

- **Commands:** OpenAI and LiveKit events produce proposals only. The current
  digest, expiry, visible confirmation, workspace/path/version/range checks,
  and local Omega effect execution remain unchanged.
- **Accounting:** every provider response and transcription usage reference
  reaches the current idempotent store with the canonical session/generation.
  Estimated audio duration or LiveKit track statistics cannot replace exact
  provider usage.
- **Settlement:** room deletion is cleanup after settlement, not proof of
  settlement. A worker crash, SFU loss, Redis failover, or OpenAI timeout must
  still close the hold deterministically.
- **Reconnect:** no LiveKit, Agents, Kubernetes, or OpenAI automatic reconnect
  may create an overlapping provider session or reuse a settled generation.
  A replacement billable generation repeats admission and disclosure.
- **Transcript:** LiveKit transcription packets are delivery only. Omega's
  bounded local JSONL remains recovery authority. Server recording, egress,
  and transcript persistence stay disabled.
- **Raw media:** audio is never written to Redis, Cloud Storage, Cloud SQL,
  logs, traces, crash reports, or packet captures used outside a separately
  admitted test.
- **E2EE:** if frame E2EE is adopted, the Sarah worker must receive a decryption
  key to hear the user. Self-hosting and E2EE do not make OpenAgents or OpenAI
  cryptographically unable to access audio in an agent conversation.

### 13.6 Delivery sequence

| Phase                        | Change                                                                                      | Exit evidence                                                                                       |
| ---------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| 0. Contract fixtures         | Add `livekit_room_v1` schemas, grants, authority mappings, fake room events, and no network | Existing Sarah laws pass for both transport variants; old clients reject the new shape safely       |
| 1. VM connectivity canary    | Disposable GCE LiveKit plus throwaway agent and packaged Omega build                        | Direct UDP, TCP fallback, TURN/TLS, audio, model, and failure results recorded; no production users |
| 2. GKE infrastructure        | Regional Standard cluster, dedicated SFU pool, Redis HA, signaling/TURN, secrets, metrics   | Load, zone/node/Redis/TURN/drain tests pass before Sarah traffic                                    |
| 3. Media-only staging        | Rust room adapter plus Sarah worker; current control WSS remains authoritative              | Exact model/tool/usage/reconnect/privacy parity against fixtures and live staging                   |
| 4. Owner-only cohort         | Server feature flag selects `livekit_room_v1` for explicit owner sessions                   | Latency, quality, cost, TURN, charge, and failure comparison against `custom_wss_v1`                |
| 5. Bounded production cohort | Small disclosed cohort with automatic stop conditions                                       | SLOs and invariants hold through a full observation window                                          |
| 6. Product decision          | Expand, retain dual mode, or remove LiveKit                                                 | Concrete room/telephony/multidevice value exceeds added infra and operational cost                  |

The server, not Omega, owns cohort selection. Rollback stops issuing
`livekit_room_v1` to new sessions; existing LiveKit rooms drain or fail under
their admitted contract; new sessions receive `custom_wss_v1`. Never swap an
active conversation from LiveKit to the direct gateway silently because that
would change credentials, audio behavior, provider generation, and billing
mid-session. [inferred]

### 13.7 Production acceptance gates

The cutover decision needs explicit, reviewable evidence for:

1. **Connectivity:** direct UDP, TCP fallback, and TURN/TLS success from the
   actual supported client networks.
2. **Latency and quality:** p50/p95/p99 admission-to-audio,
   mouth-to-first-audio, interruption, loss recovery, jitter, and device
   switching no worse than the admitted budget.
3. **Model parity:** exact model, voice, safety identifier, instructions,
   transcription, semantic VAD, function calls, cancellation, and errors.
4. **Authority:** participant or data replay cannot cause commands, work,
   effects, acceptance, receipts, release, or settlement.
5. **One-use behavior:** JWT replay and duplicate join tests cannot overlap or
   revive a billable Sarah generation; otherwise production is blocked.
6. **Accounting:** exact provider references reconcile to every hold under
   success, cancellation, crash, reconnect, and timeout.
7. **Audio ownership:** one APM chain; packaged Omega passes device,
   Bluetooth, echo, barge-in, and binary-size tests.
8. **Privacy:** no raw media/server transcript retention, secrets in logs, or
   misleading E2EE claim.
9. **Operations:** node/zone/Redis/TURN/worker/provider drills, upgrades,
   certificate renewal, capacity, quota, budget, and image rollback pass.
10. **Value:** LiveKit delivers an admitted room, telephony, multi-device,
    multimodal, or worker-orchestration capability that the direct gateway
    does not, at an accepted steady-state cost.

## 14. Recommendation after the self-hosting review

The self-hosting material strengthens, rather than reverses, the earlier
recommendation:

- build the disposable GCE canary when OpenAgents is ready to measure a real
  LiveKit path;
- build GKE Standard only as an explicit production-candidate infrastructure
  project, never as an incidental Sarah refactor;
- start Sarah with LiveKit media plus the existing control plane;
- retain the direct gateway as the default and rollback path until one-use
  admission, exact accounting, reconnect, audio, privacy, and failure gates
  pass;
- expand only for a concrete room-shaped product requirement.

The Realtime plugin answers “can LiveKit connect Sarah to OpenAI Realtime?”
with **yes**. The server, deploy, and Helm sources answer “can we self-host it
on our Google Cloud?” with **yes, on GCE for a canary and public-node GKE
Standard for production, not on Cloud Run or Autopilot**. Neither answer
proves that LiveKit should replace the simpler direct path for Sarah's current
one-user desktop contract. [source] [inferred]

## 15. One Sarah across editor, mobile, and community rooms

### 15.1 Decision

OpenAgents can build the requested product:

- Sarah remains available from the editor input bar for an owner-private 1:1
  conversation.
- The same owner-private Sarah conversation can be reached from OpenAgents
  Mobile, subject to an explicit device handoff law.
- Sarah can be invited into, or made resident in, a community channel call and
  speak to its members.
- The surrounding product can use Armada's community/category/channel/thread,
  role, moderation, direct-message, and persistent-call shape.
- Sarah can present one durable Nostr identity and publish signed room messages
  like a Buzz agent.
- OpenAI Realtime can continue to supply Sarah's voice and model behavior using
  the owner-supplied OpenAI credit, while self-hosted LiveKit, Agents workers,
  Redis, load balancing, TURN, and observability consume eligible Google Cloud
  credit.

The important qualification is that **“the same Sarah” means one canonical
identity and persona, not one shared model socket or one shared context**.
Each active private conversation or community room must have a separate
presence lease, Agents job, OpenAI Realtime session, context partition, usage
ledger, and settlement lifecycle. One global Realtime session would leak
private/editor context into rooms, merge unrelated room histories, make
speaker-level safety attribution ambiguous, and turn a provider disconnect
into a system-wide failure. [inferred]

The production community-room rendezvous is now durable. The first admitted
community Sarah session creates its ordinary LiveKit room, worker dispatch,
provider generation, authority snapshot, and accounting owner, then claims one
active rendezvous row under `(communityRef, channelRef)`. The claim is a
database compare-and-swap through a partial unique index. A simultaneous
losing provision cannot become a second active room.

Later authenticated desktops call
`POST /api/sarah/livekit/room/join` with the community and channel refs. The
server rechecks the caller's current signed membership, reads the winning
rendezvous, reuses the caller's current verified participant identity or
allocates one, mints a short-lived grant for that exact room, writes the
Nostr-user-to-LiveKit-participant binding, and returns the grant and current
authority snapshot together. The response includes the strict top-level
`role: "member" | "moderator"` derived from that same resolved membership and
the complete active verified participant roster. The matching summon response
returns the same role projection. A client therefore never has to infer its
moderation authority from a participant row, infer a room from a display name,
discover a presence lease out of band, or join media before it has
identity/floor authority.

A membership-revision mismatch retires the rendezvous, closes Sarah presence,
and stops the winning worker generation. Moderator removal does the same.
Subsequent admission creates a new room reference and rendezvous, so an old
grant cannot address the replacement even when both room-local epochs begin at
one. Private editor sessions keep their existing per-`sessionRef:generation`
isolation; only the community join path reuses a room. [source]

The closest safe composition is:

```text
Armada product shell
  community → category → channel → text/thread + optional live call
                                        │
                                        ▼
Buzz identity and participation laws
  Nostr members + Sarah npub + signed messages + room-scoped agent context
                                        │
                                        ▼
LiveKit media and agent orchestration
  room + participant grants + explicit dispatch + Sarah audio participant
                                        │
                                        ▼
OpenAI Realtime intelligence
  one server-side provider session for this exact room execution
```

This is a behavioral composition, not a code merge. Armada is the room and
call reference. Buzz is the identity, membership, and agent participation
reference. LiveKit replaces Buzz's custom voice transport and client-local
speech synthesis. Existing OpenAgents contracts remain the authority,
accounting, and privacy reference. [source] [inferred]

### 15.2 What Buzz actually does when an agent “joins a huddle”

Buzz does not place an LLM directly on a mixed audio stream. Its path is a
text-mediated voice loop:

1. A huddle is represented by a random, ephemeral NIP-29 channel. Buzz posts
   voice-mode guidelines and adds selected agent public keys as bot members.
2. Every human desktop captures its own microphone and performs local VAD and
   speech-to-text.
3. The client publishes the utterance as a signed Nostr kind `9` group message
   and `p`-tags the current agent public keys.
4. The ACP harness receives mention events, applies its author gate, and queues
   one prompt at a time for that channel. Multiple workers may share one bot
   identity, but the same channel is not processed by two workers
   concurrently.
5. The agent publishes a signed text response from its bot key.
6. Each human client subscribes live-only to admitted bot authors and performs
   local TTS. Fail-closed bot membership and event-id deduplication determine
   which messages are spoken.

Buzz's “shared identity” pattern is directly useful: users see one bot key
while executions can run concurrently across different channels. Its strict
per-channel session and queue are also the right isolation unit for Sarah.
Its voice prompt—short spoken answers, no markdown, do nothing when not
addressed—is a useful room-mode behavior profile. [source]

The parts to copy are:

- a stable agent public identity;
- explicit bot membership in a room;
- one isolated agent context and serialized response queue per channel;
- owner/allowlist/anyone author policies;
- fail-closed rules for deciding whose output may be spoken;
- signed text messages that remain useful when no voice client is present.

The parts not to copy are client-local speech recognition, client-local agent
TTS, or the custom Opus WebSocket relay. With LiveKit and OpenAI Realtime,
clients publish audio to a room, Sarah's server-side participant consumes the
current speaker's audio, and Sarah publishes one audio track that all room
members can hear. A signed Sarah text message can accompany the answer, but it
is a projection of the answer rather than the audio transport. [inferred]

### 15.3 Product surfaces and room model

Armada's strongest lesson is that the call is a capability of a durable
conversation surface, not the top-level product. OpenAgents should preserve
this hierarchy:

```text
OpenAgents community
├─ category
│  ├─ channel
│  │  ├─ ordered text messages
│  │  ├─ threads
│  │  └─ optional live room epoch
│  └─ channel
├─ members, roles, moderation, invites, bans
└─ direct conversations

Owner workspace
└─ canonical owner-private Sarah conversation
   ├─ editor surface
   └─ mobile surface
```

NIP-29 group state or another admitted OpenAgents community service owns
community membership, roles, and moderation. LiveKit owns only the live room
epoch and media participants. A channel persists when nobody is in its call;
a LiveKit room may be created on first join and deleted after the last
participant or Sarah lease leaves. Threads and text history do not become
LiveKit data. [source] [inferred]

| Surface                               | Conversation authority                                                       | LiveKit shape                                                    | Sarah policy                                                                        | Context allowed                                                                         |
| ------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Editor 1:1                            | Canonical owner-private Sarah thread plus exact workspace binding            | Generation-unique, maximum-two-participant room                  | Explicit start from the existing voice button; Sarah explicitly dispatched          | Owner-private thread plus the exact admitted workspace context and current command laws |
| Mobile 1:1                            | The same canonical owner-private Sarah thread, not a separate mobile persona | Generation-unique, maximum-two-participant room                  | Explicit start; initially require handoff from another active private voice session | Owner-private thread plus the current bounded mobile capability profile                 |
| Community channel                     | Exact community, channel, membership revision, and live room epoch           | Multi-participant channel room with Sarah as a named participant | Invite, mention/on-demand, or admin-enabled resident policy                         | Public/group-shared channel context only                                                |
| Direct conversation with another user | Exact DM participants and conversation                                       | Small private room                                               | Explicitly invite Sarah; every participant sees that Sarah is present               | Only facts admitted to that DM                                                          |

The editor voice button should keep its current user contract: the owner starts
voice from the input bar, sees capture/playback/retention truth, can mute or
end immediately, and receives proposals rather than hidden effects. The
implementation changes beneath that button from `custom_wss_v1` media to the
LiveKit media adapter only for the admitted cohort. It should not open a
second Sarah identity or a disconnected chat history. [source] [inferred]

OpenAgents Mobile already uses Expo prebuilds and a native Realtime audio
module. The pinned React Native SDK requires `@livekit/react-native`,
`@livekit/react-native-webrtc`, and `livekit-client`, calls
`registerGlobals()`, and expects an explicit `AudioSession` lifecycle. The
pinned Expo plugin adds the native configuration and itself requires the
React Native WebRTC config plugin. Therefore the mobile implementation is a
native development/store build change, not an Expo Go or over-the-air
JavaScript-only change. The existing `expo-audio` capture path must be
disabled while a LiveKit room owns the microphone; two native recorders or
two audio-session owners would make interruption and echo behavior
unreliable. [source] [inferred]

### 15.4 One identity, many isolated presences

Sarah already has the stable OpenAgents principal `principal.sarah`, a public
Nostr key, Secret Manager custody, and a sign-only interface. Preserve that
identity, but distinguish every layer that surrounds it:

| Layer                | Example                                                 | Meaning                                | Not proof of                                      |
| -------------------- | ------------------------------------------------------- | -------------------------------------- | ------------------------------------------------- |
| OpenAgents principal | `principal.sarah`                                       | Durable product and authority identity | A live process or room membership                 |
| Nostr identity       | Sarah public key / `npub`                               | Durable signed social identity         | Possession of a LiveKit participant token         |
| Conversation         | Owner-private ref or NIP-29 group/channel ref           | Context and disclosure partition       | A currently active call                           |
| Live room epoch      | Opaque channel-call generation                          | One bounded media-room lifetime        | Durable channel state                             |
| LiveKit participant  | Generation-scoped opaque identity, display name `Sarah` | Media connection in one room           | Command or workspace authority                    |
| Agent dispatch       | Named agent plus opaque dispatch ref                    | Requested worker job                   | Sarah's public identity                           |
| Sarah presence lease | Server-owned binding of all the above                   | Admitted Sarah execution in one room   | Model truth or settlement                         |
| Provider session     | One OpenAI Realtime WebSocket                           | Model/audio execution for that lease   | Durable memory or a resumable billable generation |

A versioned, server-owned presence record should bind at least:

```text
sarahPresenceLeaseV1
  principalRef = principal.sarah
  sarahPubkey
  conversationRef
  roomEpochRef
  livekitRoomRef
  livekitParticipantRef
  dispatchRef
  sessionRef
  generation
  scope = owner_private | community_shared | direct_shared
  capabilityProfileRef
  membershipRevision
  admissionDigest
  issuedAt / expiresAt
```

Only Cloud Run authority may issue this lease after membership, disclosure,
admission, entitlement, and hold checks. The LiveKit worker receives opaque
references, resolves the canonical record server-side, and rejects
client-provided metadata that disagrees. LiveKit attributes and the display
name “Sarah” are presentation data. [inferred]

Nostr supplies the durable social proof. On join, an OpenAgents-specific,
public-safe signed presence attestation can bind Sarah's public key to the
opaque room epoch and participant reference, and Sarah's room-visible text
responses can be signed kind `9` messages in the NIP-29 group. This proves
which Nostr identity authored the attestation or text. It does **not**
cryptographically sign each audio frame, prove who held the LiveKit bearer
token, or grant command authority. Participants should treat the server-owned
lease plus the Sarah signature as the display binding and should show an
unverified state when either side is missing or contested. [inferred]

Do not mount Sarah's raw Nostr secret into every autoscaled Agents pod. The
current sealed in-process signer is appropriate for a bounded Cloud Run
authority, but multiplying it across GKE workers broadens key exposure.
Introduce a narrow Google-authenticated signing service or remote-signing
boundary that accepts an exact event template and returns a signature, never
an `nsec` or private bytes. Agents pods use Workload Identity and an allowlist
of public-safe Sarah event kinds/templates. Key rotation changes the canonical
Sarah identity record and invalidates future leases without pretending old
signatures changed. [source] [inferred]

### 15.5 Google Cloud and provider topology

The self-hosted room product composes the control and media systems described
earlier:

```text
Editor / OpenAgents Mobile / room clients
        │  admission, membership, typed control
        ▼
Cloud Run OpenAgents authority ───── owner-private + NIP-29 services
        │  short-lived room grants          │
        │  explicit Sarah dispatch          └─ Sarah signed text/presence
        ▼
GKE Standard regional cluster
  ├─ public-node LiveKit SFU pods ─ Redis HA
  ├─ TURN/TLS and direct UDP/TCP media
  └─ Sarah Agents worker pool
          │  one job per active Sarah room
          ▼
OpenAI Realtime WebSocket
  gpt-realtime-2.1 + admitted voice/instructions/tools
```

Google Cloud is the production infrastructure authority. Cloudflare remains
DNS-only. GKE Standard runs the host-networked SFU and Agents workers; the
regional load-balancing, public addresses, certificates, Redis, metrics,
logs, node pools, and TURN traffic are Google Cloud resources. Cloud Run keeps
the application authority and receives exact typed provider-usage records
from the worker. OpenAI remains a separate provider reached over server-side
WebSockets. Google Cloud credit cannot pay an OpenAI invoice and OpenAI credit
cannot pay Google infrastructure. [inferred]

Every active room with Sarah consumes:

- one LiveKit participant and published agent audio track;
- one Agents job and a bounded share of a worker process;
- one OpenAI Realtime connection and its accumulated conversation;
- ingress/egress and, for restricted clients, potentially TURN relay
  bandwidth;
- Redis/control-plane, metrics, and load-balancer capacity;
- one independent hold, usage stream, and settlement record.

Text-only rooms do not need any of those media resources. A channel should
not keep an idle Sarah provider socket alive merely because Sarah remains a
visible bot member. Durable Nostr membership and an offline presence state
are cheap; a live media participant and Realtime session are not. [inferred]

### 15.6 Dispatch and lifecycle

Use [explicit agent dispatch](https://docs.livekit.io/agents/server/agent-dispatch/)
for Sarah. Register a named worker such as `sarah-room-v1`; keep the
participant display name `Sarah` separate from that internal dispatch name.
The authority, not a client, dispatches the agent with opaque lease and
generation references after the room admission commits. [source]

Self-hosting does not provide every LiveKit Cloud agent-management behavior.
In particular, the Agent Dispatch API documents its restart policy field as
Cloud-only. OpenAgents must supervise workers with Kubernetes and must not
interpret a restarted job as permission to reopen an old provider generation.
A crashed worker settles or marks the old execution indeterminate according
to the existing contract; a replacement Sarah execution requires a new
generation and, when billable, a new admission. [source] [inferred]

Recommended room policies:

- **Private 1:1:** dispatch immediately after the admitted owner joins.
- **Invite:** a member with the exact channel permission invites Sarah for the
  current live room epoch.
- **On demand:** a member uses `@Sarah` or “Talk to Sarah”; the authority
  dispatches Sarah if absent and grants that member a bounded speaking turn.
- **Resident:** an owner/admin explicitly enables Sarah for the channel. Sarah
  may keep room membership while humans are active, but the provider session
  still closes after a short, disclosed idle period.
- **Removal:** kick, ban, membership revision, room expiry, admin stop, or
  exhausted hold revokes the presence lease, disables media, closes the
  provider connection, settles the execution, then cleans up the participant
  and room.

The default for community rooms should be **on demand**, not ambient resident
listening. Resident Sarah is an opt-in later capability with a clear room
indicator, idle timeout, concurrency cap, and admin kill switch. [inferred]

### 15.7 Multi-participant speech: the non-obvious implementation constraint

The pinned `agents-js` source and official
[AgentSession documentation](https://docs.livekit.io/agents/logic/sessions/)
show that `RoomIO` links the agent to one participant. By default it chooses
the first participant; it can instead receive a participant identity or
switch with `setParticipant`. Its input holds one participant audio stream
and selects that participant's microphone track. It is not an automatic
room-wide mixer. [source]

LiveKit's official
[turn-handling guide](https://docs.livekit.io/agents/logic/turns/)
describes the relevant multi-participant pattern: a room RPC identifies the
caller, the server calls `session.room_io.set_participant(caller_identity)`,
and manual turn control determines when the agent listens and responds.
[source]

| Group-audio design                       | Benefit                                                                                                        | Failure or cost                                                                                                                 | Disposition                 |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| Explicit speaking floor                  | Exact current speaker; clear consent; bounded interruption; maps to LiveKit's documented participant switching | Members must press/tap or invoke Sarah before speaking                                                                          | **First implementation**    |
| Automatic active-speaker switching       | More conversational                                                                                            | Crosstalk can switch inputs mid-utterance; spoofed/noisy speakers; harder safety and transcript attribution                     | Later controlled experiment |
| Custom room mixer into one model input   | Sarah hears everyone continuously                                                                              | OpenAI receives one unattributed stream; overlapping speech is destructive; per-user safety identity and consent are unresolved | Reject for initial product  |
| One Realtime session per human publisher | Per-person audio and safety identity                                                                           | Multiple Sarah minds, conflicting replies, fragmented shared context, sharply higher provider cost                              | Reject                      |

The first community-room flow should be:

1. An admitted member presses “Talk to Sarah,” invokes a signed room command,
   or uses an authenticated LiveKit RPC.
2. Cloud authority validates current membership, room epoch, Sarah presence,
   rate limit, and floor availability and returns a short turn lease.
3. The worker verifies the caller identity against the server-owned mapping,
   switches `RoomIO` to that participant, and enables input for the turn.
4. The UI shows exactly who Sarah is listening to. Other microphones may
   continue serving the human call but are not forwarded into Sarah's model
   input.
5. End-of-turn detection commits the user's utterance. Sarah answers on one
   published audio track heard by the room and optionally publishes a signed
   room text projection.
6. Only the floor holder may barge in during that response. A moderator can
   mute or stop Sarah. After answer/timeout/cancel, the floor lease closes.

The turn envelope must bind member, Nostr identity, LiveKit participant,
conversation, room epoch, lease nonce, issuance, and expiry. Nostr text,
transcription, participant attributes, active-speaker events, or the model's
guess about a voice are not enough to change the linked participant.
[inferred]

OpenAI Realtime adds a separate safety issue. OpenAI's
[Realtime guide](https://developers.openai.com/api/docs/guides/realtime)
recommends a stable, privacy-preserving `OpenAI-Safety-Identifier` for each
individual end user; when used, it must be sent on every Realtime session
because it does not carry across sessions. A single provider WebSocket serving
a group has one connection-scoped identifier while its audio may come from
several people over time. The explicit floor preserves OpenAgents'
speaker-level attribution, but it does not by itself prove that one OpenAI
session can correctly represent multiple end users under current provider
policy. Before a non-owner or public-room rollout, OpenAgents must obtain a
documented provider answer or choose a reviewed architecture that satisfies
the policy. Do not label the room owner, Sarah, or a generic “community” value
as every speaker. Until resolved, keep the group pilot to authenticated,
allowlisted members under OpenAgents abuse controls and treat public rooms as
blocked. [source] [inferred]

### 15.8 Context and memory partitioning

One identity must not collapse the existing “two rooms” law. Owner-private
Sarah and a community Sarah share persona and public identity, not private
memory:

| Context partition          | Contents                                                                     | May feed                                  |
| -------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------- |
| Public Sarah profile       | Name, public description, public behavior and safety policy                  | Every Sarah execution                     |
| Owner-private conversation | Owner's durable Sarah messages and admitted memory                           | Owner-private editor/mobile sessions only |
| Workspace-private context  | Exact admitted file/editor/task context                                      | The bound editor session only             |
| Community-shared context   | Current channel messages, member-visible room summary, public Sarah messages | That exact community/channel only         |
| Direct-shared context      | Messages visible to every participant in that DM                             | That exact DM only                        |
| Realtime-ephemeral context | Current provider events and audio for one presence lease                     | That exact lease only                     |

Never reuse an OpenAI Realtime session across rooms. Never inject an
owner-private transcript, mobile tool result, editor selection, path,
credential, memory, or private thread summary into a community session.
Moving a fact from private to group context is a deliberate publication by
the owner, with an exact preview and destination—not retrieval convenience.
Likewise, a community member telling Sarah to “remember this for Christopher”
does not write owner-private memory. [inferred]

Editor and mobile may expose the same durable owner-private conversation, but
the first LiveKit release should allow only one active owner-private voice
generation at a time. Moving from desktop to mobile is an explicit handoff:
stop input, close the old provider session, record exact usage, settle the old
generation, then issue the new device's admission. A text-visible handoff
summary may be generated under the owner-private memory law; raw provider
audio state and a live model socket do not migrate. Concurrent community-room
presences are separate and cannot inherit this private thread. [inferred]

### 15.9 Tools and authority by surface

The same Sarah identity may operate under different closed capability
profiles. Capability is determined by the admitted surface, not by Sarah's
name or Nostr signature:

| Surface              | Permitted initial behavior                                                                                   | Explicitly forbidden                                                                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| Editor owner-private | Current typed proposals, visible confirmation, and local Omega effect validation against the exact workspace | Hidden edits, shell/git effects without existing authority, treating speech/transcript as confirmation                           |
| Mobile owner-private | Current bounded server-owned mobile tools and exact owner-private records                                    | Editor/file/shell/git credential authority not present in the mobile profile                                                     |
| Community room       | Converse, answer from public/group-shared context, publish signed room text, read public-safe room facts     | Workspace access, owner-private memory, payments, releases, member administration, secrets, accepting work on the owner's behalf |
| Direct shared room   | Converse over facts visible to all participants                                                              | Owner-private tools or facts that other participants cannot see                                                                  |

If a room member asks Sarah to change code, spend funds, hire an agent, release
work, moderate a member, or expose owner data, room Sarah may create only a
public-safe request proposal. It can be delivered to the owner's private
control surface for a fresh authority decision. No community member can
confirm on the owner's behalf, and Sarah must not reveal whether a private
proposal was accepted unless the owner deliberately publishes an outcome.
[inferred]

A signed Nostr request proves authorship, not permission. A LiveKit RPC proves
control of an admitted connection for its bounded lifetime, not OpenAgents
command authority. OpenAI function calls, speech recognition, Sarah's
transcript, and model prose remain untrusted proposals. Existing typed
digests, expiries, actor bindings, one-use confirmations, local effect checks,
work receipts, acceptance, release, and settlement boundaries remain
unchanged. [inferred]

### 15.10 Privacy, disclosure, E2EE, and retention

Every surface must say who can hear what:

- A community room shows Sarah as a participant and shows whether Sarah is
  absent, joining, listening to a named floor holder, speaking, idle, or
  failed.
- The first use and every material policy change disclose that audio selected
  for Sarah travels through OpenAgents' self-hosted LiveKit infrastructure and
  to OpenAI for inference.
- Mute, stop-talking-to-Sarah, remove Sarah, and leave-room controls must work
  immediately and visibly.
- Server recording, egress, SIP, raw-audio object storage, and provider-audio
  retention stay off in the first room release.
- Raw media never enters Nostr, Redis values, application logs, traces, crash
  reports, Cloud SQL, Sync, or the command/receipt stream.
- Provider and LiveKit transcripts are display/proposal data. If a room wants
  durable text, publish a visible signed room message under the room policy;
  do not silently convert every person's speech into permanent Nostr history.

If Armada-style frame E2EE is enabled, Sarah's worker must receive the room
media key to understand speech. E2EE can hide frames from an untrusted SFU,
but it does not hide them from the Sarah worker or OpenAI. Membership changes
must rotate sender/media keys and revoke departed members. The product must
not claim “only room members can hear” without naming Sarah/OpenAgents/OpenAI
as admitted processors. [source] [inferred]

For the group pilot, collect no server-side transcript by default. If a
visible text projection is enabled, show which Sarah response it represents,
sign it through the narrow Sarah signer, and keep the provider transcript
separate from the authoritative room event. Human transcription publication
should require a separately disclosed room policy and exact speaker consent.
[inferred]

### 15.11 Credit strategy and unit economics

The approximately **$10,000 OpenAI credit** and **$50,000 remaining Google
Cloud credit** are owner-supplied planning inputs. This review did not inspect
billing accounts, balances, expiration, eligible SKUs, negotiated rates,
taxes, or quota. Credits are not authority to spend and should not be modeled
as zero-cost infrastructure. [limitation]

OpenAI and Google consumption must be measured separately:

```text
openai_cost(room_execution) =
  Σ response.done input text/audio/cached usage × current admitted rates
  + Σ response.done output text/audio usage × current admitted rates
  + input-transcription usage × current admitted transcription rate

gcp_cost(room_execution) =
  allocated SFU + Agents worker + Redis + load balancer + NAT/egress
  + direct media egress + TURN-relayed bandwidth
  + logging/metrics/storage actually retained
```

OpenAI's current
[Realtime cost guide](https://developers.openai.com/api/docs/guides/realtime-costs)
says user audio is approximately one token per 100 milliseconds and assistant
audio approximately one token per 50 milliseconds. Each response is billed
over its input and output, later turns repeatedly include accumulated
conversation, prompt caching is best-effort, and enabled input transcription
is separately billed. Exact `response.done` usage—not audio duration
estimates—must drive Sarah's existing idempotent accounting. Current rates
belong in a versioned server price catalog checked at admission, not in this
teardown. [source]

Google cost is workload-dependent. Direct WebRTC traffic and TURN-relayed
traffic have materially different egress and network paths; resident idle
rooms waste worker/provider capacity; and a GKE regional base, Redis HA, load
balancers, observability, and spare failover nodes cost money before the first
conversation. Capacity and billing dashboards must separate:

- private 1:1 and community-room executions;
- direct UDP, TCP fallback, and TURN/TLS minutes;
- model input, model output, transcription, and cached tokens;
- idle, listening, and speaking time;
- SFU, worker, Redis, load-balancing, egress, logs, and metrics;
- room, owner, member, model, provider generation, and cohort—using opaque
  references rather than private transcript or identity data.

A conservative authorization ladder, subject to owner approval and verified
credit terms, is:

| Gate                | OpenAI cumulative authorization | Google Cloud cumulative authorization | Required proof                                                                    |
| ------------------- | ------------------------------: | ------------------------------------: | --------------------------------------------------------------------------------- |
| Offline/contracts   |                              $0 |        $0 beyond existing development | Fixtures, threat model, cost model, no network                                    |
| Connectivity canary |                      Up to $500 |                          Up to $2,500 | Packaged desktop/mobile audio, TURN, exact usage, forced cleanup                  |
| Owner/invited pilot |                    Up to $2,000 |                         Up to $10,000 | Measured unit cost, quality, failure drills, privacy and authority gates          |
| Expansion           |               Separate approval |                     Separate approval | Forecast from observed p95 room-hour cost and verified remaining/expiring credits |

This keeps at least $8,000 of the reported OpenAI pool and $40,000 of the
reported Google pool outside the pilot until the architecture proves value.
The exact amounts are proposed caps, not spend authority. Each environment
also needs hard technical limits: maximum concurrent Sarah rooms, per-owner
and per-community concurrency, short idle timeout, maximum room execution,
maximum response length, hold exhaustion, daily provider budget, GCP budget
alerts, and an emergency dispatch disable. OpenAI's
[Realtime conversation guide](https://developers.openai.com/api/docs/guides/realtime-conversations)
permits sessions up to 60 minutes; OpenAgents should choose a shorter admitted
room-generation limit and rotate only through settlement plus new admission.
[source]

Do not use the credits to justify ambient Sarah in every empty channel.
Credits are best spent first on the capabilities the direct gateway cannot
prove: cross-device 1:1, invited group conversation, Nostr-linked presence,
TURN connectivity, and room concurrency. The comparison metric is not “credit
burned”; it is successful, settled Sarah room-hours and useful turns per
dollar at the required latency, privacy, and authority level. [inferred]

### 15.12 Implementation path

The implementation should preserve the current direct gateway as a control
cohort and rollback path:

| Phase | Product slice              | Main code/infrastructure change                                                                                      | Exit condition                                                                                            |
| ----- | -------------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| 0     | Contracts only             | Add room scope, presence lease, speaker-turn lease, dispatch, usage, and settlement fixtures                         | Two-room, authority, replay, and context-leak tests pass without network                                  |
| 1     | Editor owner 1:1           | Add `livekit_room_v1` behind the existing voice button; deploy one named Agents worker and GCE connectivity canary   | Feature parity with `custom_wss_v1`, exact usage, one audio owner, safe rollback                          |
| 2     | Mobile owner 1:1           | Add pinned React Native SDK/Expo plugins, native build, audio-session ownership, and explicit desktop/mobile handoff | iOS/Android packaged tests pass for foreground, interruption, Bluetooth, reconnect, and handoff           |
| 3     | Invited group              | Multi-member room, Sarah invite/on-demand dispatch, explicit speaking floor, no privileged tools                     | Three-to-five authenticated members converse without attribution, privacy, context, or settlement failure |
| 4     | Nostr-native room presence | Signed Sarah presence binding and signed room text; NIP-29 membership/role projection                                | Clients fail closed on missing/contested mapping and removed members lose media                           |
| 5     | Armada-shaped product      | Durable community/category/channel/thread UI, floating call owner, moderation, channel room lifecycle                | Calls survive navigation while membership and text remain outside LiveKit                                 |
| 6     | Resident Sarah             | Admin opt-in, idle suspension, capacity controls, visible state, kill switch                                         | Unit economics, abuse, privacy, and failure SLOs hold                                                     |
| 7     | Broader rooms              | Only after OpenAI group safety-identifier policy is resolved                                                         | Provider approval/architecture plus public abuse and scale gates                                          |

Likely ownership:

- `packages/sarah`: versioned Sarah presence, conversation-scope,
  speaker-turn, public-safe signing, and two-room schemas.
- OpenAgents API/Cloud Run: membership projection, room admission, grant
  minting, explicit dispatch, generation lookup, holds, exact usage, and
  settlement.
- A new bounded `sarah-livekit-agent` service: `agents-js`, the OpenAI
  Realtime plugin, one-room job execution, dynamic participant linking,
  proposal adapter, and typed provider usage.
- Omega/editor: a LiveKit Rust media adapter beneath the current voice button;
  no restoration of Zed collaboration authority.
- OpenAgents Mobile: the LiveKit React Native SDK and Expo config plugins,
  replacing the existing recorder/player only while LiveKit owns audio.
- Community/relay service: NIP-29 membership and public signed Sarah
  presence/text, never raw audio or private workspace state.
- Google Cloud: the GCE canary, then the GKE Standard/Redis/TURN/metrics
  production candidate from sections 11–13.

An admitted Sarah join should execute in this order:

1. Resolve the exact owner or community conversation and current membership
   revision.
2. Show the correct private/group disclosure and consume one-use admission.
3. Reserve the provider hold and create the generation-scoped presence lease.
4. Create/bind the LiveKit room epoch, mint short-lived participant grants,
   and explicitly dispatch the named Sarah agent.
5. Join the human client and worker; validate both against canonical
   server-side records rather than participant metadata.
6. Publish Sarah's signed public-safe presence binding without exposing owner,
   workspace, provider, or billing data.
7. Open one OpenAI Realtime session for this exact lease, with the admitted
   model, voice, instructions, capability profile, and safety identifier.
8. In a group, issue a server-validated speaker-turn lease before linking a
   participant to `RoomIO`.
9. Stream exact provider usage and typed proposals to the existing authority
   plane.
10. On end/failure/expiry, disable input, close the provider generation,
    reconcile usage, settle the hold, sign/project any admitted terminal
    state, then remove participants and delete the room epoch.

Rollback stops issuing new `livekit_room_v1` grants and drains admitted rooms.
It never silently moves an active generation back to `custom_wss_v1`.
[inferred]

### 15.13 Acceptance scenarios

The room product is not ready merely because Sarah can be heard. It must pass:

1. Editor and mobile show the same durable owner-private conversation while
   explicit handoff prevents overlapping private voice generations.
2. Sarah serves two community rooms concurrently with one public identity but
   distinct worker jobs, provider sessions, contexts, holds, usage, and
   settlement.
3. A private editor fact, mobile tool result, file path, or owner memory never
   appears in a community response, summary, trace, or prompt.
4. A community prompt asking Sarah to ignore private boundaries, spend money,
   edit code, administer members, or expose owner data creates no privileged
   effect.
5. A member removed or banned during a turn immediately loses the Sarah floor
   and room media; Sarah's session either continues safely for remaining
   members or settles.
6. A forged LiveKit display name, participant attribute, Sarah text message,
   presence record, floor request, or replayed JWT cannot impersonate Sarah or
   a permitted speaker.
7. The Sarah Nostr private key is absent from GKE pod files, environment,
   memory dumps exposed to the app, logs, and crash reports; the signer
   rejects non-allowlisted templates.
8. Two humans speaking together, noisy active-speaker changes, barge-in,
   floor timeout, moderator stop, and Sarah removal have deterministic UI and
   model behavior.
9. LiveKit reconnect, worker restart, OpenAI disconnect, mobile
   background/foreground, device handoff, and the provider session limit
   cannot reopen or overlap a billable generation.
10. Every `response.done`, transcription charge, cancel, timeout, and failure
    reconciles exactly to one hold; modeled and invoiced cohort totals remain
    within the approved OpenAI and Google caps.
11. Raw audio is absent from Nostr, Redis values, Cloud Storage, Cloud SQL,
    Sync, logs, traces, and crash artifacts.
12. E2EE tests rotate keys on membership change, admit Sarah explicitly, and
    make accurate OpenAgents/OpenAI processor disclosures.
13. Packaged macOS, Windows, Linux, iOS, and Android clients pass microphone,
    playback, echo, Bluetooth, interruption, mute, leave, direct UDP, TCP, and
    TURN/TLS tests for their supported scope.
14. The group safety-identifier question has a documented OpenAI-compatible
    answer before public or untrusted multi-user rooms are enabled.

### 15.14 Recommendation

Build this as **one Sarah identity with many isolated room executions**:

1. preserve the existing editor voice button and prove private 1:1 LiveKit
   media there first;
2. add the native React Native client and an explicit desktop/mobile handoff;
3. introduce Sarah into a small invited channel with no privileged tools and
   an explicit speaking floor;
4. bind her LiveKit presence to her stable Nostr identity and publish
   room-visible signed text;
5. then build out Armada's durable channel/call product and consider resident
   Sarah only after measured unit economics and safety gates pass.

Do not begin with an ambient mixed room, one provider session shared across
rooms, private memory injected into community prompts, or an `nsec` copied
into autoscaled agents. The explicit floor is less magical than a room-wide
open microphone, but it is the shortest path to a Sarah who can genuinely
join rooms and talk while preserving speaker attribution, cost controls,
Nostr identity, command authority, and the existing two-room privacy law.

The resulting division is coherent: **Armada supplies the collaboration
shape, Buzz supplies the agent identity and per-channel participation
semantics, LiveKit supplies the media and dispatch plane, OpenAI Realtime
supplies Sarah's voice intelligence, and OpenAgents remains the only authority
for identity, memory boundaries, tools, accounting, and settlement.**
[source] [inferred]

### 15.15 Implemented contract substrate: EP263-LK-01

**Repository evidence, implemented for issue #9283:** the Sarah voice v1
contract now negotiates `custom_wss_v1 | livekit_room_v1`. A request that
omits `requestedTransport` still receives the exact legacy response shape;
only an explicit request receives a `transport` projection, and an unknown
transport fails decoding rather than selecting LiveKit. LiveKit admission is
one-use and binds the requested transport, private or server-resolved
community/channel scope, membership revision, capability profile, session,
generation, and current terms digest. [source]

Migration `0109_sarah_livekit_room_bindings.sql` and the Sarah voice store now
persist the canonical transport, crash-recoverable provisioning intent,
generation-unique room epoch, expected owner and Sarah participants, grant
digest, dispatch and presence-lease references, join expiry, community
membership revision, and cleanup state. The store rejects changed provider
usage replay, wrong generations, duplicate or unexpected joins, non-advancing
replacement generations, and cleanup before a settled/released receipt.
Provisioning intents use a stale lease claim so a reconciler can settle the
exact generation before idempotent broker cleanup, including recovery after a
crash between external provisioning and local binding. [source]

The API contains a typed broker seam with a canonical idempotency key,
least-privilege grant-claim projection, bounded WSS URL/epoch/expiry
validation, authoritative community policy resolution, and explicit
provision/bind/cleanup ordering. The existing authenticated gateway persists
the transport and enters provider-free control-only mode for LiveKit, so it
cannot accidentally open a second OpenAI Realtime generation or accept raw
audio frames. Internal typed lifecycle functions bind participant joins,
provider usage, terminal room cleanup, and provisioning reconciliation to the
same store authority. Focused contract, route, store, and bridge fixtures cover
legacy compatibility, explicit selection, grant minimization, community
revision/publish policy, one-use admission, usage replay, duplicate joins,
generation refusal, settlement-before-cleanup, crash reconciliation, and the
control-only bridge. [source]

**Still planned, not live evidence:** issues #9284–#9286 must provide the
production Google Cloud/LiveKit credentials and broker implementation, agent
worker event authentication, typed proposal ingress/forwarding, and OpenAI
Realtime relay, Omega Rust room media adapter, membership-revocation event
wiring, speaking-floor policy, deployment canaries, and packaged acceptance
proof. No production LiveKit room or Sarah worker is claimed by this contract
substrate. [inferred]

### 15.16 EP263-LK-02 operations substrate and cost decision

**Repository evidence, source-only for issue #9284:** the Google Cloud runbook,
deployment-bundle validator, acceptance parsers, redacted receipt projection,
and source-only receipt fixtures now define the exact operator path. Every
live read or mutation is default-off behind explicit `--apply`. The policy
pins `openagentsgemini`, `us-central1`, the three zones, the regional Standard
cluster and two node pools, Memorystore, separate signaling/TURN addresses,
chart source, server image digest, rendered configuration digest, Workload
Identity, named secret refs, and manifest digests. A changed or unknown target
refuses before a provider effect. [source]

The first application cap is 20 concurrent Sarah rooms, one owner-private
generation, two Sarah rooms per community, 120 seconds idle, and a 30-minute
room generation. Twenty rooms is not yet a capacity claim: the live gate
requires that matching small-room load with at least 20% spare capacity, hard
cap refusal, bounded CPU/packet loss/first-audio, and exact terminal
settlement. [source] [limitation]

The initial regional candidate has an approximately **$1,500/month fixed
planning floor** before direct/TURN egress, autoscaling, NAT, log/metric
volume, and OpenAI Realtime usage. The owner approved provisioning all
infrastructure needed for EP263 on 2026-07-30, overriding the repository
profile's default spend ceiling for this program. That grant does not turn
credits into zero cost or remove budget alerts, cost reconciliation, redaction,
rollback, hard application caps, or canary deletion. Google and OpenAI costs
remain separate ledgers. The label-filtered Google budget is only a guardrail:
Kubernetes-created load balancers and network egress can arrive unlabelled, so
the cost gate must reconcile the complete billing export rather than treating
the budget filter as invoice evidence. [source] [inferred]

The acceptance parser refuses to round source evidence up. Connectivity needs
packaged Omega plus observed direct UDP, TCP fallback, and TURN/TLS. Load,
eleven failure drills, secret/log/raw-media/transcript scans, cost, and scoped
rollback have separate typed observations. Failure drills may end speech; they
pass only with visible bounded failure, no provider overlap, terminal
accounting, and fresh admission. Public receipts retain opaque refs, digests,
timestamps, and outcomes while rejecting endpoints, IPs, secret-shaped
fields, transcripts, and audio. The checked-in fixtures say `source_only`,
`planned`, and `liveProof: false`. [source]

The deployment boundary is deliberately two-phase. Canary infrastructure is
applied with the VM disabled; only after four enabled secret versions, both
DNS names, a trusted/current two-SAN certificate and matching key, and an empty
VM slot pass may the six-hour VM be enabled. Production infrastructure creates
GKE, Redis, addresses, identities, observability, and empty secret containers.
Only after exact structured secret schemas, Sarah's existing production
OpenAI key, and separate DNS-only address bindings pass may the runtime start.
The runner then requires Helm 3, verifies the pinned cert-manager `v1.21.1` and
External Secrets `2.8.0` chart archive digests plus every controller, webhook,
solver, and startup OCI image digest, schedules every controller on the
application pool, and waits for controllers and CRDs before applying their
custom resources. [source]

The bundle's `sourceBaseRevision` is the reviewed contract base, not a
deployment claim. Every live operation instead requires a clean `main` whose
`HEAD` is the current remote `origin/main` and records that commit separately
as `deployedRevision`. The pinned LiveKit server image maps to source commit
`0b3fd288e3ef3263ec475ba0d78cf3ad77459981`; any later checkout used for
analysis is not runtime provenance. [source]

Production Redis is `STANDARD_HA`, TLS-only on the dedicated private VPC/PSA
path, and explicitly `auth_enabled=false`. Enabling provider-managed AUTH would
persist the computed secret in Terraform state. The retained
`oa-livekit-prod-redis-auth` name therefore projects exactly `{host,ca_cert}`
and no password; network isolation and TLS are not described as Redis AUTH.
[source] [limitation]

**Still incomplete:** no source fixture proves that GCE, GKE, Redis, DNS,
certificates, LiveKit, TURN, workers, packaged Omega media, load, failure
handling, billing, or rollback was observed. The issue close gate requires
new live-observed receipts and zero-residue canary destruction. [limitation]

## 16. Central finding

**Armada makes LiveKit part of the community product, Zed makes LiveKit a
replaceable media subsystem beneath collaboration authority, Buzz replaces
LiveKit with a custom relay-native audio system, and LiveKit Agents offers
Sarah a supported but optional room-to-OpenAI Realtime bridge. Self-hosting
that bridge is feasible on OpenAgents' Google Cloud, but it creates a
GCE/GKE/Redis/TURN operations plane rather than removing an authority plane.
One Nostr-native Sarah can serve private editor/mobile conversations and
community rooms, provided every room execution, context, capability profile,
provider session, and ledger remains isolated.**

Armada is the most relevant reference for portable calls, blind admission, and
media E2EE. Zed is the strongest reference for a native Rust client and
server-owned permission bridge. Buzz shows both what a team must own when it
declines an SFU dependency and how one signed bot identity can participate in
several isolated channels. `agents-js` is the missing server-side agent layer;
`rust-sdks` and the React Native SDK are the native client room substrates,
not agent runtimes.

The common architectural law is stable across all four: **media transport is
not membership, identity, workspace state, command authority, or settlement.**
A future LiveKit Sarah mode should be chosen for room/media capabilities only
after those boundaries, group-speaker attribution, credential semantics,
privacy claims, reconnect laws, accounting projections, provider
safety-identifier policy, and packaged proof are explicit.
