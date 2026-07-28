# Goose Development Kit, reference client, Mesh LLM, and Omega teardown — 2026-07-27

Commit-pinned source and public-surface audit of the new Goose Development Kit
(GDK) direction, the existing Goose Reference Client (GRC), Mesh LLM, and the
current Omega integration boundaries.

This report extends the broader
[Goose teardown](./2026-07-17-goose-teardown.md). That document remains the
deeper audit of Goose's loop, sessions, permissions, security, extensions,
recipes, scheduling, provider system, and release chain. This report answers a
narrower decision:

> What is GDK in source today, and should Omega use it?

## Executive decision

**GDK is a credible direction and an incomplete product boundary.** The public
announcement describes a composable agent platform with orchestration, model
access, context, tools, memory, execution, automations, routing, and language
bindings. The current source does not yet expose that whole platform through
one stable embeddable SDK. It exposes three different integration surfaces at
three different maturity levels:

1. **Goose as an ACP process or daemon is usable now.** `goose acp` serves the
   agent over stdio. `goose serve` adds authenticated HTTP/WebSocket
   transports. The TypeScript `@aaif/goose-sdk` wraps ACP, generates typed
   clients for Goose-specific extensions, resolves matching Goose binaries,
   and is used by the Goose terminal and desktop clients. [source] [test]
2. **The Rust/Python/Kotlin SDK is an alpha provider SDK today.** The public
   Rust crate re-exports Goose's ACP extension wire types. Its optional UniFFI
   surface constructs model providers and streams or completes provider
   requests. It does **not** expose the full Goose `Agent`, session store,
   context manager, MCP extension manager, permission engine, recipes,
   scheduler, or complete loop described by the GDK vision. [source] [schema]
3. **The full embedded Rust agent API is still being developed.** The core
   implementation exists in the monorepo's internal `goose` crate, and the
   Goose CLI links it directly. It is not the current public `goose-sdk`
   boundary. The GDK announcement itself uses future language for this API.
   [public] [source] [vision]

Omega should therefore **not embed, vendor, or replace its native agent loop
with GDK now**. It should not add the TypeScript SDK and bundled Goose binary as
a second application runtime. It should not adopt `goose-providers` as a
parallel provider, secret, model, and accounting plane.

Omega should instead:

- evaluate Goose as a **named external ACP executor** by launching the pinned
  `goose acp` binary through Omega's existing custom-agent-server path.
- consume standard ACP first and put every `_goose/unstable/*` method behind an
  optional, versioned Goose capability adapter.
- retain Goose's own home, credentials, sessions, provider configuration, and
  extension state. Omega projects and discloses the external work. It does not
  become authority over Goose's state by copying it.
- keep Omega's typed executor disclosure, queue/steer rules, audience
  projection, and explicit run authority around the attachment.
- treat Mesh LLM independently as an **OpenAI-compatible inference lane** or
  managed external inference service. Do not route Mesh through GDK merely
  because Goose has a launcher integration.
- revisit embedded GDK only after the full loop has a published, documented,
  versioned Rust interface with storage, policy, tool-execution, cancellation,
  observability, and authority injection seams.

The immediate value is interoperability and conformance, not dependency
adoption.

```text
recommended now

Omega UI and router
      |
      +-- native Omega loop
      |
      +-- explicit external ACP executor ---- stdio ----> pinned `goose acp`
      |         standard ACP first                          Goose-owned state
      |         typed disclosure                            and credentials
      |
      +-- OpenAI-compatible model provider ---- HTTP ----> Mesh LLM :9337/v1
                exact model + tool-call proof               separate authority

not recommended now

Omega -> @aaif/goose-sdk -> bundled Goose binary
Omega -> goose-sdk UniFFI -> provider-only alpha surface
Omega -> monorepo-internal `goose` crate -> second embedded agent authority
```

## 1. Snapshot, provenance, and limits

All reference clones under `~/work/projects/repos/` were clean and
fast-forwarded to their current `origin/main` tips before this audit.

