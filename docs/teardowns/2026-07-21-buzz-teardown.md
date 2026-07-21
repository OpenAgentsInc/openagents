# Buzz Teardown — 2026-07-21

Read-only architecture and product audit of the public `block/buzz` source
tree at an exact commit in the local reference clone
`~/work/projects/repos/buzz`. Nothing tracked was modified and nothing was
executed. Buzz is Block's open-source, self-hostable workspace where humans
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

The central OpenAgents decision: **do not adopt Buzz as a substrate, a
dependency, or a client shell. Treat it as the strongest competitor
reference in the catalog and port five bounded lessons: runtime formal
conformance replay at authority seams, the owner-decryptable agent-memory
invariant, ACP harness-pool supervision with typed stall fates, the
agent-first CLI with one skill source shared across harness directories and
the shipped product, and workflow approval gates with structural loop
prevention. Reject the Nostr-relay event log as a chat/thread authority, the
Tauri shell, the Flutter mobile lane, the non-streaming agent turn model,
and the unilateral custom-kind registry.**

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Public repository | `https://github.com/block/buzz` | Public source and history |
| Local clone | `~/work/projects/repos/buzz` | The audited tree |
| Audited commit | `e9188c03f6c2460983a3dac0fa7702b468838e62` | Exact snapshot used here |
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

This audit did not build or execute the relay, the desktop app, the agent
crates, or the test suites. All behavior claims come from reading tracked
source and docs. The TLA+ and Tamarin proof claims are read from the spec
files and vision docs, and this audit did not re-run the checkers. Only the
`block/buzz` repo is public. Block-internal builds, deploy pipelines, and the
hosted relay live in private `squareup/*` repos and are out of scope.
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

### 3.6 Clients, ops, and process

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
  workspace needs it: required `#h` tags on kind 9, relay-signed membership
  kinds that clients may not submit, channel-scoped NIP-29 discovery events
  that never reach global fan-out, no NIP-04/NIP-44 direct DMs, and about
  fifteen self-authored NIPs. In practice it is a conventional server that
  speaks Nostr framing inside one trust boundary. Adopting its kinds would
  pin OpenAgents to Block's unilateral registry. [source] [inferred]
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

## 7. What OpenAgents should reject

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
- **Custom-kind and custom-NIP adoption.** Do not implement Buzz kinds or
  its NIP-A* family in `nostr-effect` or `packages/nip90` while they remain
  a single-vendor registry with relay-required semantics. Watch for upstream
  standardization instead. [source] [inferred]
- **Running Buzz infrastructure as a dependency.** Postgres, Redis, MinIO,
  Keycloak, and a 218,000-line relay for any single wanted feature is the
  wrong trade at every point in the OpenAgents stack. [source]

## 8. Recommendation

Buzz is the most complete external instantiation of the "humans and agents
in one auditable workspace" thesis that OpenAgents also holds, built by a
funded team at very high velocity, and it validates several OpenAgents
positions independently: agents with their own identity and audit trail,
one event log over many glued tools, formal checks on tenancy seams, and an
agent-first CLI. It is also a direct competitor to the OpenAgents desktop,
Pylon, and forum surfaces, and its substrate, shells, and turn model
conflict with settled OpenAgents architecture at every layer.

The decision: **track, do not adopt.** No Buzz code, crates, kinds, or
services enter the OpenAgents stack. Port the five bounded lessons in §6
through the normal Fast Follow admission path, starting with the two
highest-value ones: a runtime conformance replay pilot on one authority
seam (6.1) and the owner-decryptable memory invariant stated and tested in
the memory package (6.2). Re-read this repo in roughly one quarter — at its
velocity the mesh, moderation, and approval-gate lanes will have moved.

## 9. Watch items

- **Buzz Mesh.** Community-pooled GPU compute gated by relay membership
  (`VISION_MESH.md`, `desktop/src-tauri/src/mesh_llm/`) overlaps directly
  with the Pylon provider and NIP-90 compute-market thesis. If Block ships
  it well, it is the closest competing story to "your community is your
  compute provider."
- **Custom NIP standardization.** Whether NIP-AA/AE/AM/AO/AP/GS move toward
  `nostr-protocol/nips` or stay a Block registry decides whether any
  interop lane ever becomes worth building.
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
