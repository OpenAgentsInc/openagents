# Armada Teardown — 2026-07-30

Read-only architecture and product audit of the public `soapbox-pub/armada`
client at an exact commit in the local reference clone
`~/work/projects/repos/armada`. Nothing tracked was modified. The audit did not
install dependencies, run the test suite, start the web or Electron shells,
join a community, open a LiveKit room, or exercise Android/iOS packaging.
[limitation]

Armada is Soapbox's end-to-end encrypted community chat client: Discord-shaped
servers, channels, threads, voice, and moderation, with a serverless default
path and optional relay-backed groups. It is the first teardown in this catalog
that ships a full **Concord** protocol implementation as a first-class product
plane rather than as a research note. [source]

## Summary

Armada makes one bet: **a community does not need a host.** The default product
path is Concord: sealed, gift-wrapped Nostr events over ordinary relays. Control,
chat, invites, rekey, and voice coordination stay inside client-derived planes.
Only members can decrypt them. Relays see ciphertext. A second path keeps NIP-29
relay-owned groups for operators who want a server that holds membership and
moderation. A third integration path can talk to Block Buzz workspaces when the
user points the client at a Buzz-shaped relay. [source]

```text
Web (React 19 + Vite)   Android (Capacitor)   iOS (Capacitor)   Electron (app://)
            |                    |                    |                |
            +--------------------+--------------------+----------------+
                                 |
                    Nostrify + wire ingest + event store
                    (SQLite-WASM/OPFS · native SQLite · IndexedDB fallback)
                                 |
        +------------------------+------------------------+
        |                        |                        |
 Concord v2 planes          NIP-29 groups            Buzz protocol
 (gift-wrap 1059 stream)    (relay as server)        (compat client)
 control · chat · guestbook · rekey · invites · voice presence
        |                        |                        |
   ordinary relays          NIP-29 relay           Buzz relay
   + blind CORD-07          + LiveKit token        + LiveKit / REST
     AV broker              HTTP from WS URL
```

The implementation is not a thin UI on someone else's SDK. `src/concord-v2/` is
about 33.6k lines of TypeScript for kinds, HKDF derivations, reversed NIP-59
stream wraps, control editions, guestbook, rekey, invites, roles, voice, and
hooks. The rest of the monorepo is a full multi-surface chat product: React
components, a doorbell wire bus, DMs, bots, polls, calendar, private Lightning
zaps, WebXDC apps, NIP-34 git activity, Android Bluetooth mesh, and release
tooling on ngit-ci. [source]

The central OpenAgents decision: **track Armada as the strongest open
serverless-E2EE community client and Concord product evidence, and as a peer
client for NIP-29 / Buzz / Nostr identity surfaces. Do not adopt Armada as a
product shell, React/Tailwind/shadcn stack, Capacitor host, or AGPL dependency.
Keep OpenAgents authority on Thread/Sync/WorkContext, grants, containment, and
receipts. Harvest selected protocol lessons — sealed collaboration planes, list
publish discipline, store-first wire, blind voice brokers, and transport-agnostic
bot manifests — only behind explicit OpenAgents policy.**

## 1. Snapshot, provenance, and limits

### 1.1 Exact source identity

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Public repository | `https://github.com/soapbox-pub/armada` | Public client source and history |
| Canonical forge claim | gitworkshop.dev `soapbox.pub/armada`; GitLab described as a read-only mirror | Nostr-git primary, GitHub/GitLab as mirrors |
| Local clone | `~/work/projects/repos/armada` | Audited tree |
| Audited commit | `5b99f88d309052abc1eeb4f0b2ef437de086e709` | Exact snapshot |
| Commit time | `2026-07-29T20:20:49-05:00` | Freshness of the tip |
| Commit subject | `Show correct provider name in GIF picker when using GIFverse` | Latest audited change |
| Product version (changelog) | `0.43.0` released 2026-07-29 | Active pre-1.0 train |
| `package.json` version | `0.1.0` (stale relative to changelog) | Source version fields are not the release authority |
| License | AGPL-3.0 (`LICENSE`); Electron shell package declares MIT | Copyleft client; shell metadata differs |
| First commit | `2026-06-12` — NIP-29 group chat stack from a Ditto template | Product is about seven weeks old at audit time |
| History scale | 1,089 commits, 12 shortlog identities | High velocity, small core team |
| Dominant authors | Chad Curtis ~723, Alex Gleason ~201, JSKitty ~66 | Soapbox-adjacent maintainership |
| Source scale | ~167k lines under `src/` (`*.ts`/`*.tsx`); ~202k including electron/android JS/Kotlin surface counts in the survey | Mid-large TypeScript product |
| Tests | 140 `*.test.ts(x)` files, ~27k lines of tests | Protocol and product both tested in-repo |
| Hosted product | `armada.buzz` (web deploy + default Concord AV broker) | Public client origin and voice default |
| Optional backend | Separate `armada-relay` (NIP-29 + LiveKit + Concord AV); not required at client build time | Host is optional infrastructure |

