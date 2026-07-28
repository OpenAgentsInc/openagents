# Mesh LLM, Buzz shared compute, and Omega teardown — 2026-07-27

Commit-pinned source audit of Mesh LLM, the current Buzz integration, and the
available Omega adoption seams.

This report is separate from the broader
[Buzz teardown](./2026-07-21-buzz-teardown.md) and the
[Goose Development Kit report](./2026-07-27-goose-gdk-omega-teardown.md).
Those reports remain source evidence. This report answers three focused
questions:

1. What is Mesh LLM in source today?
2. How does Buzz use it in the current product?
3. How should Omega use it, if at all?

## Executive decision

**Mesh LLM is an inference fabric, not an agent kit and not a compute
market.** It can pool model capacity across machines and expose that capacity
through one OpenAI-compatible endpoint. It can run a complete model on one
node, route a request to another node, or split model layers across nodes. It
also has an experimental Mixture-of-Agents route.

**Buzz proves a serious private-community integration, but it also shows the
cost of embedding the SDK.** Buzz does not expose the public Mesh discovery
path. It embeds the Rust SDK in the Tauri process, maps Buzz membership to a
Mesh owner allowlist, validates signed endpoint material, supervises the local
OpenAI ingress, and gives `buzz-agent` a private provider named
`relay-mesh`. Buzz also owns recovery, model selection, and a best-effort
serving-usage view.

The integration has three important limits:

- Buzz pins a Mesh side-branch revision, not current Mesh `main`.
- A pending SDK start can force a full Buzz process restart.
- Current live multi-node and split-model tests are manual and ignored by
  default.

**Omega should use Mesh first as an external OpenAI-compatible model
provider.** Omega already has this provider shape. The first useful Mesh work
is a named setup profile and a provider conformance suite. It is not an
embedded SDK, a new agent loop, or a public compute market.

The recommended order is:

1. Attach an operator-started Mesh endpoint on loopback.
2. Prove exact model, stream, tool, error, usage, cancel, and restart behavior.
3. Add an optional private two-node profile with explicit owner admission.
4. Add route and disclosure records that state when inference left this
   machine.
5. Consider an Omega-managed external Mesh process only after lifecycle proof.
6. Consider SDK embedding only after the SDK has a stable shutdown contract
   and no branch-only integration dependency.

Omega must keep its native agent loop, tool policy, containment, work
authority, and receipts. Mesh can supply model inference. It cannot supply
those authorities.

```text
recommended first lane

Omega native agent
      |
      | OpenAI-compatible chat or responses request
      | exact model and route disclosure
      v
127.0.0.1:9337/v1
      |
      v
external Mesh LLM process
      |
      +-- local complete model
      +-- admitted private peer
      `-- admitted private Skippy split

not recommended first

