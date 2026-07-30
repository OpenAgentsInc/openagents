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
bot manifests — only behind explicit OpenAgents policy. Section 8 turns that
disposition into a Rust-first Omega integration path.**

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

## 8. Omega integration path

This section is the 2026-07-30 Omega follow-up. It compares Armada with the
current Omega source and the existing Buzz and NIP-29 plans. It is an
integration path, not implementation admission. The important correction to
the earlier high-level disposition is that Omega is no longer starting from a
placeholder NIP-29 reader. It already has a useful native slice. [source]

This follow-up inspected OpenAgents at
`c12308f87eccef73be32bc421ac7251a844717a3` and Omega `origin/main` at
`806ed312573b5ff4228bee40bbdfb0686974c1f4`. The local Omega worktree had
unrelated in-progress changes, so the current-state claims below use the
fetched remote tree for affected public-channel and identity files. [source]

### 8.1 What Omega has now

Omega's current NIP-29 work is split across a deliberately narrow public-room
product and a broader community model:

| Current surface | Implemented behavior | Boundary that remains |
| --- | --- | --- |
| `agent_ui::omega_public_channels` | Versioned registry and descriptors; canonical `wss` relay URLs; relay-qualified `(relay_url, group_id)` coordinates; bounded kind sets and limits; two pinned destinations (`omega-alpha-feedback` and `openagents-public`); selected-channel snapshots, lifecycle, caching, and unread counts | The shipped registry is still a bounded tester-channel profile, not a directory or complete room repository |
| `agent_ui::omega_public_channel_timeline` and view | Verified timeline rows, profiles, reactions, author deletions, relay moderation tombstones, pins, content warnings, gated media with digest checks, event facts, pagination, replay/reconnect states, and visible stale/outage behavior | No complete membership, roles, invite, subgroup, migration, or branch resolver |
| `agent_ui::omega_public_channel_publish` | Kind `9` messages and kind `1984` reports; exact bounded event construction; `previous` references; `omega_identity` signing; verification of returned signed bytes; NIP-42 publication and relay acknowledgements; retry without exporting secret material | This is scoped tester-channel writing, not a general NIP-29 composer or moderation client |
| `omega_identity` | Native key custody, import/create/recovery, and admitted signing requests | Signing policy needs room coordinate, supported-kind, membership, role, expiry, rate, media, and human-confirmation grants |
| `omega_effectd` | NIP-01 sessions, NIP-42 authentication, bounded cache, failover mechanics, and publish acknowledgements | NIP-29 authoritative-relay semantics must override generic failover for writes |
| `omega_community` | Rust values for Forge audiences, relay lists, signed records, an outbox, joined rooms, presence, and command parsing | It is NIP-34/NIP-22 plus Forge membership. It must remain a separate authority from a NIP-29 room |
| `workroom_ui::community` | Typed source labels and projections for NIP-29 membership/transcript, NIP-LBR work, and awards | Much of this is a projection shell; it is not yet wired to a complete room store |
| `public-nostr-chat` skill | An agent can read or write a configured public NIP-29 group with its own signer | A procedural skill is not a product session, durable store, or ambient signing grant |

The practical result is a **signed public-channel wedge**: Omega can show and
write a carefully bounded NIP-29-flavored channel without making the relay its
work, execution, or product authority. This is the correct base for
interoperability. The next work should generalize the protocol and store below
that view rather than replace the GPUI product with Armada or Buzz UI.
[source] [inferred]

The complete NIP-29 target already exists in
`docs/nostr/2026-07-27-omega-nip29-relay-groups-integration-spec.md`. It defines
relay-qualified room identity, NIP-11 relay-self verification, NIP-42,
membership actions, relay-signed `39000`–`39005` projections, moderation,
pins, subgroups, branch/migration state, media, signer grants, and cross-client
fixtures. Current source has completed material parts of its first two phases
and part of its posting phase, but not the complete target. [source]

### 8.2 What the Buzz plan already commits Omega to

The current roadmap does not propose a Buzz fork. Packets `OMEGA-BZ-00`
through `OMEGA-BZ-09` instead reproduce selected outcomes:

1. freeze shared workroom, thread, item, actor, decision, evidence, and receipt
   identities with generated Rust and TypeScript contracts;
2. add native GPUI workroom, attention, timeline, roster, work, run, and
   receipt panes;
3. attach existing agents through capability-declaring ACP adapters;
4. add replies, reactions, mentions, pins, bookmarks, read state, reminders,
   presence, typing, notifications, direct/group threads, and authorized
   search;
