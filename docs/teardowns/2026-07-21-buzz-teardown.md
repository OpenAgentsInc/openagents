# Buzz Teardown — 2026-07-21

Read-only architecture and product audit of the public `block/buzz` source
tree at an exact commit in the local reference clone
`~/work/projects/repos/buzz`. Nothing tracked was modified. The Git follow-up
ran bounded protocol tests, but it did not run a live relay. A second source
review traced the shared-compute and usage paths at the same follow-up commit.
A third source review traced identity, authentication, and device pairing at
that commit. It did not run a live MeshLLM node or complete a real device
transfer. A fourth review on 2026-07-27 traced Forum behavior in Buzz and
OpenAgents. It used a newer Buzz source snapshot but did not run either
Forum. Buzz is Block's open-source, self-hostable workspace where humans
and AI agents are co-equal members of a Nostr-relay community. It is the
closest whole-system analog to OpenAgents in the teardown catalog so far: one
company shipping chat, forum, git forge, workflows, voice, agent harnesses,
agent identity, and a desktop app on one signed event log. The current
comparison targets are the extracted Effect-native AI SDK
(`OpenAgentsInc/ai` — harness contract, `KhalaRuntimeEvent` streams, RLM
recall), the desktop chat runtime
([`docs/desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md`](../desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md)),
and the Pylon/forum/promise surfaces.

## Summary

Buzz makes one bet: the relay is the workspace. A community is one URL backed
by one Nostr relay. Every message, reaction, forum post, canvas edit,
workflow step, review approval, moderation action, git patch, and CI status
is a signed NIP-01 event in one log, and the `kind` integer is the sole
dispatch switch. Agents are members, not bots. An agent holds its own
Nostr keypair, its own channel memberships, its own memory, and its own
audit trail, and it reaches the workspace through the same protocol humans
use. [source]

```text
Human client (Tauri desktop)   Agents (goose / codex / claude / buzz-agent)
        |                                   |
        |                     buzz-acp (WS -> ACP stdio subprocess pool)
        |                                   |
        +----------- WebSocket + REST ------+
                          |
                     buzz-relay (Axum)
   NIP-01 / NIP-29 / NIP-42 · REST · git Smart HTTP · huddle audio
                          |
   Postgres (events + generated FTS)   Redis (pub/sub)   S3/MinIO (Blossom + git CAS)
```

The system is far deeper than the README suggests. The relay emits
TLA+-shaped runtime traces that an independent checker replays against the
`MultiTenantRelay.tla` spec. Tenancy is a structural fence derived from the
request host, never a wire tag. The full-text index is a Postgres
`GENERATED ALWAYS` column, so search can never disagree with signed content.
Voice huddles tunnel Opus frames through the same relay with a WebRTC NetEq
jitter buffer on the client, so there is no separate media server. Agent
memory is encrypted with a symmetric NIP-44 conversation key, so the owner
can always decrypt what the agent remembers. [source]

The central OpenAgents decision: **adopt a selected Buzz-compatible protocol
profile, but do not adopt Buzz as a product substrate, dependency, or client
shell. Keep Cloud SQL, Khala Sync, and the current session contracts as
authority. Use Nostr for signed identity, encrypted records, discovery,
portable projections, and admitted collaboration input. Reuse standard NIPs
first. Reuse the implemented Buzz NIPs in `nostr-effect` only behind explicit
OpenAgents policy. Reject the relay event log as product authority, the Tauri
shell, the Flutter mobile lane, and the non-streaming agent turn model.**

The 2026-07-27 Forum review adds a narrower future option.
A new relay-native Forum can use NIP-29 after an authority decision.
The current Forum must stay on its current authority until that decision.
Section 6.10 gives the evidence and migration gates.

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Public repository | `https://github.com/block/buzz` | Public source and history |
| Local clone | `~/work/projects/repos/buzz` | The audited tree |
| Audited commit | `e9188c03f6c2460983a3dac0fa7702b468838e62` | Exact snapshot used here |
| Git, MeshLLM, usage, and device-auth follow-up commit | `5a3b8176aac5f4bced452ac8920477c5e059b828` | Source snapshot for the Git, shared-compute, identity, authentication, and pairing deep dives |
| Forum follow-up commit | `be13b4bb9ce228b21fa3682ce75d75cba5950561` | Source snapshot for the Buzz Forum, desktop, mobile, and protocol comparison |
| Psionic comparison commit | `54201484bb8eb11b528f7038922db02724864523` | Current native inference-mesh, topology, receipt, and service-mode comparison |
| `nostr-effect` commit | `c1603780f754d445b3cb8203ea5602b54c145996` | Local implementation snapshot for standard and Buzz NIPs |
| OpenAgents historical sources | `docs/transcripts/README.md`, episodes 142, 147, 174, 178, 201, 203, 214, 215, 237, and 238 | Data-vending, compute-market, Pylon, Psionic, and accepted-outcome lineage |
| NIP-90 status sources | `docs/nostr/2026-06-09-openagents-pylon-nostr-relay-audit.md` and `docs/fable/2026-07-21-nostr-native-pivot-analysis.md` | Current warning against making NIP-90 the market default |
| Secondary Git source | Soapbox, "What is Ngit?" (`2026-07-21`) | Ecosystem claims checked against Buzz source |
| Commit time | `2026-07-21` | Freshness of the audited tip |
| Commit subject | `chore(release): release Buzz Desktop version 0.4.22 (#2220)` | Latest audited change |
| Internal codename | Sprout / sprig (`Cargo.toml` repository field points at `block/sprout`) | Naming lineage |
| Product versions | Desktop `0.4.22`, mobile `0.4.11`, 134 git tags | Fast pre-1.0 release train |
| License | Apache-2.0 | Permissive reuse boundary |
| Maintainer | Block Inc (`@block/buzz-oss-team` owns everything in CODEOWNERS) | Single-vendor governance |
| Languages | Rust workspace (27 crates), TypeScript/React (Tauri desktop), Dart (Flutter mobile) | Runtime commitments |
| Backend scale | 306 Rust files, about 218,000 lines under `crates/` | Large implementation |
| Desktop scale | About 224,000 TS/TSX lines plus about 91,000 Rust lines under `desktop/` | App is a second large system |
| Mobile scale | About 33,500 Dart lines under `mobile/lib/` | Real but not at parity |
| Dependency weight | 964 resolved crates in `Cargo.lock` (255 KB) | Heavy Rust supply chain |
| History | First commit 2026-03-06, 1,764 commits, 37 named contributors | Four and a half months old |
| Activity | 801 commits in the 30 days before the tip, multiple releases per day | Very high velocity |

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, manifests, or config at the commit.
- **`[history]`** — Git history at or before the audited commit.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

The initial audit did not build or execute Buzz. The Git follow-up ran three
bounded checks. `nostr-effect` passed 69 NIP-34 and NIP-GS tests. Buzz passed
209 `buzz-core` tests and all 8 credential-helper integration tests. The Buzz
NIP-GS library passed 55 of 56 tests. Its parser accepted an all-zero OA public
key that the test and draft require it to reject. [source] [limitation]

This audit did not run the relay, desktop app, mobile app, a MeshLLM node,
live MinIO tests, TLA+ checker, or Tamarin checker. It did not complete a live
NIP-AB transfer. The live Git and shared-compute tests are ignored by default.
All other behavior claims come from tracked source and docs. Only `block/buzz` is public.
Block-internal builds, deploy pipelines, and the hosted relay are out of scope.
[limitation]

## 2. What Buzz is

Buzz ships seven product surfaces on one relay: Home, Stream (Slack-like
channels and threads), Forum (Discourse-like async posts), DMs (NIP-17 gift
wrap), Agents (a directory plus managed agent supervision), Workflows
(YAML automation), and Search (Postgres FTS behind Cmd+K). Canvases, media
with frame-anchored comments, custom emoji, reminders, moderation, voice
huddles, and a full git forge sit on the same event log. [source]

The design rule is stated in `ARCHITECTURE.md` and enforced in
`crates/buzz-core/src/kind.rs`: a new feature is a new event kind, about 81
kind constants at the audited tip, and the relay dispatches on the kind
integer alone. Buzz implements NIP-01, NIP-05, NIP-09, NIP-10, NIP-11,
NIP-16, NIP-17, NIP-25, NIP-29, NIP-34, NIP-42, NIP-43, NIP-50, NIP-70,
NIP-98, and Blossom media, and it authors about fifteen custom NIPs in
`docs/nips/` — agent authentication (NIP-AA), agent engrams (NIP-AE), agent
turn metrics (NIP-AM), agent observability (NIP-AO), personas (NIP-AP), git
object signing (NIP-GS), push leases (NIP-PL), read-state sync (NIP-RS), and
more. [source]

The tenancy story matters. The URL is authoritative for the community. A
hosted operator can run thousands of communities on shared Postgres, Redis,
and object storage, but the wire format never grows a tenant tag. The
community is derived from the request host into a `TenantContext`, every
query binds the `CommunityId` as its first predicate, and there is no
construction path that omits it. [source]

## 3. Architecture walk

### 3.1 The relay pipeline

`crates/buzz-relay/src/handlers/event.rs` runs one ingest pipeline: NIP-42
auth, pubkey match, ephemeral routing for kinds 20000–29999,
`spawn_blocking` Schnorr verification, membership check, idempotent Postgres
insert (`ON CONFLICT DO NOTHING`), Redis publish, subscriber fan-out, then
fire-and-forget search indexing (bounded queue), hash-chain audit
(`crates/buzz-audit`), and workflow triggering. Connection lifecycle in
`ARCHITECTURE.md` §3 is defensive throughout: a connection semaphore, a
proactive AUTH challenge, a 30-second heartbeat with a three-miss limit, and
a slow-client grace counter over `try_send`. [source]

Two storage decisions stand out. The FTS index is a
`search_tsv TSVECTOR GENERATED ALWAYS AS to_tsvector(...) STORED` column
with a GIN index, so the row write is the index update and a client cannot
make search disagree with the content it signed
(`crates/buzz-search/src/lib.rs`). Git repos are hosted by the relay itself
over Smart HTTP (`crates/buzz-relay/src/api/git/`) with objects on S3
content-addressed storage, a pre-receive hook, and an HMAC-authenticated
loopback-only policy callback. [source]

### 3.2 Formal conformance at runtime

The most unusual engineering choice in the repo: the production relay emits
an abstract trace (`TraceStep`, `TraceAction`, `AbstractState`) at the
ingest and read accept/reject boundary, and `crates/buzz-conformance`
replays that trace against a Rust reimplementation of the `Next` relation
from `docs/spec/MultiTenantRelay.tla`. The checker deliberately shares no
normalization helpers with the emitter, so a shared bug cannot hide from
both sides. It checks the cross-tenant non-interference invariant, and a
critical seam that exits without a trace step is itself a failure class
(`ImplBug`). A Tamarin proof (`docs/spec/MultiTenantAuth.spthy`) covers the
auth model, and `docs/spec/GitOnObjectStore.tla` covers the git CAS design.
[source]

### 3.3 The agent surface

Three layers, composed through protocols rather than imports. [source]

- **`crates/buzz-acp`** is the harness: relay WebSocket in, Agent Client
  Protocol (JSON-RPC over stdio) out to a pool of 1–32 agent subprocesses
  with claim/return, crash-respawn, and a per-channel queue that keeps at
  most one prompt in flight and batches the rest. The configured agent
  command defaults to `goose`, and the config maps `codex`, `claude-code-acp`,
  and their own `buzz-agent` as drop-in alternates. For Codex it injects a
  generated `CODEX_CONFIG` that opens sandbox network access so the injected
  `buzz` MCP tool can reach the relay through the macOS Seatbelt sandbox
  (`crates/buzz-acp/src/config.rs`). Recent work surfaces stall duration and
  stall fate as typed outcomes.
- **`crates/buzz-agent`** is their own minimal ACP agent: a direct LLM loop
  over OpenAI, Anthropic, and Databricks providers, non-streaming,
  tool-calls-as-output, zero unsafe, bounded tool calls per turn, bounded
  tool-result bytes, a mid-turn steer queue, and a token-budget-driven
  context handoff that summarizes its own history and continues.
  `crates/buzz-dev-mcp` gives any agent a shell and a file editor with
  process-group kill on every exit path.
- **`crates/buzz-cli`** is the agent-first tool surface: JSON in, JSON out,
  covering channels, messages, DMs, canvases, reactions, workflows, repos,
  uploads, and the social graph. The base prompt in
  `crates/buzz-acp/src/base_prompt.md` teaches the CLI, not an SDK.

Agent identity is a Nostr secret key (`BUZZ_PRIVATE_KEY`), zeroed in memory
after parse. Agent memory (engrams, kind 30174) is encrypted with the
NIP-44 conversation key between the agent and its owner. The key is
symmetric, so the owner can always decrypt everything the agent remembers
(`crates/buzz-acp/src/engram_fetch.rs`, `docs/nips/NIP-AE.md`). Agents can
create and own git repos signed with their own npub, and git auth and commit
signing run over Nostr keys (`crates/git-credential-nostr`,
`crates/git-sign-nostr`). [source]

### 3.4 Workflows and moderation

`crates/buzz-workflow` executes YAML definitions stored as canonical JSON.
Triggers: message posted (with an expression filter), reaction added,
diff posted, cron/interval schedule, and webhook. Actions include
`send_message`, `send_dm`, `call_webhook`, `delay`, and `request_approval` —
a human gate inside the automation. Loop prevention is structural: workflow
execution kinds 46001–46012, relay-signed workflow-tagged events, and gift
wraps are excluded from triggering. Moderation is a workflow, not an
admission filter: reports are private structural state that never enters the
event log, actions are signed commands validated against the roster,
enforcement bites at the identity seam, removals leave honest tombstones,
and both the author and the reporter hear the outcome
(`VISION_MODERATION.md`). [source]

### 3.5 The desktop app

`desktop/` is Tauri 2 with React 19, TanStack Query and Router, Radix
primitives, and TipTap. Three choices are notable. First, the WebSocket
lives in Rust: `desktop/src/shared/api/relayClient.ts` drives a `tungstenite`
socket in `desktop/src-tauri/src/native_websocket.rs` over Tauri IPC, behind
a hardened reconnect stack (`relayReconnectController.ts`, a stall watchdog
for half-open sockets, a rate-limit gate, reconnect replay — each with a
test sibling). Second, `desktop/src-tauri/src/managed_agents/` makes the app
an agent-orchestration host: it spawns and supervises agent subprocesses,
injects relay URL and keys, tracks readiness, and reconciles desired state.
Third, `desktop/src-tauri/src/huddle/` implements voice from scratch — Opus,
a Google WebRTC NetEq jitter buffer, custom playout and preprocessing, plus
STT and TTS so agents can listen and speak in huddles — and the relay
forwards the audio payloads opaquely. `desktop/src-tauri/src/mesh_llm/`
implements community-pooled LLM compute discovery for the `VISION_MESH.md`
"your community is your compute" story. [source]

### 3.6 Shared compute and usage accounting

Buzz uses MeshLLM as a member-to-member inference data plane. Buzz adds the
user interface, identity binding, relay discovery, and membership-derived
admission policy. The implementation pins MeshLLM `v0.73.1` and Iroh `1.0.2`.
The main code is under `desktop/src-tauri/src/mesh_llm/`. [source]

This feature has four separate planes. They are easy to confuse because each
plane uses the word "mesh." [source]

```text
Buzz member identity and NIP-43 roster
        |
        | signed kind 30003 status and discovery
        v
Buzz relay (control plane only)
        |
        | current member, model, and endpoint selection
        v
Buzz Desktop embedded MeshLLM node
        |
        | OpenAI-compatible localhost ingress on 127.0.0.1:9337
        | direct QUIC or end-to-end encrypted Iroh relay transport
        v
Member GPU or multi-node MeshLLM route

buzz-agent usage response -> buzz-acp delta tracker
        -> encrypted kind 44200 NIP-AM metric -> owner
```

The last line is an accounting projection. It is not the MeshLLM admission
plane. It also does not account for the machine that supplied the compute.
[source]

#### 3.6.1 Build and runtime shape

The desktop `mesh-llm` Cargo feature enables six optional MeshLLM crates and
Iroh. Normal local development does not enable it. Developers must use
`just mesh=1 dev` or an equivalent feature flag. Non-mesh builds register
Tauri stubs that return `mesh-llm feature not enabled`. The macOS release
workflow builds the app with `--features mesh-llm`. Thus, the release lane is
different from the default development lane. [source]

Buzz embeds the MeshLLM serving and client SDKs in the Tauri process. It does
not run a separate MeshLLM daemon. The embedded node owns these local ports:
[source]

- `9337` is an OpenAI-compatible API. Managed agents use
  `http://127.0.0.1:9337/v1`.
- `3131` is the MeshLLM console. Buzz exposes its URL only in the advanced
  Compute settings.

On first use, `mesh-llm-host-runtime` installs or loads its signed native
runtime. Serve mode downloads the selected model before node startup. This
ordering lets the Tauri progress sink show model bytes and preparation state.
Buzz gives startup 180 seconds. It configures 8 MiB Tokio worker stacks for
the deep MeshLLM futures. [source]

Both serve and client modes set `publish(false)` and `auto_join(false)`.
They do not publish MeshLLM presence to public Nostr relays. Buzz selects a
peer from its own relay status events. It passes one validated join token to
the SDK. The SDK then routes inference through direct QUIC or an encrypted
Iroh relay. The Buzz relay does not carry prompts, completions, or token
streams. [source]

#### 3.6.2 What happens when a member shares a machine

The Settings > Compute card has one consent toggle, one model field, and an
optional maximum VRAM value. A member can type a model reference, select a
cached model, or select a hardware-ranked catalog entry. Buzz surveys the GPU
and usable AI memory. It rates model fit at 60, 90, and 110 percent of usable
memory. The labels are `comfortable`, `tight`, `tradeoff`, and `too_large`.
Models in the last class are visible but disabled. [source]

Buzz puts two tested instruction models above the advanced catalog. Machines
with at least 64 GB of rated memory get the larger curated recommendation.
Smaller machines get the smaller recommendation. Draft-only speculative
models do not appear as shareable models. The picker marks cached files, but
the SDK can download a remote model after the member starts sharing. [source]

The start command resolves the current trusted owner list before it creates
the node. It starts in `serve` mode, loads the model, probes status, and stores
the preference in `mesh-sharing.json` under the app data directory. Workspace
startup restores an enabled share preference. A stop clears the runtime,
writes a disabled preference, and publishes an empty serving status. [source]

The serving runtime has one local OpenAI ingress. A serving member can also
consume another member's model through that same runtime. If an agent requests
a model that is not local, the MeshLLM router can select a remote or split
target. Buzz does not start a second client runtime for that case. [source]

The current Buzz status projection does not publish usable capacity data.
`MeshTargetCapacity` has a `vramGb` field, but locally built serve targets set
`capacity` to `None`. The shared-compute control does not set time windows,
request quotas, peer priorities, concurrency, energy limits, or compensation.
The maximum VRAM input limits local model use. It is not a community quota.
[source]

#### 3.6.3 Identity binding and relay discovery

Each machine has two identities. The Buzz member uses a Nostr secp256k1 key.
MeshLLM uses a separate Ed25519 owner key in its default
`~/.mesh-llm/owner-keystore.json` file. The MeshLLM owner identifier is the
SHA-256 digest of the Ed25519 verifying key. [source]

The coordinator publishes a client-signed NIP-51 bookmark-set event. It uses
kind `30003`, a `d` tag of `buzz-mesh-member-status:<ownerId>`, and a `k` tag
of `buzz-mesh-status`. This use needs no mesh-specific relay handler. The
status content contains the owner identifier, verifying key, ready models,
serving targets, and endpoint tokens. It contains no owner secret or local
model path. [source]

Two Ed25519 signatures bind the two identity systems. `ownerBindingSig` signs
the member Nostr public key. `ownerEndpointBindingSig` signs that member key
and a length-framed SHA-256 digest of the sorted endpoint tokens. The outer
Nostr signature proves which member published the status. The inner signatures
prove that the member controls the MeshLLM owner key. They also prevent an
unbound endpoint substitution. [source]

The desktop publishes status at startup and every 45 seconds. It also
publishes while stopped. A stopped event advertises the owner identity with
empty model and target arrays. This behavior keeps a consumer-only member in
the admission identity set. Routing ignores events older than 120 seconds.
Admission does not use that freshness limit because an offline device does not
end community membership. [source]

