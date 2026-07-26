# Exoharness (exoharness/exo) Teardown — 2026-07-25 — the agent harness

Read-only architecture and product audit of the public `exoharness/exo`
source tree at an exact commit in the local reference clone
`~/work/projects/repos/exoharness-exo`. Nothing tracked was modified and
nothing was executed: no harness ran, no sandbox started, no model was
called. Two source passes covered the substrate and executor architecture,
and the integration surfaces plus quality and governance. This is **the Exo
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