5. join rooms to project, worktree, editor, Git, terminal, task, test, diff,
   review, commit, and delivery projections;
6. add typed decisions, blockers, workflows, approvals, receipts, and
   multi-user governance;
7. add optional Nostr interoperability through an isolated signer and Nostr
   process; and
8. dogfood one complete feature from decision through delivery receipt.

The authority constraints are already clear: do not import the Buzz server,
relay authority, Tauri/Flutter clients, administrative client, forge, or broad
custom-NIP surface. Do not let membership grant file, process, provider,
release, spend, or publication authority. Git remains repository authority;
OpenAgents admission remains action authority; Omega remains execution
authority. [source]

Armada changes the ordering, not those constraints. Because Armada speaks both
NIP-29 and a Buzz-shaped protocol, Omega should make its Buzz outcome work
arrive as explicit wire profiles on one Rust room substrate. Concord then
becomes a third profile on the same product model, not a second chat
application.

### 8.3 Target architecture: one room product, three wire profiles

Omega should expose one native room/workroom experience over three non-
interchangeable protocol profiles:

```text
                              GPUI room/workroom panes
                                         |
                         typed Room / Thread / Item projections
                                         |
                  verified store + cursor + outbox + doorbell bus
                         /               |                 \
                        /                |                  \
          NIP-29 baseline       Buzz compatibility       Concord v2
         relay-owned room       relay-owned workspace   sealed client fold
      kinds 9/7/5 + 9xxx      45xxx + Buzz extensions   1059/21059 wraps
                        \                |                  /
                         \               |                 /
                    relay sessions + signer broker + media broker
                                         |
              OpenAgents admission / Omega execution / Git authority
```

The shared product model must not erase protocol authority:

| Question | NIP-29 profile | Buzz profile | Concord profile | OpenAgents/Omega authority |
| --- | --- | --- | --- | --- |
| Who may write chat? | Authoritative relay membership/policy | Buzz relay community and channel membership | Client-folded sealed control, grants, roles, and epoch | May further restrict an agent signer, but must not fabricate peer membership |
| What is a room? | Normalized relay URL plus group id | Buzz relay plus channel UUID/profile | Community commitment, epoch, plane, and stream derivation | Stable local `RoomRef` maps to the external coordinate; it does not replace it |
| What is message truth? | Verified accepted room events and relay state | Verified Buzz events under the pinned compatibility profile | Verified/decrypted wraps and folded Concord control/chat planes | Local store records provenance and gaps; it does not silently merge profiles |
| What may start work? | Nothing by signature alone | Nothing by signature alone | Nothing by membership or a sealed command alone | An explicit OpenAgents admitted command with generation, scope, idempotency, and grant |
| What proves completion/payment/release? | Not the room log | Not Buzz workflow or usage events | Not the sealed plane or private zap | Existing receipts, verification, acceptance, settlement, Git, and release gates |

This is intentionally a profile architecture rather than a universal event
translator. Lossless forwarding may preserve exact signed events or Concord
wraps. Cross-profile projection is read-only by default. A bridge that
re-publishes content must be an explicit actor, cite the source event, declare
loss, prevent loops, and obtain authority for the destination. [inferred]

### 8.4 Rust-first component boundaries

Most of this path belongs in Rust. TypeScript remains useful for the existing
web/mobile surfaces, deterministic fixtures, and protocol comparison, but
Omega Desktop should not introduce a Node or WebView runtime for chat.

1. **Protocol-neutral room contract.** Freeze `RoomRef`, `ExternalRoomCoordinate`,
   `RoomProfile`, `VerifiedEnvelope`, `TimelineItem`, `ThreadRef`,
   `MembershipState`, `RoleState`, `BranchState`, `Cursor`, `Gap`,
   `OutboxAttempt`, `MediaDescriptor`, and `SignerGrant`. Define the canonical
   schema once, generate or verify Rust and TypeScript forms, and retain
   unknown kinds as bounded raw records.
2. **Rust verified event store.** Admit bytes only after size, JSON, event-id,
   signature, tag, coordinate, profile, and timestamp checks. Persist raw
   events, source relay, receipt/acknowledgement, verification result, cursor,
   and derived projection separately. Commit before notifying GPUI. Decrypted
   Concord rumors need a separate encrypted-at-rest namespace from public
   NIP-29/Buzz events.