Discovery first reads the latest relay-signed NIP-43 direct-member snapshot,
kind `13534`. It then reads kind `30003` status events only from those member
authors. Status pages have a limit of 100. They use a timestamp and event-id
cursor. Routing accepts only a fresh event from a current member with valid
owner and endpoint signatures. It then validates every endpoint token against
local transport policy. [source]

Model collection uses only ready model records. It deliberately ignores the
MeshLLM `serving_models` field because that field can become visible before
inference is ready. Buzz removes an `@main` qualifier when it compares model
identifiers. It deduplicates by canonical model and endpoint. A saved agent
stores the model reference, not the live endpoint token. It resolves a current
bootstrap target each time it starts. [source]

#### 3.6.4 Admission and membership changes

Buzz derives the MeshLLM owner allowlist by intersecting two sets. The first
set is the current NIP-43 member roster. The second set is the valid owner
bindings in member-signed mesh status events. A status from a non-member does
not add an owner. A leaked endpoint token does not add an owner. Each node also
adds its own owner identifier to the allowlist. [source]

The node starts with `owner_required(true)`, `TrustPolicy::Allowlist`, and the
resolved owner identifiers. If the first roster query fails, startup uses a
self-only allowlist. A missing kind `13534` snapshot is an error, not an empty
community. This distinction prevents a temporary relay gap from creating an
incorrect roster. [source]

The coordinator polls the roster every 60 seconds. It restarts the embedded
node when the allowlist changes because the SDK trust store is fixed at node
start. Pure roster growth applies after one successful poll. A roster shrink
must appear in two consecutive polls. A failed poll keeps the current
allowlist. These rules reduce churn and prevent one short read from removing
active members during inference. [source]

The SDK performs the final owner check after the transport connection, during
MeshLLM gossip. The Buzz runbook explicitly does not claim authentication
before gossip. Buzz pins a MeshLLM release with the fix that prevents passive
inference use by a caller that has only a leaked invite token. [source]

#### 3.6.5 Endpoint and relay policy

Buzz treats discovery data as untrusted network input. A join token can hold
an unsigned Iroh endpoint or a MeshLLM signed bootstrap token. The validator
applies these bounds and rules: [source]

- The encoded token can contain at most 64 KiB.
- A signed token can contain from one through eight endpoint records.
- Each endpoint can contain from one through 16 transport addresses.
- A signed bootstrap envelope must pass its MeshLLM signature check.
- Direct addresses cannot use port zero, loopback, link-local, multicast,
  broadcast, unspecified, or equivalent unsafe targets.
- An unsigned endpoint keeps only locally permitted transports. It fails if no
  permitted transport remains.
- A signed endpoint cannot be rewritten without invalidating its signature.
  Thus, one disallowed address rejects the signed token. Buzz accepts the
  MeshLLM port-zero placeholder only when another address is usable.

`BUZZ_MESH_IROH_RELAYS` selects the local relay policy. An unset value enables
Iroh production relays and the two default MeshLLM public relays. `0` permits
only direct QUIC. A comma-separated value is an explicit allowlist. Custom
relays must be HTTPS origins without credentials, paths, queries, or fragments.
Only loopback development relays can use HTTP. A remote status event cannot
expand the local relay allowlist. [source]

The Iroh relays forward encrypted QUIC traffic. They can observe transport
metadata, but they cannot read the inference content. A direct endpoint on a
private address is permitted. This permits local-network peers while the
blocked-address rules prevent loopback and metadata-service targeting.
[source]

#### 3.6.6 How an agent consumes shared compute

`relay-mesh` is a native Buzz provider identifier. It is available only for a
local `buzz-agent` runtime. The model picker reads member status events from
the Buzz relay. It does not query the local `9337` API because that API might
not exist before the first peer selection. `auto` means that any live serving
target can bootstrap the client. The MeshLLM router then selects a suitable
model for each request. [source]

When an agent starts, Buzz checks for an existing embedded runtime. If one
exists, Buzz reuses its local ingress and dials a selected peer when necessary.
If no runtime exists, Buzz resolves one live target for the saved model. It
then starts an embedded MeshLLM client with that join token. Buzz distinguishes
a relay query failure from a valid query that finds no serving peer. [source]

Before it starts the agent subprocess, Buzz sends a real one-token chat request
through `127.0.0.1:9337`. It retries for as long as 120 seconds. This probe
closes the gap between an available HTTP port and a router that can perform
inference. [source]

At subprocess spawn, Buzz translates `relay-mesh` into the transport that
`buzz-agent` already understands. It sets the provider to OpenAI Chat, the
base URL to the local MeshLLM ingress, and a non-secret placeholder API key.
It removes an ambient `OPENAI_API_KEY`. It also sets a 4,096 output-token limit
and disables reasoning effort to fit smaller local model contexts. Thus, the
full path is: [source]

```text
Buzz Desktop -> buzz-acp -> buzz-agent -> localhost MeshLLM API
             -> selected member node
```

#### 3.6.7 What Buzz records about compute use

Buzz does not have a MeshLLM-specific billing ledger. It reuses NIP-AM for
agent-turn usage across all supported harnesses and providers. The accounting
starts only when an ACP agent emits a Goose-compatible
`_goose/unstable/session/update` notification with `usage_update`. Goose and
`buzz-agent` use this shape. Other agents that do not emit it produce no turn
metric. [source]

`buzz-agent` reads token counts from provider responses. For the OpenAI Chat
shape that MeshLLM exposes, it reads inclusive `prompt_tokens` and
`completion_tokens`. It adds these values to session-cumulative input and
output counters. It then emits a usage update before the ACP prompt response,
so `buzz-acp` still considers the turn active. The update includes the
effective model. It does not include a local cost estimate. [source]

`buzz-acp` keeps a baseline for each session. It computes one turn delta from
the final cumulative update and the prior published turn. Multiple updates in
one turn do not advance the baseline. The final update wins. The first observed
turn has no reliable delta. A counter decrease also makes the delta unreliable.
In both cases, the metric keeps cumulative values and sets the turn values to
null with `deltaReliable: false`. `turnSeq` advances only when ACP drains a
completed turn for publication. [source]

At turn completion, `buzz-acp` creates one regular kind `44200` event when it
has usage and an owner key. The payload contains the harness, model, channel,
session, turn identifier, turn sequence, timestamp, per-turn values,
cumulative values, reliability flag, and stop reason. The current publisher
sets `totalTokens`, cache-read tokens, and cache-write tokens to null. A cost
can come from a harness that reports cumulative cost, but NIP-AM defines it as
an estimate and not a billing record. [source]

The agent encrypts the payload to its owner with NIP-44 v2. The public envelope
has one `p` tag for the owner and one `agent` tag that must equal the event
author. It has no channel tag. The relay verifies the Nostr signature, envelope
shape, NIP-44 version prefix, and the stored agent-owner relation. It stores the
event but excludes it from full-text search. Every read path requires NIP-42
authentication and a matching owner `#p` filter. Knowing an event identifier
does not grant read access. [source]

Metric publication is fail-soft. Encryption, signing, or relay submission
errors are warnings and do not fail the agent turn. Relay submission has a
three-second timeout. Therefore, the record is useful for owner reporting, but
it is not an exactly-once or settlement-grade receipt. [source] [inferred]

The desktop can copy these records into its local SQLite archive. During
archive ingest, it decrypts kind `44200` and stores the payload as plaintext so
local calculators can read it. Decryption failures are dropped. OSS builds do
not enable this local archive by default. The user can enable **Archive my
agents' turn metrics**. An internal build flag can enable it once per identity
until the user makes an explicit choice. [source]

The live session transcript can show compatible ACP usage frames. It coalesces
multiple frames for one turn. The durable metric archive has no complete
fleet-cost or compute-contribution dashboard in the audited source. More
importantly, kind `44200` identifies the requesting agent and model, but it does
not identify the serving MeshLLM owner, endpoint, GPU time, queue time, energy,
or bytes. Buzz can estimate agent token use. It cannot use this record to
measure who supplied shared compute or how much value they supplied. [source]

#### 3.6.8 Implemented boundary versus vision

`VISION_MESH.md` says a community can split a model across several machines.
Buzz delegates routing and split inference to MeshLLM. The code accepts remote
or split targets through the same local ingress. However, the audited Buzz test
suite does not prove the full claim by default. Mesh unit tests are
deterministic. The two-node inference and split-model acceptance tests are
ignored because they require native runtimes, models, and several live nodes.
One live test remains a documented placeholder. This audit did not run the
manual GUI proof in `docs/buzz-shared-compute-dev.md`. [source] [limitation]

The separate `crates/buzz-relay-mesh` system is not the member GPU pool. It is
an inter-relay Iroh transport for huddles and reliable streams. Its `BUZZ_MESH`
configuration and relay startup code belong to horizontal relay operation.
Some `buzz-relay` MeshLLM examples are local admission and inference smokes.
The product shared-compute runtime remains in Buzz Desktop. [source]

The implemented result is a private, opt-in inference pool for current relay
members. It is not a compute market. It has no credits, payments, bidding,
provider settlement, contribution score, or provider-side usage ledger. The
NIP-AM path accounts for agent turns only. [source]

#### 3.6.9 Comparison with OpenAgents history and current Psionic

Buzz shared compute overlaps three OpenAgents ideas that appeared at different
times. Those ideas must remain separate. The transcript guide in
`docs/transcripts/README.md` gives the lineage. The individual transcripts
provide the claim detail. The recent Nostr audits provide the current protocol
disposition. [source]

| Era | OpenAgents plan | Intended economic unit | Current reading |
| --- | --- | --- | --- |
| Episodes 142 and 147 | NIP-90 Data Vending Machines and a data marketplace | A request, result, or contributed data item | Useful history for permissionless discovery and composition. NIP-90 is no longer the preferred default protocol. |
| Episodes 174 and 178 | GPUtopia 2.0 and swarm inference | A whole inference job served by an online provider | Closest historical match to Buzz. It added NIP-90 job pickup and Lightning payment. |
| Episodes 201, 203, and 214 | Pylon, Nexus, and the Compute Market | Provider work with routing, receipts, budgets, and Bitcoin settlement | Broader than Buzz. The node was meant to compete for public jobs and earn money. |
| Episode 215 | NIP-DS Data Market | A durable dataset listing, offer, access contract, and optional targeted delivery | Correctly separates durable goods from request transport. Its optional DS-DVM profile still inherits NIP-90 risk. |
| Episodes 237 and 238 | Accepted outcomes and the Tassadar run | Scoped work that is executed, independently verified, receipted, and settled to contributors | Strongest current economic model. Payment follows accepted evidence, not reported token use. |

The early DVM plan was request-first. Episode 142 demonstrated an inference
request on a public relay. Any listening provider could answer it. Payment and
competition were the intended next layer. Episode 147 extended the same model
to data collection, curation, bounties, and agent-assisted verification. This
was a market-discovery design. It was not a distributed model-execution design.
[source]

Episodes 174 and 178 moved the idea to whole-model compute. A user clicked
**Go Online**. A local Ollama-backed provider listened for NIP-90 jobs. The
demo sent an inference request from one machine to another and paid Lightning
invoices. Episode 203 named the provider node Pylon and the relay Nexus. It
described requests as signed JSON jobs that many providers could pick up. The
same episode also admitted that routing and matchmaking across several
providers still needed experimentation. [source]

Episodes 201 and 214 made the missing market plumbing explicit. Discovery,
packaging, verification, reputation, settlement, observability, replay, and
receipts were all required to turn idle hardware into procurable supply.
Episode 215 applied that lesson to data. NIP-DS created durable listings,
offers, and access contracts. NIP-90 remained only an optional targeted
request and delivery profile. [source]

Episodes 237 and 238 moved the accounting unit higher. The accepted outcome
became the economic atom. A task is scoped, executed, graded, receipted, and
settled. The Tassadar loop adds a second participant that replays the work.
Both worker and validator can receive payment. This model can pay inference,
training, data, software, or human work under one evidence rule. It does not
assume that tokens are a sufficient measure of value. [source]

The current NIP-90 audit changes the protocol conclusion. The June Pylon
Nostr audit records both NIP-90 product promises as red. It says the current
Pylon path is not a live NIP-90 marketplace. It also notes that upstream marks
NIP-90 unrecommended. The July Nostr-native pivot reaches the same result.
Use-case-specific microstandards should replace the broad 5000-series DVM
namespace. NIP-LBR is the candidate labor microstandard. A compute market
needs its own narrow event family if Nostr interoperability is required.
[source]

##### Buzz versus the old compute-market plan

Buzz implements a trusted compute commons. The old OpenAgents plan described
a permissionless compute market. The two systems share a local provider
on-ramp and an OpenAI-compatible consumption path. Their trust and accounting
models differ. [source] [inferred]

| Concern | Buzz shared compute | Historical OpenAgents market |
| --- | --- | --- |
| Who may serve | Current members of one NIP-43 community | Any admitted provider on one or more relays |
| Discovery | Member-signed kind 30003 status events | Public job and provider discovery through NIP-90 and Nexus |
| Selection | MeshLLM route selection after one Buzz bootstrap target | Providers compete for jobs. Routing and matchmaking choose supply. |
| Transport | Direct QUIC or encrypted Iroh relay | Nostr carries job coordination. Provider transport varied by runtime. |
| Work topology | Whole remote request or MeshLLM split route | Primarily independent whole jobs and large fan-outs |
| Usage record | Owner-private NIP-AM requester token deltas | Provider receipts, budgets, invoices, and settlement were intended |
| Payment | None | Lightning or Bitcoin payment was central |
| Verification | No independent verifier in the shared-compute path | Replay, receipts, and later accepted-outcome grading |
| Capacity policy | No published capacity, price, quota, or schedule | Sellable capacity, price, availability, and buyer budget were core goals |
| Failure authority | Community admission and MeshLLM runtime state | Market admission, work acceptance, and settlement require separate authorities |

Buzz therefore proves a narrower but useful fact. A social trust group can
share inference without publishing prompts through its coordination relay.
It does not prove that strangers can price, verify, attribute, and settle the
same work. Adding sats to kind 44200 would not close that gap. [source]
[inferred]

##### Buzz versus Psionic

Psionic already performed a direct MeshLLM harvest audit on 2026-04-02. That
audit treated MeshLLM as a product-layer reference, not the long-term runtime
owner. The recommended harvest included join flows, node roles, bootstrap
proxying, management status, demand signals, and operator UX. It rejected a
permanent `llama.cpp` sidecar boundary and retained Psionic ownership of
runtime, backend, artifact, topology, evidence, and refusal truth. [source]

That harvest is now represented in current Psionic contracts and code. The
comparison below uses Psionic commit `54201484`. [source]

| Concern | Buzz plus MeshLLM | Current Psionic |
| --- | --- | --- |
| Runtime owner | MeshLLM SDK and signed native `llama.cpp` runtime inside Tauri | Rust-native Psionic runtime and backend crates |
| Product packaging | One toggle inside Buzz Desktop | Installable `psionic-mesh-lane` service with durable roots, `launchd`, and `systemd` artifacts |
| Identity | Nostr member key bound to a MeshLLM Ed25519 owner key | Psionic node identity, admission material, durable join state, and session generation |
| Admission | NIP-43 membership intersected with valid owner bindings | Typed admission, join bundles, ordered membership, and explicit refusal reasons |
| Roles | Serve mode or client mode | Separate transport roles and served roles: host, worker, standby, and thin client |
| Routing | MeshLLM router behind localhost port 9337 | Router-owned model inventory, demand windows, elections, warm state, and route provenance |
| Bootstrap | Buzz selects one live status target before client startup | Thin-client and warming proxy modes publish local served truth separately from remote route truth |
| Distributed topology | Delegated to MeshLLM. Default live tests are ignored. | Typed replicated, pipeline-sharded, layer-sharded, tensor-sharded, and sparse-expert contracts |
| Current execution proof | No live proof from this audit | One real two-machine pipeline-sharded `gemma4:e4b` request is documented. Sparse placement remains bounded by model-family claims. |
| Artifact truth | Ready model and serving target status | Artifact digests, shard manifests, placement digests, topology digests, and residency truth |
| Receipts | Requester-side token metric only | Execution receipts, provider receipts, response provenance, and typed refusals |
| Market authority | Absent | Explicitly outside Psionic. OpenAgents owns wallet, payout, procurement, and settlement. |

Psionic is more demanding about claim shape. Its management API distinguishes
`remote_whole_request`, `replicated`, `dense_split`, and `sparse_expert`.
It publishes the realized topology and route provenance. Host elections carry
a term, active host, standby set, promotion reason, and lease state. Sparse
placements carry explicit expert ranges and a stable placement digest. An
unsupported topology returns a typed refusal instead of a generic unavailable
state. [source]

Psionic also separates durable service state from application state. A mesh
lane root keeps configuration, node identity, network state, logs, and model
paths across restart. Reinstalling service artifacts does not rotate identity.
Changing the namespace or admission token after durable state exists is
refused. Buzz instead stores a desktop share preference and lets the MeshLLM
keystore own the compute identity. [source]

Buzz is ahead in one important product dimension. Its social roster directly
controls who may use the pool. The member sees one consent toggle, one model
choice, and model-fit guidance. Psionic has the stronger substrate and a small
operator console, but OpenAgents still owns the work needed to turn its mesh
status into a friendly provider and buyer experience. [source] [inferred]

##### The missing bridge

The combined OpenAgents design should keep four ledgers distinct. One event
may reference the others, but no ledger can substitute for another. [inferred]

1. **Membership and admission ledger.** Who may join, route, and serve.
2. **Execution ledger.** What runtime, model, artifact, topology, route, and
   machine performed the work.
3. **Acceptance ledger.** What was requested, how it was graded, who verified
   it, and whether it was accepted.
4. **Settlement ledger.** What amount was authorized, paid, withheld,
   refunded, or split among contributors.

Buzz has the first ledger and a requester-oriented fragment of the second.
Psionic has strong machine-facing execution truth. The historical Pylon and
accepted-outcome plans define the third and fourth ledgers. A future market
should bind them with stable identifiers and hashes. It should not collapse
them into one relay event or one token counter. [source] [inferred]

### 3.7 Clients, ops, and process

`web/` is a small browser repo-viewer that the relay itself serves, which
funds the `VISION_SOVEREIGN.md` claim that one domain is both the rendered
site and the `git clone` endpoint. `admin-web/` is a tiny operator console.
`mobile/` is Flutter with Riverpod, on its own release lane, with kind
constants manually mirrored from the desktop TS constants. Self-hosting
needs Postgres 17, Redis 7, and S3-compatible object storage
(`docker-compose.yml`), with a Caddy compose path and a fully unit-tested
Helm chart under `deploy/charts/buzz/`. The toolchain is pinned through
Hermit. Releases are three independent PR-driven lanes and no human ever
pushes a git tag (`RELEASING.md`). [source]

The repo's own agent process is part of the product. `AGENTS.md` (symlinked
as `CLAUDE.md`) mandates quality gates, bans new `unwrap()`, and defines a
PR-screenshot protocol for agent contributors. The `.claude/`, `.codex/`,
`.goose/`, and `.agents/` skill directories are all symlinks to one skill
source in `desktop/src-tauri/src/managed_agents/nest_skill.md` — the same
skill the shipped desktop app gives to end-user managed agents. Custom CI
lints enforce rem-only font sizes, a 1,000-line file ceiling, and pubkey
truncation rules. [source]

## 4. What is genuinely good

1. **Runtime conformance against a formal spec.** Emitting abstract traces
   from production seams and replaying them against an independently
   reimplemented TLA+ transition relation — with "seam exited without a
   trace step" as its own failure class — is rare outside large-scale infra
   teams, and it makes the multi-tenant fence a checked property rather than
   a convention (`crates/buzz-conformance`). [source]
2. **One log, one dispatch switch.** The kind-integer discipline gives every
   feature the same identity model, the same audit trail, and the same
   search index for free. The claim "the channel becomes the record of why
   the code exists" is structurally true because patches, CI, review, and
   the merge decision are events in the same room. [source]
3. **Agents as members, scoped by identity.** An agent has a keypair,
   memberships, memory, and an audit trail instead of permission flags. The
   owner-decryptable engram design is the sharpest single idea in the repo:
   a symmetric conversation key makes agent memory auditable by construction,
   not by policy. [source]
4. **Harness-agnostic supervision.** The ACP pool treats goose, Codex,
   Claude Code, and their own agent as interchangeable subprocesses behind
   one protocol, with per-channel queues, crash respawn, and typed stall
   fates. Their own `buzz-agent` exists as a minimal auditable fallback, not
   as a moat. [source]
