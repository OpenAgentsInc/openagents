# Exoharness (exoharness/exo) Teardown — 2026-07-25 — the agent harness

Read-only architecture and product audit of the public `exoharness/exo`
source tree at an exact commit in the local reference clone
`~/work/projects/repos/exoharness-exo`. Nothing tracked was modified and
nothing was executed: no harness ran, no sandbox started, no model was
called. Two source passes covered the substrate and executor architecture,
and the integration surfaces plus quality and governance. A third pass on
2026-07-26 read every documentation file and extracted the data model from
code — see §11. This is **the Exo
that OpenAgents integrates** — a recursive-self-improvement agent harness,
distinct from exo labs' `exo-explore/exo` cluster-inference appliance
covered in [the exo labs teardown](2026-07-25-exo-teardown.md). Owner
direction 2026-07-25: add Exo to Omega **as an agent harness**, and (openagents
#9258) let an Exo agent adapter join the public NIP-29 chat channel with its
own key.

**Pin:** `baa07f6785547080d99bd2a7d3eab6d76b984e35` (2026-07-25, "cli: survive
model-call failures in the REPL instead of crashing (#156)"). Workspace
version `0.1.0`, MIT, Copyright 2026 Ankur Goyal. [source]

## Summary

Exo makes one bet: an agent should be able to rewrite itself — its prompts,
its tools, and its harness policy — while a trusted substrate keeps the one
thing it must not corrupt, the append-only event log that is its canonical
history. The project is a two-month-old research effort from the Braintrust
orbit (Ankur Goyal is Braintrust's founder — the model-calling layer is
Braintrust's `lingua`/`llm-router` — Martin Casado authored the philosophy
essay). It is a Rust workspace of four crates plus a TypeScript harness, MIT,
`0.1.0`, unpublished — the install path is `curl setup.sh | bash` cloning the
repo and building from source. [source]

```text
        exo CLI (clap)                    ./exo.sh  (bash control surface)
   agent · conversation · repl · serve    build → scheduler → adapters → REPL
        │
        ▼
   ┌─────────────────────── the harness ───────────────────────┐
   │  exoharness (Rust)         │  executor (Rust or TS)        │
   │  durable event log         │  the turn loop:               │
   │  agents · conversations    │  prompt assembly, model call, │
   │  sessions · turns · events │  tool use, compaction policy  │
   │  artifacts · sandboxes     │  basic | rlm | typescript |   │
   │  bindings · secrets        │  codex | claude-code | cursor │
   └────────────┬──────────────┴───────────────┬───────────────┘
                │ line-delimited JSON over stdio (Rust ↔ Node guest)
                ▼                               ▼
   sandbox providers                    adapters (supervised child procs)
   docker · apple-container · daytona    irc · discord · slack · signal ·
   e2b · vercel · sprites · aws          whatsapp · exochat · agent-cli
```

The spec's load-bearing idea is a clean split (`docs/spec.md`): the
**exoharness** is a trusted substrate that owns durable state, brokers
privileged resources, and provides execution plumbing but no agent
semantics — the **executor** owns the turn loop — together they are the
**harness** — the **agent** is the behavior above it. The exoharness
deliberately **stops at the point of executing an LLM call**, because to call
the model you must make semantic choices (which events become the prompt,
how to compact), and if the substrate made them, all that policy would have
to live inside it. The durable conversation does not equal the prompt: a
conversation may accumulate millions of events, and the executor sends a
compacted slice while the raw history stays queryable. [source]

What Exo is best at, on this read: a genuinely durable, forkable,
time-travellable agent-state substrate (UUIDv7 event log, artifact-backed
tool results, sandbox snapshot/rewind across providers, a provider
contract-test suite every backend runs against) with an unusually honest set
of docs. What it is not: multi-agent, permission-gated, streaming over any
wire, ACP- or MCP-speaking, published, or stable — `AGENTS.md` states the
house rule "do not write fallback code or handle backwards compatibility,"
and the docs self-declare early and unstable. [inferred]

## 1. Identity and posture

- MIT, Copyright 2026 Ankur Goyal (Braintrust). ~2 months old (first commit
  2026-05-20), 82 commits on main, `version 0.1.0`, no tags, nothing
  published to npm or crates.io. `package.json` is `"private": true`.
  [source]
- Governance is thin: bus factor 2 (Alex Krentsel + Ankur Goyal ≈ 80% of
  main), no CONTRIBUTING, CODEOWNERS, SECURITY, or roadmap file. Notably a
  committer identity `Exospooky <exospooky@local>` has ~50 commits across
  branches — the agent committing its own work. [source]
- The org renamed mid-life (`ankrgyl/exo` → `exoharness/exo`), and the agent
  product was formerly "Exoclaw." Read the project as a VC-adjacent research
  effort from the Braintrust orbit, not a neutral community protocol.
  [source]
- Docs are unusually good for the age: 21 markdown docs plus a VitePress
  site, standout files being `docs/spec.md`, `docs/RSI.md` (the philosophy),
  `docs/design/adapter-arch.md`, and `docs/exoharness-http.md` (which states
  its own limits plainly). Several design docs describe intent that is not
  implemented (MCP, streaming) — read them as roadmap. [source]

## 2. Architecture

**The Rust workspace** (edition 2024, rust-version 1.95): `crates/exoharness`
(the substrate — types, a 53-variant request protocol, a local `basic`
backend, secrets, sandbox lifecycle, HTTP transport, and an exported
provider contract-test suite), `crates/executor` (the turn loop and every
built-in executor — `basic`, `rlm`, `typescript`, and the `codex`/
`claude-code`/`cursor` bridges), `crates/cost` (a LiteLLM price-table loader
that degrades to empty so tokens still persist without pricing), and
`crates/cli` (the `exo` binary and its REPL). [source]

**The TypeScript harness** is a single private root package with `@exo/*`
path aliases, not published packages: `typescript/harness` (the SDK —
`defineHarness`, event constructors, the 1,803-line guest-side `runner.ts`),
`typescript/model-runtime` (OpenAI Responses, OpenAI Chat Completions for
OpenRouter, and Anthropic Messages normalized back to a Responses shape), and
bridges for Codex's app-server, Claude Code's agent SDK, and Cursor's SDK.
[source]

**Rust and TypeScript relate over line-delimited JSON on stdio.** A
`TypeScriptExecutor` spawns a Node child — the TS harness owns the turn loop
while every durable operation and every host capability (sandbox processes,
model calls) round-trips back into Rust as an `exo_request`/`runtime_request`.
The full exoharness API is mirrored in TS interfaces. [source]

**The model layer is Braintrust's,** via the git-pinned `lingua` and
`braintrust-llm-router` crates (and the `@braintrust/lingua` npm packages),
with provider features for OpenAI, Anthropic, Google, and Bedrock, plus
Braintrust span tracing. This is the single most load-bearing external
dependency and it is an unreleased git rev. [source]

## 3. The data model and time travel

**Agent → Conversation → Session → Turn → Event**, persisted as JSON files
per record under `.exo/` through an `object_store` layer that also supports
S3/GCS/Azure/HTTP, so remote-backed state is architecturally possible. The
event log is append-only with UUIDv7 ids for ordered pagination — reads are a
typed cursor scan (`getEvents`/`getEvent`/`watchEvents`), not raw SQL. The
hot path is `beginTurn`, which durably accepts the user input and returns a
turn handle in one operation, with head tracking and write ordering kept
inside the substrate. Compaction is not a first-class concept — an executor
writes a custom event pointing at a derived view. [source]

**Time travel is the defining property:** the entire agent state is defined
as the version of the event log, so rewind/fork from any point recreates the
whole model. Forking replays up to an inclusive event id — sandbox
snapshot/rewind (filesystem-only, not a conversation rewind, not a memory
checkpoint) is layered on top, with snapshot ids written to the log so
time travel can cover filesystem state. A staleness guard refuses to resume a
turn whose conversation head advanced outside it. [source]

**Primitives:** artifacts (opaque immutable versioned bytes), sandboxes
(pluggable VM runtimes with create/start/stop/snapshot), and bindings/secrets
(bindings are non-secret config referencing secrets — env-var, MCP-server,
and LLM bindings — with conversation scope overriding agent scope, and
secrets mountable into sandboxes so programs use them without the model
seeing them). Secrets are AES-256-GCM encrypted at rest with 0600/0700
permissions, optionally in the Apple Keychain. [source]

## 4. The harness, tools, and adapters

**A harness is one `defineHarness({ runTurn })`.** Executors are swappable
via `--harness basic|rlm|typescript|codex|claude-code|cursor|<TS module
path>`. The Codex/Claude Code/Cursor executors are the important structural
fact for OpenAgents: Exo **hosts** those agents — it spawns
`codex app-server --listen stdio://` and speaks JSON-RPC to it, mapping their
native events into canonical exoharness events, running their runtimes inside
configured exoharness sandboxes. This is the inverse of Omega, which attaches
external agents to itself over ACP. [source]

**Tools come in three trust tiers:** built-in (`shell`, executed on the Rust
host so sandbox lifecycle stays managed, plus `install_agent_tool`), library
(user TS modules loaded with `--tool-module`), and agent-authored (the agent
writes TypeScript into `.exo/agent-tools/` at runtime). There are no file
read/write/edit tools — file work is `shell` inside the sandbox. Tools are
re-registered every model round, so a tool installed mid-turn is usable
later in the same turn, and tool results are artifact-backed: the full result
becomes an artifact, the model sees a small preview. **There is no
interactive approval or permission gate** — the security model is sandbox
isolation, and the flagship Exo agent runs unrestricted `shell` in a
networked sandbox with the source tree mounted read-write plus a
`guardian_action` tool that lets it rebuild and restart itself. [source]

**Adapters are the extension seam that matters for #9258.** An adapter is a
long-running supervised host child process, distinct from a tool, speaking
newline-delimited JSON over stdin/stdout: host→worker `send_message`,
worker→host `connected`/`message`/`lifecycle`/`error`/`disconnected`. An
inbound message becomes a normal Exo turn via a conversation wakeup — the
model must explicitly call `send_adapter_message` to reply outward — model
text is never auto-posted. Shipped adapters: irc, agent-cli, discord,
whatsapp, exochat, signal, slack. Adapter secrets are injected as env vars
into the worker child, the same pattern an `nsec` would use. [source]

## 5. Integration surfaces — the honest state

**Exo speaks no interoperability protocol OpenAgents already uses.** A
repo-wide word-boundary sweep is unambiguous: [source]

| Protocol | In Exo |
| --- | --- |
| ACP / agent-client-protocol | **Zero.** Not client, not server, no awareness. |
| MCP | A `Binding::Mcp` schema variant exists as a placeholder — **no client, no server, no tool bridge** — listed as future work. |
| Nostr / NIP-29 / secp256k1 / schnorr / npub | **Zero.** No keys, no signing, no relay, no deps. |
| OpenAI / Anthropic | Yes, as **model transports** (and as wrapped hosted agents), never as a server exo exposes. |
| SSE / WebSocket server | **None in the Rust binary.** WebSocket appears only as adapter/relay *client* code. |

**The one network server is `exo serve`** — a single unary HTTP endpoint
(`POST /request`, always HTTP 200 even on error) exposing durable-state CRUD
over the 53-variant request protocol. Its own doc states it plainly: "a
transport for exoharness primitives, **not an executor API and not a model
streaming API**," unary only, and **no authentication** (a client may send a
bearer token — the server never checks it — loopback is the entire boundary).
Streaming exists only in-process as an `ExecutionStreamEvent` enum
(first-chunk, chunk, tool-call, tool-result, completed) consumed by the
terminal REPL and serialized to no transport whatsoever. [source]

**So driving Exo from a host application has four paths, all imperfect:**
shell out to `exo conversation send` (one-shot, no streaming) — the
`agent-cli` unix socket (one JSON line in, one reply line out, blocking, no
streaming, needs the full stack running) — `exo serve` HTTP (full state CRUD
but cannot run a turn or stream a response) — or write a new adapter/transport
that serializes `ExecutionStreamEvent`. Only the last gives streamed text
deltas, tool calls, and results. [source]

## 6. Quality, governance, and risk

**Tests and CI are strong for the age:** ~187 Rust test functions (20
`#[ignore]`d heavy integration cells), ~121 TypeScript cases, a provider
contract-test suite every sandbox backend runs against, wiremock-faked
OpenAI, SHA-pinned CI actions, least-privilege permissions, and clippy
`-D warnings` enforced pre-push and in CI. There is no release, publish,
security-scan, or dependabot workflow. [source]

**Dependency posture carries real risk:** the entire model-calling layer is
unreleased git-rev Braintrust crates on the critical path — the TypeScript
typechecker is a dated dev build of the TS 7 native compiler — the WhatsApp
library is a release candidate, and almost every npm dependency uses caret
ranges in a repo where the lockfile is the only thing pinning it. Everything
lives in one flat non-workspace `package.json`, so an embedder inherits
discord.js, Baileys, a React website, and all of it. [source]

**Security posture, summarized:** secrets at rest are handled well — `exo
serve` has no auth and full secret access (loopback only) — the ExoChat relay
is E2E-encrypted with the key in the URL fragment (shareable-link is
identity), and the agent capability is very high by design — unrestricted
networked `shell`, self-authored tools, and self-rebuild, with no permission
prompts anywhere. For a desktop host this is the single biggest hazard: Exo's
threat model assumes you *want* the agent to modify itself. [source]

**Admitted gaps:** no generalized computer-use of a windowed system —
recoverable portable execution for schedulers/adapters is incomplete
(they do not fully resume after process/machine/network failure) — MCP and
native tools are unimplemented — two acknowledged lost-update races in agent
memory/skill state — a race in agent-dir claiming papered over with a TODO —
and no agent-authored adapter path (adapter types are a closed Rust enum).
[source]

## 7. Comparison with the OpenAgents estate

| Dimension | Exo (exoharness) | OpenAgents today |
| --- | --- | --- |
| Core bet | An agent that rewrites itself over a durable event log | One agent (Omega Agent) over disclosed executors — Khala as the network orchestrator |
| Substrate | Append-only event log, forkable, time-travellable, object_store-backed | Event-sourced engine (`omega-effectd`), Cloud SQL authority, Khala Sync/Nostr projections |
| Relation to Codex/Claude | **Hosts** them inside its sandboxes as executors | **Attaches** them to itself over ACP as external agents |
| Interop | None (no ACP/MCP/Nostr) — OpenAI/Anthropic as model transports | ACP client (and server), typed harness contract, `KhalaRuntimeEvent` |
| Permissions | None by design — sandbox isolation only | Typed authority, scoped tokens, receipts, confirm-on-irreversible |
| Streaming | In-process only, serialized to no wire | ACP/engine streaming, disclosed per turn |
| Maturity | 2 months, `0.1.0`, unpublished, unstable-by-declaration | Shipping RC discipline, receipts, promise gates |

The overlap is the harness abstraction itself. Exo and Omega both separate
substrate from turn-loop, but they invert the host relationship: Exo runs
other agents inside itself, while Omega runs Exo (and Codex, and Claude) as
lanes beneath one disclosed identity. That inversion is exactly why the
integration is real work rather than a config entry. [inferred]

## 8. What Omega should do — Exo as an agent harness

Owner direction (2026-07-25): integrate Exo into Omega **as an agent
harness** — one more lane beneath Omega Agent, beside the native loop and the
`codex-acp`/`claude-acp` external agents. The teardown supports a concrete
shape in three parts, honoring the standing Omega laws (external systems keep
their own homes, credentials, and configuration — GPUI is projection and
command entry — the engine owns run authority — every default change is a
tested delta). [inferred]

**The core problem, stated first: Exo has no transport Omega can attach to.**
Omega attaches lanes over ACP or drives them through `omega-effectd` — Exo
speaks neither, and its only server exposes state CRUD, not turns. So a
harness integration is net-new transport work, not a provider entry. There
are three build tiers, ascending in cost and capability. [inferred]

**Tier A — drive the CLI (cheapest, no streaming).** [NEEDS BUILD] Omega runs
`exo conversation send <agent> <conversation> <prompt>` (or writes one JSON
line to the `agent-cli` unix socket) and reads the durable log via
`exo serve` / `exo conversation events` for the result. This makes Exo a
usable but coarse lane: one shot per turn, no live text deltas, tool activity
visible only after the fact by scanning the event log. It proves the loop and
the identity binding before any wire work. Omega owns the Exo process
lifecycle the way it owns any external tool — it never edits Exo's `.exo`
root, agents, or secrets, exactly as it never touches a Codex home.

**Tier B — a streaming bridge (the real harness lane).** [NEEDS BUILD] The
`ExecutionStreamEvent` enum (first-chunk, chunk, tool-call, tool-result,
completed) is already the exact shape of an ACP session-update stream — it is
simply consumed only by the terminal today. The lane is: contribute a
transport to Exo that serializes that enum, either as a new ACP-server
adapter worker (`examples/exo/adapters/acp/worker.ts`) or as a streaming mode
beside `exo serve`, then attach it in Omega through the same `agent_servers`
path that already hosts `codex-acp`. Omega's disclosure line then names Exo,
its executor, and its model per thread, and steer/queue negotiate against
Exo's turn boundary. Because Exo's adapter types are a closed Rust enum, this
is a contribution to Exo (or a maintained fork), not a configuration — budget
the upstream relationship. This is the tier that makes Exo a first-class
Omega Agent executor class in the router.

**Tier C — the recursive-self-improvement capability, gated.** [SPECULATION]
Exo's differentiator is self-modification (`guardian_action`, self-authored
tools, source mounted read-write). That is powerful and it is exactly what
Omega's authority model exists to bound. If Omega ever surfaces Exo's
self-improvement loop, it must do so behind explicit typed authority and
receipts, never as an ambient capability — Exo assumes you want the agent to
rewrite itself, and Omega must make that an owner decision per run, not a
default. Nothing here grants it — it needs its own packet and gates.

**Security boundary, stated hard.** [inferred] Omega integrates Exo as an
*unauthenticated local harness the user chose to run whose agent has
unrestricted networked shell and can rebuild itself*. The integration must:
default `exo serve` and the socket to loopback and never proxy them
off-machine through any Omega surface — never expose Exo's no-auth HTTP
endpoint as an Omega capability — present Exo threads with the executor
disclosure line like any other lane, and never silently enable Exo's
self-modification tools — the sandbox-mounted source tree and
`guardian_action` are opt-in, owner-visible capabilities, not lane defaults.
Attribution honesty applies: threads executed via Exo name Exo, its
executor, and its model.

**Refuse list.**

1. Do not treat `exo serve` as a turn/streaming API — it is state CRUD only —
   a harness lane needs Tier B's transport.
2. Do not expose Exo's unauthenticated HTTP endpoint or agent-cli socket
   beyond loopback through any Omega surface.
3. Do not enable Exo's self-modification capability
   (`guardian_action`, agent-authored tools, read-write source mount) as a
   lane default — it is a gated, owner-visible decision.
4. Do not build on Exo's git-rev/dev-build dependency graph for anything
   Omega ships — Exo is a runtime peer, never a build dependency.
5. Do not let Exo's zero-permission tool model reach Omega's users
   unmediated — Omega adds the authority gate Exo lacks.
6. Do not conflate this Exo with exo labs' cluster `exo-explore/exo` — they
   share only a name.

## 9. The #9258 seam — an Exo Nostr adapter

The public NIP-29 chat issue names an Exo agent adapter as an
interoperability target: it must join the channel with its own Nostr key as a
generic client. The teardown confirms this is achievable and net-new. Exo has
no Nostr code at all, but its adapter subsystem is a near-exact structural
match: the shipped `exochat` worker already persists a long-lived
cryptographic identity to a 0600 file, opens a WebSocket to a relay with
exponential-backoff reconnect, authenticates every frame, and translates
relay traffic to the JSONL adapter events — swap its symmetric crypto for
schnorr-signed kind-9/kind-39000 events over a Nostr relay socket and the
shape is the same. An `nsec` fits the existing secret → worker-env pattern.
[source]

The honest scope: one ~400-line TypeScript worker plus ~150 lines of
mechanical Rust enum and JSON-schema plumbing across
`crates/executor/src/adapter/tools.rs` and
`typescript/harness/adapter-tools.ts` (adapter types are a closed enum, so a
Rust rebuild is required — a contribution to Exo or a fork), plus a
NIP-01/NIP-29 signing dependency (`nostr-tools` or `@noble/secp256k1`). The
adapter speaks the frozen `openagents.public_chat.v1` profile against any
compatible relay, exactly as #9258 requires — OpenAgents is only the first
deployment preset. This lane is independent of the Omega harness integration:
the harness lane consumes Exo's turn loop over a streaming transport, while
the chat adapter is Exo joining a Nostr room as one more client. [inferred]

## 10. Watch items

- The Braintrust dependency consolidation: whether `lingua`/`llm-router`
  reach a released, versioned state determines model-layer stability for any
  long-lived integration.
- A streaming transport: if Exo grows an ACP server or a streaming `serve`
  mode upstream, Tier B collapses from a contribution into a configuration —
  watch `docs/exoharness-http.md`'s streaming "future work."
- The closed adapter enum: an agent-authored or module adapter path (named as
  possible future work) would let the Nostr adapter ship without a Rust
  rebuild.
- Permission model: any future tool-approval or capability gate in Exo would
  reduce the authority-bounding work Omega must add around it.
- Stability: `0.1.0`, no tags, "no backwards compatibility" as a house rule —
  pin an exact commit for any integration and expect breaking changes.
- Name collision: keep this Exo and exo labs' `exo-explore/exo` distinct in
  every issue, doc, and provider label.

## 11. Addendum — the deep read (2026-07-26)

A third pass, after the Omega lane shipped (omega#87, PR #94): every
documentation file in the tree — the `website/docs-src` site (the source of
`exoharness.ai/docs`), all of `docs/`, all eight `docs/design/` documents,
`examples/exo/adapter-architecture.md`, `SELF.md`, `AGENTS.md` — plus a
code-level extraction of the data model from `crates/exoharness`,
`crates/executor`, `crates/cost`, and `typescript/`. Read at the fork pin
`cd7c0d29` (upstream base `baa07f6`). The strategy companions live in
`docs/exo/`. This addendum records the model exactly, compares it to the
OpenAgents estate, names the pieces worth adopting, and shapes the deeper
integration. [source]

### 11.1 The data model, as the code defines it

**Identity.** Every core id is a UUIDv7 newtype (`Uuid7`), so ids are also
creation timestamps and sort keys (`crates/exoharness/src/uuid7.rs`). The
alias set: `AgentId, ConversationId, SessionId, TurnId, EventId,
ResponseId, ArtifactId, SnapshotId, BindingId, SecretId`. Executor-side
scheduler and adapter stores use plain `String` ids instead — two id
regimes in one codebase. [source]

**The handle hierarchy** (`types.rs`): `ExoHarness` (root — agents,
root bindings/secrets) → `AgentHandle` (conversations, agent artifacts/
bindings/secrets) → `ConversationHandle` (events, fork, sandboxes,
`begin_turn`) → `TurnHandle` (turn-scoped writes, idempotent `finish`),
with `SandboxHandle`/`SnapshotHandle` mixed in. Sessions have no record —
a session is only an id plus `session_started`/`session_ended` events.
[source]

**Events.** `EventData` is a 22-variant tagged union: conversation
lifecycle (created/updated/deleted/forked), session/turn lifecycle, LLM
traffic as Lingua messages (`messages` carries an optional `UsageRecord`
with tokens, cost, ttft, duration), `tool_requested`/`tool_result`,
`artifact_written`, seven sandbox lifecycle variants, `error`, and
`custom { event_type, payload }`. The custom namespace already carries
real weight: per-conversation model config lives as a custom event
(config-as-event, newest wins), host lifecycle (`host_reboot`,
`adapter_runner_started`) enables crash detection by absence, and the
fork-side ACP work appends `turn_cancelled` durably. [source]

**Append rules.** One primitive appends events: it takes a process-wide
async write lock, optionally compares an `expected_head` against the
conversation record's `latest_event_id`, and refuses with a typed
head-mismatch error — "turn is stale and cannot be resumed" — when a turn
writer lost the race. `begin_turn` is genuinely atomic: session start,
turn start, and the user's input land in one durable batch before the
model is called. `fork` copies bindings, secrets, artifacts, and sandbox
records, then replays events up to the chosen point while re-minting
every event id fresh. [source]

**The protocol.** A 52-variant `Request` enum spanning five scopes (root
10, agent 13, sandbox 10, conversation 16, turn 3) with a 26-variant
`Response` enum, served identically over JSONL and over the loopback-only
HTTP `POST /request`. `watch_events` has no wire variant — event
streaming is in-process only, and the HTTP client hard-errors on it. The
TS harness runner tunnels the same enum verbatim over stdio as
`exo_request` frames. [source]

**Storage.** JSON file per record on an `object_store` abstraction
(local filesystem today, S3/GCS-shaped by construction): agents,
conversations, one file per event, artifact versions as
`<version>.json` plus `<version>.bin`, encrypted secrets, snapshot
manifests plus payload blobs. Ordering is UUIDv7 file-name order.
Concurrency is one in-process mutex — the code itself warns that
multiple processes on one root are not the design. [source]

**Sandboxes.** Nine provider variants (Docker, Apple container,
local-process, Daytona, E2B, Sprites, Vercel, AWS AgentCore, External)
behind one backend trait, warm-reuse keyed by spec hash and labels,
snapshot payloads as opaque tagged bytes, and a real cross-provider
teleport: a Docker image tar restored into Daytona, make-before-break.
Request/response-only backends fake process streaming through an embedded
Python bridge script inside the sandbox. [source]

**Secrets, the honest picture.** AES-256-GCM at rest, file-backed or
Apple Keychain master key. But the documented phrase "securely mounted
within sandboxes" has no mount implementation — secrets reach work only
as environment variables on host-controlled paths (adapter workers, model
calls, provider credentials), and the master key and nonces are generated
from UUIDv4 bytes rather than a dedicated CSPRNG call. [source]

### 11.2 Docs against code — the drift catalog

The corpus is honest but layered by vintage, and the diff matters for any
integrator:

- **Documented, not in code:** MCP is a stored-config placeholder (one
  binding variant, no client, no transport, no tool import). Cloning,
  lineage, and migration — the README's third act — have zero code beyond
  conversation `fork` and sandbox teleport. Computer use: none. The
  prompt-surface doc references a Fal image tool directory that does not
  exist. The email adapter is a proposal only.
- **In code, not documented:** the ACP stdio server (the fork's transport
  work — upstream's own tree at the base pin has no doc for transports
  beyond unary HTTP), a fully shipped skills system (agentskills.io-format
  `SKILL.md` stored as versioned agent artifacts with progressive
  disclosure), a complete Slack adapter, `/cost` and `/usage` REPL
  commands, web-search/web-fetch/todo tools, and deep Braintrust tracing.
- **Plans contradicted by shipping:** the server design doc prescribes a
  Unix-socket JSON-RPC server and the deletion of the TypeScript stdio
  bridge as "temporary scaffolding" — the shipped system is the opposite
  (HTTP transport, bridge still the live path, tutorials teach it). A
  parallel design doc (`local-sandbox-design.md`) carries a whole
  alternative vocabulary (workstreams, branches, checkpoints) that never
  became the data model.
- **The safety asymmetry, confirmed at code level:** append-only is
  policy, not physics. The head CAS protects turn writers, and everything
  else is convention — `RSI.md` footnote 2 admits the substrate is
  modifiable and merely "disallowed on the default configuration."
  Likewise the cost design doc states plainly that usage is
  "agent-reported telemetry, not an attested ledger," a direct
  consequence of the substrate never making the model call. [source]

### 11.3 Their model against ours

| Dimension | exoharness (code truth) | OpenAgents |
| --- | --- | --- |
| Truth store | One JSON-file event log per conversation, single-process lock, head CAS for turn writers | Event-sourced engine (`omega-effectd`), Cloud SQL authority, Khala Sync projections, multi-client |
| Claim discipline | Usage self-reported, prose wakeup routing, config in three substrates (artifacts, custom events, store files) | Exact token rows, typed routing/receipts, "prose is never authority" |
| Integrity | Append-only by policy, CAS on one path | Gates, receipts, digest binding, delta tests — and still short of hash-chained logs |
| Replay | Native: fork re-mints history, snapshots anchor filesystem state to events | Receipts reference history, replay is per-surface work |
| Multi-writer | Explicitly out of design (one coordinated local runtime) | The entire Khala/Sync problem, solved with authority separation |
| Extension seam | Custom namespaced events, executor swap, adapter workers | Typed schemas, behavior contracts, capability manifests |

The comparison sharpens the original §7: they built the cleanest
single-writer replayable substrate in the field and deliberately deferred
every multi-party question — attestation, authority, verification,
synchronization — that the OpenAgents estate is organized around. The two
systems still compose rather than compete, and now the seam is visible at
type level. [inferred]

### 11.4 Pieces of their model worth adopting

Adoption candidates, each bounded, none granted here — these are design
inputs for the owning surfaces, filed as FastFollow-style lessons rather
than code to copy:

1. **Atomic turn acceptance with a head-CAS handle.** `begin_turn`
   durably accepts input, and every later turn write carries
   `expected_head` — a stale writer refuses with a typed error instead of
   interleaving. This is the same law as Omega's stale-proposal refusal,
   applied to the transcript itself. Engine lanes and thread fabric
   should hold this property explicitly.
2. **UUIDv7 ids as id, timestamp, and sort key in one.** Cheap, uniform,
   and it makes cursor pagination and recency keys trivial. Worth making
   the default for new event-shaped stores.
3. **Log-is-not-prompt, compaction as derived custom events.** The
   durable record stays complete while the prompt is a view, and a failed
   compaction is inspectable because the raw history remains queryable.
   Directly relevant to thread-fabric compaction design.
4. **Artifact-backed tool results with inline previews.** Full results
   become versioned artifacts, the event carries a compact preview with
   truncation flags. We already split raw Codex event chunks from public
   traces — this is the same discipline generalized to every tool result.
5. **Snapshot ids in the event log.** Filesystem state anchored to
   transcript position is what makes environment time travel composable.
   Agent Computer and workroom lanes should record checkpoint identity in
   the run's event stream the same way.
6. **The adapter laws.** Inbound wakes a normal turn, outbound happens
   only through an explicit tool call, subsystem state stays in subsystem
   stores, and the outbox claims work by atomic rename for at-least-once
   delivery. Our agent-presence work (the public chat lane included)
   should keep all four laws.
7. **Progressive-disclosure skills on versioned storage.** Name and
   description per turn, body on demand, index written after content so a
   listed skill always resolves. The format is already the one our
   `.agents/skills` tree speaks — interop is nearly free.
8. **Per-round tool registration.** A tool installed mid-turn is usable
   in the next round because the registry is rebuilt every round —
   freshness by construction, no cache invalidation protocol.

And the explicit non-adoptions, because they collide with our laws:
prompt-text reply routing (routing context belongs in typed fields, not
prose the model must preserve), self-reported usage as accounting truth,
configuration split across three last-write-wins substrates, and
append-only-by-convention without tamper evidence. [inferred]

### 11.5 What deeper integration looks like

The shipped lane (§8 tiers, delivered through omega PR #94) drives Exo as
a disclosed ACP executor. The deep read supports four further steps, in
cost order:

1. **Read the log, not just the stream.** `exo serve` exposes the full
   52-variant protocol on loopback. A read-only engine client in
   `omega-effectd` — typed to exactly the query variants
   (`conversation_get_events`, artifact reads, agent/conversation show) —
   would let the omega#95 workspace render Exo's durable history,
   artifacts, and sandbox records directly, instead of only what ACP
   metadata carries. Loopback only, query variants only, never the
   write or secret families. [inferred]
2. **Fork and snapshot as verification primitives.** `conversation_fork`
   plus `start_sandbox { snapshot_id }` is a complete episode-reset
   mechanism: recreate the agent and its filesystem at a chosen event,
   run a candidate against an oracle, discard or promote. This is the
   concrete API surface under the workbench direction in
   `docs/exo/2026-07-26-exo-verifiable-software-gym-vision.md`, and it
   needs no upstream changes at all. [inferred]
3. **The virtualization seam, disclosed end to end.** Exo hosts Codex,
   Claude Code, and Cursor as executors inside its sandboxes, and its
   spec's ambition is to virtualize their config state. If Omega ever
   routes to an Exo-hosted vendor executor, the disclosure line must name
   the full chain — Omega Agent to Exo to the hosted runtime and model —
   or the lane misattributes. Worth deciding before anyone wires it, not
   after. [inferred]
4. **Upstream contributions matched to their admitted gaps.** Streaming
   endpoints for event watch and process output (their HTTP doc names
   both as future work — the fork's ACP transport is the working
   template), turn lease tokens (their architecture doc admits turns have
   no lease), and event hash-chaining (their most-engaged open issue,
   #154, and the prerequisite for treating an Exo history as evidence).
   Each is small, additive, and directly in their own stated direction.
   Owner-gated as always. [inferred]

Boundaries that the deep read reconfirms rather than relaxes: never point
two processes at one `.exo` root (single-writer storage), never treat the
MCP binding as MCP support, never trust self-reported usage numbers into
our accounting (receipts must mark them as harness-reported), and secrets
stay Exo-owned — the env-var injection paths are theirs to run, not ours
to reach through. [source]

### 11.6 Relation to Omega Agent, restated after the deep read

The Omega Agent program holds unchanged: the router owns routing,
disclosure, and receipts, and Exo remains one `ExternalAcp` executor
beneath it. The deep read strengthens three specific decisions already
taken — refusing `exo serve` as a turn API (it truly has none), pinning
bytes rather than commits (no releases, single-writer local builds), and
labeling the provider as not disclosed (their LLM binding genuinely does
not carry one). It adds two forward obligations: any engine-side log
reader stays a read-only capability probe consistent with the engine
owning readiness truth, and any future use of Exo-reported usage or cost
in OpenAgents metering must be typed as unattested harness telemetry,
never merged with exact provider rows. [inferred]

### 11.7 Watch items added by the deep read

- The server-plan divergence: whether upstream doubles down on HTTP or
  returns to the socket-and-client plan decides what a long-lived engine
  client should bind to.
- JavaScript isolation: agent-authored tools run in the host Node
  process today, and upstream's own plan names QuickJS as the candidate
  fix — a material boundary change for any host embedding Exo.
- The UUIDv4-sourced master key and nonce generation — small, fixable,
  and worth an upstream report.
- The TS bridge `listConversations` mismatch (request field omitted,
  response shape misread) — a live bridge bug and a cheap first
  upstream fix.
- The skills system: shipped, undocumented, and format-compatible with
  our skill tree — the quietest interop surface in the codebase.