3. **Rust doorbell bus.** Emit coalesced invalidations keyed by room, plane,
   thread, membership, outbox, and media. GPUI controllers re-read durable
   projections instead of owning a second timeline cache. This adapts Armada's
   strongest substrate lesson without importing its React Query stack.
4. **Rust relay/session manager.** Reuse `omega_effectd` NIP-01/NIP-42,
   reconnect, cache, and acknowledgement mechanics. Add per-profile
   subscriptions, NIP-11 self-key verification, authoritative-write routing,
   explicit replica reads, backfill bounds, and relay capability reports.
5. **Rust signer broker.** Extend `omega_identity` admitted requests. The
   broker receives a typed unsigned intent, checks the current external room
   state and Omega grant, shows the exact actor/room/kind/audience, obtains any
   required gesture, signs once, verifies returned bytes, and records the
   immutable attempt. It never accepts raw arbitrary JSON from a view or agent.
6. **Rust profile adapters.** Each adapter owns accepted kinds, validation,
   fold rules, outbound builders, capability discovery, and golden vectors.
   The GPUI view receives only typed projections and capability flags.
7. **TypeScript edge packages.** Keep Effect Schema decoders and the current
   `public-nostr-chat` behavior for web/mobile. Consume the same JSON vectors.
   TypeScript can also generate upstream conformance fixtures from pinned
   Armada/Buzz source, but it is not Desktop signing or store authority.

Implement these as additions to existing crates where the boundary already
exists. New crates are justified only for a substantial reusable protocol core
or Concord crypto/fold module; UI, transport, identity, and community behavior
should not be fragmented into many small crates. [inferred]

### 8.5 Profile 1: finish standards-first NIP-29

This is the shared interoperability floor for native NIP-29 relays, Buzz, and
Armada's relay-owned mode.

1. Promote the tester-channel descriptor into a versioned room-profile
   registry with source revision, capability evidence, relay-self key, and
   profile-specific kind meanings.
2. Move current timeline/session state into the durable verified store while
   preserving current pagination, reconnect overlap, all-`EOSE` currentness,
   media gates, tombstones, report flow, and visible degraded states.
3. Implement NIP-11 and NIP-42 per authoritative relay. Treat relay-self key
   changes as reviewable security events.
4. Fold `39000`–`39005` state, including optional/missing projection behavior.
   Add membership `9021`/`9022`, relay results `9000`/`9001`, moderation
   `9002`/`9005`, invites `9009`, pin replacement `9010`, and explicit relay
   capability mappings. Do not infer powers from role labels.
5. Add join/leave, membership-aware composer state, invites, roles,
   moderation, pins, subgroups, and branch/migration review.
6. Add bounded background subscriptions and durable unread/read state only
   after selected-channel correctness survives restart and replay.

Acceptance requires conformance against at least the owned OpenAgents relay
profile and one independently implemented NIP-29 relay, plus malicious-frame,
wrong-relay-state-key, duplicate, fork, late-event, outbox, and restart tests.
A room with the same group id on two relays must remain two branches. [inferred]

### 8.6 Profile 2: Buzz compatibility

Buzz compatibility should be a pinned extension of NIP-29, not a claim that
Buzz implements every current NIP-29 behavior. The known differences matter:
Buzz uses custom Forum roots/replies/votes (`45001`–`45003`), adds channel
types, community commands, and other workspace kinds, and historically used
`39005` for a thread-summary overlay where the pinned NIP-29 assigns group
pins. Its direct-reply conventions also differ from the pinned NIP-10 text.
[source]

The adapter path is:

1. Pin exact Buzz and Armada commits and build a compatibility manifest:
   accepted kinds, required tags, deletion/edit/reply semantics, state kinds,
   authentication, search/pagination, media, and known collisions. Never
   dispatch a kind using only its integer; dispatch by `(profile, kind)`.
2. Add golden inbound and outbound vectors from Buzz Desktop, Buzz relay, and
   Armada's `src/buzz/` adapter. A vector must state whether each peer accepts,
   renders, ignores, or rejects it.
3. First ship read-only channel and Forum interop: channel metadata/roster,
   kind `9` timeline, `45001` roots, `45003` replies, kind `7` reactions,
   kind `5` author deletion, `9005` moderation, profiles, search cursors, and
   media references.
4. Then ship bounded writes for the intersection proven against both peers.
   Keep `45002` votes separate from reactions. Reject Buzz thread-summary
   `39005` as a NIP-29 pin; support it only under the pinned Buzz profile and
   map it to a non-authoritative summary projection.