Local path pin: `~/work/projects/repos/armada` at
`5b99f88d309052abc1eeb4f0b2ef437de086e709`.

Selected content digests at that commit:

| File | SHA-256 |
| --- | --- |
| `README.md` | `53ad2438de1edecd48251ffb5b4d41d6da069f5eb4edde79645104fc577701df` |
| `CORD.md` | `2a0431def56324ea04ad383eeb43011d5c965792d1cbecccc7a1b6c0992d643c` |
| `NIP.md` | `ca4e5f0750624bb4bec9c18986a3c05ec6eceb3752880d72362ff06b807c093c` |
| `AGENTS.md` | `5af28c32fa9fed7e3b558ab5c5c547566e6072b919eaf6acb23bd35fd9bafdb6` |
| `package.json` | `3392e934ef9b93df88c114781a6e09150931f9097bcbb5fd909715ab46520162` |

### 1.2 Evidence labels

- **`[source]`** — tracked Armada source, docs, or manifests at the audited commit.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — corroborated by a linked public page or repository description.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

### 1.3 Audit limits

This audit is source-only. It does not prove live relay behavior, LiveKit
signaling correctness, Android notification reliability, iOS keychain behavior
on device, Electron auto-update signing in CI, or cryptographic security of
Concord under adversarial network conditions. The optional `armada-relay`
backend was not cloned. Concord protocol authority lives upstream at
`concord-protocol/concord`; this audit treats Armada's client implementation and
`CORD.md` extensions as product evidence, not as the protocol specification.
[limitation]

## 2. What Armada is

### 2.1 Product thesis

The README positions Armada as "Discord without the company" and "No host
required." Communities are serverless by default via Concord. NIP-29 remains for
operators who want a relay that owns membership, moderation, and data. Auth is
key-based: nsec, NIP-07 extension, or NIP-46 bunker/nostrconnect. Identity is
portable. Voice is WebRTC through LiveKit with client-side encryption under
per-sender keys on the Concord path. [source]

### 2.2 Surfaces

| Surface | Path | Notes |
| --- | --- | --- |
| Web | `src/` React 19 + Vite 8 + Tailwind + shadcn/Radix + Nostrify | Primary product |
| Android | `android/` Capacitor 8 | Signed APK/AAB on tags via ngit-ci; native notification service, SQLite, NIP-55 Amber, Bluetooth mesh, Credential Manager |
| iOS | `ios/` Capacitor 8 + SwiftPM | Manual Mac builds only; secure storage for nsec; no APNs notifications or universal links yet |
| Desktop | `electron/` | Bundled web over `app://armada/…`; tray, unread badges, screen-share picker; `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true` |
| Optional relay | external `armada-relay` | Not a build dependency |

Shipped builds intentionally leave `VITE_PLATFORM_RELAYS` empty so a fresh client
has no baked-in servers. App relays for profiles/lists default to Ditto-family
relays and remain user-editable, including empty for air-gapped use. [source]

### 2.3 Dual (really triple) community models

1. **Concord communities** — default. Client-side protocol under
   `src/concord-v2/`. Planes are addressed by HKDF-derived stream keys. Outer
   events are kind `1059` / `21059` wraps that reverse classic NIP-59 roles:
   fixed stream author, ephemeral `p`, encryption under the stream self-ECDH
   conversation key. Inside: a real-key seal around an unsigned rumor. [source]
2. **NIP-29 servers** — relay URL as server; groups as channels. Requires a
   NIP-29-capable relay. LiveKit token HTTP is derived from the WebSocket URL
   at runtime. [source]