5. **Integrity by construction.** The generated FTS column, the host-derived
   structural tenancy fence, the fail-closed reaction channel derivation,
   and the p-gated subscription rules all move correctness from review into
   shape. [source]
6. **Voice without a media server.** Tunneling Opus through the existing
   relay with a NetEq jitter buffer client-side keeps the deployment story
   at three services and gives agents ears and a voice in the same rooms.
   [source]
7. **Dogfooding with one source of truth.** Four agent-runtime skill
   directories symlinked to the single skill file that also ships inside the
   product is the cleanest repo-agent/product-agent unification in the
   catalog. [source]

## 5. Weaknesses and frictions for OpenAgents

- **The substrate conflicts with settled OpenAgents authority.** OpenAgents
  runs Khala Sync plus Cloud SQL on Google Cloud as conversation and
  projection authority. Buzz's whole design is a Nostr relay as the single
  source of truth. Re-platforming chat, threads, or receipts onto a relay
  event log would reopen decided architecture. [inferred]
- **The Nostr claim is partial.** Buzz deviates from vanilla Nostr where the
  workspace needs it. It requires `#h` tags on kind 9. It has relay-signed
  membership kinds and local NIP-29 routing rules. It does not use NIP-04 or
  open NIP-44 DMs. It also authors 15 NIPs. These choices make Buzz a strong
  Nostr profile inside one server policy boundary, not a generic Nostr client.
  OpenAgents can reuse the wire formats, but it must define its own authority
  and compatibility profile. [source] [inferred]
- **Shell and mobile stacks are the ones OpenAgents rejected.** Tauri 2 for
  desktop and Flutter for mobile conflict directly with the Electron plus
  Effect Native desktop mandate and the Expo/Effect Native mobile mandate.
  The desktop app also duplicates no shared UI package across its three JS
  apps, and mobile mirrors kind constants by hand — a known fragility.
  [source]
- **The agent turn model is non-streaming.** `buzz-agent` treats text as
  reasoning and tool calls as the output. OpenAgents' desktop runtime and
  the AI SDK make live token streams to the UI a central contract (the
  STREAM program, `KhalaRuntimeEvent`). The Buzz model is honest for
  headless work but is the opposite of the OpenAgents UI thesis. [source]
- **Shared compute has no provider accounting.** Buzz records requester-side
  agent token estimates through NIP-AM. It does not bind a turn to the member,
  endpoint, GPU, duration, or energy that supplied inference. Membership is an
  admission rule, not a quota, contribution, settlement, or fairness system.
  This design is sufficient for a trusted compute commons. It is not a compute
  market or a billing substrate. [source] [inferred]
- **Operational weight.** Adoption of any large piece means Postgres, Redis,
  object storage, 964 crate dependencies, and a 218,000-line Rust backend —
  outside the OpenAgents boundary that keeps Rust to the Cloud crates and
  the bounded audio helper. [source]
- **Single-vendor gravity.** Governance is Block org-wide, internal builds
  are preferred over the OSS release for Block staff, and the deploy truth
  lives in private repos. The OSS repo is real, but the center of mass is
  corporate. [source]
- **Youth.** Four and a half months old, pre-1.0, with workflow approval
  gates self-described as "glue still drying" and mobile not at parity.
  Velocity is high, and so is churn. [source] [history]

## 6. What OpenAgents should adapt

These are Fast Follow candidate lessons in the `docs/teardowns/` evidence
lane. Nothing here is dispatch authority. Implementation requires the normal
admission path.

**6.1 Runtime conformance replay at authority seams.** The
`buzz-conformance` pattern — production code emits abstract traces at
accept/reject boundaries, an independent checker replays them against the
formal spec, and a missing trace at a critical seam is itself a failure —
maps directly onto the workspace formal-verification mandate
(`INVARIANTS.md`), the Cloud contract crates
(`crates/openagents-cloud-contract`, `fixtures/cloud/`), and the
AssuranceSpec Observer design (`docs/assurance/`). The concrete candidate:
pick one bounded OpenAgents authority seam (FleetRun admission, Pylon
dispatch gating, or promise transition authority), write the small spec,
emit traces, and replay them in the normal test sweep with a deliberately
independent checker. [source] [inferred]

**6.2 The owner-decryptable agent-memory invariant.** NIP-AE's symmetric
conversation key gives the owner unconditional read access to agent memory.
OpenAgents already stores owner-only ATIF traces and is building durable
owner-profile memory (`packages/agent-experience-memory`) and RLM recall in
the AI SDK. The lesson to port is the invariant, not the encryption scheme:
every durable memory an OpenAgents agent holds must be readable by its owner
by construction, and the audit path must not depend on the agent's
cooperation. State it in the memory package contract and test it. [source]
[inferred]

**6.3 ACP pool supervision with typed stall fates.** The desktop runtime
already has ACP lanes (`grokAcpLane`, `cursorAcpLane` in
`provider-lane-acp.ts`) and a harness contract
(`packages/agent-harness-contract`, and the SDK harness contract in
`OpenAgentsInc/ai`). Buzz's `buzz-acp` adds the parts OpenAgents lanes are
still growing: a claim/return subprocess pool, per-conversation queues with
at-most-one in-flight prompt plus batching, crash respawn, and stall
duration/fate as first-class typed outcomes rather than generic timeouts.
The Codex sandbox-config injection trick (opening sandbox network access so
an injected MCP tool can reach the backend) is directly relevant to Pylon's
Codex lanes. [source]

**6.4 Agent-first CLI plus one shared skill source.** Buzz teaches its
agents one CLI (JSON in, JSON out) instead of an SDK, and it keeps one skill
file that serves four harness directories and the shipped product via
symlinks. OpenAgents already mirrors `.agents/skills` into `.claude/skills`.
The delta worth adopting: make Pylon the single agent-facing CLI story the
way `buzz` is, and collapse any duplicated skill/prompt text so the repo's
own agents and the product's managed agents consume the same tested source.
[source] [inferred]

**6.5 Workflow approval gates and structural loop prevention.** The
`request_approval` action inside YAML automation, plus trigger exclusion of
the workflow's own event kinds, is a compact pattern for the Full Auto and
FleetRun loops: human gates as typed steps inside the automation, and loop
prevention by construction (excluded event classes) rather than by
convention. The moderation design — reports as private structural state,
signals never triggers, enforcement at the identity seam, honest tombstones
— is a strong reference for Forum moderation when that surface grows.
[source] [inferred]

**6.6 Integrity-by-generated-column.** The `GENERATED ALWAYS` tsvector move
generalizes: where OpenAgents keeps projections beside authoritative rows in
Cloud SQL, prefer database-derived columns over application-maintained
mirrors so a projection cannot drift from the row it summarizes. [source]
[inferred]

**6.7 Voice reference for the audio helper.** The huddle stack (Opus over
the existing transport, NetEq jitter buffer, reconnect, STT/TTS for agents
in rooms) is a working reference for the bounded `crates/oa-desktop-audio`
lane and `packages/audio-contract` — evidence that agent-audible voice
rooms do not require a media server, at the cost of relay bandwidth.
[source] [inferred]

### 6.8 Shared-compute lessons

**6.8.1 Reuse the roster-to-admission bridge.** Buzz turns social membership
into compute admission without trusting an endpoint token by itself. The
outer Nostr signature binds the status to a member. Inner MeshLLM signatures
bind the member to the compute owner and endpoints. The current roster then
intersects with those bindings. OpenAgents should reuse this pattern for
private organizations, teams, and trusted clusters above Psionic admission.
The public market still needs a separate admission policy. [source] [inferred]

**6.8.2 Reuse fail-safe membership reconciliation.** Buzz applies roster
growth after one successful poll. It requires two consecutive observations
for shrinkage. Query failure preserves the current allowlist. Missing initial
truth starts self-only. Psionic already owns ordered membership and session
generation. These hysteresis rules are useful policy inputs for a product
that reconciles a Nostr group into that substrate. [source] [inferred]

**6.8.3 Preserve Psionic as the execution owner.** Buzz validates MeshLLM as
a strong product reference. It does not reverse the Psionic ownership split.
Psionic should continue to own backend, model, artifact, topology, routing,
execution receipt, and refusal truth. OpenAgents should own the consent UX,
organization policy, market workflow, wallet, acceptance, and settlement.
[source] [inferred]

**6.8.4 Add a friendly layer above Psionic management truth.** Copy the user
experience, not the hidden runtime boundary. A member should see one clear
share toggle, model-fit guidance, current model, current role, who may use the
machine, resource bounds, active work, and earnings. Every displayed state
should come from the Psionic management API or the market ledgers. The app
must not infer topology or provider work from logs. [source] [inferred]

**6.8.5 Publish supply-side receipts.** A provider receipt should bind the
request, accepted route, Psionic execution receipt, provider node, artifact,
topology, wall time, token counts, queue time, resource class, and price
commitment. Split execution requires one contribution record per participating
node. The aggregate receipt must preserve every contributor reference. NIP-AM
may remain owner-private telemetry. It must not become the payment basis.
[inferred]

**6.8.6 Settle accepted outcomes, not raw usage.** Tokens can price a quoted
inference lane. They do not prove correctness or value. Payment release should
bind to an explicit acceptance policy. Deterministic work can use independent
replay. Subjective work needs a declared rubric or buyer acceptance. Failure,
partial completion, duplicate work, and verifier disagreement need explicit
withhold or refund states. [inferred]

**6.8.7 Replace broad NIP-90 dependence with narrow event families.** Keep
Nostr as a portable multi-party market record where it adds exit and audit
rights. Define separate, bounded event families for compute offers, job terms,
provider results, acceptance, and settlement references. Reuse standard NIPs
for identity, encryption, handler discovery, payments, and reputation. Do not
put new market identity inside the unrecommended NIP-90 namespace. [source]
[inferred]

**6.8.8 Require live proof before a distributed claim.** Buzz's default suite
does not prove its live split-inference story. Psionic documents a real
two-machine dense split but keeps wider topology claims bounded. An
OpenAgents pooled-inference release should require a repeatable multi-host
acceptance lane. It should capture topology, artifact, route, output,
disconnect, retry, contribution, acceptance, and settlement evidence in one
reviewable bundle. [source] [inferred]

### 6.9 Counterfactual: a much more Nostr-centric OpenAgents

The implementation state changes the original premise of this audit.
`nostr-effect` now implements all 15 Buzz custom NIPs at commit `c160378`.
It also has broad support for standard NIPs, including NIP-34 and the GRASP
server-list kind. OpenAgents can now test a Buzz-compatible protocol profile
without importing Buzz code or running the Buzz relay. [source]

Protocol support is not product support. Some `nostr-effect` modules are
complete client services. Other modules are wire formats, readers, or relay
stubs. OpenAgents must add product policy, custody, persistence, and evidence
rules around each module. [source] [inferred]

#### 6.9.1 Standard NIPs to use first

| Priority | NIPs | OpenAgents use |
| --- | --- | --- |
| 1 | NIP-01, NIP-16, NIP-33 | Signed events, replacement rules, and stable addressable records |
| 2 | NIP-42, NIP-43, NIP-70, NIP-98 | Relay authentication, membership, protected events, and HTTP authentication |
| 3 | NIP-44, NIP-59, NIP-17 | Owner-private content, gift wrap, and direct messages |
| 4 | NIP-29, NIP-10, NIP-25, NIP-50 | Groups, threads, reactions, and search |
| 5 | NIP-34 and Blossom | Repository events, code collaboration, and content-addressed blobs |
| 6 | NIP-40, NIP-65, NIP-78 | Expiration, relay preferences, and application state |

The first three groups form the base profile. They define identity,
lifecycle, access, and privacy. NIP-34 is the first large product profile
to add because OpenAgents already has coding sessions, review, and Git
workflows. Large checkpoints and repository packs must stay outside Nostr.
Events should carry references and digests for those bytes. [inferred]

#### 6.9.2 Buzz NIPs to use by product value

| Order | NIPs | Value | Current `nostr-effect` depth |
| --- | --- | --- | --- |
| 1 | NIP-OA, NIP-AA, NIP-AP, NIP-AE | Owner to agent proof, agent auth, persona, and owner-readable memory | OA and AE have full services. AA has client, verifier, and optional relay modules. AP covers personas and managed instances, but not the full Buzz team surface. |
| 2 | NIP-AM, NIP-AO | Per-turn usage and live agent telemetry | Client and crypto services exist. Product admission and relay owner gates do not. |
| 3 | NIP-RS, NIP-ER, NIP-AB | Read state, reminders, and device pairing | Client services and pairing state exist. Product scheduling, push, and pairing UI still need work. |
| 4 | NIP-GS | Nostr signatures in Git commits and tags | Signing and verification exist. A Git program, custody adapter, and product trust display do not. |
| 5 | NIP-CW, NIP-DV, NIP-WP, NIP-IA | Window projections, DM visibility, workspace profile, and identity archive | Readers and builders exist. Relay derivation and the NIP-IA state machine are incomplete. |
| 6 | NIP-PL | Encrypted push leases | The wire format exists. A push gateway and delivery system do not. |

The first profile should be OA, AA, AP, and AE. These NIPs make the agent
identity legible across products. They also match the current sovereign
identity direction. A signer can expose a public key, sign an admitted event,
and use NIP-44 without exporting an `nsec` or mnemonic. NIP-AB can add a
device without copying the raw key. [source] [inferred]

AM and AO should follow as projections. An AM event can report usage, but it
must not become billing authority. An AO frame can show live state, but it
must not prove that a command ran or that an outcome was accepted. OpenAgents
must keep command, outcome, assurance, settlement, and public-claim gates
separate. [inferred]

#### 6.9.3 Four possible product postures

| Posture | Nostr role | Canonical authority | Assessment |
| --- | --- | --- | --- |
| Signed protocol edge | Identity, encryption, export, and discovery | Current OpenAgents stores | Low-risk first step |
| Signed projection bus | Public-safe or owner-encrypted projections from an outbox | Cloud SQL and Khala Sync | Best fit with current specs |
| Admitted collaboration input | Signed events enter as proposals | OpenAgents policy and command handlers | Useful after replay and scope gates exist |
| Relay as workspace | Relay events own messages, sessions, Git state, and workflow state | A new workspace relay | A new product and authority model |

The signed projection bus is the useful middle path. A Cloud SQL outbox can
produce deterministic events after a canonical write. Consumers can verify
the author, schema version, and content digest. A relay failure can delay the
projection, but it cannot reverse the canonical write. This model gives
OpenAgents portable signed records without a dual-authority system. [inferred]

Admitted input needs a separate result event. A valid signature proves who
signed the proposal. It does not prove permission, admission, execution,
acceptance, release, payment, or a public claim. The handler must check the
Effect Schema, scope, generation, idempotency key, and current policy before
it changes canonical state. [inferred]

#### 6.9.4 What the product could feel like

- A person, agent, device, and workspace has an explicit public key role.
  The product never displays or exports a raw secret key.
- An agent card combines an AP persona, OA owner proof, AE memory status,
  AM usage, and AO live state.
- A channel combines NIP-29 membership, NIP-10 threads, NIP-25 reactions,
  CW windows, and NIP-34 work items.
- A portable coding session publishes signed activity and checkpoint
  references. The checkpoint bytes remain in the admitted object store.
- A third-party client can read public records or owner-encrypted records.
  It does not need access to the OpenAgents database.
- Forum and NIP-90 work requests become the best public pilot. Those surfaces
  already treat Nostr as a protocol and transport rail.

The Git profile is unusually valuable. It gives each coding agent a stable
signing identity across a session, patch, review, and commit. It also lets a
portable session name a repository with a NIP-34 address instead of a host
account ID. Section 7 examines the limits of the Buzz implementation.
[inferred]

#### 6.9.5 Reconciliation with current OpenAgents specs

The current specs support the first three postures. They do not support the
fourth posture without revision. The portable coding-session spec requires an
owner-minted, host-independent session identity. It also requires a durable
event log, exclusive attachment generations, and secret-free checkpoints.
Signed Nostr projections can carry that identity and those checkpoint
references. They cannot replace the canonical session graph by assumption.
[source] [inferred]

The desktop workbench and Cursor-parity specs keep the workbench, harness,
model, placement, sync, and persistence layers separate. A Nostr adapter fits
that separation. A relay-owned workspace would merge the sync and persistence
layers and would require a new ProductSpec. The mobile spec also requires an
owned encrypted reachability path and separates authorization from reachability.
A required third-party relay would violate that rule. [source] [inferred]

The web trust spec already permits Bitcoin, Lightning, and Nostr as product
rails. The authority and assurance specs require explicit roles and separate
proof gates. A Nostr signature can be one proof input under those specs. It is
not an independent reviewer, an accepted outcome, or release authority.
[source] [inferred]

#### 6.9.6 Hard considerations

- **Key custody and recovery.** OpenAuth users, people, agents, devices, and
  workspaces need a clear key map. Rotation and revocation need durable policy.
- **Ordering.** Nostr `created_at` values and event IDs are not dense versions.
  Multi-relay heads can disagree. OpenAgents needs scope versions and replay
  rules above the wire.
- **Deletion.** NIP-09 is a signed deletion request. It cannot remove every
  relay copy. Verified deletion and retention claims need a different proof.
- **Privacy.** NIP-44 and NIP-59 protect content. They still expose timing,
  authors, recipients, sizes, and relay access patterns.
- **Search and moderation.** Encrypted content limits server search and review.
  Public projections need spam, abuse, and Sybil controls.
- **Tenancy.** Buzz derives community tenancy from the host. That rule is a
  Buzz server convention, not a property of Nostr.
- **Compatibility.** The Buzz NIPs are drafts. OpenAgents needs versioned
  profiles, collision checks, extension discovery, and migration tests.
- **Dual writes.** The system must define one direction of authority. It must
  not accept both a database row and a relay event as independent truth.
- **Infrastructure.** A future owned relay must run on Google Cloud. It cannot
  restore the deleted relay app or use retired Cloudflare runtime products.
- **Schemas.** Effect Schema remains mandatory at OpenAgents boundaries.
  Direct NIP reuse supplies protocol mechanics, not product policy.

#### 6.9.7 A staged path

1. Pin a reviewed `nostr-effect` revision and expose stable profile exports.
   Add cross-language vectors for each selected NIP.
2. Add OA, AA, AP, AE, and AB through the sovereign signer boundary.
   Do not export raw keys.
3. Add a transactional projection outbox for a small public-safe record set.
   Make relay publication retryable and non-authoritative.
4. Admit one low-risk collaboration input. Return a separate accepted or
   refused outcome.
5. Pilot one private workspace on a new Google Cloud relay. Keep Cloud SQL
   and Sync authoritative during the pilot.
6. Consider relay authority only after new ProductSpec, AssuranceSpec,
   migration, rollback, retention, and owner-gate work.

These stages are research candidates. They are not implementation dispatch.
The normal claim and admission rules still apply.

### 6.10 Current OpenAgents Forum, Buzz Forum, and NIP-29 rebuild

This section is the 2026-07-27 Forum follow-up.
It compares current source at exact revisions.
It does not implement or admit a Forum migration.

The OpenAgents source pin is
`3702ce35406dc4c8afacb6994c85b3d96170752a`.
The Buzz Forum source pin is
`be13b4bb9ce228b21fa3682ce75d75cba5950561`.
The local NIP source pin is
`db5fe3de8c5d1443b634c9bbf66ecb004f337057`.

The companion target specification is:

- [Omega NIP-29 relay groups integration specification](../nostr/2026-07-27-omega-nip29-relay-groups-integration-spec.md)

That specification owns the complete NIP-29 target.
This section owns the Forum comparison and rebuild gates.

#### 6.10.1 Result

The current OpenAgents Forum is not a Nostr forum.
Its ordinary topics and posts are API and database records.
Its web presentation reads public Worker projections.
Separate Worker routes own all content writes and moderation.

Buzz does use Nostr for its Forum.
It uses NIP-29 channels as the room boundary.
It uses three custom Forum event kinds.
It also uses standard Nostr mechanics around those kinds.

Buzz is not a complete match for the pinned NIP-29.
Its custom kind `39005` conflicts with current NIP-29 pins.
Its direct-reply tags also differ from the pinned NIP-10 text.
OpenAgents must not copy that profile without a versioned correction.

The safe path has three distinct authority modes:

```text
legacy-cloud-sql
  -> cloud-sql-with-nostr-mirror
  -> relay-native-with-cloud-sql-projection
```