| Artifact | Audited identity | Relevance |
| --- | --- | --- |
| [Goose](https://github.com/aaif-goose/goose/tree/021b0db8dbee8d6c7e9ffbab580a4143598a3560) | `021b0db8dbee8d6c7e9ffbab580a4143598a3560`, workspace `1.44.0`, Rust `1.94.1`, Apache-2.0 | GDK source, GRC, ACP, MCP, providers, packages |
| [Mesh LLM](https://github.com/Mesh-LLM/mesh-llm/tree/d91d62b72f320f9c433ab0e51c397a3c7c8ba485) | `d91d62b72f320f9c433ab0e51c397a3c7c8ba485`, workspace `0.72.1`, MIT OR Apache-2.0 | Peer inference, Goose launcher, OpenAI-compatible lane |
| [Omega](https://github.com/OpenAgentsInc/omega/tree/5230c50d003b6c26ccab1da40e8f167a6311a841) | `5230c50d003b6c26ccab1da40e8f167a6311a841`, current `origin/main` | Native loop, ACP client, router, disclosure, model provider, authority |
| OpenAgents | Research checkout `51b36e68594e4ec6b7284768ca7bb2cb07655338`, delivery base `9b82c7ecd2380bbe9b10921b198a86dbc6e000d7` | Product decisions and prior teardowns |
| [Spiral GDK announcement](https://spiralxyz.substack.com/p/spiral-x-goose-the-best-is-yet-to) | Published 2026-07-07 by Spiral lead Steve Lee | Official GDK/GRC intent and organizational change |

The OpenAgents worktree already contained unrelated uncommitted application,
UI, assurance, and semantic-baseline edits. This report does not inspect,
modify, or treat those drafts as shipped evidence.

The local Omega worktree was clean but five commits behind its fetched
`origin/main`. It was not moved. The audit inspected the local source plus the
five-commit diff and uses the current remote tip above. Those commits refine
mobile projection, tool guards, community work, and window behavior without
changing the Goose integration decision.

### Evidence labels

- **`[source]`** — tracked source, documentation, manifests, or workflows at
  the pinned revision.
- **`[schema]`** — a typed Rust, TypeScript, ACP, MCP, settings, or wire
  contract.
- **`[test]`** — an executable test, compatibility check, smoke harness, or CI
  gate present in source.
- **`[history]`** — Git history at or before the pinned revision.
- **`[public]`** — a linked official public source or package registry.
- **`[vision]`** — announced direction not fully present at the source tip.
- **`[inferred]`** — a decision reasoned from multiple observations.
- **`[limitation]`** — something the available evidence cannot establish.

### Limits

This was a source and registry audit. It did not build Goose or Mesh LLM,
install their packages, connect a provider, run a model, start an inference
mesh, mutate local Goose configuration, or execute a live Goose-to-Omega ACP
turn. It inspected tests and CI but did not run upstream build suites.

The current package registry views are point-in-time observations from
2026-07-27. Registry state can change independently of this report. No source
inspection proves that a published binary has the audited source, that all
platforms work, or that the current Omega and Goose ACP crate generations are
wire compatible. [limitation]

## 2. The name is ahead of the boundary

The official announcement says Goose is changing from one application into a
shared development platform. GDK is intended to supply:

- resumable orchestration.
- unified model access and in-process local models.
- context, tools, memory, and a flexible loop.
- ACP server and client support.
- remote execution.
- slash commands, subagents, and skills.
- automations, schedules, and routing. [public] [vision]

It names two integration paths:

1. ACP, with Goose running as another process or daemon.
2. a lower-level Rust API that embeds the agent and will later receive Kotlin,
   Swift, Python, and other bindings. [public] [vision]

The announcement is careful where the implementation is not finished: the
team **is developing** the embedded Rust API and **expects** ACP eventually to
be built on it. The current repository confirms that distinction. [public]
[source]

There is no separate `gdk` repository in the audited project set. GDK is the
program and intended architecture around parts that currently live in the
`aaif-goose/goose` monorepo. The closest package named as GDK is the Maven
coordinate `io.github.aaif-goose:gdk`. The primary Rust package is still
`goose-sdk`, and the current TypeScript package with the broadest client API is
also named `@aaif/goose-sdk`. [source]

The marketing phrase "powered by the Agent Communication Protocol" should also
not become a second term in Omega. The actual protocol, crates, commands, and
documentation use **Agent Client Protocol (ACP)**. [source]

## 3. Artifact map

| Artifact | What it is now | Maturity at the pin | Omega disposition |
| --- | --- | --- | --- |
| `crates/goose` | Full internal Goose engine: agent loop, sessions, context, MCP, permissions, providers, recipes, schedules, hooks, security | Production core inside monorepo, workspace version `1.44.0` | Do not link or vendor |
| `crates/goose-sdk` default feature set | Re-export of Goose ACP custom request and notification types | Source `0.1.0-alpha.5`, registry `0.1.0-alpha.1` | Reference schemas only |
| `crates/goose-sdk --features uniffi` | In-process provider construction and completion API for Rust/Python/Kotlin | Alpha, provider-focused | Do not adopt as Omega engine |
| `crates/goose-sdk-types` | Large generated/shared schema for Goose-specific ACP methods | Alpha, 114 request structs | Optional adapter reference |
| `crates/goose-provider-types` | Provider messages, model and usage contracts | Alpha release family | Do not duplicate Omega's model plane |
| `crates/goose-providers` | Native and declarative provider implementations | Alpha release family, used internally | Watch as a donor, not dependency |
| `ui/sdk` / `@aaif/goose-sdk` | ACP TypeScript client, generated Goose extensions, HTTP stream, binary resolver | Published `0.20.2`, used by GRC clients | Do not add Node/TS sidecar to Omega |
| `ui/goose-binary/*` | Platform packages carrying matching `goose` binaries | Published `0.20.2` for queried platforms | Useful release reference only |
| `ui/text` / `@aaif/goose` | TypeScript terminal client that starts `goose acp` | Published `0.20.1`, pins SDK `0.20.2` in source | GRC and ACP reference |
| `ui/desktop` | Electron client that starts authenticated `goose serve` and uses the workspace SDK | Active GRC surface | Reference transport/security behavior |
| `crates/goose-cli` | Native CLI, scripting commands, `goose acp`, and `goose serve`. It links core directly | Active GRC surface | External executable, not library |
| Mesh LLM | Separate P2P inference system and OpenAI-compatible API | Active, explicitly early-stage in Goose blog | Separate inference adapter |

The table reveals the central adoption trap: saying "use GDK" is not precise
enough to choose a dependency. ACP, the TypeScript client, provider UniFFI, and
the internal engine have materially different APIs, authority, packaging, and
stability.

## 4. The mature path: Goose as an ACP agent

### 4.1 Stdio

`goose acp` starts the Goose agent as an ACP server over stdio. The public
guides name initialization, session creation and loading, concurrent sessions,
prompt streaming, cancellation, model and mode switching, client file and
terminal operations, and client-supplied MCP servers. Sessions are saved into
Goose history. [source]

This is the seam Omega already understands:

```json
{
  "agent_servers": {
    "goose": {
      "type": "custom",
      "command": "goose",
      "args": ["acp"]
    }
  }
}
```

That shape is illustrative, not a shipped Omega default. A real integration
must resolve and pin an exact binary, verify initialization and capability
negotiation, classify the process as `ExternalAcp`, and preserve Goose-owned
configuration. An ambient `PATH` command is insufficient release identity for
automatic routing.

### 4.2 HTTP and WebSocket

`goose serve` exposes ACP over Streamable HTTP and WebSocket. It refuses to
start without `GOOSE_SERVER__SECRET_KEY` unless the caller explicitly supplies
`--dangerously-unauthenticated`. The server accepts the secret in
`X-Secret-Key`. WebSocket clients can use a query token because browser
WebSocket APIs cannot set custom headers. CORS and WebSocket Origin rules are
loopback-oriented by default, and remote deployment documentation recommends
TLS and certificate fingerprint pinning. [source]

Goose Desktop follows the stronger local pattern:

- generate a secret.
- bind a sidecar to loopback.
- launch `goose serve`.
- pass the token to the WebSocket client.
- probe readiness and retain a backend lease.
- pin a TLS fingerprint for remote servers. [source] [test]

This is useful precedent for Omega's remote and multi-client work, but a bearer
secret remains connection authentication, not owner admission or per-effect
authority. Omega should keep engine generation, executor identity, audience,
and authority receipts above that transport. [inferred]

### 4.3 A current TypeScript HTTP sharp edge

`GooseClient` accepts either a ready ACP `Stream` or a URL. The URL path creates
the package's Streamable HTTP client. That helper appends `/acp` and issues
`fetch` calls with ACP connection/session headers, but its public constructor
does not accept authentication headers or an authentication callback. The same
repository documents that authenticated `goose serve` requires
`X-Secret-Key`. [source] [schema]

This does not prove the package cannot be adapted—callers can supply their own
`Stream`, and Desktop uses a separate WebSocket implementation—but it means the
convenient URL API is not sufficient evidence of a complete authenticated
remote-client contract. Omega should validate the exact transport it uses
rather than inheriting the helper by reputation. [inferred]

## 5. The broadest client API is TypeScript, and mostly unstable

The published `@aaif/goose-sdk` is the most developed external client package
at this pin. `GooseClient` wraps the official ACP client connection and exposes
standard protocol methods for:

- initialization and authentication.
- new and loaded sessions.
- prompts and cancellation.
- mode and session configuration options.
- list, fork, resume, close, and model selection where ACP exposes them.
  [schema]

Its generated `GooseExtClient` adds 114 Goose-specific request types. The
families include:

- session extensions, working directory, system prompt, steering, metadata,
  archive, rename, truncation, import, export, and Nostr sharing.
- tool listing, permission updates, direct calls, resources, and MCP Apps.
- provider inventory, catalogs, custom provider CRUD, authentication, secrets,
  defaults, and preferences.
- recipes and scheduled jobs.
- sources, agent mentions, and slash commands.
- dictation and local inference model management. [schema]

Almost all of those methods are explicitly named
`_goose/unstable/...`. The generated Rust types, TypeScript types, Zod
validators, and client methods reduce transcription errors. The namespace
still declares that the contract can move. [schema]

The TypeScript SDK and matching binary packages have a compatibility smoke
test that boots the freshly built `goose acp`, initializes it, and checks the
client against the same source revision. The terminal client pins a specific
SDK version and warns contributors that a locally built binary can disagree
with the published schema. [test] [source]

That is good release discipline and also a warning for Omega:

> Goose's rich client contract is currently a matched client/binary release
> unit, not a generic stable ACP extension surface.

Omega should use standard ACP capabilities for its base adapter. Any
Goose-specific methods should sit behind:

- exact executable identity.
- a negotiated capability/version record.
- generated or fixture-checked request/response schemas.
- loss and fallback behavior.
- tests against the exact supported Goose versions.

It should never send an unstable method and silently interpret "method not
found" as success.

## 6. The Rust/Python/Kotlin SDK is provider-focused

The current `goose-sdk` crate documents itself as "the bindings layer for
Goose." With default features its public Rust surface is effectively:

```rust
pub use goose_sdk_types::{custom_notifications, custom_requests};
```

Its ACP example spawns `goose acp` and speaks to a separate agent process. The
crate does not re-export `goose::agents::Agent`. [source]

With the optional `uniffi` feature, it exposes a real in-process API, but the
boundary is model completion:

- provider messages with user, assistant, and tool roles.
- text, image, tool request, tool result, thinking, and redacted-thinking
  content.
- tool schemas.
- model configuration, context and output limits, temperature, reasoning,
  timeout, headers, and provider request parameters.
- usage and streaming events.
- typed authentication, rate-limit, context, output-limit, timeout, provider,
  and generic errors.
- a process-wide request logger.
- declarative provider construction from JSON.
- OpenAI, Anthropic, Groq, Databricks, and Databricks v2 constructors.
- streaming and non-streaming completion. [schema]

That is a useful provider abstraction. It is not an agent development kit in
the product-level sense Omega would need. Missing from the public UniFFI
surface are:

- agent-loop construction and lifecycle.
- durable session creation, loading, migration, search, and lineage.
- context compaction.
- MCP extension lifecycle and host tool execution.
- permissions and approval interception.
- recipes, hooks, schedules, subagents, and memory.
- ACP server construction.
- cancellation and recovery of a whole agent turn.
- host policy, containment, admission, and receipt injection. [source]

The internal `crates/goose` library already contains most of those systems.
That does not make them a stable GDK interface. The Goose CLI imports the
internal core directly. GRC therefore does not yet prove that an independent
Rust application can consume the complete agent through the public SDK
boundary. [source] [inferred]

## 7. Packaging is real, but fragmented

The current publication surfaces do not share one version or one level of
availability.

| Registry or artifact | Observed state on 2026-07-27 | Consequence |
| --- | --- | --- |
| Cargo `goose-sdk` | registry search `0.1.0-alpha.1`, source `0.1.0-alpha.5` | published Rust API trails source |
| Cargo `goose-sdk-types` | registry search `0.1.0-alpha.1`, source `0.1.0-alpha.5` | matched alpha family required |
| Cargo `goose-providers` / `goose-provider-types` | registry search `0.1.0-alpha.1` | explicitly early dependency surface |
| [PyPI `goose-sdk`](https://pypi.org/project/goose-sdk/) | `0.1.0a1`, released 2026-07-02. One macOS arm64 wheel. No source distribution | not a portable Python SDK release yet |
| npm `@aaif/goose-sdk` | `0.20.2` | most current external client package |
| npm `@aaif/goose` | `0.20.1` | TUI version differs from SDK/binaries |
| npm native binaries | queried Darwin arm64, Linux x64, Windows x64 at `0.20.2` | matched binary distribution exists |
| Maven `io.github.aaif-goose:gdk` | build/publish project exists. Maven Central search returned no artifact | packaging path exists. Public availability was not observed |
| Goose workspace/app | `1.44.0` | product version is a separate sequence |

The Rust crate uses `cdylib`, `staticlib`, and `rlib` outputs and requires Rust
`1.94.1`. Python wheels are generated through UniFFI. Current workflow source
has manual build-and-optional-publish matrices for macOS arm64/x64, Linux x64,
and Windows x64. Maven builds native libraries for Darwin arm64, Linux x64,
and Windows x64 and packages them into a JVM artifact. [source]

The workflows are materially better than a single developer wheel: they smoke
import the Python package, package native libraries, and gate publication
behind environments. npm publishing builds Goose binaries for five targets,
runs an SDK/binary compatibility check, and publishes with provenance.
[source] [test]

The adoption conclusion is still "watch":

- source `alpha.5` is not the generally available Cargo/Python artifact.
- Python's observed release is one architecture.
- Maven's claimed coordinate was not discoverable in the queried central
  index.
- Swift, named in the vision, is not a current binding.
- the broad TypeScript client is on an unrelated version sequence and depends
  on unstable custom methods. [public] [source] [vision]

## 8. GRC is three reference patterns, not one

The public announcement renames the existing Goose application the Goose
Reference Client. It says meaningful GDK releases should be reflected in GRC
and names Desktop and CLI as the user-facing forms. [public]

The repository currently demonstrates three implementation patterns:

| GRC surface | Engine connection | What it proves |
| --- | --- | --- |
| Native CLI | Links the internal `goose` crate in process | The monorepo core can support a native shell and scripting |
| TypeScript TUI | Uses `@aaif/goose-sdk`, spawns `goose acp`, and can resolve a matching npm binary | The daemon/client package boundary works and can be compatibility-pinned |
| Electron Desktop | Uses workspace `@aaif/goose-sdk`, launches authenticated `goose serve`, and uses WebSocket and backend leases | A rich desktop can be a thin client over the Goose engine |

This is valuable evidence for one engine with multiple projections. It also
shows the refactor is incomplete:

- the CLI is not consuming the public `goose-sdk` as a full embedded engine.
- the TypeScript clients depend on 114 unstable Goose extensions for rich
  behavior.
- source documentation from April describes consolidation away from the older
  `goosed`/REST split, while the current tree is still actively changing ACP
  tool-call conversion and client state. [source] [history]

GRC is therefore a strong behavioral and architectural reference. It is not
yet a proof that the announced full Rust GDK boundary is stable for a separate
product.

Omega also should not fork GRC. Omega has a native GPUI editor, project model,
language stack, diff review, terminal, agent surface, device bridge, and
authority contracts that GRC does not replace. A GRC fork would create a
second product shell and move Omega away from its accepted primary surface.

## 9. ACP and MCP play different roles

The open-protocol story is coherent when the roles remain distinct:

```text
client -- ACP --> agent
agent  -- MCP --> tools, resources, and apps
```

Goose also supports the reverse composition where it drives other ACP agents
and passes its MCP servers into them. This is one of its strongest design
lessons: clients, agents, and tools are separate roles rather than one plugin
API pretending to be all three. [source]

Omega already speaks ACP as a client to external agents and exposes a bounded
ACP server in the other direction. It already hosts external ACP agents
through `CustomAgentServer`. It does not need GDK to acquire the protocol.

The current dependency pins require caution:

- Goose pins `agent-client-protocol` `1.0` and
  `agent-client-protocol-schema` `1.1` with unstable features.
- Omega pins `agent-client-protocol` exactly `2.0.0` with unstable features.
  [source]

Package-major mismatch does not by itself prove wire incompatibility, and
Goose is listed in the ACP agent ecosystem. It does rule out claiming
compatibility solely because both sides say ACP. Omega needs a live,
version-pinned conformance run covering the methods and client callbacks it
uses. [inferred] [limitation]

MCP compatibility also does not authorize a tool. A successful MCP handshake
and a valid schema establish syntax and reachability. Omega must still decide
which identity requested the call, what resolved arguments mean, what audience
can see the result, whether containment exists, and what receipt records the
effect.

## 10. Mesh LLM is adjacent, not part of GDK

Mesh LLM is a separate peer-to-peer inference network. Each node exposes an
OpenAI-compatible API, normally at `http://localhost:9337/v1`. `/v1/models`
lists currently available exact model identifiers, and chat requests select
one through the `model` field. Mesh can pool peers, place models, and split
work across machines. [source]

Goose's April announcement calls Mesh "pretty early-stage." That warning still
matters for an Omega decision even though the repository has substantial
protocol, routing, model, plugin, FFI, benchmark, and reliability work.
[source]

### 10.1 The Goose integration is configuration composition

`mesh-llm goose`:

1. finds or starts a local Mesh node.
2. checks the model list and requires the requested model to exist.
3. chooses a model when none is supplied.
4. writes `~/.config/goose/custom_providers/mesh.json`.
5. adds a streamable-HTTP Mesh MCP extension at
   `http://127.0.0.1:3131/mcp`.
6. installs Mesh skills for Goose.
7. launches Goose Desktop or `goose session` with `GOOSE_PROVIDER=mesh` and
   the chosen model. [source]

The relationship is therefore loose and useful:

```text
Goose agent loop -- OpenAI-compatible completion --> Mesh inference API
Goose tools      -- MCP ---------------------------> Mesh management/tools
```

Mesh is not imported into `goose-sdk`, and GDK is not required to call it.

### 10.2 Mesh's validation posture is worth adapting

Mesh's live Goose smoke harness:

- uses a temporary isolated Goose root.
- discovers an exact model from the endpoint.
- writes a declarative provider.
- disables keyring use in the fixture.
- runs Goose headlessly with the developer builtin.
- requires a multi-turn coding/tool-call fixture, not merely a text reply.
  [test]

The broader agent documentation says to use an exact `/v1/models` identifier
and a tool-capable model. Dedicated probes exercise tool-call request shape,
streamed deltas, tool-result continuation, repeated turns, and soak behavior.
[source] [test]

Omega should adapt that admission law for all "OpenAI-compatible" local
providers:

> A successful `/models` or plain chat response does not qualify an endpoint
> for agent work. The exact model and endpoint generation must pass the tool
> call, continuation, cancellation, context, and failure contract Omega
> requires.

### 10.3 Omega can integrate Mesh directly

Omega already has an `openai_compatible` provider map and a native provider
implementation. A Mesh profile can point at `http://127.0.0.1:9337/v1`, store
an explicit dummy key if the current provider UI requires a non-empty key, and
list only exact models admitted from Mesh. [source]

That should remain distinct from the Goose executor:

- **Goose ACP lane:** another agent owns the loop, tools, provider choice,
  sessions, and permissions.
- **Mesh model lane:** Omega's own loop owns the turn and sends model
  completions to Mesh.

Conflating them would obscure whether Goose or Omega executed the tools and
which system owned authority.

Omega should not let a Mesh launcher silently edit its settings, install
skills, or start a background node. Those are explicit setup and lifecycle
actions with their own ownership and receipt requirements.

## 11. Reconciliation with the existing teardown set

Every prior teardown document containing a Goose or Omega reference was
included in this pass. They reinforce rather than contradict the decision.

### The broad Goose teardown

The July 17 Goose audit establishes the engine's real depth: one Rust core
serves CLI, Desktop, ACP, TUI, SDK, scheduled work, and delegated agents. It
also establishes the reasons not to inherit Goose wholesale: autonomous host
execution as a default, model judgment mistaken for policy, timer-plus-JSON
scheduling mistaken for recovery, and session durability mistaken for
receipted portable work.

GDK changes the packaging question, not those findings. The public embedded
boundary is currently narrower than the internal engine audited there.

### The ACP and product-adaptation teardowns

The T3 ACP analysis and the cross-product adaptation report both state the same
protocol law:

- wire compatibility comes from protocol version.
- optional behavior comes from capabilities.
- generated compatibility comes from the schema/SDK artifact.
- registry membership never erases a peer-specific launch, authentication,
  extension, and conformance profile.

Goose being in the ACP ecosystem is therefore a discovery fact, not an Omega
compatibility receipt. The 114 unstable Goose methods make the peer profile
more important, not less.

The adaptation report also distinguishes a real external runtime adapter from
an in-process harness emulation. A Goose-shaped prompt or tool set inside
Omega would still be Omega's runtime. Launching Goose through ACP is the path
that honestly says Goose ran.

### Buzz

The Buzz teardown supplies a useful independent Goose integration:
`buzz-acp` defaults to a Goose subprocess and can substitute other ACP agents.
Its turn accounting begins only when the agent emits Goose's
`_goose/unstable/session/update` notification with a `usage_update`. Agents
without that extension produce no metric.

That is direct evidence for two conclusions in this report:

1. Goose is already useful as an external ACP engine.
2. Goose-specific extensions must have explicit loss behavior. A generic ACP
   session cannot be assumed to expose Goose usage or any other private method.

### Omega, T3 Code, and the Exo teardowns

The current Omega/T3 analysis says Omega's primary gap is composition across
its native loop, external ACP agents, Full Auto, desktop, and mobile—not lack
of agent capability. Replacing the native loop with Goose would leave that
projection problem in place and add another authority.

Both Exo teardowns use the boundary this report recommends for Goose:

- external systems keep their own home, credentials, configuration, and
  native history.
- Omega uses an explicit adapter.
- the peer is a runtime dependency, not a build dependency.
- executor disclosure names the real chain.
- an OpenAI-compatible endpoint is a provider lane only after exact
  capability and lifecycle proof.

The TokenRelay teardown refers to the same Omega workroom and typed receipt
model but adds no Goose-specific integration requirement.

## 12. What Omega already has

Omega's current product purpose is a native IDE and durable workroom where
identity and authority are explicit and external agents keep their own runtime,
authentication, billing, and configuration. [source]

Relevant current mechanisms include:

- a native first-party agent loop.
- external ACP agents hosted through the project agent-server store and
  `CustomAgentServer`.
- an Omega router that can attach external ACP executors.
- typed `ExecutorDisclosure` distinguishing `NativeLoop`, `ExternalAcp`, and
  `EngineLane`, with optional provider/model and run identity.
- exact external executor IDs for routed subagents, with named refusal instead
  of silent substitution.
- explicit queue and steer semantics.
- an `omega-effectd` lane as the sole Full Auto run authority.
- an ACP server in the other direction whose exposed surface is inside the
  authority partition.
- an OpenAI-compatible model-provider implementation.
- a signed mobile command and device bridge plane.
- one accepted Omega/Zed/GPUI primary surface. [source] [test]

This means GDK does not fill a missing "agent framework" slot. Omega already
has each architectural category GDK wants to provide. Omega's current gap is
composition and complete cross-client projection, not absence of an agent
loop.

## 13. Fit and conflict matrix

| GDK/GRC capability | Omega already has | Fit | Decision |
| --- | --- | --- | --- |
| ACP agent daemon | External ACP agent-server path | High | Integrate Goose as named peer |
| ACP client SDK | Native Rust ACP client at a different crate generation | Medium | Keep the native client, and conformance-test Goose |
| Full embedded loop | Native Omega loop and router | Low, duplicative | Do not adopt now |
| Provider abstraction | Native provider traits and OpenAI-compatible provider | Low, duplicative | Do not add parallel plane |
| Context management | Native thread/model context behavior | Potential donor | Watch published modular API |
| MCP tool system | Native tools plus MCP/ACP infrastructure | Medium | Interoperate at protocol edge |
| Goose sessions | Omega threads and external ACP sessions | Medium | Preserve Goose-native session identity, and project loss explicitly |
| Recipes/schedules | Full Auto and admitted work direction | Conceptual donor | Do not adopt timer/JSON authority |
| Goose permissions | Omega authority and external-agent boundaries | Conflict if treated as sufficient | Treat as peer-local policy, not Omega receipt |
| GRC desktop | Omega native primary surface | Direct conflict | Do not fork or embed |
| GRC CLI | Useful external executable and test fixture | High | Pin for conformance |
| Mesh inference | Omega OpenAI-compatible provider | High | Separate optional provider lane |
| Generated unstable SDK types | Omega can generate/fixture adapters | Medium | Use only behind exact capability/version |

## 14. Why not embed the internal Goose core

Linking `crates/goose` directly could expose the complete loop earlier than the
public SDK, but it would create the worst boundary for Omega:

- a large fast-moving internal monorepo API with no external compatibility
  promise.
- a second session and persistence authority.
- a second provider, credential, model, cost, and retry plane.
- a second tool, MCP, permission, hook, and scheduling policy plane.
- a second context and compaction truth.
- release coupling to Goose's workspace Rust version and dependency graph.
- unclear mapping between Goose session identity and Omega thread/run
  identity.
- no stable injection point for Omega's admission, containment, audience, and
  receipts.
- an ambiguous product claim: the window says Omega while the embedded Goose
  loop decides effects. [source] [inferred]

Out-of-process ACP makes those differences visible and recoverable. The peer
can be upgraded, stopped, refused, or removed without turning Omega's internal
database and tool contracts into Goose contracts.

## 15. Why not use the TypeScript SDK in Omega

`@aaif/goose-sdk` is the strongest current GDK client deliverable, but Omega is
a Rust/GPUI application with a native ACP client. Adding it would require:

- a Node or browser-compatible runtime path.
- npm SDK and native-binary packages pinned as one release set.
- a JavaScript-to-Rust bridge for session updates and client callbacks.
- another process supervisor and updater.
- duplicate ACP transport code beside Omega's Rust implementation.
- adoption of Goose's unstable extension schemas to get rich behavior.

That buys less than launching `goose acp` directly through Omega's existing
server abstraction. The TypeScript package remains useful as a compatibility
oracle and as evidence for generated clients, not as Omega's runtime.

## 16. Why not adopt `goose-providers`

The provider packages are the most tangible modular code beneath GDK, and the
UniFFI work makes them attractive. Omega still should not adopt them now.

Omega already owns:

- provider registration and settings.
- credentials through its provider state.
- model identity and selection.
- request conversion and stream projection.
- rate limiting and UI error propagation.

Adding Goose providers would require a precise answer for which system owns
credentials, model metadata, tool schemas, request logs, retries, usage, and
errors. Without that answer it creates two providers with the same names and
different semantics. Mesh demonstrates that an OpenAI-compatible boundary is
the cheaper integration when an inference service is all Omega needs.

The provider crates are worth watching for a later narrow adapter only if they
become stable independently versioned packages and eliminate meaningful
provider-specific work without importing Goose's session or authority model.

## 17. Authority and security consequences

The prior Goose teardown found a capable but permissive engine whose documented
default is autonomous host execution. Goose has approvals, per-tool policies,
security inspection, Electron hardening, private state, request-scoped smart
approval fixes, and optional containment. Those are meaningful defenses. They
do not become Omega authority merely because Goose is attached through ACP.

For a Goose external executor:

- Goose owns its model, tools, extensions, loop, and local permission mode.
- Omega owns the decision to attach or route to Goose and the disclosure of
  that fact.
- an ACP permission request can ask a human. It does not prove an OS sandbox.
- Goose tool completion is evidence from the peer, not an Omega Full Auto run
  receipt.
- an external ACP thread must never be labeled `NativeLoop` or `EngineLane`.
- a model-initiated route must not acquire Full Auto authority through Goose.
- client-supplied MCP servers must remain capability-bounded. If Goose receives
  all Omega tools, this would blur the boundary the adapter is meant to
  preserve.

For Mesh:

- a remote peer can receive prompts, code context, tool schemas, and outputs.
- model placement can move within the mesh.
- exact serving peer, model material, and inference path may not be equivalent
  to a single local process.
- the OpenAI-compatible API authenticates syntax, not model provenance or data
  handling.

Omega should make Mesh's audience and placement visible and admit only the data
projection appropriate for that endpoint.

## 18. Ordered Omega plan

### Phase 0 — Record, do not adopt

Keep this teardown and the existing broad Goose audit as design evidence. Do
not add a GDK dependency, GRC fork, Mesh daemon, or shipped Goose default from
the report alone.

### Phase 1 — Goose ACP conformance fixture

Build a test-only fixture around an exact Goose release and SHA-256 digest:

1. resolve a known `goose` binary without editing the user's Goose home.
2. start `goose acp` under a temporary isolated `GOOSE_PATH_ROOT`.
3. initialize from Omega's actual ACP client.
4. record advertised protocol and capabilities.
5. create a session with an explicit working directory.
6. send a text-only prompt through a deterministic test provider.
7. exercise a client file or terminal callback without granting broad host
   access.
8. exercise tool-call updates and final stop reason.
9. cancel an in-flight prompt.
10. load or resume only where both sides advertise support.
11. verify mode/config option behavior without assuming Goose custom methods.
12. close the process and prove cleanup.

The fixture should fail with a named incompatibility when the ACP generation
or required capability differs. It should never fall back to Omega's native
loop while reporting Goose.

### Phase 2 — Optional named executor

If the fixture passes:

- offer Goose as an explicitly installed external executor.
- show its exact binary version and source/provenance state.
- classify every thread and subagent it runs as `ExternalAcp`.
- keep provider and model absent when Goose does not disclose them.
- keep Goose's home and credentials in Goose.
- require explicit human selection until routing and permission behavior has
  sufficient evidence.
- preserve the peer-native session identifier beside Omega's loss-accounted
  projection.

Do not add Goose to Omega's automatic detected-agent preference list merely
because a `goose` terminal command exists. Detection, supported ACP identity,
and automatic routing are separate decisions.

### Phase 3 — Narrow Goose extensions

Adopt no `_goose/unstable/*` method until a product outcome requires it. For
each selected method:

- pin compatible Goose releases.
- generate or vendor only the exact schema under an owned adapter.
- negotiate the capability.
- test success, method-not-found, malformed response, timeout, and downgrade.
- classify persistence and authority effects.
- record the loss from Goose's richer state into Omega's portable thread.

Session steering is the most plausible first extension because Omega already
has an explicit steer law. Provider-secret CRUD, config mutation, schedule
creation, direct tool calls, and Nostr sharing should remain Goose-owned unless
separately admitted.

### Phase 4 — Direct Mesh inference experiment

Use Mesh through Omega's existing OpenAI-compatible provider in a local,
test-only profile:

1. start or attach to an explicitly selected Mesh endpoint.
2. query `/v1/models`.
3. choose an exact model ID.
4. record endpoint, model, mesh generation, and local/remote disclosure.
5. verify chat, streaming, tool call, tool result continuation, cancellation,
   context limit, timeout, and error mapping.
6. run a bounded coding fixture.
7. stop only a Mesh process Omega itself started.

Do not use `mesh-llm goose` for this experiment. That command tests Goose over
Mesh, not Omega over Mesh, and it mutates Goose configuration.

### Phase 5 — Re-evaluate embedded GDK

Re-open the embedded decision only when all of these are true:

- the full `Agent` loop is exported from a documented public Rust package.
- the package is beyond alpha and has an explicit compatibility policy.
- the published crate matches source and supports Omega's release targets.
- storage and session implementations are injectable.
- context, memory, provider, tool, and permission components can be selected
  independently.
- host tool execution has an interception point before effects.
- cancellation and shutdown are deterministic.
- observability does not require a process-global singleton.
- Omega can supply its own policy, containment, audience, and receipt hooks.
- GRC consumes the same public embedded boundary rather than a monorepo-only
  shortcut.
- the resulting dependency removes more code and authority than it duplicates.

Until then ACP is the intended stable architectural boundary.

## 19. Acceptance packet

No proposal to ship Goose or Mesh in Omega should close without the following
evidence.

### Goose

- exact repository commit, release, executable digest, and packaging source.
- ACP crate/wire compatibility matrix against the supported Omega release.
- initialization and capability transcript with secrets removed.
- new/load/resume/cancel/close behavior for supported methods.
- working-directory and client callback proof.
- tool-call update and permission-request proof.
- executor disclosure showing `ExternalAcp` before, during, and after restart.
- no silent fallback to native loop.
- process cleanup and orphan recovery.
- explicit list of Goose custom methods used.
- migration/downgrade test for every custom method.
- audience projection and export loss accounting.
- confirmation that no Goose credential or home state was copied into Omega.

### Mesh

- exact repository/release and daemon digest.
- explicit endpoint and whether it is local, private mesh, or public mesh.
- exact model ID from `/v1/models`.
- tool-call and continuation reliability result.
- streaming, cancellation, timeout, and context-pressure result.
- model/peer placement disclosure or an explicit statement that placement is
  not provable.
- prompt/context audience decision.
- lifecycle receipt for a process Omega started.
- proof that no launcher silently edited Omega, Goose, or another agent's
  configuration.

## 20. Watch list

The GDK project is moving quickly. Re-check these specific signals:

- whether `goose-sdk` begins exporting the complete `Agent` lifecycle.
- whether GRC CLI migrates from the internal core to that public API.
- whether ACP server implementation becomes a consumer of the public Rust API,
  as announced.
- whether Swift bindings appear.
- whether Cargo, PyPI, Maven, and npm versions converge or publish an explicit
  compatibility matrix.
- whether Python gains all supported wheels and a source distribution.
- whether Maven Central contains the claimed GDK coordinate.
- whether Goose-specific ACP methods graduate from `_goose/unstable`.
- whether the TypeScript HTTP helper gains an authenticated transport contract.
- whether Goose and Omega converge on compatible ACP protocol generations.
- whether storage, permission, tool execution, and request logging become
  injectable rather than process-global or engine-owned.
- whether Mesh proves stable tool use, model identity, placement, privacy, and
  recovery under real multi-peer loss.

## 21. Final answer

GDK is not vaporware: Goose already has a substantial engine, a real
multi-client ACP architecture, generated schemas, matching binary packages,
provider bindings, and a working reference client. Its organizational move
into Spiral and foundation stewardship make it a credible open ecosystem bet.

It is also not yet the fully featured embedded development kit described in
the announcement. The current public Rust/Python/Kotlin SDK is an alpha
provider boundary. The mature product-integration boundary is the Goose
process over ACP, with the TypeScript SDK and matching binary as the best
current reference implementation.

For Omega, that is good news. Omega does not need to wait for GDK or contort
itself around an alpha library. It already has the correct seam:

- Goose can join as an external ACP executor with honest identity and
  Goose-owned state.
- Mesh can join separately as an OpenAI-compatible inference provider.
- Omega remains the native workroom, router, audience projection, and
  authority boundary.

Use GDK **at the protocol edge now**, learn from its generated and
one-engine/many-client architecture, and reconsider its embedded components
only when the public API catches up to the announced platform.