3. **Buzz compatibility** — `src/buzz/` mirrors Buzz timeline/thread/edit/delete
   semantics so the same client can participate in Buzz-shaped workspaces when
   pointed at such a relay. This is a peer protocol adapter, not a fork of
   Block's product. [source]

### 2.4 Release and forge posture

Primary CI is **ngit-ci**: Nostr-native workflows under `.ngit/act/workflows/`
(`test`, `deploy-web`, `release`, `desktop`) run with `act` in Linux containers.
Results and artifacts publish to Nostr and surface on gitworkshop.dev. GitLab CI
remains as a mirror path for macOS `.dmg` and some release links. Version codes
for Android are tag-derived, not pipeline-counter-derived, after shallow-clone
version collisions. [source]

There are no lightweight Git tags in the audited clone tip survey; release
history is carried in `CHANGELOG.md` and release commits such as
`Release v0.43.0`. Product versioning is therefore changelog/commit driven more
than annotated-tag driven in this checkout. [source] [limitation]

## 3. Concord v2 architecture

### 3.1 Kind and plane registry

`src/concord-v2/lib/kinds.ts` freezes the envelope and rumor kinds used by the
client. Outer durable wraps are `1059`; ephemeral wraps are `21059`. Seals are
`20013` (encrypted rumor) or `20014` (plaintext control for re-wrap compaction).
Chat reuses familiar kinds where they fit (`9` message, `1111` comment, `7`
reaction, `5` delete) and uses a dedicated `33xx` block for control, rekey,
guestbook, invites, WebXDC, edits, kicks, and snapshots. [source]

Control editions (`3308`) are sub-kinded by a `vsk` tag: metadata, role,
channel, grant, banlist, invite registry, dissolved. That is a compact
membership and policy log folded client-side, not a server ACL table. [source]

### 3.2 Derivation and address space

`derive.ts` treats HKDF labels as wire format. Channel, control, guestbook,
rekey pseudonyms, voice signer/media/sender, grants, banlist, invite keys, and
dissolved markers all derive from a community secret with labeled info strings.
Changing a label re-addresses prior events. Epoch commitments and community id
commitments are explicit. [source]

### 3.3 Stream cryptography

`stream.ts` implements the reversed gift-wrap stream. It enforces the NIP-44
65,535-byte plaintext cap at publish time, verifies seal signatures, checks
rumor id binding, and fails closed on malformed millisecond timestamps. Control
editions can use plaintext seals so compaction can re-wrap signed editions into
a new epoch without re-signing. [source]

### 3.4 Membership, roles, and recovery

Roles are bitmasks with admin and moderator presets, per-member grants, channel
scopes, and outrank rules (`roles.ts`). Guestbook tracks join/leave, kicks with
authority citation, and refounder snapshots. Rekey and stranded-recovery hooks
exist so members can follow epoch rotation and recover after missing history.
Relay mirroring republishes verbatim wraps so a joiner on a new relay does not
fold a half-arrived control plane mid-epoch. [source]

Recent changelog entries show active hardening: dissolved communities refuse
writes; moderation actions require a verified authority citation; rekey retry
must resume rather than fork. [source]

### 3.5 Armada-local CORD extensions

`CORD.md` documents client conventions that are not ratified CORDs:

- **Private zaps** — payer-attested Lightning proof (invoice + preimage) sealed
  into the chat plane as kind `9735` rumor; no public NIP-57 receipt.
- **On-chain zaps** — Bitcoin attribution kept off public Nostr while the
  transaction itself remains on-chain.
- **Polls** — NIP-88 shapes sealed as rumors.
- **Calendar events** — NIP-52 shapes sealed as rumors.
- **In-call reactions / raise-hand** — voice-room UX on top of CORD-07.

These are the strongest product-layer lessons above the base Concord stack:
payment and social features that refuse to emit public beacons. [source]

## 4. Client substrate

### 4.1 Wire and stores

The wire layer ingests events into a durable store first, then emits coalesced
scope notifications on a 50 ms bus (`src/wire/bus.ts`). Scopes distinguish
NIP-29 groups, DMs, Concord v1/v2 channels and control planes, parked wraps
awaiting keys, and NIP-34 git repositories. Hooks re-read the store rather than
owning parallel live caches. [source]

