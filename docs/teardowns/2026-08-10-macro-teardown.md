# Macro Teardown — 2026-08-10

Read-only product and architecture audit of the public `macro-inc/macro`
monorepo at an exact commit. Macro (macro.com) is an AGPLv3, fully open-source
all-in-one workspace — email + channels + docs + tasks + agents + calls + CRM
in one SolidJS + Rust system — built by a ~15-person team in NYC/Toronto and
dogfooded for two years. Nothing in the reference clone was modified, built,
or executed. The audit's purpose is what OpenAgents should learn from the
closest open evidence for "many product domains, one workspace product, one
backend, agents as first-class citizens" — and where Macro's choices must not
override existing OpenAgents decisions, especially the Omega/Zed desktop
disposition and the self-hosted Sarah LiveKit plane.

## Summary

Macro is the strongest open whole-company-workspace reference in this catalog.
Its distinctive engineering is not any single block (mail, chat, docs, tasks,
CRM are each competent but conventional) — it is the **unification machinery**:

- one sealed Rust registry of 13 Kafka topics that *generates* the infra
  topic list with a CI drift check; GitHub workflows themselves are generated
  from Rust (`cargo x workflows --check`). [source]
- one GraphQL schema composed from ~10 domain crates, generic over hexagonal
  ports, exported as SDL that feeds three consumers: TS operation codegen, a
  Rust build-time schema-meta generator, and an SDL contract test. [source]
- a typed authorization capability — `EntityAccessReceipt<T>` — minted only at
  sanctioned per-entity axum extractors and threaded inward, with an
  agent-skill style guide that machine-checks the boundary. [source]
- a bidirectional @link graph stored as one doubly-indexed
  `comms_entity_mentions` table over a 16-variant `EntityType`, so "any entity
  mentions any entity" is a schema fact, not a feature per pair. [source]
- agents that join collaborative documents as **genuine CRDT peers**: the AI
  editing worker speaks the same Bebop-over-WebSocket Loro protocol as the
  browser, runs a headless Lexical session, and writes under a reserved
  peer-id block with live cursors. [source]
- an MCP server that is a ~1:1 projection of the internal tool surface
  (rmcp, stateless streamable HTTP, JSON-only) behind a small OAuth broker
  that fakes Dynamic Client Registration in front of FusionAuth. [source]

The same audit found honest weaknesses: the MCP broker hands raw FusionAuth
user access tokens to third-party MCP clients with no downscoping; "team-level
memory" is a per-user markdown row with team-wide inputs; the self-host story
is admitted aspiration (Doppler-anchored env layering and ~18 hard-required
third-party secrets to boot the main API); desktop Tauri is an internal
byproduct, not a shipped product; calls depend on LiveKit Cloud. [source]

