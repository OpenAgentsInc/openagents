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
| NIP-29 | Relay-based groups | The channel model. Group admin commands (9000-9022) and relay-signed addressable group state (39000-39003) are used, but discovery is channel-scoped through the required `#h` tag and never fans out to a global group directory. DMs are surfaced as NIP-29-style membership (kind 39002) as well. | yes |
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
signed. The relay sees only opaque ciphertext addressed to throwaway keys and
learns nothing about the payload. Trust model. Man-in-the-middle resistance
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
| KIND_NIP29_CREATE_INVITE | 9009 | Create a group invite | command |
| KIND_NIP29_JOIN_REQUEST | 9021 | Request to join a group | command |
| KIND_NIP29_LEAVE_REQUEST | 9022 | Request to leave a group | command |

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
| KIND_NIP29_GROUP_ROLES | 39003 | Addressable group roles definition | relay |
| KIND_THREAD_SUMMARY | 39005 | NIP-CW thread summary overlay | relay |
| KIND_WINDOW_BOUNDS | 39006 | NIP-CW window bounds overlay (has_more authority) | relay |

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
absent by design, and DMs ride NIP-17 gift wrap only. The practical meaning for
OpenAgents is that the kind registry is the lock-in. Adopting any Buzz kind
means adopting Block's unilateral integer assignments, the relay-signed
semantics attached to them, and the mandatory-auth plus required-tenant-tag
posture that makes them work, which is precisely why the body recommendation to
reject the relay-as-substrate holds. The genuinely portable ideas live above
the wire, chiefly the owner-decryptable NIP-AE memory invariant and the NIP-PL
write-time non-amplification discipline, and those port as design lessons
without importing a single kind number.