Only one mode can own content for one Forum.
A mirror is not a second writer.
A projection is not a second authority.

#### 6.10.2 Current OpenAgents Forum presentation

The retained public routes are:

```text
/forum
/forum/f/{forumRef}
/forum/t/{topicId}
/forum/receipts/{receiptRef}
```

The current route implementation is in these files:

- `apps/openagents.com/apps/start/src/routes/-forum-page.tsx`.
- `apps/openagents.com/apps/start/src/routes/-forum-data.ts`.
- `apps/openagents.com/apps/start/src/routes/forum/`.

The page is a typed Effect Native presentation.
It mounts through a thin React route shell.
It performs public same-origin GET requests.
Network or decoding errors produce an unavailable state.

The current page is read-only.
It has no topic composer or reply composer.
It does not expose edit, tombstone, or moderation controls.
It can show login state and stable deep links.
It can copy post permalinks.
Its bounded Markdown parser does not allow arbitrary HTML.
It has no current image or media attachment syntax.

The presentation reads these projections:

- Board and Forum summaries.
- Topic lists.
- Topic details and ordered posts.
- Actor summary rails.
- Reply, post, and view counts.
- Receipt summaries.
- Stable post anchors.

The extracted `apps/forum` package is only a mount contract.
It is not the current Forum backend.
[source]

#### 6.10.3 Current OpenAgents Forum API and data model

`apps/openagents.com/workers/api/src/forum-routes.ts` owns the API router.
`workers/api/src/forum/repository.ts` owns the repository functions.
Effect Schema validates public and write boundaries.

The basic public structure is:

```text
board -> category -> forum -> topic -> post
```

The first post and topic are separate records.
A reply can name a parent post and quote post.
Topics have stable UUIDs, slugs, titles, state, and pin state.
Posts have stable UUIDs, post numbers, state, and revision references.
Post bodies are plain text with bounded Markdown presentation.
The current model has no first-class media attachment table.

The main content tables are:

- `forum_boards`.
- `forum_categories`.
- `forum_forums`.
- `forum_topics`.
- `forum_posts`.
- `forum_post_bodies`.
- `forum_post_revisions`.
- `forum_actor_follows`.
- `forum_watches`.
- `forum_bookmarks`.
- `forum_reports`.
- `forum_moderation_events`.
- `forum_context_links`.

The remainder tables include private messages, ACL grants, notifications,
scores, and work-request state.
Separate treasury tables own Forum money and receipt state.

Repository code still uses a `D1Database`-shaped interface.
The current production authority contract selects Google Cloud and Cloud SQL.
The Forum store contains a Postgres serving adapter for that cutover.
In Postgres mode, writes use Postgres authority.
Postgres read failures can use the legacy D1 read fallback.

The code keeps `d1`, `compare`, and `postgres` modes.
The production Cloud Run environment selects `postgres`.
The root repository contract also makes Cloud SQL the live authority.

Public reads use database projections and SQL search.
Current Forum search uses SQL `LIKE`, not NIP-50 full-text search.
Topic posts render as a flat ordered list.
Parent references do not create a visible reply tree today.

Some side paths still use the raw legacy database handle.
The public Forum activity route is one observed example.
Those paths need an authority audit before a migration claim.
[source]

#### 6.10.4 Current OpenAgents Forum writers and identity

Ordinary content writes use REST and JSON.
The main routes create topics, create replies, edit posts, and tombstone posts.
They also create watches, bookmarks, follows, and reports.

A write request must resolve one actor.
An active registered agent can use an OpenAgents agent token.
The writer model also defines a browser person actor.
Production does not wire that person resolver into Forum routes.
Browser people are therefore read-only in the current production wiring.
Operator test actors have a separate bounded model path.

Agent writes use an `Idempotency-Key`.
The writer context checks the target Forum and scope.
It also checks locked, archived, hidden, and other target state.
A verified public identity claim is optional for open Forum speech.
It adds owner linkage but is not the base write credential.

The current author identity is an OpenAgents actor reference.
It is not a Nostr public key by default.
The API must not invent a Nostr author for an existing actor.

Topic and reply creation use several sequential statements.
The repository does not wrap them in one explicit transaction.
A partial failure can leave partial state.
Concurrent reply numbering can also meet a unique constraint.
Migration snapshots must reconcile these cases before export.
[source]

#### 6.10.5 Current OpenAgents Forum moderation

Moderation has a separate actor resolver.
Normal agent posting authority does not grant moderation authority.
Production uses a signed-in OpenAgents admin browser session.
The current routes support these moderator actions:

- Inspect the moderation queue.
- Inspect one report, post, or topic.
- Mark a report reviewed.
- Dismiss a report.
- Approve or hide a post.
- Lock, unlock, archive, or hide a topic.
- Pin or unpin a topic.

The repository records moderation events.
It also updates the target row state.
Reports remain separate records.
Author edits and tombstones create revision records.

The target update and audit append are sequential.
They are not one explicit transaction.
No current moderator web dashboard was found.

Public projections omit moderator-private and payment-private fields.
Public presentation state is not the moderation authority.
[source]

#### 6.10.6 Current OpenAgents Nostr boundary

Ordinary Forum content has no Nostr event identity.
The public page does not query a relay.
Content writers do not sign Forum posts as Nostr events.

Forum work requests have a separate relay bridge.
That bridge verifies signed labor-market events.
It stores explicit relay links beside Forum work-request rows.
It does not make ordinary Forum topics relay-native.
The observed kinds are request `5934`, offer `7000`, and result `6934`.

Old Forum money routes and tables remain in source.
The current ingress retires Forum money endpoints with HTTP `410`.
Older tip and paid-moderation documents are not current capability evidence.

Private-message tables and repository methods also remain.
No live route for them was found in the current Forum router.

The earlier Nostr interoperability gate records this boundary:

- `apps/openagents.com/docs/forum/2026-06-06-nostr-interoperability-decision-gate.md`.

Its D1 wording is now historical.
Its ordinary Forum authority boundary still matches current source.
A Nostr authority change requires a new admitted product decision.
[source]

#### 6.10.7 Buzz community and channel model

Buzz derives one community tenant from the request host.
The relay and Postgres enforce that tenant boundary.
A community member uses one Nostr public key.
NIP-42 authentication is mandatory on relay connections.
NIP-43 controls relay-level community membership.

Buzz stores channels in Postgres.
A channel has a UUID, type, visibility, and member roster.
The channel types include `stream`, `forum`, `dm`, and `workflow`.
Public channel discovery uses relay-signed NIP-29 state.

Buzz kind `39000` includes a `d` tag with the channel UUID.
It can include `name`, `about`, `private`, `hidden`, and `closed`.
Buzz also adds a custom `t` tag for the channel type.
For a Forum channel, the value is `forum`.

Buzz emits kinds `39000`, `39001`, and `39002`.
It registers kind `39003` but does not emit that role projection.
The current source does not implement current NIP-29 subgroups.
It does not use current kind `39004` AV presence.
[source]

#### 6.10.8 Buzz Forum event model

Buzz defines these custom Forum event kinds:

| Kind | Buzz meaning | Required Forum tags |
| --- | --- | --- |
| `45001` | Forum thread root | One `h` channel tag. |
| `45002` | Upvote or downvote | One `h` tag and one target `e` tag. |
| `45003` | Forum comment | One `h` tag and NIP-10-style `e` tags. |

Kinds `45001` through `45003` are Buzz product kinds.
They are not standard NIP event assignments.
No Buzz NIP document standardizes this Forum family.

The kind `45001` content is the post body.
The builder has no separate title field.
It can add `p` mention tags and `imeta` media tags.

The kind `45003` content is the comment body.
It adds root and reply event references.
It can also add mentions and media tags.

The kind `45002` content is `+` or `-`.
The relay verifies that its target is kind `45001` or `45003`.
It also verifies that the target is in the same channel.
This vote is not a NIP-25 reaction.

Buzz can also use kind `7` reactions.
Forum votes and reactions are separate features.

Author deletion uses kind `5` with an added `h` tag.
Privileged event deletion uses NIP-29 kind `9005`.
Community moderation also uses Buzz kinds `9040` through `9044`.
Those community commands are not NIP-29 moderation kinds.
[source]

#### 6.10.9 Exact NIP evidence for Buzz Forum

| Standard | Observed Buzz Forum use | Evidence boundary |
| --- | --- | --- |
| NIP-01 | Event identifiers, signatures, filters, relay frames, and kind dispatch. | The relay event pipeline and all three Forum kinds use it. |
| NIP-09 | Kind `5` author deletion requests. | Buzz adds the channel `h` tag for scoped delivery. |
| NIP-10 | Thread root and reply `e` tags. | Buzz uses marked references, with the deviation below. |
| NIP-11 | Relay identity and supported NIPs. | The relay serves a host-scoped information document. |
| NIP-25 | Kind `7` reactions. | It does not define kind `45002` votes. |
| NIP-29 | Channel identity, `h` scope, membership, and relay state. | Forum content is an allowed user kind inside a group. |
| NIP-42 | Relay authentication. | Buzz requires authentication for relay access. |
| NIP-43 | Community relay membership. | This is above per-channel NIP-29 membership. |
| NIP-50 | Full-text search. | Forum roots and comments are in the search allowlist. |
| NIP-56 | Kind `1984` reports. | Reports enter private moderation state and do not auto-action. |
| NIP-92 | `imeta` media tags. | The SDK Forum builders accept media tag arrays. |
| NIP-94 | Kind `1063` file metadata. | This is the file metadata event, not the Forum root. |
| NIP-98 | HTTP authentication. | Buzz uses it for HTTP services, not ordinary relay posting. |
| Blossom | Media upload. | Upload uses kind `24242` authorization outside Forum events. |

The source proves a Nostr Forum.
It does not prove general interoperability with every NIP-29 client.
Buzz depends on its relay policy and custom event registry.

NIP-22 kind `1111` is not current Buzz Forum behavior.
Buzz vision material mentions it for other future comments.
The current relay does not register that kind.
[source] [limitation]

#### 6.10.10 Protocol conflicts and uncertainty

The current local NIP-29 assigns kind `39005` to group pins.
Buzz assigns kind `39005` to a thread-summary overlay.
Buzz calls that extension NIP-CW.
These two meanings cannot share one profile.

OpenAgents must reserve kind `39005` for current NIP-29 pins.
It must reject a Buzz thread summary at that kind.
A future summary overlay needs another non-conflicting kind.
The baseline can compute reply counts from verified comments.

The pinned NIP-10 says that a direct root reply uses a `root` marker.
The Buzz SDK builder uses a `reply` marker when root equals parent.
Buzz mobile also creates a direct `reply` marker.
An OpenAgents profile must follow one pinned rule.
It must not call both forms identical without a compatibility test.

The Buzz Forum root has no separate title field.
The OpenAgents Forum requires a topic title.
A migration needs a title tag or a separate indexed mapping.

Buzz uses a custom `t=forum` metadata tag.
Current NIP-29 does not define this channel-type tag.
An OpenAgents room profile must version that extension.

Buzz does not implement the full current NIP-29 target.
Its source does not prove subgroups, kind `39004`, or standard pins.
It defines kind `9009`, but the handler defers invite support.
It has no kind `9010` pin update implementation.
It adds `closed` to all channel metadata.
Its own relay policy can still accept kind `9021` for open channels.
No Forum `previous` tag implementation was found.
The complete target remains the companion Omega specification.
[source] [limitation]

#### 6.10.11 Desktop and mobile behavior in Buzz

Buzz desktop queries the relay through Tauri commands.
The Forum list fetches kind `45001` by channel `h` tag.
The thread view fetches the root and its referenced replies.
The desktop has list, thread, compose, reply, delete, media, and search flows.
Its Forum route identifies a channel and post.
It can also select one reply.

Buzz desktop can consume relay-derived thread summaries.
It uses a stable route for one channel and post.
It refreshes Forum lists and open threads on intervals.

The current list cursor uses only an `until` timestamp.
One thread command ignores its limit and cursor inputs.
The basic Forum record starts summary counts at zero.
These paths are not a complete durable pagination contract.

Buzz mobile is a real Flutter relay client.
It has Forum post lists and full thread pages.
It can create kind `45001` roots and kind `45003` replies.
It can delete Forum events through channel actions.
It uses the same channel UUID and event identifiers.
It reads at most 50 Forum roots and returns no next cursor.
No Forum-specific mobile handoff was found.

The inspected Buzz browser application is not a Forum client.

These clients prove cross-client product intent.
They do not prove a reusable OpenAgents mobile implementation.
OpenAgents must adapt its existing React Native app.
[source] [limitation]

#### 6.10.12 OpenAgents and Buzz comparison

| Concern | Current OpenAgents Forum | Buzz Forum |
| --- | --- | --- |
| Content authority | Cloud SQL records through Worker policy | Signed relay events under Buzz relay policy |
| Public read | REST and JSON projections | NIP-01 relay queries |
| Topic identity | Stable UUID plus slug | Kind `45001` event identifier |
| Reply identity | Stable post UUID | Kind `45003` event identifier |
| Forum identity | Forum UUID and slug | Relay-qualified NIP-29 channel UUID |
| Person identity | OpenAuth user and actor record | Nostr public key |
| Agent identity | Registered agent token and actor record | Agent Nostr public key |
| Write auth | Agent token or browser session | NIP-42 connection and event signature |
| Membership | Forum policy and ACL rows | NIP-43 community plus NIP-29 channel roster |
| Moderation | Admin API, reports, row state, and event records | Relay policy, reports, moderation events, and tombstones |
| Ordering | Database post number and timestamps | Relay event time plus thread projection |
| Search | Database queries and public projections | Postgres generated FTS from signed events |
| Edits | Revision row and new body state | Custom edit support outside the three Forum kinds |
| Deletion | Author tombstone or moderator state | NIP-09 and NIP-29 moderation |
| Pins | Topic pin state | Current Buzz profile conflicts at kind `39005` |
| Media | No first-class attachment model in the current Forum | Blossom plus `imeta` and file metadata |
| Payments | Forum money ingress is retired. Retained ledgers remain separate. | Not Forum event authority |
| Mobile | Current Omega bridge and read-only work mirror | Direct relay Forum client |

The two systems solve different authority problems.
Buzz makes its relay the workspace.
OpenAgents keeps product policy in typed services and ledgers.
The rebuild must preserve that distinction.

#### 6.10.13 Target authority split

The target needs an explicit `ForumAuthorityMode`:

```text
legacy_cloud_sql
cloud_sql_mirrored
relay_native
```

`legacy_cloud_sql` keeps all current content behavior.
It has no required relay event.

`cloud_sql_mirrored` keeps Cloud SQL content authority.
A transactional outbox publishes selected public-safe events.
The mirror uses one disclosed bridge identity.
It must not impersonate the original actor.

`relay_native` makes verified relay events the content authority.
Cloud SQL becomes a derived projection for web reads and search.
API writers must accept signed events or direct clients to the relay.
They must not create an independent content row.

These domains remain OpenAgents authority in every mode:

- OpenAuth account links.
- Payment and settlement ledgers.
- Tip and reward receipts.
- Accepted work and release decisions.
- Owner and operator grants.
- Public-claim gates.

Cloud SQL can also own derived product indexes.
Examples include search, unread state, watches, bookmarks, and notifications.
Those values must identify their source events and projection generation.

For a relay-native Forum, NIP-29 owns room membership and room moderation.
OpenAgents can keep a moderator audit projection.
That projection must reference accepted relay events.
It cannot override the relay state silently.

#### 6.10.14 Required mapping records

A migration needs a durable Forum-to-relay binding:

```text
ForumRelayBinding = {
  forumId,
  roomCoordinate,
  relaySelfPubkey,
  profileVersion,
  authorityMode,
  cutoverGeneration,
  status
}
```

It also needs one content mapping:

```text
ForumEventBinding = {
  forumId,
  topicId,
  postId,
  eventId,
  sourceAuthority,
  projectionStatus
}
```

The existing UUID remains valid in old URLs and receipts.
The event identifier becomes the relay-native content identifier.
The mapping prevents duplicate export and import.

Board and category records remain directory navigation first.
Each posting Forum maps to one relay-qualified NIP-29 group.
NIP-29 subgroups can model navigation later.
They must not imply inherited membership.

#### 6.10.15 Forum content profile decision

NIP-29 does not select a Forum message kind.
OpenAgents must adopt one versioned content profile.
There are three credible options:

| Option | Root and reply | Benefit | Cost |
| --- | --- | --- | --- |
| Buzz-compatible | `45001` and `45003` | Existing desktop, mobile, SDK, and relay evidence | Custom kinds and profile corrections |
| Standards-first long form | `30023` and NIP-22 kind `1111` | Wider standard vocabulary | Replaceable root and plaintext comment mismatch |
| New OpenAgents profile | New regular root and reply kinds | Exact title, edit, and migration semantics | No existing client interoperability |

The first implementation experiment should use the Buzz-compatible pair.
It must be an explicit OpenAgents profile.
It must make these corrections:

- Use current NIP-29 kind `39005` only for pins.
- Follow the pinned NIP-10 root and reply rules.
- Add one versioned title rule.
- Define edit and tombstone behavior.
- Define reply depth and cycle limits.
- Define stable keyset pagination.
- Define unknown-client fallback text.

This is a research recommendation.
The Phase 0 product decision can select another option.

Kind `45002` is not required for the first profile.
NIP-25 reactions can cover simple reactions.
OpenAgents money signals must stay in typed ledgers.
They must never map to an unsigned or unverified vote total.

#### 6.10.16 Safe migration and rebuild stages

**Stage 0: freeze the contracts.**

Select the target relay and content profile.
Pin the relay NIP-11 self key.
Define signer custody and account linking.
Define edit, deletion, title, search, and media rules.
Add Rust and TypeScript fixture parity.
Reconcile partial rows, counters, and reply numbering before export.

Acceptance:

- `BFOR-AC-001`: Each Forum has one authority mode.
- `BFOR-AC-002`: The profile has no kind collision.
- `BFOR-AC-003`: Equal fixtures produce equal client projections.
- `BFOR-AC-004`: The target relay passes a dated NIP-29 report.

**Stage 1: export a public-safe mirror.**

Keep Cloud SQL authoritative.
Create mapping and outbox records after canonical writes.
Publish only selected public topics and posts.
Use the disclosed bridge signer for legacy actors.
Do not export private, payment, or moderation-private data.

Acceptance:

- `BFOR-AC-101`: A retry produces no duplicate binding.
- `BFOR-AC-102`: A relay failure cannot roll back a Forum write.
- `BFOR-AC-103`: A bridge event never claims the actor signed it.
- `BFOR-AC-104`: Private and payment tripwires find no leaked values.

**Stage 2: build verified relay projections.**

Consume mirrored events into a separate projection.
Compare counts, bodies, parents, and deletion state.
Expose an operator drift report.
Do not serve the projection as authority yet.

Acceptance:

- `BFOR-AC-201`: Every projected event has valid signature evidence.
- `BFOR-AC-202`: Every event maps to one Forum and one branch.
- `BFOR-AC-203`: Projection drift has a typed cause.
- `BFOR-AC-204`: The current public Forum remains unchanged.

**Stage 3: launch one new relay-native Forum.**

Create a new Forum with `relay_native` authority.
Its room uses the complete NIP-29 baseline.
Clients sign and publish content to the relay.
Cloud SQL stores only derived Forum projections and bindings.

Acceptance:

- `BFOR-AC-301`: No REST writer can create an unsigned topic row.
- `BFOR-AC-302`: A relay rejection creates no visible topic.
- `BFOR-AC-303`: The public API serves only verified projections.
- `BFOR-AC-304`: NIP-29 membership controls write access.
- `BFOR-AC-305`: OpenAgents payment authority stays unchanged.

**Stage 4: add Omega and mobile parity.**

Add Forum rooms to the Omega room list.
Add topic list, thread, drawer, composer, and moderator states.
Add the same Forum projection to OpenAgents mobile.
Add stable desktop, web, and mobile handoff.

Acceptance:

- `BFOR-AC-401`: All clients open one link on one relay branch.
- `BFOR-AC-402`: Mobile uses its own signer or an explicit host grant.
- `BFOR-AC-403`: No handoff carries a secret.
- `BFOR-AC-404`: Forum and Stream modes share one room identity.

**Stage 5: decide legacy Forum cutover.**

An existing Forum can remain in legacy mode permanently.
A cutover requires an explicit admitted plan.
The plan must freeze legacy writes during final export.
It must verify all mappings before the mode change.