Omega process -> embedded Mesh SDK -> dynamic native runtime
Omega -> public `serve --auto` mesh
Omega -> `model: "mesh"` as the default coding route
Mesh counters -> OpenAgents compute receipt or settlement
```

## 1. Snapshot, provenance, and limits

The reference repositories under `~/work/projects/repos/` were clean and
synced to their remote `main` branches before this audit.

| Artifact | Audited identity | Relevance |
| --- | --- | --- |
| [Mesh LLM](https://github.com/Mesh-LLM/mesh-llm/tree/d91d62b72f320f9c433ab0e51c397a3c7c8ba485) | `d91d62b72f320f9c433ab0e51c397a3c7c8ba485` | Current architecture, SDK, runtime, trust, API, and tests |
| [Mesh LLM v0.74.0](https://github.com/Mesh-LLM/mesh-llm/releases/tag/v0.74.0) | Tag commit `e60b2fe43aa05271569fbeff2a457133aef456a1`, published 2026-07-27 | Current public release at audit time |
| [Buzz](https://github.com/block/buzz/tree/be13b4bb9ce228b21fa3682ce75d75cba5950561) | `be13b4bb9ce228b21fa3682ce75d75cba5950561` | Current embedded Mesh integration |
| [Buzz Mesh pin](https://github.com/Mesh-LLM/mesh-llm/tree/f455d493a2ae82baf2a326e2d0fda351433b4b30) | `f455d493a2ae82baf2a326e2d0fda351433b4b30` | Exact Mesh SDK revision in Buzz Desktop |
| [Omega](https://github.com/OpenAgentsInc/omega/tree/5230c50d003b6c26ccab1da40e8f167a6311a841) | `5230c50d003b6c26ccab1da40e8f167a6311a841` | Native agent and OpenAI-compatible provider seam |
| OpenAgents delivery base | `8febe299e2e76282d672128c3cff33abcbbc97ad` | Current product and research authority |

The Mesh `v0.74.0` tag has one release-preparation commit that is not on
current `main`. Current `main` has four later commits. The root workspace still
reports version `0.72.1`. Thus, a branch name or workspace version is not an
adequate source identity. An integration must bind an exact commit and release
artifact.

Mesh uses the `MIT OR Apache-2.0` workspace license. Buzz uses Apache-2.0.
License compatibility does not remove release, notice, or artifact-provenance
work.

### Evidence labels

- **`[source]`** means tracked source, documentation, or build configuration.
- **`[schema]`** means a typed API, protocol, settings, or event contract.
- **`[test]`** means an executable test or test harness in the source.
- **`[history]`** means Git history at or before the pinned revision.
- **`[public]`** means an official release or repository surface.
- **`[inferred]`** means a conclusion from multiple observations.
- **`[limitation]`** means the evidence cannot establish the claim.

### Selected source corpus

The main Mesh evidence is:

- [README and operator summary](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/README.md)
- [mesh workflow and trust guide](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/docs/MESHES.md)
- [runtime design](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/docs/design/DESIGN.md)
- [Skippy design](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/docs/SKIPPY.md)
- [Mixture-of-Agents design](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/docs/design/MOA_GATEWAY.md)
- [Rust SDK guide](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/docs/sdk/rust.md)
- [agent integration guide](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/docs/AGENTS.md)
- [workspace manifest](https://github.com/Mesh-LLM/mesh-llm/blob/d91d62b72f320f9c433ab0e51c397a3c7c8ba485/Cargo.toml)

The main Buzz evidence is:

- [Desktop Mesh dependencies](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/Cargo.toml)
- [embedded Mesh runtime](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/mesh_llm/mod.rs)
- [membership and discovery policy](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/mesh_llm/discovery.rs)
- [identity binding](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/mesh_llm/identity.rs)
- [endpoint transport policy](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/mesh_llm/transport_policy.rs)
- [runtime recovery](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/mesh_llm/recovery.rs)
- [agent provider translation](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/managed_agents/relay_mesh.rs)
- [agent model policy](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/crates/buzz-agent/src/llm.rs)
- [serving usage projection](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/desktop/src-tauri/src/mesh_llm/usage.rs)
- [shared-compute runbook](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/docs/buzz-shared-compute-dev.md)
- [live Mesh test harness](https://github.com/block/buzz/blob/be13b4bb9ce228b21fa3682ce75d75cba5950561/crates/buzz-test-client/tests/e2e_mesh_llm.rs)

### Limits

This was a source, history, and release audit. It did not start a Mesh node.
It did not download a model or native runtime. It did not run a local, remote,
split, or Mixture-of-Agents request. It did not run Buzz.

The report inspects upstream tests but does not treat an unexecuted test as
behavioral proof. It also does not prove that a release artifact contains the
audited source. [limitation]

## 2. What Mesh LLM is

### 2.1 Product boundary

Mesh LLM pools model-serving capacity across machines. Each node exposes an
OpenAI-compatible inference API on port `9337`. It also exposes a management
API and optional web console on port `3131`. [source]

A node can have one or more roles:

- a host serves the local HTTP API.
- a worker supplies model stages or complete model capacity.
- a client supplies API access without model capacity.

One process can both serve local compute and route work to peers. Role and
model state move through mesh gossip. [source] [schema]

Mesh is not an agent runtime. It does not own an agent thread, agent tool
policy, worktree, terminal, or code-edit loop. Its built-in Goose, Claude Code,
OpenCode, and Pi commands configure those agents to use the Mesh model
endpoint. [source]

### 2.2 Request path

The normal request path is:

```text
OpenAI-compatible client
        |
        | /v1/models
        | /v1/chat/completions
        | /v1/completions
        | /v1/responses
        v
node ingress on :9337
        |
        +-- complete model on this node
        +-- complete model on a remote peer
        +-- plugin-backed endpoint
        `-- Skippy stage-split route
```

The requested `model` drives route selection. A complete local fit is the
preferred simple path. If the model is remote, Mesh can tunnel the request to a
peer. If one machine cannot hold the model, Mesh can plan a contiguous
layer-stage topology. [source]

This is a provider and placement system. An OpenAI-compatible response does not
tell the caller which topology executed unless the integration retains
additional Mesh route data. [inferred]

### 2.3 Skippy and split inference

Skippy is the embedded model-serving runtime. It is based on a patched
`llama.cpp` path and supports package-backed stages. A large model package can
contain one manifest and GGUF fragments. Each peer downloads only the stage
material it needs. [source]

For a split route, the coordinator:

1. inspects the model and available capacity.
2. assigns contiguous layer ranges.
3. starts downstream stages first.
4. waits for stage readiness.
5. publishes the stage-zero route.