5. Add optional Buzz identity projections in roadmap order: OA/AA/AP/AE first,
   then AM/AO and other families only where product policy exists. Usage and
   live-state events remain telemetry, not billing or completion receipts.
6. Project Buzz agents as room actors, then attach them to Omega ACP sessions
   through explicit grants. An inbound mention or command creates a proposal;
   it cannot directly start a process, spend money, publish, or merge code.

The first meaningful parity milestone is mutual channel and Forum
participation: a Buzz user, Armada user in Buzz mode, and Omega user can read
and reply in one relay-owned workspace, observe consistent deletions and
reactions, and see explicit degradation for every unsupported feature.
[inferred]

### 8.7 Profile 3: Armada Concord v2

Concord is not an encryption toggle on a NIP-29 room. It has different
addresses, membership authority, replay, rekey, and failure modes. It therefore
needs a dedicated Rust protocol core and store namespace.

1. Pin upstream Concord and the audited Armada implementation separately.
   Treat upstream as protocol authority and Armada as peer-client evidence.
   Record every Armada-local `CORD.md` extension independently.
2. Implement derivation labels and kind registry as typed constants with
   byte-for-byte golden vectors. A changed HKDF label is an address migration,
   not a refactor.
3. Implement read-only stream admission: kind `1059` and `21059` wraps,
   reversed gift-wrap roles, NIP-44 bounds, seal signature checks, rumor-id
   binding, timestamp validation, community/epoch/plane coordinates, parked
   wraps awaiting keys, and deterministic fold ordering.
4. Implement control-plane folds before chat writes: metadata, roles, channel
   grants, banlist, invite registry, dissolved state, guestbook join/leave,
   kicks with authority citations, control editions, and epoch commitments.
5. Add secret custody and recovery: community secrets in native secure
   storage, explicit invite import/export, rekey resume without fork, stranded
   recovery, relay mirroring of verbatim wraps, and deletion of expired
   plaintext working material. Never route Concord plaintext through normal
   telemetry, public search, or the device bridge.
6. Add chat-plane publish and common social features: messages, comments,
   reactions, deletions, edits, polls, calendar, and WebXDC only after each
   kind has cross-client vectors. Preserve sealed content; do not "improve
   compatibility" by also publishing public beacons.
7. Add CORD-07 voice last. Reuse the admitted native LiveKit/WebRTC path or a
   separate Rust media component, keep per-sender media encryption, and use a
   blind token broker. Broker reachability is not membership authority.

The first Concord milestone should be read/join/rekey/message interoperability
with Armada across two ordinary relays, including offline catch-up and a
missed-epoch recovery case. The release gate needs adversarial crypto review;
passing peer fixtures is interoperability evidence, not a security proof.
[inferred]

### 8.8 Product parity after wire interoperability

Wire support alone will still feel far behind Armada. After the three profiles
share the native room shell, apply the Buzz roadmap's product packets in this
order:

| Product slice | Native Omega outcome | Interoperability rule |
| --- | --- | --- |
| Room navigation and attention | Multi-room rail, unread/mentions, bookmarks, pins, reminders, presence, typing, notifications | Store a local normalized projection, but retain original external coordinates and capability gaps |
| Threads and Forum | Timeline threads plus long-form topic list/thread panes | Use profile-native reply semantics; never rewrite one signed topology into another silently |
| People and agents | Roster, identity facts, role/membership state, agent ownership and grant display | A room role never implies Omega tool or execution authority |
| Work and code | Link room items to WorkContext, project, worktree, task, diff, review, commit, and receipt | Carry references and digests; Git and OpenAgents remain authoritative |
| Bots and commands | Discover transport-agnostic manifests and render `/command` affordances | Plain text remains valid; invocation requires destination membership plus an Omega command admission |
| Files and apps | Safe previews, content-addressed blobs, then sandboxed WebXDC/canvas surfaces | Large bytes stay off Nostr; signed events carry digests and references |
| Voice | Native room calls, reactions, raise hand, agent STT/TTS only after audio admission | NIP-29/Buzz and Concord use separate membership/token paths behind one capability UI |
| Payments | Optional private tips/proofs in sealed rooms and public-safe payment references elsewhere | Payment proof is not work acceptance or settlement authority |