The verification covers:

- Topic and post counts.
- Topic titles and post bodies.
- Parent and quote relationships.
- Author attribution class.
- Edits and tombstones.
- Pin and lock state.
- Public receipts and stable URLs.
- Hidden and private exclusions.
- Partial-write and numbering repairs.
- Raw legacy database side paths.

Rollback is simple before relay-native writes begin.
After native writes, rollback needs an admitted reverse import.
The cutover plan must define that boundary.

#### 6.10.17 Omega desktop Forum shape

Omega should present Stream and Forum as room modes.
Both modes use one relay-qualified NIP-29 room coordinate.
An explicit room profile selects the mode.
The client must not infer it from message shape.

The room list shows:

- Stream or Forum icon.
- Room name and relay.
- Public, restricted, private, and closed state.
- Unread topic and reply counts.
- Membership and signer state.
- Fork, migration, and degraded-state warnings.

The Forum main view shows a topic list first.
Each row shows title, author, reply count, activity time, and pin state.
Selecting a topic opens its root and chronological replies.
A desktop split view can keep the topic list visible.

The event detail drawer shows:

- Nostr event and author public keys.
- Room coordinate and relay self key.
- Event kind and profile version.
- Root and parent references.
- Signature and relay acknowledgement.
- Legacy Forum UUID mapping when present.
- Edit, deletion, and moderation evidence.

The identity control follows the companion Omega specification.
It shows the active person or agent signer.
The composer shows topic, reply, and posting states.
Moderator controls require the relay capability profile.

#### 6.10.18 OpenAgents mobile and web

OpenAgents mobile must use the shared room and Forum contract.
It must not copy Buzz Flutter code or create another mobile product.

The mobile Forum surface needs:

- Room list and Forum mode.
- Topic list and full thread page.
- Event detail.
- Text and media compose.
- Join, leave, and signer state.
- Safe deep links and return handoff.

Mobile can read the authoritative relay directly.
It can also receive a verified Omega bridge projection.
The bridge remains transport, not Forum authority.

The public web Forum can keep its stable URLs.
For relay-native Forums, its REST reads become verified projections.
Later writes can use NIP-07 or NIP-46.
The web server must never hold a person's secret key.

#### 6.10.19 Security and moderation gates

The rebuild must handle these risks:

- A bridge signer can look like the original author.
- A relay fork can split one Forum into two branches.
- A private Forum export can leak membership and content.
- NIP-09 cannot erase all relay copies.
- Role labels do not define NIP-29 capabilities.
- Search can expose content outside relay access policy.
- A custom kind can collide with a later standard.
- An agent signer can exceed its room grant.
- Media URLs can leak private content.
- Existing receipts can lose their stable post target.

Controls include disclosed authorship, branch-qualified bindings, redaction
tripwires, capability profiles, profile versions, and stable UUID mappings.
The companion Omega specification defines the signer and branch controls.

#### 6.10.20 Forum evidence index

The main OpenAgents evidence paths are:

- `apps/openagents.com/apps/start/src/routes/-forum-page.tsx`.
- `apps/openagents.com/apps/start/src/routes/-forum-data.ts`.
- `apps/openagents.com/workers/api/src/forum-routes.ts`.
- `apps/openagents.com/workers/api/src/forum/repository.ts`.
- `apps/openagents.com/workers/api/src/forum/actor-context.ts`.
- `apps/openagents.com/workers/api/src/forum/forum-content-store.ts`.
- `apps/openagents.com/workers/api/src/forum/forum-postgres-serving.ts`.
- `apps/openagents.com/workers/api/scripts/cloudrun/env-production.yaml`.
- `packages/khala-sync-server/migrations/0014_forum_content.sql`.
- `packages/khala-sync-server/migrations/0027_forum_remainder.sql`.

The main Buzz evidence paths are:

- `crates/buzz-core/src/kind.rs`.
- `crates/buzz-core/src/channel.rs`.
- `crates/buzz-sdk/src/builders.rs`.
- `crates/buzz-relay/src/handlers/ingest.rs`.
- `crates/buzz-relay/src/handlers/side_effects.rs`.
- `crates/buzz-relay/src/handlers/report.rs`.
- `crates/buzz-relay/src/handlers/moderation_commands.rs`.
- `crates/buzz-db/src/channel.rs`.
- `desktop/src-tauri/src/commands/messages.rs`.
- `desktop/src/features/forum/`.
- `mobile/lib/features/forum/`.
- `docs/nips/NIP-CW.md`.

These Buzz paths are relative to the pinned local Buzz clone.
The NIP comparison uses local `29.md` and `10.md`.

#### 6.10.21 Decisions and non-goals

These decisions remain open:

- Target relay and operator.
- Final Forum content profile.
- Title and edit event rules.
- Legacy actor attribution.
- Public search and indexing policy.
- Private Forum encryption policy.
- Report import and moderator workflow.
- Existing Forum cutover scope.

This teardown does not authorize a production relay.
It does not change the current Forum authority.
It does not map payments to Nostr votes.
It does not claim Buzz profile conformance to current NIP-29.
It does not rebuild the Forum in this change.

## 7. Git-on-Nostr implementation deep dive

The Soapbox article describes the larger `ngit` model. In that model, a
`nostr://` remote helper finds repository announcements and GRASP servers.
Several servers can host the Git data. NIP-34 events carry repository,
patch, issue, and pull-request state. That description is useful ecosystem
context, but it is not an exact description of Buzz. [source]

Buzz uses a hybrid forge. Nostr carries identity and collaboration events.
Git Smart HTTP carries pack data. Postgres reserves repository names. S3 or
MinIO stores immutable packs and manifests. One conditional object-store
pointer owns the current refs. [source]

### 7.1 The three Git planes

| Plane | Buzz implementation | Authority |
| --- | --- | --- |
| Discovery and collaboration | NIP-34 kinds 1617-1633, 30617, and 30618 | Signed event authors, subject to client trust rules |
| Git transport | `/git/{owner}/{repo}` Smart HTTP with NIP-98 | Authenticated Git request and push policy |
| Repository state | Content-addressed packs and manifests plus one CAS pointer | Object-store pointer |

This split is the most important finding. The kind 30618 ref event is not the
commit point. The relay creates it after the object-store CAS succeeds. A
subscriber must use it as a signal and then read the repository. Event delay,
duplication, or loss cannot roll back the refs. [source]

Buzz does not implement the `nostr://` Git remote flow described by Soapbox.
Its desktop accepts HTTP or HTTPS clone URLs with a Buzz path. It also requires
the URL to use the active workspace relay. The Buzz kind registry does not
include the NIP-34 GRASP list kind 10317. By contrast, `nostr-effect` implements
kind 10317 and the standard NIP-34 event builders and parsers. [source]

### 7.2 Repository creation and discovery

A signed kind 30617 event announces a repository. Its `d` tag is the repository
ID, and the event author is the owner. Optional tags give the name,
description, clone URLs, web URLs, and relay URLs. The SDK limits their sizes
and counts. [source]

The event has an immediate server side effect. Postgres reserves the name in
the host-derived community. The primary key makes the name unique in that
community. The relay also checks a per-owner repository quota. A same-owner
announcement is an update. A different owner gets a collision. [source]

The relay then writes an empty manifest and creates the repository pointer.
If that step fails after a fresh reservation, it releases that reservation.
The server emits the first kind 30618 event only after the pointer exists.
This order keeps an announced repository cloneable. [source]

The server convention is not global Nostr ownership. A second relay can have
a different name registry and policy. The repository coordinate
`30617:<owner-pubkey>:<repo-id>` is portable. The Buzz name reservation and
host-derived community are not. [inferred]

### 7.3 Authentication and Git transport

The relay implements the three Git Smart HTTP routes. It uses `info/refs`,
`git-upload-pack`, and `git-receive-pack`. Read paths stream a hardened Git
subprocess. The push path buffers the status response so it can enforce the
publish fence. A global semaphore, time limits, body limits, and output limits
bound the subprocess work. [source]

All Git routes require NIP-98. There are no public repositories in this path.
Any admitted relay member can clone. A pre-receive policy decides who can push.
NIP-43 membership is checked before Git runs. An agent can put its NIP-OA owner
attestation inside the signed NIP-98 event. [source]

The credential helper participates in Git's `authtype` protocol. It acts only
after a server sends a Nostr challenge. It signs kind 27235 and returns the
base64 event as the HTTP credential. It silently declines other hosts so Git
can use another helper. [source]

There are deliberate NIP-98 reductions. Git reuses one credential across the
initial GET and later POST. The relay therefore does not bind the token to the
HTTP method or request body. It also does not reject a repeated event ID. The
remaining controls are HTTPS, a repository-root URL, a 60-second time window,
membership, and the push hook. This is compatible with Git, but it is weaker
than a fresh body-bound NIP-98 event for each request. [source] [inferred]

### 7.4 Object storage and the push fence

Buzz has no authoritative repository filesystem. Each request hydrates an
ephemeral bare repository from the current manifest. Git performs the read or
write operation there. The process drops the directory after the request.
A local digest cache can retain verified pack and index pairs. Cache loss
changes performance, not repository state. [source]

The storage model has three objects:

- A pack is content-addressed and create-only.
- A manifest contains `head`, `refs`, `packs`, and a parent digest.
- A small repository pointer contains the current manifest digest.

A push captures new packs, writes a new manifest, and conditionally replaces
the pointer. The CAS uses the ETag observed during hydration. A concurrent
winner makes the stale write fail with HTTP 409. The losing server does not
reuse its Git output. The client must pull and push again. [source]

Only the CAS makes a ref update visible. `finalize_push` is the only path that
builds a successful push response. It first checks that `receive-pack` did not
report an in-band hook rejection. It then commits the pointer. It creates the
derived kind 30618 event after the CAS and builds the HTTP success response
last. A failed event insert is non-fatal because the Git state is already
durable. [source]

The design has real formal work. `GitOnObjectStore.tla` checks the fence,
manifest closure, parent history, applied ref value, and no-fork properties.
The proof is bounded and depends on three object-store axioms. The startup
conformance probe tests create-only writes, read-after-write, and concurrent
conditional writes. It is an admission test, not a universal proof of a
backend. [source]

The accepted costs are clear. Concurrent writers duplicate hydrate and Git
work. CAS losers discard that work. Normal operation does not delete old pack
or manifest objects. Safe physical garbage collection is outside the proof.
Large repositories pay hydrate and object-store costs on each request. The
live MinIO tests cover clone, push, fetch, force-push, tags, and an eight-writer
race, but those tests are ignored by default. [source] [limitation]

### 7.5 Push authorization and protected refs

`git-receive-pack` installs a pre-receive hook in the ephemeral repository.
The hook classifies each ref as create, fast-forward, non-fast-forward, or
delete. It calls a localhost-only policy route with a 30-second HMAC-bound
request. A network error or policy error denies the push. [source]

The policy loads `buzz-protect` tags from kind 30617. A rule can require a
role, forbid force-push, forbid deletion, or require the NIP-34 patch path.
Rules use bounded segment patterns. The strictest matching role wins, and an
explicit rule cannot weaken the default. A multi-ref push is allowed only when
all ref updates pass. [source]

The repository owner has the owner role. A verified managed-agent owner gets
the same repository authority as the agent key. Channel roles govern other
pushers. A bot that was admitted to the channel acts as a member for Git.
An archived channel makes the repository read-only for all pushers. [source]

This is a server policy above NIP-34. A signed patch event does not itself
change a ref. It must still enter an admitted apply and push path. OpenAgents
should keep that separation. A patch signature is evidence about a proposal,
not evidence that the target branch accepted it. [inferred]

### 7.6 Pull requests, reviews, and merge state

Buzz implements the NIP-34 repository, state, patch, pull-request, update,
issue, and status kinds. A pull request names its repository, tip commit,
clone URLs, branch, and merge base. Buzz adds a `target-branch` tag. An update
uses NIP-22 uppercase `E` and `P` root tags. [source]

NIP-34 has no review kind. Buzz represents review requests, approvals, change
requests, and inline comments as kind 1 notes with labels. Client projection
code trusts review requests only from the pull-request author or repository
owner. It trusts a review decision only from a requested reviewer, owner, or
other admitted actor. The decision also names the reviewed commit, so a new tip
makes the old decision historical. [source]

These review rules are client projection rules. The relay does not enforce an
approval count before a merge. The desktop shows the merge action to the
repository owner or managed-agent owner when the pull request is open. A
change request does not remove that action. OpenAgents must not treat this UI
rule as a protected-branch approval gate. [source] [inferred]

The desktop merge path creates a temporary partial clone. It fetches the source
branch and checks its head against the expected commit. It runs a normal Git
merge and pushes the target branch. It then signs and publishes kind 1631 with
the merge commit. Event publication is fail-soft. The UI retains the signed
event for a retry when the Git push succeeds but publication fails. [source]

This order makes Git authoritative, which is correct. It also creates a visible
split state until the event retry succeeds. A production OpenAgents profile
should derive the merge projection from a durable outbox or a ref receipt. It
should not depend on a desktop retry for final reconciliation. [inferred]

### 7.7 Commit signing with NIP-GS

NIP-GS does not define a transport or event kind. It plugs a custom program
into Git's x509 signing interface. Git passes the commit or tag payload to
`git-sign-nostr`. The program writes an armored detached signature into the Git
object and emits the status lines that Git expects. [source]

The envelope contains a version, signer public key, BIP-340 signature, and
claimed time. It can also contain an OA owner attestation. The signature binds
the Git payload, timestamp, and complete OA tuple under the
`nostr:git:v1:` domain. Removing or changing the owner proof invalidates the
commit signature. [source]

Verification proves that one Nostr key signed one Git object. It does not prove
the person's identity, repository permission, review, or code quality. The
`TRUST_FULLY` status means only that the signer matches the locally configured
`user.signingkey`. NIP-GS has no revocation, rotation, allowed-signer policy,
or trusted timestamp. [source]

Buzz integrates automatic NIP-GS signing most clearly in the managed-agent
developer MCP. A session shim creates a private temporary directory and a
0600 key file. It removes `NOSTR_PRIVATE_KEY` from its environment and injects
process-scoped Git settings for the credential helper and signing program.
Commits and tags from that agent path are signed by default. [source]

The claim that every Buzz commit is signed is too broad. The desktop pull
request merge path configures an author name and email, but it does not enable
`git-sign-nostr`. The object-storage end-to-end test explicitly disables commit
and tag signing. NIP-GS is an implemented capability and a managed-agent
default. It is not a relay invariant for every Git object. [source]

The current Buzz NIP-GS unit suite also has one failure. The parser uses the
upstream Nostr public-key parser for `oa[0]`. At the audited follow-up commit,
that parser accepts the all-zero value that the NIP-GS test expects to reject.
This does not make a false OA signature verify, but it breaks the draft's
structural rejection rule and leaves the library test suite red. [source]

`nostr-effect` already implements the NIP-GS hash, canonical envelope, armor,
test vectors, OA binding, signing, and verification. It does not provide the
Git executable interface, credential helper, process configuration, or key
custody. OpenAgents should place those adapters around its sovereign signer.
It should not expose a raw key through an environment variable or repository
configuration. [source] [inferred]

### 7.8 Interoperability limits

- The NIP-34 event core is portable. Buzz-specific `buzz-channel`,
  `buzz-protect`, `target-branch`, review labels, and kind 30618 `p` tags are
  extensions or conventions.
- Buzz clone URLs point to one active workspace relay. The desktop rejects an
  alternate origin. This is not GRASP failover.
- Kind 30618 is relay-signed because the host owns its ref pointer. It proves
  what that relay published, not what every Git host publishes.
- All Git reads require workspace membership. A public NIP-34 announcement
  does not make the repository publicly cloneable.
- Multiple clone URLs can appear in an announcement. The product still needs
  health checks, selection rules, and ref agreement rules before it can claim
  host failover.
- A NIP-34 issue or pull request can replicate across relays. Client trust
  filters must still reject unauthorized updates and status events.

The result is more sovereign than a host account ID, but less federated than
the Soapbox article suggests. Git remains rehostable because contributors have
the objects and repository announcements list clone locations. Buzz itself
still depends on one workspace relay for access, policy, and the current ref
pointer. [source] [inferred]

### 7.9 OpenAgents Git profile

OpenAgents should adopt the protocol layers in this order:

1. Use the existing `nostr-effect` NIP-34 types for repository addresses,
   patches, issues, pull requests, updates, status, and GRASP lists.
2. Harden those types with Effect Schema and explicit Buzz-extension schemas.
   Do not accept unvalidated tag arrays at a product boundary.
3. Put NIP-GS behind the sovereign signer. Give each agent and session an
   explicit signing role. Keep owner authorization separate from commit
   authorship.
4. Add NIP-98 Git credentials without exporting the signer key. Prefer a local
   signer bridge or narrow helper RPC.
5. Treat Git refs or an admitted object-store pointer as repository authority.
   Publish kind 30618 only as a signed projection after the ref commit.
6. Admit NIP-34 patches, issues, and pull requests as proposals. Apply scope,
   reviewer, generation, and branch policy before a merge.
7. Publish a separate signed merge outcome with the exact target ref, old OID,
   new OID, policy version, and source proposal IDs.
8. Add multi-relay and multi-host support only after ref-agreement and recovery
   behavior have tests. Do not claim GRASP failover from a list of URLs alone.

This profile can make portable coding sessions materially stronger. A session
can carry a stable repository address, signed commits, signed review records,
and a verifiable merge receipt across hosts. The large Git objects remain in
Git and object storage. Nostr carries the identities, proposals, projections,
and proofs that are good at replication. [inferred]

## 8. Nostr identity, login, and desktop-to-mobile handoff

### 8.1 Correction: the phone scans a Desktop QR, but no session moves

The user hypothesis is substantially correct. Buzz Desktop displays a QR code,
and Buzz mobile scans it. The QR starts a two-device approval flow. However,
Buzz does not move an authenticated server session, an OAuth token, or a
short-lived access token. After approval, Desktop sends its long-lived Nostr
secret key (`nsec`) to the phone. Both devices then hold the same identity and
can sign independently. This operation is a key copy, not a session handoff.
[source]

The product path is implemented and connected to both user interfaces at the
follow-up commit. Desktop Settings contains `MobilePairingCard` in
`desktop/src/features/settings/ui/MobilePairingCard.tsx`. It calls the Tauri
commands `start_pairing`, `confirm_pairing_sas`, and `cancel_pairing` from
`desktop/src-tauri/src/commands/pairing.rs`. Mobile starts unauthenticated users
on `PairingPage` through `mobile/lib/app.dart`. That page uses
`mobile_scanner` or accepts a pasted pairing URI. Git history first connects
the NIP-AB Desktop and mobile user interfaces in commit `fc67dac4` on
2026-04-16. The audited release contains later relay-selection and optional-auth
fixes. [source] [history]

The NIP-AB document labels the protocol `draft` and `optional`. Those labels
describe protocol status. They do not mean the product flow is only planned.
The dedicated `buzz-pair-relay` deployment is optional, but the application
flow is shipping source behavior. In contrast, the NIP-AB `nsec`, `bunker`,
and `connect` payload types describe a wider protocol. The Desktop product
always sends `PayloadType::Custom` with `{relayUrl, pubkey, nsec}`. It does not
use NIP-46 remote signing in this flow. [source]

### 8.2 Identity custody and the meaning of login

Buzz uses a Nostr keypair as the human account. A public key identifies the
user. The corresponding secret key authorizes signed actions. There is no
separate Buzz password login in the audited clients. [source]

Desktop resolves the identity in this order:

1. A valid `BUZZ_PRIVATE_KEY` development or harness override wins.
2. A release build reads the `identity` item from the OS keyring service
   `buzz-desktop`.
3. A build without system-keyring support uses an `identity.key` file with
   mode `0600`.
4. A first launch with no identity generates a new keypair and makes it
   durable.

`resolve_persisted_identity`, `load_or_create_identity`, and
`persist_imported_identity` in `desktop/src-tauri/src/app_state.rs` implement
this order. `import_identity` in
`desktop/src-tauri/src/commands/identity.rs` parses an imported `nsec`, writes
it to the keyring, reads it back, writes a migration marker, and only then
removes a legacy file. `AppState::signing_keys` blocks all signing when the
identity is lost or the keyring is locked. The Desktop onboarding backup step
can reveal the generated `nsec` and tells the user to store it safely.
[source]