This topology can make a model available when no single device has enough
memory. It also puts network latency and peer health on the inference path.
Each split therefore needs route and failure disclosure. [source] [inferred]

### 2.4 Network and protocol

Mesh uses Iroh and QUIC for peer traffic. It can use direct links or encrypted
relay transport. Published-mesh discovery uses Nostr by default. A LAN mode
uses mDNS. Private meshes use invite material. [source]

The preferred peer protocol uses protobuf over ALPN `mesh-llm/1`. A legacy
`mesh-llm/0` JSON path remains for mixed versions. The owner-control plane uses
a separate additive protocol lane. This compatibility design reduces forced
lockstep updates, but it increases the test matrix. [source]

Discovery and admission are separate:

- discovery finds a possible mesh or peer.
- invite and requirement checks control mesh entry.
- owner policy controls which owners this node trusts.
- routing chooses a live model path after admission.

An Omega adapter must preserve those as separate facts. A found endpoint is
not an admitted provider. [inferred]

### 2.5 OpenAI compatibility

The OpenAI surface is the most portable Mesh boundary. It includes models,
chat completions, text completions, and Responses. The shared frontend also
has streaming and tool-call support. [source]

Compatibility is still behavioral, not nominal. Agent use requires proof for:

- streamed text and streamed tool calls.
- tool result continuation.
- multiple tool calls and call identifiers.
- request cancellation.
- context and output limits.
- `usage` fields.
- model-list changes during a session.
- error status and error body stability.
- chat versus Responses behavior.

Omega cannot infer this support from the port or endpoint name. It must run a
conformance suite against each pinned Mesh release. [inferred]

### 2.6 SDK and embedding surfaces

Mesh has a Rust SDK for client and serving applications. It has separate API
client, API server, embedded-node, Node, FFI, Swift, and Kotlin surfaces in the
workspace. Serving applications can install and load a dynamic native runtime.
[source] [schema]

The embedded SDK gives a host more control than the HTTP provider boundary. It
also imports more obligations:

- a large Rust dependency graph.
- Iroh and peer lifecycle inside the host process.
- dynamic native runtime discovery and ABI matching.
- model downloads and cache ownership.
- fixed local ports unless the host overrides them.
- shutdown and replacement behavior.
- platform-specific release assembly.
- cross-version protocol and runtime tests.

The SDK is useful when one product must own the Mesh experience. It is not the
smallest reliable Omega seam. [inferred]

### 2.7 Mixture-of-Agents

The virtual model `mesh` is an experimental route. It fans one prompt to all
callable models in the mesh. A code arbiter can choose an agreed result. A
strong model acts as a reducer when results conflict. Tool-result turns use
the reducer path. [source]

The route needs at least two distinct models. Worker roles receive different
amounts of the real agent context. The gateway can return one worker result,
call a reducer, or use a fallback when workers fail. [source]

This changes more than placement:

- multiple models can receive the prompt and tool definitions.
- the effective model set can change with live mesh membership.
- latency includes the slowest admitted worker and possible reducer work.
- token use can exceed one model call.
- one output can contain several hidden inference branches.
- tool-call normalization adds another semantic layer.

Omega should keep `model: "mesh"` off by default. A future lab mode must show
the complete model roster, reducer, route, fallback, usage, and disclosure.
[inferred]

## 3. Trust, privacy, and release truth

### 3.1 Private and public meshes

`mesh-llm serve --auto` discovers a public mesh and can start serving local
capacity. This command optimizes for quick participation. It is not the right
default for private source code or customer data. [source]

A private mesh starts without publication and returns invite material. Mesh
requirements can also require an exact protocol generation, node version, or
release attestation policy. Requirement changes create a new mesh identity.
[source]

Omega should use a private mesh only. It should require explicit user action
before any prompt can leave the current machine. It should never convert a
configured provider into automatic compute contribution. [inferred]

### 3.2 Owner identity and admission

Mesh supports owner keys and local trust policies. A node can prefer owned
peers, require owned peers, or use an owner allowlist. Requirement-aware
meshes can use signed bootstrap material. [source] [schema]

This is useful peer admission. It is not OpenAgents work authority. An admitted
model peer has permission to serve inference in that mesh. It does not gain
permission to execute an Omega tool, modify a project, accept an outcome, or
receive settlement. [inferred]

### 3.3 Release attestation is not runtime attestation

Mesh documentation states this boundary clearly. A valid embedded release
attestation proves that a trusted signer published a binary. It does not prove
that the remote process still runs those bytes. It does not attest the host OS,
hardware, loaded model, or result correctness. [source]