Omega should not chase every Armada feature before proving the shared work
loop. The dogfood gate remains one feature that starts with a typed decision,
uses a room with a separately granted agent, produces code/tests/review, and
ends in an OpenAgents delivery receipt. Repeat that journey once over NIP-29 or
Buzz and once over Concord. [inferred]

### 8.9 Delivery packets and gates

The detailed path can be admitted as these bounded packets:

| Packet | Deliverable | Exit evidence |
| --- | --- | --- |
| `OMEGA-ROOM-00` | Profile-neutral Rust/TypeScript contract, authority table, source pins, collision registry, and cross-language fixtures | Schema round trips and unknown-kind preservation |
| `OMEGA-ROOM-01` | Durable verified event store, outbox, cursor/gap model, and coalesced invalidation bus | Restart, corruption, replay, duplicate, and partial-write tests |
| `OMEGA-ROOM-02` | Complete standards-first NIP-29 adapter and native room administration | Two-relay conformance and person/agent signer tests |
| `OMEGA-ROOM-03` | Buzz read interop, Forum projections, and three-client fixture matrix | Buzz and Armada-Buzz captures render equivalently or expose named gaps |
| `OMEGA-ROOM-04` | Buzz bounded writes and agent identity projections | Cross-client post/reply/reaction/delete/report journey |
| `OMEGA-ROOM-05` | Concord read/control fold, secure secret store, invites, and rekey/recovery | Armada cross-client replay and missed-epoch recovery |
| `OMEGA-ROOM-06` | Concord chat writes and common sealed social kinds | Two-relay offline/reconnect journey and security review |
| `OMEGA-ROOM-07` | Native attention, threads, roster, work links, bots, files, and search | Packaged GPUI accessibility/restart proof |
| `OMEGA-ROOM-08` | Profile-specific voice and optional payments | Blind-broker/media crypto proof; explicit payment authority review |
| `OMEGA-ROOM-09` | Web/mobile adapters, migration/export, dogfood, and parity ledger | Rust/TypeScript vector parity and end-to-end release evidence |

Every packet must record current behavior as `equivalent`, `stronger native
replacement`, `intentionally unsupported`, or `blocked`. "Parsed the event"
does not count as product support. Every write path needs peer acceptance,
local acknowledgement, restart recovery, refusal UX, and an authority test.

### 8.10 Decisions required before admission

The path still needs owner decisions on:

- whether Concord is an optional community profile or a strategic primary
  private-room profile;
- which exact Buzz revision and feature subset constitute compatibility;
- whether Omega runs only client adapters or also an owned Buzz-compatible
  NIP-29 relay profile;
- the encrypted-at-rest store and backup policy for Concord secrets and
  decrypted rumors;
- the NIP-29/Buzz media upload provider and the CORD-07 blind AV broker;
- the first web/mobile write surface and its signer custody;
- whether any bridge may re-publish between profiles, and under whose visible
  identity; and
- the license review boundary for clean-room protocol implementation from
  upstream specifications versus Armada's AGPL source.

Until those are decided, the safe next admission is `OMEGA-ROOM-00` followed
by the durable store work. Neither commits the product to Concord authority or
to running Buzz infrastructure. [inferred]

## 9. Suggested follow-ups (not admitted work)

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
6. Turn the current Omega tester-channel implementation into an exact
   current-state evidence matrix against the July 27 NIP-29 target.
7. Capture three-client golden vectors from Buzz, Armada Buzz mode, and Omega
   before any outbound Buzz compatibility work.
8. Decide whether the first Concord proof is an external Rust crate, an Omega
   crate, or a reviewed upstream dependency before implementing crypto.

## 10. Central finding

Armada is the best open product evidence that a Discord-class community client
can default to **serverless end-to-end encryption over ordinary Nostr relays**,
while still offering relay-owned NIP-29 groups, Buzz compatibility, multi-
surface packaging, private payments, bots, git activity, and offline mesh on
Android. The protocol depth is real and unusually well documented for a seven-
week-old repository. OpenAgents should learn from its sealed-plane, store-first,
list-discipline, and blind-broker patterns — and should refuse to adopt its
stack, license, or membership log as product authority. Omega already has the
right first wedge: a Rust/GPUI, identity-backed, relay-qualified public channel
that keeps signed chat separate from work authority. The path toward Armada
parity is to finish that standards-first NIP-29 substrate, add a pinned Buzz
profile, then add Concord as a separate Rust crypto/fold profile over the same
durable room product. Shared UI does not require shared authority, and protocol
interoperability does not require replacing Omega's execution, receipt, Git,
or settlement owners.