`ArmadaEventStore` is SQLite-first: native SQLite on Android (shared with the
notification service), SQLite-WASM over OPFS on web/Electron, IndexedDB as the
degraded fallback. Concord decrypted rumors have their own store path. [source]

### 4.2 Auth and key storage

Login uses Nostrify's login provider with a storage adapter: Capacitor secure
storage on native (Keychain/Keystore), `localStorage` on web. Native migrates
legacy plaintext copies into secure storage on first read. Web still stores nsec
in `localStorage`, which the code itself treats as weaker. NIP-46 remote signers
and Android NIP-55 external signers are supported paths. [source]

Agent conventions hard-ban automatic publish of user lists (follows, mutes,
NIP-65, DM relays, Concord membership lists). Empty reads are treated as
indistinguishable from failures; replaceable-list clobber was a real user-data
loss bug twice. That operational rule is as important as any crypto claim.
[source]

### 4.3 Desktop security defaults

Electron loads the bundled build over a privileged custom scheme so service
workers and Web Push work without hosting. Renderer preferences are deny-by-
default: context isolation on, Node integration off, sandbox on. External
navigation opens in the system browser. Close-to-tray keeps relay subscriptions
alive with `backgroundThrottling: false`. Screen sharing uses an explicit main-
process source enumeration plus an in-app picker. [source]

### 4.4 Mobile asymmetry

Android is the feature-complete native host: background notification relay
service, shared SQLite, Amber signing, Bluetooth mesh (vendored bitchat
stack), Credential Manager nsec export, App Links for `armada.buzz`. iOS has
secure storage and LiveKit media permissions but no notifications, no mesh, no
NIP-55, and no universal links yet. Agents are told not to use
`isNativeRuntime()` for notification-gated UI. [source]

## 5. Adjacent product surfaces

These are not side curiosities; they show how far the client reaches beyond
"encrypted Discord."

| Surface | Evidence | Relevance |
| --- | --- | --- |
| Direct messages | NIP-17 gift wraps, request tier for unknown senders, voice DMs | Social graph hygiene without confirming receipt |
| Bot commands | Draft `kind:10304` manifests in `NIP.md`; transport-agnostic `/command` text | Machine-readable agent interfaces without new invocation kinds |
| WebXDC apps | Sandboxed `.xdc` games/apps with chat-plane sync | In-channel collaborative mini-apps |
| NIP-34 git | Issues/PRs/status/CI assembly on repository coordinates | Community-adjacent forge activity inside the client |
| Bluetooth mesh | Android-only offline BLE mesh; Nostr nickname only | Offline continuity, separate crypto identity |
| Cashu / Lightning / on-chain | Ecash cards, private zaps, wallets via NWC/WebLN | Value transfer without public social metadata |
| Discover | Follow-pack gated feeds | Social discovery without a global public feed default |
| Plausible analytics | Optional tracker dependency | Product analytics present; not audited live |

## 6. Comparison anchors already in this catalog

| Peer | Overlap | Important difference |
| --- | --- | --- |
| [Buzz](./2026-07-21-buzz-teardown.md) | Nostr communities, NIP-29, voice, agents-as-members, git | Buzz makes the **relay the workspace**. Armada's default is **serverless sealed communities**; Buzz is one optional compatibility plane |
| [multAIplayer](./2026-07-18-multaiplayer-teardown.md) | Multi-human room around a coding agent | multAIplayer is MLS + single-host Codex; Armada is E2EE community chat without an agent control plane |
| [Paseo](./2026-07-17-paseo-teardown.md) | Cross-device agent/work daemon clients | Paseo is an agent-daemon product; Armada is a community messenger that can host bots |
| Soapbox Shakespeare / nostrify (ngit docs) | Same org stack, Nostrify client libraries, Nostr git culture | Armada is the community/chat product, not the AI app builder |

OpenAgents already treats Buzz as a protocol peer and refuses the relay event
log as product authority. Armada reinforces that refusal for a different reason:
Concord's value is that relays are dumb transport, but the **membership and
policy truth still live in client-folded sealed logs**. That is not the same as
OpenAgents' receipted Work Unit / grant / verification chain. [inferred]

## 7. OpenAgents lessons

### 7.1 Adapt

1. **Sealed collaboration planes as an interoperability option.** When humans
   and agents need a community that generic relays cannot read, Concord-style
   gift-wrapped planes are the strongest open product evidence. Use them as an
   optional interoperability profile, not as OpenAgents' primary work authority.