An invalid stamped binary can also follow the normal startup path when mesh
policy does not require certification. SDK and other native artifacts are
outside the executable attestation. [source]

Omega must record release proof as one provenance field. It must not display
that proof as remote runtime integrity.

### 3.4 Local HTTP authentication

The local OpenAI examples use no meaningful API secret. Client integrations use
a placeholder key when a library requires one. The normal safety boundary is
loopback binding. [source]

Thus:

- `127.0.0.1:9337` is acceptable only as a local provider boundary.
- `--listen-all` expands reachability but does not add authentication.
- a placeholder bearer key is not a credential.
- remote access needs a separate authenticated transport and policy.

Omega must never treat network reachability as provider authority. [inferred]

### 3.5 Prompt and topology disclosure

End-to-end relay encryption protects traffic from a transport relay. It does
not hide inference data from the serving runtime that must process it.

A complete-model provider receives the request needed for inference. In a split
route, the first stage processes prompt tokens and later stages process hidden
state. The exact exposure depends on the topology. A Mixture-of-Agents route
sends context to multiple model workers. [source] [inferred]

Omega should disclose:

- whether inference stayed local.
- whether a remote complete-model peer served it.
- whether a split route used several peers.
- whether a Mixture-of-Agents route used several models.
- the requested and effective model identities.
- the trust and release-proof state.
- any route change during the thread.

## 4. How Buzz uses Mesh LLM

### 4.1 Build and dependency boundary

Buzz Desktop has an optional Cargo feature named `mesh-llm`. It enables seven
optional dependencies, including the SDK, host runtime, client, node, system,
events, and Iroh crates. A normal local `just dev` build keeps this feature off.
The macOS release and signed canary paths enable it. The Windows canary path
does not include Mesh. [source]

Buzz pins every Mesh crate to:

```text
f455d493a2ae82baf2a326e2d0fda351433b4b30
```

That revision is not on current Mesh `main`. It is based on `v0.73.1` with
three later fixes. The fixes separate owner-control listeners, keep client-only
nodes out of elections, and update a configuration fixture. At audit time, the
pin has four commits that are absent from Mesh `main`. Mesh `main` has 74
commits that are absent from the Buzz pin. [history]

This is a major integration fact. Buzz depends on exact behavior that was not
available from the current release line when the pin was selected. Any Buzz
upgrade must reconcile its local lifecycle and admission assumptions against
new Mesh behavior.

### 4.2 Embedded process topology

Buzz embeds Mesh in the Tauri process. It does not start a separate
`mesh-llm` daemon. One process-wide slot owns either a serving node or a client
node. The node opens the normal `9337` and `3131` ports. [source]

On first use, Buzz resolves or installs the signed native runtime and reports
download progress to the UI. Serving startup has a 180-second timeout. The
runtime uses large Tokio thread stacks for deep Mesh futures. [source]

The embedded shape is:

```text
Buzz Desktop Tauri process
      |
      +-- Buzz membership and status coordinator
      |
      +-- embedded Mesh SDK
      |       |
      |       +-- dynamic native Skippy runtime
      |       +-- Iroh QUIC endpoint
      |       +-- :9337 OpenAI ingress
      |       `-- :3131 management API
      |
      `-- buzz-acp -> buzz-agent -> :9337/v1
```

The UI and agent path therefore share one Mesh lifecycle. This gives Buzz one
integrated experience. It also means a Mesh failure can become an application
lifecycle problem. [inferred]

### 4.3 Buzz does not use public Mesh discovery

Buzz hard-codes Mesh publication off and automatic public join off. It does not
give Mesh a public Nostr relay list. A stable mesh name comes from the Buzz
relay origin, but the origin itself is not exposed in the name. [source]

Buzz can use Iroh relay transport. The local
`BUZZ_MESH_IROH_RELAYS` setting selects default managed relays, direct-only
mode, or a custom allowlist. Those relays carry encrypted transport. Buzz relay
status does not carry prompts or completions. [source]

This split is important:

```text
Buzz relay
  membership, signed status, model and endpoint discovery

Iroh and Mesh
  inference request, model route, token stream
```

The Buzz relay is the admission projection. It is not the inference data plane.

### 4.4 Identity binding

Buzz has two identity systems in this path:

- a Nostr secp256k1 key identifies a Buzz member.
- an Ed25519 Mesh owner key identifies one Mesh runtime owner.

Buzz stores the Mesh owner key in the Mesh owner keystore. A kind `30003`
status event contains the owner identifier, verifying key, models, targets, and
endpoint tokens. The event has an outer member signature. Inner signatures
bind the Buzz member key to the Mesh owner and its endpoint set. [source]

Buzz accepts the status only when:

1. the reporter is a current relay member.
2. the owner identifier matches the verifying key.
3. the owner binding signature is valid.
4. the endpoint binding signature is valid.
5. the endpoint token passes transport policy.
6. the status is fresh enough for routing.

This design prevents a relay member from advertising an unrelated Mesh owner
or an altered endpoint list without the owner key. It does not prove model
correctness or runtime integrity. [source] [inferred]

The status coordinator refreshes a running-node event every 45 seconds.
Routing ignores status that is more than 120 seconds old. The roster loop
polls every 60 seconds, and join reconciliation runs every 15 seconds. These
cadences bound normal discovery and revocation delay. They do not make a relay
event an inference-health fact. [source]

### 4.5 Roster to owner allowlist

Buzz reads the current NIP-43 member roster. It intersects that roster with
valid owner-bound status events. The result becomes a Mesh owner allowlist.
The local owner is also included. [source]

Startup fails closed. If Buzz cannot obtain an authoritative roster, the node
starts self-only. A transient later query failure keeps the current allowlist.
Roster growth can trigger an immediate restart. Roster shrink must appear in
two observations before restart. [source]

The restart is necessary because the pinned Mesh trust store is fixed at node
start. Buzz therefore converts a changing community roster into a series of
immutable process-local trust sets.

This is a strong reference for fail-closed membership projection. OpenAgents
should not copy NIP-43 as canonical authority. It should derive Mesh admission
from its own enrolled-device and work-grant authorities. [inferred]

### 4.6 Endpoint validation

Buzz accepts signed Mesh bootstrap material with bounded size and structure.
It checks token signatures and bounds endpoint and transport counts. It rejects
unspecified, loopback, link-local, multicast, and broadcast peer addresses.
It handles a port-zero placeholder only when another usable address exists.
[source] [test]

Buzz does not rewrite signed invalid material. Rewriting would break the
signature and create a different endpoint claim. This is the correct pattern.
[inferred]

### 4.7 Share-compute experience

Buzz exposes a **Share compute** setting. The user selects an exact model and
can set a VRAM limit. The UI ranks models against local hardware. Buzz stores
the sharing preference and restores it at startup. [source]

A serving node can still route the local user's agent to another Mesh model.
Stopping share mode stops only the serving role. It does not stop a client that
an active agent needs. [source]

Switching from client to server can require an application restart. The pinned
SDK can leave its port-owning runtime active after the Buzz-side start future
is no longer controllable. Buzz uses process restart as the final cleanup
boundary. [source]

This is the strongest argument against an embedded Omega integration today.
The external-process boundary makes port, ABI, crash, and restart ownership
more explicit. [inferred]

### 4.8 How a Buzz agent consumes shared compute

`relay-mesh` is a Buzz provider name. It is only valid for the local
`buzz-agent` harness. The UI changes the harness to `buzz-agent` when the user
selects this provider. It does not pass `relay-mesh` to Goose. [source]

Before agent launch, Buzz:

1. resolves a live signed target for the requested model.
2. starts or reuses the embedded Mesh client.
3. waits for the local OpenAI ingress.
4. sends a small real chat request as a readiness proof.
5. launches `buzz-agent` through `buzz-acp`.

The provider translation sets:

```text
BUZZ_AGENT_PROVIDER=openai
OPENAI_COMPAT_BASE_URL=http://127.0.0.1:9337/v1
OPENAI_COMPAT_API_KEY=buzz-mesh-local
OPENAI_COMPAT_API=chat
BUZZ_AGENT_MAX_OUTPUT_TOKENS=4096
BUZZ_AGENT_THINKING_EFFORT=none
```

It also sets the selected model and removes ambient OpenAI credential
confusion in the launch path. The key value is a placeholder. Loopback and the
embedded lifecycle are the actual local boundary. [source]

Thus, Buzz uses Mesh through two interfaces:

- the Rust SDK owns lifecycle, membership, and serving.
- the local OpenAI API owns agent inference.

The agent does not call the Rust SDK directly.

### 4.9 Buzz auto mode and Mesh Mixture-of-Agents

Buzz adds policy above Mesh `auto`. When the user selects `relay-mesh` with
model `auto`, `buzz-agent` can switch to the experimental `mesh` virtual model.
It does so only after the catalog advertises `mesh` and at least two physical
models. It requires stable observations before the switch. [source] [test]

Buzz caches the catalog briefly. A Mesh-specific failure or malformed tool
output starts a cooldown. Buzz then retries with normal `auto`. A long-running
agent can make this choice without a process restart. [source] [test]

This is not transparent provider routing. It is a second policy layer:

```text
Buzz provider choice: relay-mesh / auto
        |
        +-- Mesh virtual model available and stable -> model `mesh`
        |
        `-- otherwise -> model `auto`