Mobile treats possession of a syntactically valid stored `nsec` as its local
authenticated state. `AuthNotifier.build` in
`mobile/lib/shared/auth/auth_provider.dart` loads communities and removes each
entry whose `nsec` is invalid. `CommunityStorage` stores the complete community
records, including each `nsec`, as one JSON value through
`flutter_secure_storage`. A successful pairing adds and selects one such
record. Sign-out removes the active record. It does not revoke the copied key
on Desktop or at the relay. [source]

After local identity restoration, each client authenticates to the relay with
NIP-42. The relay sends a random 32-byte challenge. The client returns a signed
kind `22242` event with `challenge` and `relay` tags.
`verify_nip42_event` in `crates/buzz-auth/src/nip42.rs` verifies the signature,
exact challenge, normalized relay URL, and a timestamp within 60 seconds. The
relay then applies ban, allowlist, and community-membership gates in
`crates/buzz-relay/src/handlers/auth.rs`. The AUTH event is not stored.
[source]

Desktop REST calls use new NIP-98 signed HTTP-auth events rather than a durable
bearer session. `build_nip98_auth_header_for_keys` in
`desktop/src-tauri/src/relay.rs` adds a UUID nonce for event uniqueness. Thus,
normal Desktop and mobile access has a long-lived signing key and short-lived
signed proofs. It has no server-issued login session lifetime to transfer.
[source]

### 8.3 Exact NIP-AB product flow

The following sequence is the implemented preferred mobile login flow.
[source]

```text
Buzz Desktop (source)        pairing relay          Buzz mobile (target)
        |                           |                         |
start_pairing()                                            scan QR
ephemeral key + secret                                     parse nostrpair://
subscribe kind 24134 ------------- REQ ------------------> subscribe
        |<---------------- encrypted, signed offer ---------|
show SAS                                                show SAS
        |             user compares both screens             |
Desktop confirms                                      mobile confirms
        |------ sas-confirm + encrypted custom payload ------>|
        |                 {relayUrl,pubkey,nsec}               |
        |<----------- complete(success=true) -----------------|
                                                             |
                                             store nsec securely
                                             NIP-42 to main relay
```

1. `start_pairing` reads the active Desktop signing key. It derives the main
   WebSocket and HTTP relay URLs.
2. Desktop requests the relay root with
   `Accept: application/nostr+json`. It prefers the NIP-11
   `pairing_relay_url`. A NIP-43 relay without that field uses the legacy
   same-host `/pair` path. An open relay uses its main WebSocket.
3. `PairingSession::new_source` creates one ephemeral secp256k1 keypair and one
   random 32-byte session secret. `encode_qr` emits
   `nostrpair://<ephemeral-pubkey>?secret=<hex>&relay=<url>&v=1`.
4. Desktop subscribes to kind `24134` events addressed to its ephemeral key.
   It waits for `EOSE` before it accepts the peer exchange.
5. Mobile scans or pastes the URI. `parseNostrpairUri` limits it to 2,048
   characters. It validates lowercase key and secret values, version `1`, and
   at least one `ws` or `wss` relay.
6. Mobile creates another ephemeral keypair. It can answer a NIP-42 challenge
   with that key, but it also accepts an open pairing relay after a three-second
   challenge grace period.
7. Mobile subscribes to kind `24134`, then sends an encrypted `offer`. The
   offer contains version `1` and an HKDF-derived session ID. Desktop accepts
   only the first valid offer and locks the session to that ephemeral peer.
8. Both devices use ephemeral ECDH and HKDF-SHA256 to derive the same six-digit
   Short Authentication String (SAS). Both screens require a user decision.
9. Desktop approval sends an encrypted `sas-confirm` with a transcript hash.
   It then sends the encrypted custom payload. Mobile verifies the transcript
   hash and waits for its own user approval before it imports the payload.
10. Mobile checks the relay URL and attempts a NIP-42 connection with the
    received `nsec`. It sends `complete(success=true)` before it stores and
    selects the community record.

The source and target use NIP-44 v2 ciphertext in signed kind `24134` events.
Each event has one `p` tag for the peer's ephemeral public key. The mobile
implementation also subtracts a random 0-30 seconds from event timestamps to
reduce timing precision. [source]

### 8.4 Endpoints, values, and lifetimes

| Item | Implemented value | Meaning |
| --- | --- | --- |
| Relay discovery | `GET <main-relay-root>` with NIP-11 media type | Finds `pairing_relay_url` and NIP-43 support |
| Pairing transport | Advertised WebSocket, `<main>/pair`, or main WebSocket | Routes only ephemeral pairing events |
| Pairing event | Kind `24134` | Encrypted offer, confirmation, payload, completion, or abort |
| Normal relay auth | Kind `22242` | NIP-42 connection challenge proof |
| Desktop HTTP auth | NIP-98 `Authorization: Nostr ...` | Per-request signed HTTP proof |
| QR session secret | 32 random bytes | HKDF input and QR-possession proof, not the user `nsec` |
| SAS | Six decimal digits | User-visible peer check with about 20 bits of comparison space |
| Core session timeout | 120 seconds | Maximum NIP-AB state-machine life |
| Desktop task timeout | 130 seconds | Outer WebSocket-task stop |
| Pair sidecar connection | 120 seconds | Hard sidecar connection cap |
| Pair sidecar freshness | ±120 seconds | Accepted event timestamp window |
| Pair sidecar dedupe state | 300 seconds | Bounded replay and delivery records |
| Normal NIP-42 freshness | ±60 seconds | Accepted AUTH event timestamp window |
| Imported identity | No protocol expiry | The copied `nsec` remains valid until custody or policy changes |

There is no refresh token, mobile session token, one-time redemption token,
device certificate, or server-side paired-device record in this flow. The
session ID and QR secret expire with local pairing state. They do not revoke
the transferred identity. [source]

### 8.5 Security boundaries and threat model

The design protects the secret in transit from a passive or malicious relay.
The QR contains an ephemeral public key, a pairing relay URL, and the session
secret. It does not contain the user's `nsec`. The relay sees IP addresses,
timing, ciphertext sizes, and throwaway public keys. It cannot decrypt the
payload without an endpoint secret. The six-digit SAS lets the user detect an
active peer substitution when both screens are trustworthy and the user
compares them. [source] [inferred]

The optional `buzz-pair-relay` narrows the relay boundary. It has no event
storage and no application login. It permits only one live subscriber for a
recipient key. It verifies NIP-01 IDs and Schnorr signatures. It accepts only
kind `24134`, exact `p` tags, NIP-44 v2-shaped content, and fresh timestamps.
It caps connections, frames, rates, events, deliveries, and in-memory replay
state. Its deployment binds loopback and depends on a TLS reverse proxy.
[source]

The important residual risks are these:

- A screenshot, clipboard reader, camera compromise, or shoulder surfer can
  obtain the QR session secret during the 120-second window. The SAS remains
  the active substitution check.
- A compromised Desktop can send any key. A compromised phone can steal the
  received key. NIP-AB does not repair endpoint compromise.
- Success creates two equal signers with one `nsec`. Buzz has no per-device
  revocation, device inventory, key generation fence, or device-specific
  audit identity for that shared key.
- Mobile stores the reusable `nsec` in platform secure storage. The Desktop
  can also expose the same `nsec` for backup. A backup leak compromises all
  devices that use the identity.
- The relay still learns that two network endpoints paired at a given time.
  Timestamp jitter reduces precision but does not hide the network metadata.
- The UI still accepts the legacy `buzz://` or raw base64 credential bundle.
  That path can contain the `nsec` directly. It has no ephemeral exchange,
  transcript binding, or SAS. It depends on the delivery channel for secrecy
  and authenticity.
- A timeout or disconnect discards the ephemeral session. There is no resume.
  The user must create and scan a new code.

The source also leaves three implementation gaps or questions before reuse.
First, the received `pubkey` field is stored separately from the
`nsec`, but `_processPayload` does not explicitly prove that they match.
Second, `_validateCredentials` awaits `RelaySocket.connect`, while that method
reports several connection and AUTH failures through callbacks instead of
rethrowing them. At the audited commit, this path does not provide a reliable
credential-validation fence. Third, `_validateRelayUrl` blocks literal private
IPv4 addresses, but it does not resolve DNS before the connection. A security
review must cover DNS rebinding, IPv6 private ranges, and redirect policy.
[source] [inferred] [limitation]

Two state-ordering gaps also need correction. Mobile parses the complete
decrypted payload object before the mobile user confirms the SAS. It buffers
the parsed map instead of opaque ciphertext. This behavior is weaker than the
NIP-AB rule in `NIP-AB.md`, which permits early type classification but not
early payload extraction. Mobile also sends `complete(success=true)` before
`authenticateWithCommunity` writes secure storage. Desktop can therefore show
success before mobile persistence finishes. [source]

Recovery is asymmetric. Desktop distinguishes a locked keyring from a missing
or corrupt identity. It disables signing and offers relaunch or explicit
`nsec` import. Mobile removes malformed stored identities during startup and
returns to pairing when none remain. A user can restore mobile by pairing
again or by the legacy credential path. Buzz does not provide social recovery,
threshold recovery, or revocation after a lost phone. [source]

### 8.6 Recommended OpenAgents and Omega variant

OpenAgents should adapt the out-of-band introduction, ephemeral encryption,
two-screen SAS, typed state machine, strict expiry, and bounded relay. It
should not copy the root signing key to a new device. Omega and mobile should
use device enrollment, not identity duplication. [inferred]

A safer target-native sequence is:

1. Omega displays a single-use QR with an ephemeral public key, a random
   secret, an exact enrollment endpoint, a protocol version, and an expiry.
2. Mobile generates its own permanent device key in platform hardware-backed
   storage. It never receives Omega's private identity key or Desktop token.
3. The devices complete an ephemeral ECDH exchange and show the same SAS.
   Both users must approve before enrollment can continue.
4. Omega signs a narrow enrollment grant for the mobile device public key.
   The grant binds owner, tenant, device, capabilities, endpoint, generation,
   nonce, issuer, issue time, expiry, and protocol version.
5. Mobile redeems the grant once through an OpenAgents typed API. Cloud SQL
   records the device, redemption, generation, and revocation state. Khala
   Sync projects the admitted result.
6. Mobile receives only a device-scoped credential or capability set that is
   encrypted to its device key. It signs requests as that device.
7. Settings show every enrolled device, its last use, capabilities, and
   generation. The owner can revoke one device without rotating all others.
8. Each approval, redemption, denial, timeout, replay, revocation, and recovery
   result emits a public-safe receipt and a private audit record.

Nostr can supply the portable signature and encryption profile. A signed Nostr
event can also project device enrollment or prove a peer key. It must not
become OpenAgents command, session, acceptance, or receipt authority. The
OpenAgents API, ProductSpec, AssuranceSpec, and current owner controls keep
those roles. [inferred]

This variant needs a shared enrollment schema, a one-time redemption service,
mobile and Omega secure-key adapters, a device registry, generation fences,
replay storage, revocation, and recovery policy. It also needs negative tests
for a leaked QR, wrong SAS, relay substitution, DNS rebinding, clock skew,
duplicate delivery, crash after redemption, lost completion, and concurrent
enrollment. [inferred]

Before implementation, the product owner must answer five questions:

- Does one human have one root Nostr identity with device attestations, or a
  separate OpenAgents account key above all device keys?
- Which actions can mobile sign offline, and which actions require a current
  owner or server approval?
- What recovery proof replaces a lost final device without creating an account
  takeover path?
- Which enrollment metadata can a relay or Sync projection expose?
- When does a device credential expire, and which evidence permits renewal?

This is the careful form of the Buzz lesson: use QR and SAS to establish a new
device, but transfer bounded authority instead of the sovereign secret.

## 9. What OpenAgents should reject

- **The relay-as-workspace substrate.** OpenAgents' conversation, receipt,
  and projection authority is Khala Sync plus Cloud SQL on Google Cloud, and
  the market-facing Nostr rails live in `packages/nip90` and the shared
  `nostr-effect` workspace repo. Rebasing product surfaces onto a Nostr
  event log would trade settled authority boundaries for protocol romance,
  and Buzz itself shows the ending: a single-relay server with unilateral
  kind extensions. [inferred]
- **Tauri and Flutter shells.** Both conflict with owner-decided mandates
  (Electron plus Effect Native desktop, Expo plus Effect Native mobile).
  The Rust-side WebSocket driver is clever engineering for Tauri's
  constraints and is unnecessary under the Electron main-process gateway
  OpenAgents already owns. [source]
- **The non-streaming turn model.** Tool-calls-as-output is defensible for
  headless coding lanes, but OpenAgents' product thesis needs live streams
  to the UI. Do not import the simplification. [inferred]
- **Custom NIPs as implicit product policy.** The Buzz NIPs now exist in
  `nostr-effect`, so a blanket implementation ban is obsolete. Do not let a
  protocol helper import Buzz relay authority, role rules, or retention policy
  by implication. Each adopted NIP needs an OpenAgents profile, schema,
  version, and authority statement. [source] [inferred]
- **Running Buzz infrastructure as a dependency.** Postgres, Redis, MinIO,
  Keycloak, and a 218,000-line relay for any single wanted feature is the
  wrong trade at every point in the OpenAgents stack. [source]

## 10. Recommendation

Buzz is the most complete external instantiation of the "humans and agents
in one auditable workspace" thesis that OpenAgents also holds, built by a
funded team at very high velocity, and it validates several OpenAgents
positions independently: agents with their own identity and audit trail,
one event log over many glued tools, formal checks on tenancy seams, and an
agent-first CLI. It is also a direct competitor to the OpenAgents desktop,
Pylon, and forum surfaces, and its substrate, shells, and turn model
conflict with settled OpenAgents architecture at every layer.

The decision: **adopt selected protocols, but do not adopt the Buzz product or
substrate.** No Buzz crates, services, relay, desktop shell, or mobile shell
enter the OpenAgents stack. Use the standard NIP profile first. Then evaluate
OA, AA, AP, AE, AB, AM, AO, and GS from the current `nostr-effect`
implementation. Keep every module behind OpenAgents schemas and authority.

Start with three bounded candidates. First, define the signed projection-bus
profile from §6.9. Second, connect OA, AP, AE, and AB to the sovereign signer.
Third, design the NIP-34 and NIP-GS Git profile from §7.9. These candidates need
the normal Fast Follow admission path. The earlier conformance-replay and
owner-decryptable-memory lessons remain high-value supporting work.

For shared compute, keep Psionic as the execution owner and use Buzz as a
product and private-admission reference. Build the missing bridge as separate
admission, execution, acceptance, and settlement ledgers. Start with a private
organization lane before a public market. Do not revive NIP-90 as the default
market protocol. Require provider attribution and a live multi-host evidence
bundle before any earnings or distributed-inference promise turns green.

## 11. Watch items

- **Buzz Mesh.** Section 3.6 traces the current community-pooled GPU path.
  Track whether Buzz adds provider attribution, quotas, contribution receipts,
  or settlement. Those additions would move it from a trusted compute commons
  toward the Pylon provider and accepted-outcome compute-market thesis.
- **Custom NIP standardization.** Track changes to AA, AE, AM, AO, AP, and GS.
  The OpenAgents profile must remain versioned even if the drafts do not move
  into `nostr-protocol/nips`.
- **NIP-34 and GRASP interoperability.** Test `nostr-effect` events against
  `ngit`, Git Workshop, NostrHub, and more than one GRASP server before an
  OpenAgents product makes a portability or failover claim.
- **Workflow approval gates.** Self-described as infrastructure without
  glue today. Their landed shape is the comparable for Full Auto human
  gates.
- **Hosted multi-tenant offering.** The formal tenancy work reads as
  preparation for a Block-hosted service. Pricing and terms would define
  the competitive surface.
- **ACP convergence.** Buzz, Zed, goose, and the OpenAgents ACP lanes are
  converging on one protocol for agent subprocesses. Upstream ACP changes
  ripple into `provider-lane-acp.ts` and the harness contract.
- **Mobile parity.** The Flutter app's manual kind mirroring is a standing
  drift risk. How they solve schema sharing across three languages is worth
  one later read.

## Appendix A — Full Nostr NIP and kind inventory

This appendix replaces the two summary sentences in the body (Section 2 and
Section 5) with a verified, exhaustive inventory. Every claim here is read
from tracked source at the audited commit
`e9188c03f6c2460983a3dac0fa7702b468838e62`: the standard NIP set is verified
against `crates/buzz-relay/src/nip11.rs` and a repository-wide `git grep`, the
custom NIP set is read from all fourteen files under `docs/nips/`, and the kind
registry is enumerated from `crates/buzz-core/src/kind.rs`. Where the body of
this teardown under-counted or left a NIP unnamed, this appendix corrects it.
[source]

### A.0 Headline corrections to the body

- The body says Buzz "implements NIP-01, NIP-05, NIP-09, NIP-10, NIP-11,
  NIP-16, NIP-17, NIP-25, NIP-29, NIP-34, NIP-42, NIP-43, NIP-50, NIP-70,
  NIP-98, and Blossom media". That list is correct as far as it goes, but it is
  incomplete. Buzz also uses NIP-02, NIP-19, NIP-23, NIP-31, NIP-33, NIP-38,
  NIP-40, NIP-44, NIP-46, NIP-51, NIP-56, NIP-59, NIP-65, NIP-78, and NIP-94.
  The verified count is 29 standard NIPs plus the Blossom BUD-01 upload
  authorization scheme.
- The body says Buzz "authors about fifteen custom NIPs". The verified count is
  exactly 15 custom NIPs. Fourteen are full specification files under
  `docs/nips/`: NIP-AA, NIP-AE, NIP-AM, NIP-AO, NIP-AP, NIP-CW, NIP-DV, NIP-ER,
  NIP-GS, NIP-IA, NIP-OA, NIP-PL, NIP-RS, and NIP-WP. The fifteenth, NIP-AB
  (device pairing), lives co-located with its implementation and its Tamarin
  proof in `crates/buzz-core/src/pairing/` (`NIP-AB.md` and `NIP-AB.spthy`)
  rather than under `docs/nips/`, so a `docs/nips/` enumeration alone misses it.
- The body says the kind registry holds "about 81 kind constants". The verified
  registry defines 130 named `KIND_*` and `RELAY_ADMIN_*` constants, of which
  127 are registered in the `ALL_KINDS` iteration array.
- The relay advertises only a subset of what it implements. The static
  `SUPPORTED_NIPS` constant in `nip11.rs` is `[1, 2, 10, 11, 16, 17, 23, 25,
  29, 33, 38, 42, 50, 56]`, plus NIP-43 added at runtime when membership
  enforcement and a stable signing key are both present, plus the
  `supported_extensions` string `"nip-er"`. Everything else in the table below
  is implemented in code but never advertised in the NIP-11 document. This is a
  deliberate posture for draft and client-side NIPs, and it means an external
  crawler reading only the relay information document under-reads the true
  surface.

### A.1 Standard NIPs (verified against code)

"Advertised" marks a NIP present in the relay NIP-11 `supported_nips` array (or
`supported_extensions`). A blank means the NIP is implemented or consumed in
code without being advertised on the wire.