2. **Store-first wire with a doorbell bus.** Durable store admission before UI
   invalidation is the right shape for multi-plane event products. Compare with
   OpenAgents Sync and Desktop event stores; do not copy React Query ownership
   of timeline truth.
3. **List and replaceable-event publish discipline.** Never publish a user's
   lists without an explicit action; refuse to build on empty/failed reads when
   local state says a list existed. This maps directly to OpenAgents identity,
   follow, capacity, and membership projections.
4. **Payer-local payment proofs inside sealed rooms.** Private zap design (no
   public receipt beacon; local preimage verification; uniqueness per payment
   hash) is a useful pattern for any sealed collaboration market or tip surface.
5. **Blind media token brokers.** CORD-07's LiveKit broker that should learn
   nothing about the community is the correct separation between media
   infrastructure and membership authority.
6. **Transport-agnostic bot manifests.** `kind:10304` plus plain-text
   invocations keep ordinary clients participating while rich clients add
   discovery and validation. Relevant to agent-facing tools without inventing a
   second chat protocol.
7. **Electron defaults.** Custom secure scheme, context isolation, no Node in
   renderer, sandbox, external navigation deny, explicit screen-share picker.
   Aligns with OpenAgents Desktop hardening goals.
8. **Empty baked-in servers for shipped clients.** Runtime-configured relays
   and brokers preserve user sovereignty and make air-gapped operation real.

### 7.2 Reject or fence

1. **AGPL client adoption as a dependency or fork base.** Treat Armada as
   read-only design evidence unless there is an explicit owner license decision.
2. **React + Tailwind + shadcn as product architecture.** OpenAgents UI remains
   Effect Native with thin host renderers.
3. **Web `localStorage` nsec as a model for Desktop secrets.** Native secure
   storage is better; web key storage remains a known weak path.
4. **Capacitor WebView mobile as the OpenAgents mobile architecture.** Useful
   for packaging lessons; not a substitute for Effect Native on Expo.
5. **Relay or sealed-plane fold as release/acceptance authority.** Membership
   editions and chat tallies are not OpenAgents receipts, verification, or
   product-promise admission.
6. **Host-optional chat as a substitute for brokered execution.** Concord removes
   a chat host. It does not provide sandboxing, spend gates, placement, or
   worktree execution.
7. **Buzz/NIP-29/Concord triple surface as one OpenAgents UI authority.** Keep
   explicit adapters and projections; do not collapse three membership models
   into one ambient "server" object.

### 7.3 Decision for the catalog

**Disposition:** high-relevance teardown. Add Armada to the competitive and
interoperability set beside Buzz, Paseo, and the Soapbox ngit stack. Prefer it
as evidence for:

- serverless E2EE community design,
- dual public-protocol and sealed-plane products in one client,
- Nostr-native release/CI culture,
- privacy-preserving in-room payments and bots.

Do not open an implementation packet from this document alone. Any adoption
requires a separate admitted plan or issue, authority reconciliation, and a
license decision for anything beyond protocol interop.

## 8. Suggested follow-ups (not admitted work)

These are research or candidate seeds only:

1. Diff Armada Concord stream crypto and rekey against upstream
   `concord-protocol/concord` at a pinned commit.
2. Read `armada-relay` for the blind AV broker and NIP-29/LiveKit coupling.
3. Compare Armada's Buzz adapter kinds with the Buzz teardown's event catalog
   for exact interop coverage and gaps.
4. Evaluate whether OpenAgents Forum or owner-private threads ever want a
   sealed-plane profile versus current Cloud SQL / Sync authority.
5. If bot-manifest interop becomes useful, treat `NIP.md` kind `10304` as a
   draft external convention, not as OpenAgents tool authority.

## 9. Central finding

Armada is the best open product evidence that a Discord-class community client
can default to **serverless end-to-end encryption over ordinary Nostr relays**,
while still offering relay-owned NIP-29 groups, Buzz compatibility, multi-
surface packaging, private payments, bots, git activity, and offline mesh on
Android. The protocol depth is real and unusually well documented for a seven-
week-old repository. OpenAgents should learn from its sealed-plane, store-first,
list-discipline, and blind-broker patterns — and should refuse to adopt its
stack, license, or membership log as product authority.