```

The user-facing word `auto` can therefore mean one model route or a
multi-model arbitration route. Omega should not copy that ambiguity. A
multi-model route must have its own explicit mode and receipt shape.
[inferred]

### 4.10 Recovery

Buzz probes `/v1/models` because that is the interface agents use. It does not
use management API readiness as the final agent-health fact. [source]

The recovery logic distinguishes:

- a live ingress.
- a closed port.
- a bound but unhealthy port.
- a startup task that has no control handle.
- a stopped runtime whose port is not released.

A foreground start can evict a clearly dead runtime. The background watchdog
requires repeated failures. Buzz attempts a bounded stop and then waits for
port release. It does not start a replacement while the old port is still
owned. [source] [test]

If the SDK start is pending without a control handle, Buzz requests an
application restart. For running Mesh-backed agents, the watchdog can re-arm a
client and stores a scoped sentinel error. It clears only errors that the Mesh
recovery path owns. [source]

These are good lifecycle laws. They also reveal that the embedded SDK boundary
does not yet give Buzz complete deterministic cleanup.

### 4.11 Usage and contribution truth

Buzz now shows local serving activity. It reads Mesh status and projects:

- current and peak in-flight requests.
- total requests and completion tokens.
- recent tokens per second.
- local, remote, and endpoint attempts.
- visible peer count.

The UI can say that another member is using this machine. This is useful
operator feedback. [source]

The projection is best effort and process-local. Missing fields become zero.
It does not identify the requesting member. It does not record GPU time,
energy, queue time, model artifact, route stages, prompt digest, result digest,
or accepted outcome. It is not signed and it is not exactly once. [source]

Buzz also has requester-side agent usage records. Those records describe the
agent turn and provider token counts. They do not attribute work to each Mesh
supplier. [source] [inferred]

Therefore, Buzz shared compute is a trusted compute commons. It is not a
compute market and it has no settlement-grade contribution ledger.

### 4.12 Test posture

Buzz has extensive deterministic unit tests for identity, membership,
transport policy, catalog behavior, model policy, lifecycle, recovery, and UI
states. It also has a Mesh feature test command and a Playwright shared-compute
suite. [test]

The multi-node trust and inference harness is ignored by default. It needs a
live membership-gated relay, Mesh-enabled desktops, model artifacts, and
hardware. The split-model test remains a manual runbook row. [test]

Current source therefore proves much of the policy logic. It does not provide
default continuous proof for:

- two physical Buzz nodes.
- one remote full-model agent turn.
- one real split-model turn.
- peer loss during streaming.
- membership revocation during a live route.
- signed release behavior across every supported platform.

The product claim must stay narrower than those missing runs. [limitation]

## 5. Buzz lessons to adapt

Omega should adapt these mechanisms:

| Buzz mechanism | Omega adaptation |
| --- | --- |
| Public Mesh publication is always off | Private and explicit Mesh profile only |
| Member key binds a separate Mesh owner | Bind an enrolled device or provider grant to the Mesh owner |
| Startup without a roster becomes self-only | Fail closed when provider admission cannot be resolved |
| Roster shrink uses confirmation | Apply hysteresis before disruptive trust-set replacement |
| Signed endpoint material is validated without rewrite | Preserve exact signed endpoint bytes and rejection reasons |
| Readiness uses the real inference ingress | Prove one model and tool request, not only a listening port |
| Serve and client roles stop independently | Never stop capacity that an active agent still needs |
| Replacement waits for port release | Fence process generations and prove cleanup before restart |
| Usage separates local and remote attempts | Show operator contribution separately from requester use |
| Public relay and inference data planes stay separate | Keep discovery and workload data as separate authorities |

Omega should reject these copied shapes:

- NIP-43 or kind `30003` as OpenAgents authority.
- an embedded Tauri or GPUI Mesh runtime as the first integration.
- one process-global Mesh runtime slot.
- application restart as normal provider lifecycle.
- the word `auto` for both one-model and multi-model execution.
- session counters as provider receipts.
- a side-branch SDK pin without an explicit patch budget and upstream plan.

## 6. Omega fit

### 6.1 Existing seam

Omega already supports configured OpenAI-compatible providers. A provider has a
base URL, model identifiers, context limits, output limits, and capability
flags. The native Omega agent already owns the thread, project context, tools,
permission policy, and local history. [source]

This gives Omega the correct first seam:

```json
{
  "language_models": {
    "openai_compatible": {
      "mesh-llm": {
        "api_url": "http://127.0.0.1:9337/v1",
        "available_models": [
          {
            "name": "exact-model-id",
            "max_tokens": 32768,
            "capabilities": {
              "tools": true,
              "chat_completions": true
            }
          }
        ]
      }
    }
  }
}
```

This is an illustrative current configuration. It is not a shipped Mesh
profile. The model identifier and limits must come from a tested model catalog.
Omega must not assume that every model has the same tool or context behavior.

### 6.2 Provider, executor, and capacity remain separate

Omega has three executor classes:

- native loop.
- external ACP.
- engine lane.

Mesh does not add a fourth class. It supplies model capacity to the native
loop. Goose remains a separate external ACP executor even when Goose itself
uses Mesh as a model provider. [inferred]

```text
Omega executor identity          Model provider identity