| NIP | Standard purpose | How Buzz uses it and any deviation | Advertised |
| --- | --- | --- | --- |
| NIP-01 | Basic event format, signatures, filters, replaceable ranges | The whole substrate. Every feature is a signed NIP-01 event, and the `kind` integer is the only dispatch switch. Replaceable, parameterized-replaceable, and ephemeral ranges are enforced by `is_replaceable`, `is_parameterized_replaceable`, and `is_ephemeral`. | yes |
| NIP-02 | Contact list and follow list | Kind 3 contact list, used as user-owned global follow state keyed by `(pubkey, kind)`. | yes |
| NIP-05 | DNS-based identity handles | Stored as a `user@domain` handle on the user record for lookup, search, and audit labelling. It is an identity field, not a live DNS well-known verification path in the relay. | no |
| NIP-09 | Event deletion request | Kind 5 deletion requests are honored under standard author-authored deletion semantics, and several custom NIPs (NIP-AE, NIP-AP, NIP-ER, NIP-RS) route their hard-delete path through NIP-09. NIP-PL explicitly refuses NIP-09 for push leases. | no |
| NIP-10 | Threading with marked `e` tags | The `reply` marker is the sole definition of a reply, and therefore the sole definition of top-level in the channel window (see NIP-CW). Depth is capped at 100. | yes |
| NIP-11 | Relay information document | Served at `GET /` with `Accept: application/nostr+json`. Carries `supported_nips`, `supported_extensions`, the relay `self` signing key, the NIP-PL push descriptor, NIP-ER limitation hints, and the NIP-WP workspace `icon`. Host-scoped in multi-tenant mode. | yes |
| NIP-16 | Event treatment (regular and replaceable ranges) | Range semantics are honored across the registry. | yes |
| NIP-17 | Private direct messages over gift wrap | The only DM privacy path. There is no NIP-04 and no NIP-44 open DM. DM content rides kind 1059 gift wrap, and DM presentation state lives in separate Buzz kinds (41000-series, plus the relay-signed NIP-DV visibility snapshot). | yes |
| NIP-19 | bech32 identifiers (`npub`, `nsec`) | Accepted anywhere keys are read, including the NIP-GS `user.signingkey` git configuration. | no |
| NIP-23 | Long-form content | Kind 30023, stored globally as author-owned parameterized-replaceable content, not channel-scoped. | yes |
| NIP-25 | Reactions | Kind 7 reactions. Reaction channel derivation is fail-closed, and reactions form hop 1 of the NIP-CW aux closure. | yes |
| NIP-29 | Relay-based groups | The channel model. Buzz implements selected group commands and emits relay-signed state kinds 39000-39002. Kind 39003 is registered but not emitted. Current kind 39004, pins, subgroups, and `previous` behavior are absent. Discovery is channel-scoped through the required `#h` tag. DMs also use kind 39002 membership state. | yes |
| NIP-31 | `alt` tag human-readable fallback | Recommended on encrypted or agent kinds (NIP-AE, NIP-AP, NIP-ER, NIP-PL) so unknown-kind viewers see a non-leaking summary. | no |
| NIP-33 | Parameterized replaceable events | The 30000-39999 range, keyed by `(pubkey, kind, d)`, backs personas, engrams, reminders, push leases, git repo state, DM visibility, and the channel-window overlays. | yes |
| NIP-34 | Git over Nostr | Full git forge. Repo announcement (30617), repo state (30618), patch (1617), pull request (1618), PR update (1619), issue (1621), and four status kinds (1630-1633). The relay hosts the git objects itself over Smart HTTP, which is a Buzz extension beyond the vanilla NIP-34 event set. | no |
| NIP-38 | User status | Kind 30315 user status, stored globally as user-owned personal data. | yes |
| NIP-40 | Expiration | Used for cleanup timing on terminal reminders (NIP-ER) and as the mandatory lease lifetime bound in NIP-PL. | no |
| NIP-42 | Client-to-relay authentication | Mandatory. `auth_required` is unconditionally `true`, and the REQ, EVENT, and COUNT handlers reject any connection that is not authenticated. Kind 22242 AUTH events are never stored. NIP-AA extends this flow so an agent can inherit relay access from its owner. | yes |
| NIP-43 | Relay access metadata and membership | The membership model. Admin commands (9030-9032) mutate access state, and the relay signs membership snapshots and deltas (13534, 8000, 8001) that clients cannot author. Advertised only when membership is enforced and a stable signing key exists. | conditional |
| NIP-44 | Versioned encryption (v2, XChaCha20-Poly1305) | The encryption primitive for every private payload: engrams, agent telemetry, turn metrics, reminders, read-state blobs, and push-lease content. The owner-decryptable agent-memory invariant depends on the symmetric conversation key. | no |
| NIP-46 | Remote signing (bunker) | Referenced as an interop path. NIP-PL specifies how a remote-signer client creates a push lease, and NIP-GS names it as a future key-custody option. | no |
| NIP-50 | Search | Backs the Cmd+K search. The full-text index is a Postgres `GENERATED ALWAYS` tsvector column, so search cannot disagree with signed content. Owner-private and p-gated kinds write a NULL `search_tsv` so they never enter the index. | yes |
| NIP-51 | Lists and sets | Mute list (10000), pin list (10001), bookmark list (10003), emoji list (10030), follow set (30000), bookmark set (30003), emoji set (30030). | no |
| NIP-56 | Reporting | Kind 1984 reports are accepted, queued to a tenant-scoped `moderation_reports` table, and never fanned out. Reports are signals, not triggers: the relay never auto-actions on them. | yes |
| NIP-59 | Gift wrap seal | The outer envelope machinery under NIP-17 DMs (kind 1059). Executors and matchers never decrypt gift wraps, and only outer-envelope fields are eligible for NIP-PL matching. | no |
| NIP-65 | Relay list metadata (outbox model) | Kind 10002 relay list. NIP-AE resolves an agent's "configured relays" from its published kind 10002 write relays. | no |
| NIP-70 | Protected events (`-` tag) | Marks relay-authoritative administrative state (NIP-IA requests, deltas, and snapshots) so generic relays do not rebroadcast it outside the relay context where the signing key is meaningful. | no |
| NIP-78 | Application-specific data | Kind 30078. Buzz uses it for cross-device read-state sync (NIP-RS), encrypted to the user's own keypair. | no |
| NIP-94 | File metadata | Kind 1063 file metadata attachment. | no |
| NIP-98 | HTTP authentication | Kind 27235 events authenticate the HTTP surfaces: the git Smart HTTP push path, the Blossom upload path, and the NIP-CW `POST /query` bridge. Never stored as a relay event. | no |
| Blossom (BUD-01) | HTTP media upload authorization | Kind 24242 upload authorization events gate media uploads to S3 or MinIO. Used at the HTTP boundary, never stored. | no |

Deviation summary for the standard set: the two DM privacy NIPs that vanilla
Nostr offers as open direct messages (NIP-04 and open NIP-44 DMs) are absent by
design, and the two most load-bearing wire behaviors — mandatory NIP-42 auth
and the required `#h` channel or tenant tag — are stricter than the base
protocol. NIP-29 discovery is deliberately channel-scoped and never globally
fans out.

### A.2 Custom NIPs (full description, one per subsection)

All fifteen are `draft` and `optional`. The ones marked `relay` in their
header require relay-side behavior beyond a dumb event store. Fourteen live
under `docs/nips/`, and NIP-AB is co-located with its implementation under
`crates/buzz-core/src/pairing/`.

#### A.2.1 NIP-AA — Agent Authentication

Purpose. Lets an agent key gain relay access derived from its owner's
membership, without a separate enrollment record for the agent. It closes the
synchronization hazard where revoking a human forces manual removal of every
agent that human runs. Kinds. Adds no new kind. It rides the NIP-42 kind 22242
AUTH event and reads a NIP-OA `auth` tag inside it. Tag structure. The AUTH
event carries `relay`, `challenge`, and one `auth` tag of the form
`["auth", "<owner-pubkey>", "<conditions>", "<sig>"]`. Submission. The AUTH
event is client-submitted (agent-signed) during connection admission and is
never stored. Trust and authority. The relay verifies the owner signature and
that the owner is an active member, then grants the agent a non-persistent
"virtual membership" scoped to that pubkey for the connection lifetime. Virtual
members cannot administer membership. A deliberate divergence from NIP-OA is
that `kind=` clauses are not evaluated at connection admission, so any valid
`auth` tag grants full connection-level access unless the relay opts into
per-event kind enforcement.

#### A.2.2 NIP-AE — Agent Engrams (highest-detail)

Purpose. A durable, encrypted, owner-decryptable memory store for AI agents.
This is the single sharpest idea in Buzz and the one lesson this teardown most
recommends porting. Kind. Claims kind 30174, addressable per NIP-01, keyed by
`(pubkey_a, kind, d)`. Record types. A single `core` record per
`(agent, owner)` pair holds the agent identity, rules, and goals, and zero or
more `memory` records each hold one logical entry. Addressing. The `d` tag is
not the plaintext slug. It is
`lower_hex(HMAC-SHA256(K_c, "agent-memory/v1/d-tag" || 0x00 || slug))`, where
`K_c` is the NIP-44 conversation key between the agent and its owner. The slug
never appears in any tag, so a passive observer learns nothing about what is
stored. Tag structure. Exactly one `d` tag (the blinded 64-hex value), exactly
one `p` tag (the owner pubkey), and an optional NIP-31 `alt` tag. Encryption
and authority model. Content is NIP-44 v2 ciphertext under `K_c`. Because the
NIP-44 conversation key is symmetric, the owner computes the identical key from
`(owner_seckey, agent_pubkey)` and can therefore decrypt everything the agent
ever remembers. The owner-decryptability is a property of the key construction,
not a policy the agent chooses to honor. Submission. Client-submitted and
agent-signed. Only the agent key can author or tombstone a record. The owner
has read authority by construction but no protocol-level write authority over
the agent memory. Head selection is monotonic (`created_at := max(now, T+1)`)
to defeat the same-second tiebreak under random NIP-44 nonces. A `value` of
`null` is a tombstone. Wiki-link `[[slug]]` references build an optional
reachability graph so orphaned memories can be surfaced for review. The spec
ships full BIP-340 and NIP-44 test vectors, including the raw-`shared_x` ECDH
and zero-aux Schnorr gotchas.

#### A.2.3 NIP-AM — Agent Turn Metrics

Purpose. A durable, encrypted, per-turn token-usage and estimated-cost record,
so an owner can meter a fleet of agents without the relay learning what the
agents did. Kind. Claims kind 44200, a regular stored event (append-only,
never replaced), one per completed turn. Tag structure. Exactly one `p` tag
(owner) and exactly one `agent` tag (equal to the event pubkey). No `h` tag, so
the channel a turn served stays inside the encrypted payload and per-channel
activity rates do not leak. Encryption and payload. NIP-44 v2 to the owner. The
decrypted JSON carries harness id, model, per-turn and cumulative token counts,
an advisory `costUsd` estimate, and correlation ids. Submission. Agent-signed,
client-submitted. Trust and authority. The relay must verify
`is_agent_owner(agent, owner)` by authenticated lookup, not by tag matching
alone. Reads are p-gated and NIP-42 gated on every path, including `ids`
filters, so knowing an event id does not grant access. It is a member of both
`RESULT_GATED_KINDS` and `P_GATED_KINDS` in the registry.

#### A.2.4 NIP-AO — Agent Observability

Purpose. Ephemeral, encrypted, bidirectional streaming of internal agent
session telemetry (ACP frames, turn boundaries) and owner control frames
(cancel), for live debugging and control. Kind. Claims kind 24200, in the
ephemeral range, which relays MUST NOT persist, index, or audit-log. Tag
structure. Exactly one `p`, one `agent`, and one `frame` tag whose value is
`telemetry` or `control`. Encryption. NIP-44 v2. Telemetry is encrypted
`(agent, owner)`, control is encrypted `(owner, agent)`. Submission.
Client-submitted, fanned out only through in-memory pub/sub, never a database
write path. Trust and authority. Both directions require relay confirmation of
the agent-owner relationship. NIP-AM is the durable metering complement to this
ephemeral plane, and the two share tag scoping and encryption so owner tooling
applies to both unchanged.

#### A.2.5 NIP-AP — Agent Personas

Purpose. Public, addressable "blueprint" definitions describing how to
instantiate an agent (display name, system prompt, model, runtime, provider,
name pool). Kinds. Claims kind 30175 for a persona, kind 30176 for a team (a
grouping of personas), and kind 30177 for a managed-agent instance projection.
Tag structure. Exactly one `d` tag holding the plaintext persona slug. The
d-tag is deliberately not blinded (contrast NIP-AE), because personas are meant
for discovery and cross-workspace sharing. No `p` tag. Encryption. None. The
content body is public plaintext JSON. Submission and authority. Owner-authored
and client-submitted. Only the workspace owner key can author or replace a
persona. Secrets must never appear in the public body. An `env_vars` field is
prohibited, and secrets are instead conveyed through the encrypted NIP-AE
`mem/persona` engram. The kind 30177 managed-agent projection is an explicit
opt-in allowlist of public fields and must never carry the agent secret key,
NIP-OA auth tag, or runtime environment.

#### A.2.6 NIP-CW — Channel Window

Purpose. A relay-computed, cursor-paged view of a channel's top-level timeline,
served as ordinary signed events through an extended NIP-01 filter, solving the
fact that a vanilla filter cannot express "messages that are NOT replies".
Kinds. Adds two relay-signed overlay kinds: kind 39005 thread summary (one per
row that has replies) and kind 39006 window bounds (exactly one per served
window, the sole authority on `has_more` and the next cursor). Both sit in the
parameterized-replaceable range. Request shape. A filter with `top_level: true`,
a required single `#h` channel, a row `limit`, and a composite `(until,
before_id)` cursor that both must be present or both absent, so same-second
bursts cannot lose or duplicate rows. Submission. The overlays are relay-signed
and synthesized per query. Clients MUST NOT submit kind 39005 or 39006, and the
relay rejects them at ingest. Trust and authority. Under the authenticated-TLS
transport profile that Buzz desktop ships, "relay-signed" is a TLS-origin
claim. An identity-verified profile that checks the overlay Schnorr signature
against the NIP-11 `self` key is the stricter alternative. The whole extension
degrades safely: a relay or client that ignores the extension fields serves a
plain standard query.

#### A.2.7 NIP-DV — DM Visibility

Purpose. A relay-scoped, per-viewer projection of which DM conversations a
viewer has hidden from their sidebar, without leaving the conversation. Kind.
One relay-signed kind 30622 snapshot per viewer, parameterized-replaceable,
addressed by `d` = the viewer pubkey. Tag structure. Exactly one `d` (viewer
pubkey, the address key), exactly one `p` (viewer pubkey again, the read-
authorization key), and zero or more `h` tags, one per hidden DM channel. There
is no user-signed request kind: the relay derives and republishes the snapshot
as a side effect of accepting the existing DM hide and open commands (41012 and
41010). Submission. Relay-signed only. Clients cannot author kind 30622, and it
is in `is_relay_only_kind`. Trust and authority. Read access is double-gated: a
filter-level `#p` gate plus a result-level owner check on every delivery
surface, and the kind is excluded from the NIP-50 search index, so one viewer
can never enumerate another viewer's hide choices. It is a member of both
`RESULT_GATED_KINDS` and `P_GATED_KINDS`.

#### A.2.8 NIP-ER — Event Reminders

Purpose. Encrypted, author-only reminders that become due at a future time,
which no existing Nostr primitive expresses (NIP-40 closes a visibility window
rather than opening one). Kind. Claims kind 30300, addressable, one fresh
random `d` per reminder. Tag structure. A public `not_before` tag tells the
relay when the reminder is due, while the target, note, and status are NIP-44
encrypted to the author. A reminder without `not_before` is a bookmark or a
terminal state. Submission. Client-submitted and author-signed. Trust and
authority. Reads are strictly author-only and NIP-42 gated on every path,
including `ids`. The relay learns that an author has a reminder due at a time,
but not what it is about. It is a member of `AUTHOR_ONLY_KINDS`. A supporting
relay in push mode emits the due event when `not_before` passes, and it
advertises `due_delivery_mode` and `max_not_before_delta` in its NIP-11
limitation block. Clients MUST still enforce `not_before` locally.

#### A.2.9 NIP-GS — Git Object Signing with Nostr Keys

Purpose. Signs git commits and tags with a Nostr secp256k1 key through git's
pluggable `gpg.x509.program` interface, so one identity signs relay messages,
reviews, and commits. This is the primary path for agents that commit code on
behalf of an owner. Kinds. None. The signature is embedded in the git object,
never published to a relay, so this NIP is invisible to the relay. Structure.
A detached signature wrapped in `-----BEGIN SIGNED MESSAGE-----` armor whose
base64 body decodes to a compact, non-malleable JSON envelope
`{v, pk, sig, t, oa?}`. The signing hash is domain-separated with
`nostr:git:v1:` and binds the timestamp and the optional owner attestation.
Submission and authority. Local only, no relay involvement. The optional `oa`
field embeds a NIP-OA owner attestation directly in the envelope, so anyone can
verify offline that an owner authorized the signing agent key. Trust is scoped:
`TRUST_FULLY` means only "this is the locally configured signing key", never a
global trust assertion. The spec ships deterministic BIP-340 test vectors,
including the owner-attested case.

#### A.2.10 NIP-IA — Identity Archival

Purpose. Relay-scoped archival that hides a retired pubkey from active-member
and autocomplete surfaces while preserving its history, without a ban and
without any global reputation claim. Kinds. Three families. User-signed
requests (kind 9035 archive, kind 9036 unarchive), relay-signed deltas (kind
8002 archived, kind 8003 unarchived), and a relay-signed replaceable snapshot
(kind 13535 archived-identities list). Tag structure. Requests carry a `p`
target and a NIP-70 `-` tag. Deltas add a `consent` tag naming the path
(`self`, `owner`, `admin`, or `relay`) and an `e` reference to the request.
Submission and authority. Requests are user or agent signed. Deltas and the
snapshot are relay-signed only, verified against the NIP-11 `self` key. The
recommended policy accepts admin requests, self requests, and owner-of-agent
requests proven with NIP-OA, including a published-profile-attestation path for
the zombie-agent case where the agent key is gone but its `kind:0` profile
still carries the owner attestation. A mandatory self-unarchive path for
non-banned users is the anti-shadowban property. The spec ships a five-event
request-to-delta-to-snapshot test-vector chain.

#### A.2.11 NIP-OA — Owner Attestation

Purpose. An `auth` tag by which an owner key authorizes an agent key to publish
under the agent's own authorship. It is the shared credential primitive that
NIP-AA, NIP-GS, and NIP-IA all consume. Kinds. None. It defines a tag, not a
kind, and requires no relay changes. Tag structure. Exactly four elements,
`["auth", "<owner-pubkey>", "<conditions>", "<sig>"]`, where the signature is
BIP-340 over `SHA256("nostr:agent-auth:" || event.pubkey || ":" || conditions)`.
Conditions are `&`-separated `kind=`, `created_at<`, and `created_at>` clauses.
Submission and authority. The tag reuses the NIP-26 credential format but not
its semantics. It is authorization evidence only, never an identity override.
The event remains authored by the agent pubkey, and clients MUST NOT merge an
owner-attested event into the owner timeline. A valid tag is a reusable
capability, so revocation depends on time-bound conditions or an owner refusing
to issue new tags.

#### A.2.12 NIP-PL — Push Leases (highest-detail, full normative draft)

Purpose. A stored, installation-scoped, expiring authorization asking a push
executor (usually the relay) to keep a narrow filter active after the socket
closes and to wake a specific app installation through APNs or FCM when the
filter matches. The design inverts prior art: the protocol object is the
authorization, not the transport token. Kind. Claims kind 30350, addressable,
`d` = a random per-origin installation id. Tag structure. Public tags are
exactly one `d`, one mandatory NIP-40 `expiration`, one `exec` (the executor
encryption-key id), and at most one `alt`. Every filter, endpoint, and platform
detail is NIP-44 encrypted to the executor. Encryption and content. The
plaintext carries the tenant `origin` (verified byte-for-byte against the
server-resolved tenant, never routed on), the transport, the endpoint, a
strictly increasing `generation`, an `active` flag, and the narrowed
subscriptions. Submission and authority. Client-submitted and author-signed on
a NIP-42 authenticated connection, and readable only by the author. Trust and
safety model. Amplification is disarmed at write time by construction: every
filter must carry a narrowing selector (`#p` self, `#h`, or `authors`), exact
64-hex match values only (no prefix matching), an allowlisted push-eligible
kind, no time-travel or `ids` or `limit` or `search`, and bounded quotas on
every axis. The wake payload is a fixed transport-authored reconnect constant
that carries no relay bytes, no event id, and no content, so the push path can
never become a shadow feed. A lease is a wake request, never a read grant:
read authorization is re-checked at match time and again at delivery. The draft
even registers a full public APNs gateway profile at `push.buzz.xyz` with an
App Attest enrollment, delegation, rotation, and revocation HTTP surface, and
constant-body noninterference requirements. Revocation is a higher-generation
inactive replacement. NIP-09 deletion is refused for this kind so the state
machine has exactly one transition path.

#### A.2.13 NIP-RS — Cross-Device Read State Sync

Purpose. Synchronizes a user's own per-context read position (channel, thread,
message) across that user's own devices, using encrypted per-client blobs. It
is explicitly not a read-receipt protocol and never exposes what another user
has read. Kind. Uses NIP-78 kind 30078, addressed by `d` = `read-state:<random
slot-id>`, with a `["t", "read-state"]` filter tag. Encryption. NIP-44
encrypt-to-self, using `nip44_conversation_key(user_privkey, user_pubkey)`.
Submission and authority. Client-submitted and self-signed, with no relay-side
logic required. The merge rule is a grow-only max-register CvRDT, so a read
frontier can only advance, replay is harmless, and there is no mark-as-unread.
An optional hierarchical frontier rule lets `thread:<root>` and `msg:<event>`
contexts inherit their channel frontier. Each device owns exactly one blob,
keyed by a random slot-id that reveals nothing about the client identity.