The central OpenAgents decision: **treat Macro as a pattern donor, not a code
donor and not an architecture veto.** Adapt the typed-registry/generated-infra
discipline, the GraphQL-domain-crate composition shape, the receipt-typed
authorization boundary, the entity-mention graph, the agents-as-CRDT-peers
design, and the stateless MCP server + auth-broker seam (with strictly
stronger token scoping) into the OpenAgents Rust gateway and product surfaces.
Reject AGPL code reuse outright (read for ideas; never copy into non-AGPL
repos), reject Tauri as a desktop direction (Omega/Zed stands; Macro's own
evidence supports it — their Tauri desktop never shipped), reject LiveKit
Cloud posture (Sarah's self-hosted explicit-dispatch plane is stronger), and
reject broad long-lived media/API tokens.

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact           | Identity                                                                | What it establishes                    |
| ------------------ | ----------------------------------------------------------------------- | -------------------------------------- |
| Public repository  | `https://github.com/macro-inc/macro`                                    | Public source and history              |
| Audited commit     | `dd1eee50f222f47287a6f9afb48feb79a86c4e53` on `main`                    | Exact snapshot used here               |
| Commit time        | `2026-08-10T15:10:01-04:00`                                             | Same-day freshness                     |
| Commit subject     | `fix: open MCP connection auth in the system browser on native apps (#5539)` | Latest audited change             |
| Product version    | `VERSION` file `v2026.4.28.0`; web app `@coparse/web` 2.5.0; iOS 2.0.6 (build 178) | Active release trains       |
| License            | GNU AGPL v3, whole repo, verbatim text, no riders (`LICENSE.txt`); relicensed from BSL 2026-05-31 per `apps/docs/faq.mdx` | Strong copyleft reuse boundary |
| Source scale       | 13,249 tracked files; 3,526 `.rs` files (~652k lines); 6,012 TS/TSX files (~196k lines) | Rust-majority monorepo |
| Services / crates  | 44 dirs under `services/` (37 Rust, 4 Bun/TS, 1 Python, 1 lockfile-only, 1 multi); 173 dirs under `crates/` (176 `Cargo.toml`) | README's "42 services, 167 crates" slightly stale |
| Toolchain          | `rust-toolchain.toml` channel `1.94.0`, targets incl. `wasm32-unknown-unknown` + both linux-gnu for zigbuild | Confirms prior report |
| History            | 4,939 commits since 2025-11-08 (history evidently re-rooted at open-sourcing; product predates it) | ~9 months public history |
| Maintainers        | Distributed: top contributors 770/699/552/473/425 commits, ~15 active authors | Team product, no single-owner risk |

Local audited path: `~/work/projects/repos/macro` (read-only external
reference clone; clean, `main` == `origin/main` at audit time).

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, manifests, or workflows at the commit.
- **`[schema]`** — a wire schema, SQL migration, GraphQL SDL, or typed contract.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — corroborated by a linked public source (App Store, docs site).
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this source-only audit can prove.

### 1.3 Limits

Nothing was built or run. Claims about hosted behavior (macro.com, LiveKit
Cloud deployment, App Store distribution, Cloudflare production topology) rest
on checked-in configs, deploy workflows, and docs, not observation of live
systems. `services/coding-agent-worker` ships only a `bun.lock`; its source is
not in the open repo, so all statements about it are lockfile-inferred.
[limitation]

## 2. Product and repository shape

Macro presents ten "blocks" (Email, Messages, Tasks, Docs, Canvas, Agents,
Calls, Files, Pull requests, CRM) as one keyboard-first workspace with a
built-in split window manager, a unified Signal/Noise inbox, channel-based
permissions ("join a channel, gain access"), bidirectional @linking, and
nightly-refreshed agent memory. Tasks are deliberately not a separate tracker:
they are documents with a `task` sub-type, tightly coupled to channels — the
README's argument is that issue trackers die because they live apart from the
conversation. CRM is the same move: company records are channels-plus-email
aggregation, not a separate system. [source] [public]

Layout at the root: `apps/` (web SPA + docs site), `services/` (deployables:
ECS Fargate services, Lambda handlers, four Cloudflare Workers, one Python
LiveKit agent), `crates/` (domain logic, models, db clients), `packages/`
(shared TS: `collaboration`, `lexical-core`, `loro-mirror`, `observability`,
`sdk`), `infra/` (39 Pulumi TypeScript stacks, all AWS `us-east-1` — ECS
Fargate + Lambda, no EKS, no Cloudflare provider), `docker/` (local compose),
`nix/` (pinned dev shell and crane builds), `tooling/` (xtask workspace, seed
CLI). [source]

Corrections to the going-in hypotheses worth recording: the stack claims
(Axum + Tokio + async-graphql 7.2.1 + SQLx 0.8.6 offline + rdkafka 0.37 +
pgvector + OpenSearch + FusionAuth + Pulumi + cargo-zigbuild + Rust 1.94.0)
all verified. But "sync-service syncs the workspace" is wrong — it syncs
*collaborative documents only* (§4); general workspace data flows through
GraphQL + Kafka-fed subscriptions into a Rust client cache (§5). And the
"Tauri desktop dogfooded but not public" claim understates the finding: the
shipped native surface is **iOS via Tauri 2**, while desktop DMG/AppImage
artifacts are tag-built with an ad-hoc-signing fallback, no notarization, no
updater, and no download link anywhere (§8). [source] [inferred]

## 3. One monorepo, many domains: the boundary system

This is the seam OpenAgents asked about hardest — how email, chat, docs,
tasks, agents, calls, and CRM ship as one product without collapsing into
mud — and Macro's answer is unusually mechanical.

### 3.1 Hexagonal layout enforced as an agent skill

Services follow inbound-adapters / domain-with-ports / outbound-adapters. 47
of 173 crates carry a `src/domain/` directory (channels, documents, email,
soup, chat, call, crm, teams, projects, properties, entity_access,
macro_event_broker, webhook…); services are mostly thin composition roots.
The style guide is executable culture: `docs/STYLE_GUIDE.md` holds 50 numbered
`CS-##` Rust rules and 29 `FE-##` frontend rules, each with PR evidence and an
enforcement mechanism; `clippy.toml` `disallowed-methods` bans `std::env::var`
(use `macro_env_var`) and `sqlx::query` (use the compile-checked macro);
`rules/ast-grep/` adds four more executable rules; and the hexagonal
architecture contract itself lives as an agent skill at
`.claude/skills/cloud-storage-hexagonal-architecture/SKILL.md` (mirrored under
`.agents/skills/`), complete with forbidden-import lists per layer, a
review checklist, and `rg` inspection commands. The repo is multi-harness by
construction: `.claude/`, `.agents/`, `.cursor/`, `.pi/`, `opencode.json`.
[source]

The load-bearing authorization idea is **`EntityAccessReceipt<T>`**
(`crates/entity_access/src/domain/`): entity access crosses the
inbound→domain boundary as a typed capability. Receipts are minted only at
per-entity-kind axum extractors (`crates/entity_access/src/inbound/
axum_extractors/` — one file per entity kind) or via
`generate_entity_access_receipt::<RequiredLevel>`; domain code never branches
on raw roles. Tools follow the same pattern — e.g.
`crates/documents/src/inbound/toolset/read_content.rs:118` obtains a receipt,
then calls an `internal_*` repo method under a `// SAFETY:` comment naming it.
[source] [schema]

### 3.2 GraphQL composed from domain crates, generic over ports

The GraphQL API ("soup") is served by `document_storage_service` (the
confusingly named main API monolith) at `/items/soup/graphql` (+ `/ws` for
graphql-ws subscriptions), not by `connection_gateway`.
`crates/complete_graph/src/schema.rs` composes it: `CompleteMutationRoot` is
an `async-graphql` `MergedObject` over per-domain mutation roots
(properties, entity mutation, channel, notification, email), and the schema
type is generic over ~15 port traits, so every domain capability is a bound,
not a dependency. Queries use a viewer pattern (`SoupQueryRoot.user()` →
`GraphqlUser`) with `#[graphql(flatten)]` splicing domain sub-objects in.
Cross-domain edges (`crates/complete_graph/src/edges.rs`) hang notifications,
properties, favorites, permissions, and email content off every
`GraphqlSoupEntity` regardless of owning crate. Each domain crate ships a
`NoOp*` implementation so the schema builds with zero infrastructure for SDL
export. [source] [schema]

The exported SDL (`static_assets/schema.graphql`, 4,158 lines) has three
consumers: `apps/web/codegen.ts` (typed TS operations), the Rust client
cache's `build.rs` (§5), and an SDL contract test
(`crates/complete_graph/src/sdl_test.rs`). Mutations are deliberately batch
and entity-type-agnostic — one `renameEntities` covers docs, tasks, emails,
channels — because `crates/entity_mutation` defines capability traits
(`RenameEntity`, `MoveEntity`, `TrashEntity`, …) that heterogeneous entity
types implement. [source] [schema]

### 3.3 Typed event topics that generate the infrastructure

`crates/macro_event_topics` defines a `#[sealed] trait Topic` and a `topics!`
macro producing 13 Kafka topics (`macro.channels`, `macro.email`,
`macro.documents`, `macro.soup`, `macro.mentions`, `macro.chats`, …). The
header comment states the contract: the file is programmatically consumed by
infra. `cargo x kafka-topics --check` regenerates
`.github/kafka-cluster-topics.json`, which `infra/stacks/kafka-cluster/
topics.ts` reads to create MSK topics (partitions 6, RF 3, 7-day retention);
CI fails on drift. Each domain crate owns a `TopicEvent` enum with a
`SCHEMA_VERSION`, enforced at decode time
(`EventBrokerError::UnsupportedSchemaVersion`). `crates/kafka_util`
centralizes plaintext-local vs MSK-IAM construction. The same
registry-generates-infra trick covers SQS queues (`macro_queues::queue!` with
per-environment defaults and `OVERRIDE_*` escapes), service URLs
(`macro_service_urls`), and — most striking — **all 35 GitHub workflow files
are generated from Rust** (`tooling/xtask/crates/xtask_workflows/`, checked by
`cargo x workflows --check`, actions pinned to SHAs). [source] [schema]

### 3.4 The entity graph

`crates/model-entity` defines one 16-variant `EntityType` (User, Chat,
Channel, ChannelMessage, Document, Project, EmailThread, CalendarEvent, Team,
Call, ForeignEntity, StaticFile, CrmCompany, CrmContact, Reminder, Skill —
no Task; tasks are Document sub-types). The bidirectional @link graph is one
table, `comms_entity_mentions` (`crates/macro_db_client/migrations/
20251104101012_comms_db_schema.sql:77`): `(source_entity_type,
source_entity_id, entity_type, entity_id, user_id, created_at)` with covering
indexes in *both* directions, so outbound links and inbound backlinks are both
single-index scans. Mentions extract from Lexical content
(`crates/lexical_mention_extractor`), propagate on the `macro.mentions` topic,
and GitHub PRs enter as `ForeignEntity`. Realtime is user-scoped fan-out:
entity mutations → `macro.soup` keyed by recipient → access-expanded
`soup_realtime` consumer → one `soupUpdates` GraphQL subscription per client →
normalized cache patch, including an explicit `GraphqlCacheDeletion` union
member whose doc comment says it instructs a normalized client cache to delete
one record. The wire schema is literally shaped around the client cache.
[source] [schema]

### 3.5 Build and dev tooling

`cargo x` dispatches to per-command xtask crates through a dependency-free
launcher, so `cargo x cache-wasm` never compiles guppy or AWS SDKs. Local dev
(`just run_local` under `nix develop`) cross-compiles service binaries on the
host with `cargo-zigbuild` and bind-mounts them read-only into one shared
`debian:trixie-slim` runtime image (`docker/Dockerfile.runtime`, no CMD, no
baked binaries) — Rust changes never rebuild an image. Production images are
Nix/crane-built with per-artifact pruned sources so a leaf's store hash only
moves when its own closure changes; Lambdas build via crane + zigbuild against
the pinned Lambda glibc. `.github/services-config.json` is the single deploy
manifest read by both Nix and the generated workflows. `tooling/seed_cli`
applies declarative JSON seed scenarios with deterministic `5eed`-prefixed ids
and a `matrix` command verifying computed vs actual ACLs; `--instance` gives
agents concurrent isolated local stacks. `.sqlx/` holds 1,553 offline query
JSONs at the workspace root (per-crate dirs banned by CS-09). [source]

## 4. Collaborative docs: Loro CRDT with agents as peers

The single most novel seam for OpenAgents. One document type uses CRDTs — the
markdown doc, schema `MARKDOWN_LORO_SCHEMA`
(`packages/lexical-core/markdown-loro-schema.ts`): recursive `LoroMap` nodes
with a `$` metadata submap carrying a durable id, `LoroText` bodies, and a
`LoroMovableList` of children. Loro is pinned to 1.13.7 on both sides (npm
`loro-crdt` catalog pin; Rust crate `loro = "1.13"`). [source] [schema]

### 4.1 The edge authority

`services/sync-service` is a Rust `cdylib` compiled to wasm32 and deployed as
a **Cloudflare Worker with one Durable Object per document**
(`DocumentSyncSession`, `src/durable_object.rs`, ~1,265 lines). The DO holds
an authoritative `LoroDoc`, merges every client update, persists snapshots to
DO SQLite (KV fallback, R2 behind a feature), keeps an op log in DO KV,
maps peers to users and stores per-node blame in D1, and defeats Cloudflare's
~10s in-memory eviction with a 5s self-rearming alarm purely to keep the
expensive-to-rebuild doc warm. The wire is **Bebop over WebSocket**
(`bebop/schema.bop`: `FromPeer` = update/awareness/request-since/
request-snapshot/register-peer; `FromRemote` = initial-sync/update/awareness/
snapshot/ack/update-since), with codegen to both TS and Rust and the worker
serving its own schema at `GET /schema`. Two scars are encoded in the
protocol: catch-up requests send a version vector (not frontiers) because
offline clients can reference points the server never had, and the server
echoes the vv byte-identically because decode→encode is not stable for
multi-peer vectors. Updates are acked only after durable storage (client ack
timeout 5s against a documented 4.5s server storage budget), and acked before
broadcast so a dead peer socket cannot block the sender. [source] [schema]

Client resilience is layered: an IndexedDB WAL (7-day TTL, delivered-marking
on ack), an IndexedDB snapshot store persisted every 5s, a four-way seed race
(optimistic golden / local / S3 cached snapshot / live initial sync, first
wins) followed by anti-entropy `PeerRequestSince`, causal-gap tolerance
(`ImportPending` is not an error), and cross-tab convergence over
`BroadcastChannel`. Every document is seeded from one shared "golden" snapshot
(`static_assets/markdown-golden.1.bin`, checked into both Rust and TS) so
optimistic client-side creation and server creation share a CRDT ancestor and
converge instead of duplicating. Undo is Loro's `UndoManager` wearing
Lexical's history-merge heuristics. Shallow snapshots push to the main API
(→ S3) and the search extractor on alarm/last-leave. [source]

### 4.2 Agents as CRDT peers

There is **no privileged server-side mutation path** for AI document edits.
The `EditDocument` tool mints a 1h HS256 document-permission JWT and POSTs to
`services/ai-editing-worker` (a TS Cloudflare Worker), which dials the same
`wss://…/document/{id}/connect?token=…` endpoint with the same shared
`SyncServiceSource`/`createSyncSocket`/`WALSyncer` classes the browser uses
(`src/sources.ts` comment: "nothing is reimplemented here"). It runs a
**headless Lexical session** (`src/editing-workspace.ts`) so concurrent human
edits fold into the AI's working copy and its next diff is a clean delta, then
propagates through the same Mirror diff → Loro ops path. [source]

Identity is a reserved peer-id block: `999_999_999_999_999_000..+1000`,
defined twice with a "keep in sync" comment
(`packages/collaboration/src/collab/ai-peer.ts`,
`services/sync-service/src/ai_peer.rs`). It is load-bearing for attribution
because the AI authenticates with a *human's* token — the history UI collapses
AI peers to the Macro bot id before consulting the peer→user map. A `PeerPool`
semaphore (max 6) hands out concurrent AI writer identities with names and
colors ("Wolf (AI)", "Teo (AI)", … — the team roster), each publishing real
Loro cursors through the ephemeral awareness store; the worker polls human
presence every 2.5s and runs edit animation at 2× speed when nobody is
watching, slowing to 1× when a human joins. Server-side blame walks each
changed container to its nearest id-bearing ancestor map and batch-writes
last-writer-wins attribution to D1. [source]

This is the best open answer yet to "how does an agent edit a live shared
document without a fork/merge ceremony": make the agent an ordinary peer,
reserve its identity range, and let CRDT convergence handle conflicts — the
README's daily "Pool Games doc" automation works precisely because a human's
concurrent edit is just another peer's ops. [source] [inferred]

## 5. The Rust client cache

Macro replaced urql's in-memory graphcache with a **Rust normalized GraphQL
cache compiled per platform**: `crates/client/cache-core` (pure engine,
natively testable) + `cache-idb` (wasm32/IndexedDB) + `cache-sqlite`
(Tauri/rusqlite) + `cache-wasm` (browser shell in a SharedWorker, ~460 KiB).
The design doc is checked in
(`apps/web/docs/graphql-normalized-cache-plan.md`). `build.rs` parses the SDL
with apollo-compiler and enforces a presence-of-id convention at compile time
(a type with `id: ID!` normalizes under `__typename:id`; nullable ids panic
the build). Records are postcard-encoded with explicit format/epoch versions;
writes return changed entity keys + affected operations so the urql exchange
re-executes exactly the invalidated queries; optimistic mutations persist in a
durable, lease-claimed queue replayed across restarts; and identity witnessing
wipes and rebinds the whole cache if a response arrives for a different user
(`crates/complete_graph/AGENTS.md` documents `GraphqlUser.id` as the session
tag). The Tauri host runs the same engine natively and broadcasts
`graphql-cache://ops-affected` to every webview. [source] [schema]

For OpenAgents this is significant prior art on the exact seam the Effect
Native direction cares about — one typed data layer with swappable platform
storage — implemented in Rust rather than Effect, and *driven by the server
schema*: the API's `GraphqlCacheDeletion` union exists so the server can speak
to the client cache in its own key vocabulary. [source] [inferred]

## 6. MCP: the inbound server and the auth broker

The seam studied hardest, because OpenAgents runs an MCP tool surface today
(CRM MCP server at `POST /api/mcp`, stateless Streamable-HTTP JSON-RPC, per
`docs/mcp/README.md`) and is migrating tool serving toward a Rust API gateway.

### 6.1 `mcp_service`: the internal toolset, projected

`services/mcp_service` is a ~105-line binary over the official Rust SDK
(`rmcp` 1.7, workspace-pinned 1.6): **streamable HTTP, `stateful_mode =
false`, `json_response = true`** — no SSE, no session affinity, every POST
self-contained, which is what lets it scale horizontally on ECS behind an ALB
(public endpoint `https://mcp-server.macro.com/mcp`, advertised in the README
as a one-line `claude mcp add`). Tools-only capabilities (no resources or
prompts). The bridge from the internal surface is a direct 1:1 map: each
`ai_toolset::AsyncToolCollection` entry becomes an `rmcp::model::Tool` reusing
the same schemars JSON schema the Anthropic/OpenAI tool definitions use;
`call_tool` dispatches into the same `try_tool_call` path as in-app chat, with
tool-level errors returned as `CallToolResult::error` rather than protocol
errors. The MCP toolset is a *curated subset*: `ai_tools::mcp_tools()` omits
`SendEmail`, `SearchTools`/`LoadTools`, and display tools, and several ports
are deliberately no-op'd (calls RTC, notifications ingress, schedules) while
channel message sends keep real side effects so mentions still notify.
Instructions are rewritten for foreign hosts: `prompt::mcp_instructions` drops
the in-app mention-markup section and renders entity links as plain
`{base_url}/app/<type>/<id>` URLs, with tests asserting both variants.
Multi-tenancy is *not* an MCP-layer concept: one shared context, per-call
`RequestContext { user_id }`, and each tool's own `EntityAccessReceipt` check
is the tenancy boundary. Even `list_tools` requires authentication. Usage is
metered per call through `ai_usage`. [source] [schema]

### 6.2 `mcp_auth_proxy`: faking DCR in front of FusionAuth

FusionAuth has no Dynamic Client Registration, and MCP clients (Claude.ai,
Claude Code) expect DCR + PKCE + loopback public clients. The broker — a
library crate that also owns the `/mcp` router, so "the MCP service" and "the
auth proxy" are one process — serves RFC 9728-style protected-resource
metadata (`WWW-Authenticate: Bearer resource_metadata=…` on 401),
authorization-server metadata (S256-only), a **fake `POST /register`** that
mints a random UUID client_id and never persists or re-checks it, an
`/authorize` that parks `{code_challenge, client_state, client_redirect_uri}`
in Redis (10-min TTL, `GETDEL` single-use) and 307s to FusionAuth with
`idp_hint=google`, a callback that exchanges the upstream code and parks an
issued code (5-min TTL), and a `/token` that verifies PKCE *itself* (the
upstream leg uses a confidential client, no PKCE) and returns **the raw
FusionAuth access token** as the MCP bearer. Redirect URIs allow any `https://`
plus loopback `http://`. CORS mirrors the request origin with credentials —
required for claude.ai's browser dance. [source] [schema]

Three findings OpenAgents must be strictly stronger than:

1. **No downscoping.** The MCP client ends up holding a first-class Macro
   user token valid against any Macro API, not an audience-bound `/mcp`
   token. [source]
2. **DCR is decorative.** Registration state is never persisted; redirect
   URIs are never checked against a registration; security rests on the
   https/loopback allowlist + PKCE + single-use short-TTL codes. [source]
3. **Per-user only.** `mcp_servers` (outbound) and inbound grants are
   user-keyed; there is no team/workspace-shared connector, so every teammate
   authorizes Linear separately. [schema]

### 6.3 Outbound MCP and tool search

Macro is also an MCP *client*: `crates/mcp_client` stores per-user server rows
with AES-256-GCM-encrypted credentials in Postgres, negotiates client
identity three ways (pre-registered creds for Slack/GitHub, CIMD when the
server supports client-id-metadata-documents, DCR fallback), refreshes tokens
write-through to the DB, and mangles foreign tools as
`mcp__<server>__<tool>`. The economics detail worth stealing: external MCP
tools are **not** advertised on every model request — `CombinedToolSet` keeps
them in a searchable catalog behind `SearchTools`/`LoadTools`, auto-loading at
most 8 matches, because "every loaded tool's schema is advertised on all
subsequent requests for the session, so auto-loading must be bounded."
The pinned commit itself is on this seam's UX: on native apps, connector
OAuth now opens in the system browser via `openExternalUrl` because `_blank`
popups are silently dropped on iOS webviews. [source] [history]

### 6.4 Mapping to the OpenAgents Rust gateway

Macro independently converged on the same transport posture the OpenAgents
CRM MCP server already chose (stateless Streamable-HTTP, JSON-only) — useful
confirmation. What the gateway migration should additionally take: the 1:1
internal-toolset→MCP projection with a *curated* MCP subset (Macro's
no-SendEmail choice is a policy statement, not an accident); per-call identity
threading into typed entity-access checks rather than an MCP-layer tenancy
concept; RFC 9728 challenge metadata; foreign-host prompt/link rewriting; and
bounded tool-search over large catalogs. What it must do better: mint
audience-bound, capability-scoped, short-lived tokens for MCP clients (never
the raw session token), persist and verify client registrations, and support
workspace-shared connectors with explicit grants and receipts — the existing
`packages/mcp-contract` authority taxonomy already specifies this posture.
[source] [inferred]

## 7. Agent runtime, memory, bots, and the coding agent

The in-process agent loop (`crates/agent`) rides a Macro fork of `rig-core`,
routing Anthropic native, OpenAI Responses, and OpenAI-compatible
chat-completions (Cerebras registered); predefined models map `Smart` →
`claude-opus-4-8`, `Fast` → `claude-haiku-4-5`, with per-model thinking
params. Sessions default to 16 turns, are cancellable cross-instance via
Redis, and inject a system-prompt notice telling the model to trust the
runtime-supplied model id over its training data. Tool contexts narrow from
one broad `ToolServiceContext` per tool via `FromRef`. Eight surfaces drive
the same loop: product chat, structured completion, the `@Macro` channel bot
(which runs *as the mentioning user*, with the reply merely authored by the
bot id — an attribution split worth copying), nightly memory generation,
scheduled cron actions, AI projections, onboarding import, and a `Subagent`
tool whose usage rolls up into the spawning feature. [source]

"Team-level memory" deserves the plain statement: storage is a **per-user
markdown row** (`memory(user_id, memory, …)`, unique per user) regenerated
when older than 24h by a full-toolset research agent plus an LLM judge that
rejects thin or speculative output; team-ness is in the *inputs* (the shared
workspace DB) not the storage scope. Stale memory is served immediately while
regeneration runs in the background. Call transcripts enter memory-relevant
context through a real ACL grant, not a side channel (§9). [source] [schema]

Skills are markdown documents (a `skill` document sub-type) discovered by the
same search tools, plus code-defined system skills. External bots get a
webhook contract with typed tokens (`mbot_…`) and user/team scope headers;
two production examples are TS Cloudflare Workers in-repo
(`services/bots/anthropic-status-bot`, `stripe-payment-bot`). [source]

Coding agents: `crates/agent_runtime_protocol` defines `agent-runtime.v0`, a
WebSocket envelope carrying **ACP** (Zed's `agent-client-protocol` 1.2.0)
between the backend and a runtime-initiated container connection, with a mock
example launching `@zed-industries/claude-code-acp` — but it is wired into no
service in this repo, and `services/coding-agent-worker` publishes only a
`bun.lock` whose direct deps (Daytona SDK, ACP TS, hono, zod, ink) imply a
Bun harness running ACP agents in Daytona sandboxes. The one place Macro is
*not* fully open is exactly its coding-agent execution lane. [source]
[inferred] [limitation]

## 8. Client surfaces: SolidJS everywhere, Tauri 2, and the Omega question

One Vite bundle serves browser, desktop, and iOS; platform is detected at
runtime (`isTauri()` = `'__TAURI_INTERNALS__' in window`;
`getPlatform()` → web/desktop/ios/android). The SPA is SolidJS + Tailwind v4
(CSS-first, lightningcss) + patched Kobalte + TanStack solid-query + urql +
Lexical 0.45, with a URL-addressed split window manager
(`app/<split_type>/<split_id>/…`) that collapses to a swipe stack on mobile.
The Tauri 2 shell adds: a custom asset protocol serving embedded-or-OTA
bundles; an **OTA JS-bundle updater** (server side in
`crates/native_app_service`, S3-backed manifests, `NativeUpdateRequired`
forcing an App Store update when the bundle outruns the native shell); the
native SQLite GraphQL cache; navigation interception that hands every
non-allowlisted URL to the system browser and returns via `macro://` deep
links; forked HTTP/WebSocket plugins with a replay buffer for
webview-suspension gaps; and APNs/CallKit/share-extension/photo-picker Swift
plugins on iOS. ~55 `isNativeMobilePlatform()` call sites branch behavior;
auth, MCP connector OAuth, and external links all route through the system
browser on native. [source]

Distribution reality: **iOS is shipped** (App Store id 6743133649, v2.0.6
build 178, real entitlements, share + notification-service extensions,
associated domains). **Desktop is not**: DMG/AppImage build on tag with an
ad-hoc-signing fallback "for build validation", no notarization, no updater
(the desktop update endpoint unconditionally returns `NoUpdateAvailable`), no
download link; the docs' "Desktop" section means the *browser*. **Android is
scaffolding** (stock `MainActivity`, template `assetlinks.json` placeholder,
no CI, no signing). [source] [public] [inferred]

**Does this change the Omega/Zed decision? No.** Macro's own evidence cuts
against Tauri-as-desktop: a two-year-dogfooding team with a full Tauri shell
in-tree still ships desktop as a browser tab. Where Macro's Tauri path *is*
genuinely instructive is mobile: one web codebase reaching a real App Store
product with OTA bundle updates, native call UX via CallKit plugins, and a
disciplined native/web seam is a serious alternative pattern to React Native.
OpenAgents mobile is already invested in `apps/openagents-mobile` and the
Effect Native direction, so this lands as *study* (especially the OTA
manifest + forced-native-update state machine and the suspension-replay
WebSocket wrapper), not adoption. [inferred]

## 9. Calls: LiveKit Cloud versus Sarah's self-hosted plane

Macro's calls are hexagonal Rust (`crates/call` mounted in the main API) over
**LiveKit Cloud**: the transcription agent literally deploys with `lk agent
deploy` to LiveKit Cloud subdomains, and the docs FAQ names LiveKit as a
sublicensed dependency a self-hoster must keep or disable; no LiveKit
container exists in the local stack. Tokens are 6h broad grants
(join/publish/subscribe/data) minted in batches into APNs VoIP pushes — the
TTL pinned to survive push delay, with a TODO acknowledging calls over 6h
break — and the same LiveKit JWT doubles as a bearer for Macro's ring-status
endpoint. Room lifecycle is careful: room name = channel UUID, a DB unique
constraint collapses creation races, egress (room-composite MP4 straight to
S3 under static keys handed to LiveKit) stops explicitly before room delete
because the cascade is unreliable, and a Lambda thumbnails recordings via an
ffmpeg layer. Transcription is one Python agent process spawning an
`AgentSession` per participant: Deepgram Nova-3 via LiveKit Inference, Silero
VAD, and two-tier diarization (Resemblyzer embeddings clustered per track,
Deepgram speaker ints as fallback), POSTing finalized turns to an
internal-secret endpoint; ingest upserts pgvector voice embeddings
best-effort. Transcripts become "team memory" by the front door: a
`share_with_team` default-true flag materializes an `entity_access` View grant
to the creator's team at archive time, the transcript indexes into OpenSearch
parent/child, and a `ReadCallRecord` tool exposes it to agents (with a schema
note telling the model to prefer diarized speaker ids). Sonnet prompts
summarize, title, and map diarized speakers to real user ids. [source]

Against the OpenAgents voice plane: Sarah's LiveKit worker runs on
**self-hosted LiveKit**, joins one admitted room per explicit dispatch with a
generation-bound HMAC-derived token whose raw value never lands in jobs or
logs, and exits rather than reconnecting across generations
(`apps/sarah-livekit-agent`). That posture is stronger on every axis Macro is
weak (custody of the SFU, token scope and lifetime, dispatch admission), so
nothing here argues for LiveKit Cloud. What is worth harvesting is the
*product* layer OpenAgents has not built: transcript-to-ACL-grant as the
memory ingestion mechanism, per-participant transcriber sessions with
embedding-based diarization, the VoIP-push token delivery shape for mobile
ring (rescoped to short-lived tokens), and the explicit
egress-before-room-delete ordering. [source] [inferred]

## 10. AGPL posture and the self-host reality

The license posture is clean and unusually honest: verbatim whole-repo
AGPLv3, no CLA (inbound = outbound per `CONTRIBUTING.md`), no `ee/` carve-outs,
an explicit "fully open source — not open core" claim that holds up, and a
recorded BSL→AGPL relicensing (2026-05-31). One vestigial
`apps/web/LICENSE` one-liner ("All rights reserved", untouched since a
restructuring commit) muddies the frontend subtree on paper. Monetization is
hosted SaaS plus selling non-AGPL licenses. [source] [history]

**Consequence for OpenAgents, stated plainly: read for ideas, never copy.**
No Macro code, schema file, prompt text, or config may be vendored into
non-AGPL OpenAgents repositories. Every adapt-candidate in §11 is a
pattern to re-express independently, exactly as the Paseo and Armada
teardowns handled their AGPL subjects. If OpenAgents ever wanted literal
reuse, it would require an isolated AGPL-licensed component with a network
boundary — not currently justified by anything found here. [source]
[inferred]

Self-hosting is admitted aspiration ("as of June 2026, this hasn't been our
primary focus"). The architecture shows real portability care — one
`LOCAL_AWS_URL` flips every AWS SDK to LocalStack, SES has a first-class SMTP
alternative, an xtask validator enforces "local mode never requires real
AWS" — but the practical path is "read the code and fork": the base env layer
is Macro's private Doppler project; ~18 hard-required env vars
(LiveKit, OpenAI, Cohere, GitHub app, Cal.com, Meta pixel, CloudFront signing
keys…) boot-block the main API with no checked-in template; CloudFront and
Lambda have no local substitute; the local stack runs 14 of 44 services; and
there are zero self-hosting docs. A lesson worth internalizing for
OpenAgents' own claims: "AGPL + repo public" is not a self-host story;
env-var closure, secret substitutability, and a services inventory are the
actual product surface of self-hostability. [source] [inferred]

## 11. Adapt / study / reject

Adapt (re-expressed independently — no AGPL code copying — with the owning
OpenAgents seam named):

- **Stateless JSON-only MCP projection of one internal toolset, with a
  curated MCP subset and per-call identity → typed access checks.** Lands in
  the Rust API gateway MCP migration and `packages/mcp-contract` Phase 1+;
  confirms the CRM MCP server's transport choice. Add what Macro lacks:
  audience-bound short-lived tokens, persisted client registration,
  workspace-shared connectors, receipts. [source]
- **RFC 9728 challenge + auth-broker seam for DCR-less identity providers.**
  Same gateway seam, for any OpenAgents IdP that lacks DCR — but broker-minted
  scoped tokens, never raw upstream session tokens. [source]
- **Bounded tool-search over large foreign-tool catalogs** (catalog +
  `SearchTools`/`LoadTools`, auto-load cap) instead of advertising every MCP
  tool on every request. Lands wherever OpenAgents composes external MCP
  servers into agent loops. [source]
- **Typed registries that generate infrastructure with CI drift checks**
  (Kafka topics → infra JSON → Pulumi; workflows generated from code;
  schema-versioned per-domain event enums). Lands in the monorepo's
  contract/codegen discipline and any future event-backbone work. [source]
- **`EntityAccessReceipt<T>`-style typed authorization capabilities minted
  only at sanctioned extractors** — convergent with the existing OpenAgents
  grants/receipts direction; the enforcement packaging (numbered style rules +
  clippy disallowed-methods + ast-grep + an agent skill that machine-checks
  the boundary) is directly portable practice. [source]
- **The doubly-indexed any-entity-mentions-any-entity table over one sealed
  `EntityType`** as the storage shape for bidirectional linking. Lands in the
  All Work model (`docs/allwork/`), where OpenAgents' version should be the
  signed Nostr-centric causal graph — Macro shows the query-side schema that
  makes backlinks cheap. [schema] [inferred]
- **Agents as CRDT peers with a reserved identity block, headless editor
  sessions, and real presence.** Lands as design evidence for any future
  OpenAgents collaborative-document surface, and for Omega: Zed already has a
  native CRDT buffer plane, so "agent as ordinary collaborator with reserved
  identity and honest attribution" is the pattern to re-express there rather
  than a privileged apply path. [source] [inferred]
- **Transcript-to-ACL-grant as memory ingestion**, per-participant
  transcriber sessions, and egress-before-delete ordering. Lands in the Sarah
  voice lane's future recording/transcription work, on self-hosted LiveKit
  with short-lived generation-bound tokens. [source]
- **Ack-after-durable-storage, byte-identical vv echo, golden-ancestor
  seeding, and WAL/snapshot/anti-entropy layering** as sync-protocol laws.
  Lands in Khala/Sync design reviews as tested contracts, not copied code.
  [source]

Study (no current action):

- **Tauri-2-iOS-from-one-web-bundle with OTA JS updates and a
  forced-native-update state machine** — a real shipped alternative to React
  Native; OpenAgents mobile stays on its Effect Native path, but the OTA
  manifest design and suspension-replay WebSocket wrapper are worth a read
  when mobile update work lands. [source] [inferred]
- **The Rust-compiled-per-platform normalized client cache** (schema-driven
  build-time key policy, durable optimistic queue, identity witnessing,
  server-spoken cache-deletion vocabulary) — strong prior art for the Effect
  Native data layer, in a different implementation language. [source]
- **Rust-in-wasm CRDT authority on Cloudflare Durable Objects** — elegant,
  but OpenAgents infrastructure is Google Cloud; the DO-per-document +
  keep-warm-alarm shape maps to different primitives there. [source]

Reject (with reasons):

- **Any code-level reuse.** AGPLv3: reading is free, copying into non-AGPL
  OpenAgents repos is not permitted. This includes prompts, schemas, and
  config shapes verbatim. [source]
- **Tauri as the OpenAgents desktop direction.** The Omega/Zed disposition
  stands; Macro's own desktop is an unshipped byproduct and its docs define
  "desktop" as the browser. Nothing here reopens the decision. [source]
  [inferred]
- **LiveKit Cloud and broad 6h media tokens.** Sarah's self-hosted,
  explicit-dispatch, generation-bound-token plane is strictly stronger; also
  reject reusing a media token as an API bearer. [source]
- **Raw upstream access tokens as MCP bearers, decorative DCR, and
  any-https redirect allowlists.** The gateway must be strictly stronger on
  all three. [source]
- **Per-user-only connector and memory scoping presented as team-level.**
  OpenAgents' team memory and shared connectors need honest storage scope,
  explicit grants, and receipts — Macro documents the marketing/schema gap to
  avoid. [source] [schema]
- **SolidJS/Kobalte/urql/Lexical as an OpenAgents frontend stack**, and the
  duplicated Rust/TS constant definitions ("keep in sync" comments for AI
  peer ids) — OpenAgents' contract-codegen discipline should never accept
  twin definitions without a generator. [source]
- **Doppler-anchored env layering as a self-host story.** If OpenAgents
  claims self-hostability anywhere, the env closure and secret
  substitutability must be checked product surface, not a private secrets
  project. [source] [inferred]

## 12. Watch items

1. **The coding-agent lane opening up.** `agent_runtime_protocol`
   (ACP-over-WebSocket to runtime-initiated containers) is in-tree but
   unwired, and `coding-agent-worker` is lockfile-only (Daytona + ACP
   inferred). If Macro open-sources that worker, it becomes a direct
   comparable for the OpenAgents execution lane. [source] [inferred]
2. **MCP token scoping fixes.** The raw-FusionAuth-token and decorative-DCR
   findings are the kind upstream tends to fix under scrutiny; their chosen
   fix (broker-minted tokens? FusionAuth DCR?) is worth reading when it
   lands. [inferred]
3. **Desktop and Android shipping decisions.** A notarized, updated Tauri
   desktop or a released Android app would change the evidence base for the
   one-web-bundle thesis. [source]
4. **CRDT expansion beyond markdown docs.** Today exactly one document type
   is Loro-backed; extending CRDT semantics to tasks/canvas/CRM would test
   whether the peer model generalizes. [source]
5. **Self-host productization.** They advertise `self-host@macro.com` for
   FedRAMP-class clients; a real self-host package would make the AGPL
   posture commercially load-bearing and worth re-auditing. [source]
6. **Bebop and Loro version discipline.** Client Bebop schema is fetched
   from a stale worker URL at build time and the AI peer-id block is defined
   twice by hand — drift here is a live convergence risk they will eventually
   pay for; instructive either way. [source]

## 13. Bottom line

Macro is proof that a small team can ship the "one operating system for a
company" thesis — the same thesis OpenAgents holds for machine work — by
making unification *mechanical*: one entity vocabulary, one mention graph,
one event registry that generates its own infrastructure, one GraphQL surface
composed from hexagonal domain crates, one tool surface projected to both the
in-app agent and the public MCP endpoint, and agents admitted to shared
documents as ordinary peers rather than privileged mutators. Those are the
patterns to take, re-expressed under OpenAgents' stronger authority model
(typed grants, receipts, signed identity, self-hosted media, scoped tokens)
and its existing platform decisions (Omega/Zed desktop, Effect Native
clients, Google Cloud, Nostr/Lightning rails). The AGPL boundary makes the
reuse mode unambiguous — study everything, copy nothing — and Macro's candor
about its own gaps (self-host, per-user memory, unshipped desktop) makes it
an unusually trustworthy reference for exactly where those patterns stop.

## Source map

Primary evidence at the audited commit (paths relative to
`~/work/projects/repos/macro`):

- `README.md`, `CLAUDE.md` (`AGENTS.md` symlinks to it), `LICENSE.txt`,
  `CONTRIBUTING.md`, `VERSION`, `rust-toolchain.toml`, `clippy.toml`,
  `docs/STYLE_GUIDE.md`, `docs/RUNNING_LOCALLY.md`, `apps/docs/faq.mdx`,
  `apps/docs/apps.mdx`.
- Boundaries: `.claude/skills/cloud-storage-hexagonal-architecture/SKILL.md`,
  `crates/entity_access/src/{domain,inbound/axum_extractors}`,
  `crates/model-entity/src/lib.rs`, `crates/entity_mutation/`,
  `crates/macro_db_client/migrations/20251104101012_comms_db_schema.sql`,
  `crates/complete_graph/{src/schema.rs,src/edges.rs,AGENTS.md}`,
  `crates/macro_event_topics/src/lib.rs`, `crates/macro_event_broker/`,
  `tooling/xtask/`, `.github/services-config.json`, `docker/Dockerfile.runtime`,
  `nix/cloud-storage.nix`, `tooling/seed_cli/`.
- Collaboration: `services/sync-service/{README.md,bebop/schema.bop,
  src/durable_object.rs,src/websocket.rs,src/state.rs,src/ai_peer.rs,
  wrangler.toml}`, `packages/collaboration/src/{collab,sync-service}/`,
  `packages/lexical-core/markdown-loro-schema.ts`,
  `apps/web/src/features/block-md/`, `services/ai-editing-worker/src/`.
- Client cache: `apps/web/docs/graphql-normalized-cache-plan.md`,
  `crates/client/cache-{core,idb,sqlite,wasm}/`,
  `apps/web/src/lib/graphql-cache/`, `apps/web/tauri/graphql_cache_plugin/`.
- MCP and agents: `services/mcp_service/src/{main.rs,tool_service.rs,
  context.rs}`, `services/mcp_auth_proxy/src/{inbound,domain,outbound}/`,
  `crates/mcp_client/`, `crates/{ai_tools,ai_toolset,agent,prompt,memory,
  skills,system_skills,channel_bots,bots,agent_runtime_protocol}/`,
  `services/coding-agent-worker/bun.lock`.
- Clients: `apps/web/{package.json,vite.base.ts,src/lib/core/util/platform.ts,
  src/lib/tauri/,src/components/app/split-layout/}`,
  `apps/web/tauri/{src-tauri,macro_bundle_updater_plugin,navigation_plugin,
  callkit_plugin,gen/apple,gen/android}/`, `crates/native_app_service/`,
  `.github/workflows/build_desktop_on_tag.yml`.
- Calls and self-host: `crates/call/src/{domain/service.rs,
  outbound/livekit_rtc_client.rs,outbound/ai_call_summarizer.rs,
  inbound/toolset/read_call_record.rs}`, `services/transcription/
  {transcriber.py,livekit.prod.toml,justfile}`,
  `services/call_recording_preview_handler/`, `crates/macro_aws_config/`,
  `crates/ses_client/`, `tooling/xtask/crates/xtask_local/src/local/
  {env_layer.rs,validate.rs,inventory.rs}`, `infra/stacks/`.

OpenAgents-side grounding: `docs/mcp/README.md` (CRM MCP server, contract
phases), `packages/mcp-contract`, `apps/sarah-livekit-agent/README.md`,
`docs/effect-native/`, `docs/teardowns/README.md` (Omega disposition),
`docs/allwork/`.