native_loop / Omega Agent   +    mesh-llm / exact model
external_acp / Goose        +    Goose-owned provider choice
engine_lane / effectd       +    lane-owned admitted provider
```

The two columns must not collapse. A Mesh route says where inference ran. It
does not say which agent loop or authority executed the work.

### 6.3 Recommended phase zero: conformance lab

Before a product profile exists, run one exact Mesh release as an external
process on loopback. Use a small complete model. Keep public discovery,
automatic join, serving, plugins, and Mixture-of-Agents off.

The conformance suite must test:

1. `/v1/models` and exact model identity.
2. non-streamed and streamed chat.
3. one tool call and one tool result.
4. parallel tool-call rejection or support.
5. cancellation during prompt and generation.
6. context overflow and output-limit behavior.
7. invalid model and unavailable route errors.
8. usage fields and missing usage.
9. server restart during a thread.
10. Responses behavior as a separate profile.

Record the Mesh commit, release artifact digest, native runtime digest, model
artifact digest, backend, hardware, API profile, and test result.

### 6.4 Recommended phase one: explicit local provider

Add a named Mesh profile only after phase zero passes. The setup should:

- require a loopback URL.
- fetch the live model catalog.
- let the user select an exact model.
- store explicit capability settings.
- show that the API key is a placeholder.
- refuse unauthenticated non-loopback URLs.
- show Mesh process ownership as external.
- stop only a process that Omega started and still owns.

The initial profile should not start Mesh automatically. It should attach to an
operator-started endpoint. This keeps install, license, model, cache, and
hardware choices with the operator.

### 6.5 Recommended phase two: private two-node mesh

The next phase can add an explicit private mesh profile. Use two enrolled and
owned machines. Do not use public Nostr discovery. Use exact invite and owner
allowlist material.

The proof must include:

- one local complete-model turn.
- one remote complete-model turn.
- member removal.
- endpoint rotation.
- relay outage with direct recovery.
- remote peer crash.
- cancellation and timeout.
- restart without duplicate process or port ownership.
- route and privacy disclosure in the Omega thread.

The private profile is not complete until the user can see which machine
received the inference request.

### 6.6 Recommended phase three: managed external process

Omega can later manage a Mesh process as a child service. This remains a
separate process. The adapter should own:

- exact executable and artifact verification.
- one generation-fenced process lease.
- configuration and port allocation.
- health, degraded, stopping, and cleanup states.
- model download and cache disclosure.
- provider logs and bounded diagnostics.
- safe restart and rollback.

An external service contains native ABI and runtime crashes better than an
in-process SDK. It also gives Omega a clean deletion path.

### 6.7 Optional later phase: split models

Split inference should be a separate capability. It must not appear as normal
remote inference. The user must see:

- every participating device.
- stage topology.
- model and package digests.
- route generation.
- network requirement.
- partial failure and retry result.
- whether a fallback changed the topology.

Acceptance needs a real oversized model and at least two physical machines. A
mock route cannot prove split inference.

### 6.8 Optional lab phase: Mixture-of-Agents

Mixture-of-Agents should have an explicit mode name. It must not hide behind
`auto`. The thread receipt should bind:

- all worker model identifiers.
- role assignment.
- context projection policy.
- arbiter version.
- reducer model.
- worker failures and timeouts.
- fallback result.
- aggregate token use and latency.

Tool conformance is the release gate. Coding-agent quality depends on stable
structured tool calls, not only good natural-language answers.

## 7. Required Omega disclosure and receipts

### 7.1 Provider disclosure

For each Mesh-backed model call, Omega should retain:

- requested provider and model.
- effective provider and model.
- API profile.
- endpoint identity.
- local, remote, split, or multi-model route.
- peer owner identifiers when available.
- release-proof state and its limits.
- model and runtime artifact identities when available.
- start and end times.
- prompt, output, and usage commitments under privacy policy.
- cancellation, timeout, fallback, and route-change facts.

The UI can summarize this data. The private receipt must keep the exact facts.

### 7.2 Tool authority

Mesh returns model output and tool-call proposals. Omega still decodes each
proposal into its native typed tool intent. Omega applies its existing policy,
approval, and containment before any tool effect. [inferred]

A remote model peer receives no direct shell, filesystem, Git, MCP, or
credential authority. It can only propose work through the normal native agent
boundary.

### 7.3 Compute contribution receipt

If OpenAgents later lets a user contribute compute, the Mesh usage snapshot is
not enough. A provider receipt must bind:

- the admitted work request.
- provider and device identity.
- model and artifact identity.
- route and stage contribution.
- start, stop, queue, and compute measurements.
- output commitment.
- requester acknowledgement.
- acceptance result.
- duplicate and retry identity.
- settlement reference, if settlement exists.

This receipt must come from OpenAgents authority and observed effects. Mesh
counters can be evidence inputs. They cannot become the receipt by conversion.

### 7.4 Public compute is a separate product

Mesh can make public supply reachable. It does not define procurement,
budgets, bids, acceptance, disputes, or settlement. A public OpenAgents compute
market would need new ProductSpec, AssuranceSpec, privacy, abuse, legal, and
settlement decisions.

No Mesh adoption phase in this report authorizes that product.

## 8. Acceptance matrix

| Phase | Scope | Required evidence | Fail-closed result |
| --- | --- | --- | --- |
| 0 | One local external node | Exact release, model, chat, stream, tool, cancel, error, usage, restart | No Mesh profile |
| 1 | Named loopback provider | Setup UX, live catalog, exact capabilities, non-loopback refusal | Provider unavailable |
| 2 | Private two-node route | Identity, owner admission, remote route, revocation, outage, disclosure | Self-only or unavailable |
| 3 | Omega-managed process | Generation fence, process ownership, health, bounded stop, port release, rollback | No replacement until cleanup |
| 4 | Split model | Real large model, physical nodes, stage proof, loss and retry | Split capability unavailable |
| 5 | Mixture-of-Agents lab | Full roster, arbiter, reducer, tools, fallback, aggregate usage | Normal exact-model route only |
| 6 | Compute contribution | Content-bound provider receipt and accepted outcome | No contribution or settlement claim |

### Failure injection set

Each admitted phase should test:

- no model node.
- stale model catalog.
- invalid or oversized endpoint token.
- untrusted owner.
- membership shrink.
- direct-path failure.
- transport-relay failure.
- stage failure.
- bound but unhealthy local port.
- old process that does not release a port.
- model download interruption.
- process crash during streaming.
- cancellation after a tool proposal.
- missing or contradictory usage.
- route fallback to a different model.
- release proof that is missing or invalid.

Every timeout must produce an inconclusive or unavailable state. It must not
become a passing proof.

## 9. Options and disposition

| Option | Value | Cost and risk | Disposition |
| --- | --- | --- | --- |
| Generic external OpenAI provider | Immediate use of current Omega model seam | Needs conformance and disclosure | **Do first** |
| Named Mesh setup profile | Better catalog and capability UX | Small product and test surface | **Do after conformance** |
| Private two-node Mesh | Pools owned devices | Identity, privacy, route, and recovery work | **Evaluate next** |
| Omega-managed Mesh child process | Better lifecycle UX | Release, cache, ports, and rollback ownership | **Later** |
| Embedded Rust SDK | Deep integrated control | ABI, dynamic runtime, process lifecycle, branch pin risk | **Do not do now** |
| Default public mesh | Easy reach to spare compute | Prompt disclosure and weak operator control | **Reject** |
| Default Mixture-of-Agents | Model diversity | Hidden multi-call route and unstable tool behavior | **Reject** |
| Mesh counters as receipts | Low implementation work | False accounting and settlement claims | **Reject** |
| Replace Omega native loop | No relevant benefit | Wrong product boundary and duplicate authority | **Reject** |

## 10. Final recommendation

Mesh LLM is useful to Omega when it stays in its proper layer. It can be a
strong local and private-network inference provider. Its OpenAI-compatible
endpoint gives Omega a low-coupling start. Its owner admission and split
runtime are valuable later.

Buzz shows both the opportunity and the warning:

- the opportunity is a friendly private compute pool with strong membership
  projection and real agent use.
- the warning is a branch-pinned embedded runtime whose cleanup can require a
  full application restart.

The correct Omega move is not to import Buzz's integration. It is to adapt
Buzz's trust and lifecycle laws around an external Mesh provider:

1. exact pin.
2. loopback first.
3. private peers only.
4. inference readiness, not port readiness.
5. visible route and model truth.
6. native Omega tool authority.
7. no receipt or market claim from provider counters.
8. no SDK embedding until deterministic lifecycle evidence exists.

This report supplies research evidence only. It does not admit implementation,
provider spend, deployment, release, compute contribution, or public claims.