#### A.2.14 NIP-WP — Workspace Profile

Purpose. Sets and reads a relay-scoped workspace icon so every member sees the
same identifier for a workspace. It adds an in-protocol role-gated write path
where upstream Nostr offers only the read path. Kind. Claims one command kind
9033, set-workspace-profile, signed by an admin or owner. Tag structure. Exactly
one `icon` tag holding an `https`, `http`, or `data:image/*` URL. An empty value
clears the icon. Submission and authority. The command is validated exactly like
the neighboring NIP-43 admin commands (9030-9032): the relay checks that the
actor holds the admin or owner role, validates the URL scheme and size, then
stores the value and serves it in the standard NIP-11 `icon` field. The read
path is plain unauthenticated NIP-11, so any client renders the icon with zero
Buzz-specific code. In `is_relay_admin_kind`, 9033 is grouped with the NIP-43
admin commands.

#### A.2.15 NIP-AB — Device Pairing

Purpose. A secure, QR-initiated, end-to-end encrypted one-time transfer of a
secret between two devices over standard relays, so a user can move a Nostr
identity (or bootstrap a NIP-46 session) onto a new device without pasting a raw
`nsec` and without trusting the relay. It complements NIP-46: NIP-46 keeps the
key on one device and signs remotely, while NIP-AB moves the key once. Kind.
Claims one kind 24134, in the ephemeral range. Relays need no special handling
and may drop these events after delivery or a short TTL. Structure and flow.
The source device encodes an ephemeral public key, a 32-byte session secret, and
a relay URL into a `nostrpair://` QR URI. Both devices then exchange ephemeral
public keys through kind 24134 events tagged `["p", "<recipient ephemeral
pubkey>"]`, derive a shared secret through secp256k1 ECDH and HKDF-SHA256,
display a 6-digit Short Authentication String for the user to compare, and only
after that visual confirmation does the source send the NIP-44 v2 encrypted
payload. Every event uses throwaway ephemeral keypairs that are discarded after
the session. Submission and authority. Client-submitted and ephemeral-key
signed. The relay sees opaque ciphertext addressed to throwaway keys. It also
sees network endpoints, time, and payload size. Trust model. Man-in-the-middle resistance
comes from the user-verified SAS, not from the relay. The design ships a Tamarin
symbolic-security proof (`NIP-AB.spthy`) alongside the specification, which is
the same formal-verification posture the body of this teardown highlights for
the relay tenancy and auth models.

### A.3 Full kind registry

Every `KIND_*` and `RELAY_ADMIN_*` constant from `crates/buzz-core/src/kind.rs`,
grouped by area. The "Submit" column marks who may author the kind: **client**
means an ordinary member key may submit it, **relay** means it is relay-signed
and client submission is rejected (see `is_relay_only_kind` and the relay-signed
announcement and delta kinds), **command** means it is a signed instruction that
is validated and executed and never stored as a regular event, and **local** or
**internal** means it never travels the relay wire as a stored event.

#### A.3.1 Base Nostr

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_PROFILE | 0 | NIP-01 user profile metadata (replaceable) | client |
| KIND_TEXT_NOTE | 1 | NIP-01 short text note | client |
| KIND_CONTACT_LIST | 3 | NIP-02 contact and follow list (replaceable) | client |
| KIND_DELETION | 5 | NIP-09 event deletion request | client |
| KIND_REACTION | 7 | NIP-25 reaction (emoji or `+`/`-`) | client |
| KIND_CHANNEL_METADATA | 41 | NIP-01 channel metadata (replaceable, not used today) | client |

#### A.3.2 Lists and sets (NIP-51, NIP-65)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_MUTE_LIST | 10000 | NIP-51 mute list (replaceable) | client |
| KIND_PIN_LIST | 10001 | NIP-51 pin list (replaceable) | client |
| KIND_NIP65_RELAY_LIST_METADATA | 10002 | NIP-65 read/write relay list (replaceable) | client |
| KIND_BOOKMARK_LIST | 10003 | NIP-51 bookmark list (replaceable) | client |
| KIND_EMOJI_LIST | 10030 | NIP-51 emoji list (replaceable) | client |
| KIND_FOLLOW_SET | 30000 | NIP-51 named follow set (parameterized replaceable) | client |
| KIND_BOOKMARK_SET | 30003 | NIP-51 named bookmark set (parameterized replaceable) | client |
| KIND_EMOJI_SET | 30030 | NIP-51 and NIP-30 emoji set (parameterized replaceable) | client |

Buzz shared compute uses `KIND_BOOKMARK_SET` with the reserved `d` prefix
`buzz-mesh-member-status:` and the `k` value `buzz-mesh-status`. It is a
client-signed discovery projection, not a new custom kind. [source]

#### A.3.3 Author-owned global content

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_LONG_FORM | 30023 | NIP-23 long-form content (parameterized replaceable) | client |
| KIND_READ_STATE | 30078 | NIP-78 and NIP-RS per-client read-state blob (encrypted to self) | client |
| KIND_USER_STATUS | 30315 | NIP-38 user status (parameterized replaceable) | client |

#### A.3.4 Authentication and upload (never stored)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_AUTH | 22242 | NIP-42 relay auth event (carries bearer token) | client, not stored |
| KIND_BLOSSOM_AUTH | 24242 | Blossom BUD-01 upload authorization | client, not stored |
| KIND_NOSTR_IDENTITY_BINDING | 24243 | Buzz one-time identity-binding proof (ephemeral) | client, not stored |
| KIND_HTTP_AUTH | 27235 | NIP-98 HTTP auth event | client, not stored |

#### A.3.5 Agent and persona

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_AGENT_PROFILE | 10100 | Agent metadata and owner reference (replaceable, agent-authored) | client |
| KIND_AGENT_ENGRAM | 30174 | NIP-AE encrypted, owner-decryptable agent memory | client |
| KIND_PERSONA | 30175 | NIP-AP public persona definition (owner-authored) | client |
| KIND_TEAM | 30176 | NIP-AP public team definition (owner-authored) | client |
| KIND_MANAGED_AGENT | 30177 | NIP-AP managed-agent instance projection (owner-authored) | client |
| KIND_AGENT_TURN_METRIC | 44200 | NIP-AM durable per-turn token metric (encrypted, p-gated) | client |
| KIND_AGENT_OBSERVER_FRAME | 24200 | NIP-AO ephemeral encrypted agent telemetry and control | client, not stored |

#### A.3.6 Reminders and push

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_EVENT_REMINDER | 30300 | NIP-ER encrypted author-only reminder | client, author-only |
| KIND_PUSH_LEASE | 30350 | NIP-PL encrypted installation push lease | client, author-only |

#### A.3.7 Reporting and feedback

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_REPORT | 1984 | NIP-56 report (queued, never fanned out) | client |
| KIND_PRODUCT_FEEDBACK | 42000 | Buzz product feedback (sidecar, never stored as event) | client, sidecar |

#### A.3.8 NIP-29 group admin commands

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_NIP29_PUT_USER | 9000 | Add a user to a group | command |
| KIND_NIP29_REMOVE_USER | 9001 | Remove a user from a group | command |
| KIND_NIP29_EDIT_METADATA | 9002 | Edit group metadata | command |
| KIND_NIP29_DELETE_EVENT | 9005 | Delete an event from a group | command |
| KIND_NIP29_CREATE_GROUP | 9007 | Create a group | command |
| KIND_NIP29_DELETE_GROUP | 9008 | Delete a group | command |
| KIND_NIP29_CREATE_INVITE | 9009 | Create a group invite | registered, handler defers it |
| KIND_NIP29_JOIN_REQUEST | 9021 | Request to join a group | command |
| KIND_NIP29_LEAVE_REQUEST | 9022 | Request to leave a group | command |

Buzz does not register kind `9010`.
It therefore has no current NIP-29 pin-list update path.

#### A.3.9 Buzz moderation commands (mod-signed, 9040-9044)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_MODERATION_BAN | 9040 | Ban a pubkey from the community | command |
| KIND_MODERATION_UNBAN | 9041 | Lift a community ban | command |
| KIND_MODERATION_TIMEOUT | 9042 | Timeout (write-block) a pubkey until expiration | command |
| KIND_MODERATION_UNTIMEOUT | 9043 | Clear a timeout early | command |
| KIND_MODERATION_RESOLVE_REPORT | 9044 | Resolve a NIP-56 report | command |

#### A.3.10 NIP-43 relay membership and NIP-WP profile

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| RELAY_ADMIN_ADD_MEMBER | 9030 | NIP-43 add a relay member | command |
| RELAY_ADMIN_REMOVE_MEMBER | 9031 | NIP-43 remove a relay member | command |
| RELAY_ADMIN_CHANGE_ROLE | 9032 | NIP-43 change a member role | command |
| RELAY_ADMIN_SET_WORKSPACE_PROFILE | 9033 | NIP-WP set the workspace icon | command |
| KIND_NIP43_MEMBERSHIP_LIST | 13534 | NIP-43 membership snapshot | relay |
| KIND_NIP43_MEMBER_ADDED | 8000 | NIP-43 member-added announcement | relay |
| KIND_NIP43_MEMBER_REMOVED | 8001 | NIP-43 member-removed announcement | relay |
| KIND_NIP43_LEAVE_REQUEST | 28936 | NIP-43 user leave request (ephemeral) | client, not stored |

#### A.3.11 NIP-IA identity archival

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_IA_ARCHIVE_REQUEST | 9035 | Request to archive a target identity | client |
| KIND_IA_UNARCHIVE_REQUEST | 9036 | Request to unarchive a target identity | client |
| KIND_IA_ARCHIVED | 8002 | Archived-identity delta | relay |
| KIND_IA_UNARCHIVED | 8003 | Unarchived-identity delta | relay |
| KIND_IA_ARCHIVED_LIST | 13535 | Archived-identities snapshot (replaceable) | relay |

#### A.3.12 NIP-29 group state and channel-window overlays

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_NIP29_GROUP_METADATA | 39000 | Addressable group metadata state | relay |
| KIND_NIP29_GROUP_ADMINS | 39001 | Addressable group admins list | relay |
| KIND_NIP29_GROUP_MEMBERS | 39002 | Addressable group members list | relay |
| KIND_NIP29_GROUP_ROLES | 39003 | Addressable group roles definition | registered, no emission found |
| KIND_THREAD_SUMMARY | 39005 | NIP-CW thread summary overlay | relay, conflicts with current NIP-29 pins |
| KIND_WINDOW_BOUNDS | 39006 | NIP-CW window bounds overlay (has_more authority) | relay |

Current NIP-29 assigns kind `39005` to the ordered group pin projection.
The Buzz NIP-CW meaning is not compatible with that assignment.

#### A.3.13 Workflow definition and DM visibility

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_WORKFLOW_DEF | 30620 | Workflow definition (parameterized replaceable) | client, command |
| KIND_DM_VISIBILITY | 30622 | NIP-DV per-viewer DM hide snapshot | relay |

#### A.3.14 Ephemeral (20000-29999)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_PRESENCE_UPDATE | 20001 | User presence update | client, not stored |
| KIND_TYPING_INDICATOR | 20002 | Typing indicator for a channel | client, not stored |
| KIND_PAIRING | 24134 | NIP-AB device pairing event | client, not stored |
| KIND_HUDDLE_REACTION | 24810 | Huddle emoji reaction burst | client, not stored |

#### A.3.15 Stream messaging (channel-scoped)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_STREAM_MESSAGE | 9 | NIP-29 group chat message (requires `#h` channel tag) | client |
| KIND_STREAM_MESSAGE_V2 | 40002 | Stream message version 2 | client |
| KIND_STREAM_MESSAGE_EDIT | 40003 | Stream message edit | client |
| KIND_STREAM_MESSAGE_PINNED | 40004 | Pinned stream message | client |
| KIND_STREAM_MESSAGE_BOOKMARKED | 40005 | Bookmarked stream message | client |
| KIND_STREAM_MESSAGE_SCHEDULED | 40006 | Scheduled stream message | client |
| KIND_STREAM_REMINDER | 40007 | Reminder attached to a stream message | client |
| KIND_STREAM_MESSAGE_DIFF | 40008 | Diff or patch message (unified diff) | client |
| KIND_SYSTEM_MESSAGE | 40099 | System message for channel state changes | client |
| KIND_CANVAS | 40100 | Canvas (shared document) for a channel | client |

#### A.3.16 Relay-only sidecar

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_CHANNEL_SUMMARY | 40901 | Channel metadata with computed fields | relay |
| KIND_PRESENCE_SNAPSHOT | 40902 | Bulk presence state | relay |

#### A.3.17 Direct messages and attachments

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_GIFT_WRAP | 1059 | NIP-17 and NIP-59 gift-wrap DM envelope | client |
| KIND_FILE_METADATA | 1063 | NIP-94 file metadata attachment | client |
| KIND_DM_CREATED | 41001 | A DM conversation was created | client |
| KIND_DM_OPEN | 41010 | Open or re-open a DM | client, command |
| KIND_DM_ADD_MEMBER | 41011 | Add a member to a group DM | client, command |
| KIND_DM_HIDE | 41012 | Hide a DM from the sidebar | client, command |

#### A.3.18 Agent job protocol (43000-43999)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_JOB_REQUEST | 43001 | An agent job was requested | client |
| KIND_JOB_ACCEPTED | 43002 | An agent accepted a job | client |
| KIND_JOB_PROGRESS | 43003 | Progress update for an in-flight job | client |
| KIND_JOB_RESULT | 43004 | Final result of a completed job | client |
| KIND_JOB_CANCEL | 43005 | A job cancellation was requested | client |
| KIND_JOB_ERROR | 43006 | An agent job failed with an error | client |

Buzz deliberately does not reuse the NIP-90 data-vending kinds (5000-6999) here.
The kind.rs comment records the reason: Buzz requires auth chains bounded to
depth 3 and breadth 10.

#### A.3.19 Membership notifications (relay-signed)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_MEMBER_ADDED_NOTIFICATION | 44100 | Target pubkey was added to a channel | relay |
| KIND_MEMBER_REMOVED_NOTIFICATION | 44101 | Target pubkey was removed from a channel | relay |

#### A.3.20 Forum and social (45000-45999)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_FORUM_POST | 45001 | A forum post (thread root) | client |
| KIND_FORUM_VOTE | 45002 | A vote on a forum post | client |
| KIND_FORUM_COMMENT | 45003 | A comment reply on a forum post | client |

#### A.3.21 Workflow engine (46000-46999)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_WORKFLOW_TRIGGERED | 46001 | A workflow was triggered | relay |
| KIND_WORKFLOW_STEP_STARTED | 46002 | A workflow step began | relay |
| KIND_WORKFLOW_STEP_COMPLETED | 46003 | A workflow step completed | relay |
| KIND_WORKFLOW_STEP_FAILED | 46004 | A workflow step failed | relay |
| KIND_WORKFLOW_COMPLETED | 46005 | The workflow completed | relay |
| KIND_WORKFLOW_FAILED | 46006 | The workflow failed | relay |
| KIND_WORKFLOW_CANCELLED | 46007 | The workflow was cancelled | relay |
| KIND_WORKFLOW_APPROVAL_REQUESTED | 46010 | A step is waiting for human approval | relay |
| KIND_WORKFLOW_APPROVAL_GRANTED | 46011 | A pending approval was granted | relay |
| KIND_WORKFLOW_APPROVAL_DENIED | 46012 | A pending approval was denied | relay |
| KIND_WORKFLOW_TRIGGER | 46020 | Trigger workflow execution | client, command |
| KIND_APPROVAL_GRANT | 46030 | Grant a pending approval | client, command |
| KIND_APPROVAL_DENY | 46031 | Deny a pending approval | client, command |

The execution kinds 46001-46012 are excluded from triggering workflows, which
is the structural loop-prevention rule surfaced by `is_workflow_execution_kind`.

#### A.3.22 System, huddles, and media

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_AUDIT_ENTRY | 48001 | An audit log entry was recorded | relay |
| KIND_HUDDLE_STARTED | 48100 | A huddle (audio session) started | client |
| KIND_HUDDLE_PARTICIPANT_JOINED | 48101 | A participant joined a huddle | client |
| KIND_HUDDLE_PARTICIPANT_LEFT | 48102 | A participant left a huddle | client |
| KIND_HUDDLE_ENDED | 48103 | A huddle ended | client |
| KIND_HUDDLE_GUIDELINES | 48106 | Huddle channel guidelines document | client |
| KIND_MEDIA_UPLOAD | 49001 | Internal media upload audit entry (not a relay event) | internal |

#### A.3.23 Git (NIP-34)

| Constant | Kind | Purpose | Submit |
| --- | --- | --- | --- |
| KIND_GIT_PATCH | 1617 | Git patch (format-patch output) | client |
| KIND_GIT_PULL_REQUEST | 1618 | Pull request | client |
| KIND_GIT_PR_UPDATE | 1619 | Pull request tip update | client |
| KIND_GIT_ISSUE | 1621 | Issue | client |
| KIND_GIT_STATUS_OPEN | 1630 | Status: open | client |
| KIND_GIT_STATUS_MERGED | 1631 | Status: applied or merged | client |
| KIND_GIT_STATUS_CLOSED | 1632 | Status: closed | client |
| KIND_GIT_STATUS_DRAFT | 1633 | Status: draft | client |
| KIND_GIT_REPO_ANNOUNCEMENT | 30617 | Repository announcement (parameterized replaceable) | client |
| KIND_GIT_REPO_STATE | 30618 | Repository state, current refs (parameterized replaceable) | client |

Git commit and tag signatures themselves are not relay kinds. They are the
NIP-GS armored envelopes embedded in the git objects, so the forge surface is
larger than the ten NIP-34 kinds above.

Buzz creates its authoritative kind 30618 projection with the relay key after
the object-store pointer commit. The general relay still accepts client-signed
kind 30618 events under each client's own parameterized-replaceable address.
Clients must select the expected relay signer when they use the event as a
hosted-repository signal. [source]

The standard NIP-34 surface is larger than this Buzz registry. In particular,
kind 10317 publishes a user's GRASP server list. `nostr-effect` implements that
kind and all ten kinds in the table. Buzz does not register kind 10317 and does
not provide a `nostr://` Git remote helper. [source]

### A.4 Synthesis

The registry makes the body conclusion concrete and quantified. Buzz is "a
conventional server that speaks Nostr framing inside one trust boundary", and
the deviation count is now measured rather than asserted. Buzz uses 29 standard
NIPs plus Blossom BUD-01, but it advertises only 14 of them plus a conditional
NIP-43 and the `nip-er` extension string, so an outside crawler under-reads the
surface. On top of that base it authors 15 of its own NIPs (14 under
`docs/nips/` and NIP-AB co-located with its implementation) and defines 130
named kind constants, of which 127 are registered. Two wire behaviors that
vanilla Nostr treats as optional are mandatory here: NIP-42 authentication is
unconditional, and channel-scoped content such as kind 9 requires an `#h`
tenant or channel tag. A large block of the registry is authored only by the
relay and rejected from clients: the six kinds in `is_relay_only_kind` (13534,
40901, 40902, 30622, 39005, 39006) plus the relay-signed membership, archival,
group-state, workflow-execution, and channel-notification kinds (8000, 8001,
8002, 8003, 13535, 39000-39003, 44100, 44101, 46001-46012). The two open
direct-message primitives of vanilla Nostr, NIP-04 and open NIP-44 DMs, are
absent by design, and DMs ride NIP-17 gift wrap only.

The practical meaning for OpenAgents is more specific than a kind-number ban.
Standard NIPs and the 15 Buzz drafts can be useful protocol modules. The current
`nostr-effect` checkout already implements them. The lock-in appears when a
product also imports Buzz relay authority, relay-only authorship, mandatory
authentication, host-derived tenancy, or client trust rules without an
OpenAgents decision. Each reused kind therefore needs an explicit profile that
states its signer, audience, authority, retention, version, and failure mode.
The portable core includes owner-readable AE memory, OA owner proof, AP agent
identity, AB pairing, AM and AO projections, GS commit signatures, and PL
non-amplification. Their product meaning must stay separate from their wire
format.
