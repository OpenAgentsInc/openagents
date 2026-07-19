# OpenCode V2 Architecture Teardown — 2026-07-10

Read-only analysis of the OpenCode 2.0 beta documentation at
[v2.opencode.ai](https://v2.opencode.ai/) and the open-source V2 branch, pinned
to commit
[fe09a2e9b7dde68296a6243e2aaf12ce60410e49](https://github.com/anomalyco/opencode/tree/fe09a2e9b7dde68296a6243e2aaf12ce60410e49).

This is an additive companion to the
[OpenCode Desktop source teardown](./2026-07-10-opencode-desktop-app-teardown.md),
which examined the V1 product snapshot at
[9976269ab1accfc9f9dc98a4a688c516934de422](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422).
The separate
[OpenCode Effect architecture teardown](./2026-07-10-opencode-effect-architecture-teardown.md)
drills into the service graph, scopes, Schema, structured concurrency,
persistence, plugin, observability, and test architecture across both
generations.
It asks what changed when the next-generation engine became authoritative, what
those changes reveal about lessons learned from V1, and what OpenAgents should
adapt beyond the first teardown.

Evidence labels:

- **[source]** — observed in the pinned V2 source tree
- **[schema]** — encoded in a public schema, HTTP API, OpenAPI, or database
  contract
- **[test]** — encoded in a test, generation check, or contributor guardrail
- **[public]** — stated by the deployed V2 documentation
- **[history]** — supported by commit history or a checked-in decision record
- **[inferred]** — concluded from several observations
- **[limitation]** — a boundary on what the beta evidence proves

No OpenCode source, user state, credentials, conversations, or runtime data was
modified or inspected. V2 is a beta development surface. Source and docs show
intended architecture at the pinned point. They do not prove every path is
stable, released, or migrated into Desktop.

## TL.DR

OpenCode V2 is where the server-first direction visible inside V1 becomes the
product architecture instead of a compatibility layer.

V1's key insight was that terminal, desktop, web, SDK, and remote clients should
drive one agent server. V2 keeps that insight and rebuilds around six harder
conclusions:

1. **Durable admission precedes execution.** A prompt first becomes an
   idempotent inbox fact, then enters model-visible history at a safe boundary.
   Steer and queue are explicit delivery policies.
2. **Live activity is not durable truth.** Process-local drains, durable
   lifecycle history, pending input, projected messages, and replayable events
   are different concepts.
3. **Networked and embedded are the same application.** Promise and Effect
   clients come from one HTTP contract. The embedded SDK invokes the same
   router, middleware, codecs, handlers, and errors through memory transport.
4. **Workspace context is a service scope.** Filesystem, tools, permissions,
   agents, providers, plugins, MCP, and the runner enter an explicit Location
   graph rather than depending on ambient process working directory.
5. **Mutable runtime inputs need owned generations.** Tools, plugins, catalogs,
   and instructions are scoped, replaceable, and derived from replayable facts.
6. **Recovery needs several honest mechanisms.** V2 has a durable session log,
   projections, pending input, staged conversation/file revert, compaction, and
   graceful service-restart continuation. It explicitly does not claim
   hard-crash exactly-once provider or tool execution.

The process topology changes too. The V2 TUI normally connects to a
discoverable, version-gated background service shared across invocations. A
private registration contains endpoint, PID, version, and password. Clients
health-check it, replace incompatible instances, and may instead choose an
isolated standalone server or explicit remote endpoint.

~~~text
OpenCode V2 clients
  ├─ TUI / non-interactive CLI
  ├─ generated Promise client
  ├─ generated Effect client
  └─ embedded Effect host
          │
          │ same HttpApi, middleware, codecs, handlers, errors
          ▼
managed server or in-memory router
  ├─ process-global SessionExecution coordinator
  ├─ global durable database and event log
  └─ LocationServiceMap
       └─ per-location filesystem, tools, permissions, agents,
          providers, plugins, MCP, instructions, PTY, and runner
~~~

V2 is not finished. Data may be reset. Server, client, SDK, and plugin APIs may
change. Sharing is unimplemented, several accepted config fields are inert,
cluster placement is reserved rather than built, hard-crash continuation is
unresolved, shell matching is policy rather than containment, plugins run as
trusted server code, and Electron Desktop still bundles the V1 server.

For OpenAgents, V2 adds four high-value requirements:

- make command admission durable and idempotent before dispatch.
- separate volatile events, durable event logs, and current projections.
- make local, remote, and embedded transports enter the same Effect request
  processor. And
- model instruction change, compaction, restart, and staged rewind as engine
  state rather than UI conveniences.

## 1. Identification and scope

| Field | V1 teardown snapshot | V2 snapshot | Meaning |
| --- | --- | --- | --- |
| Repository | anomalyco/opencode | anomalyco/opencode | Same MIT monorepo |
| Commit | 9976269ab1 | fe09a2e9b7 | Same-date sibling snapshots, not a linear release diff |
| Commit time | 2026-07-10 14:13 EDT | 2026-07-10 20:46 CDT | Point-in-time evidence |
| V2 branch point | — | 0e2dd4ad15, 2026-06-26 | V2 packages already existed before the beta branch |
| Branch delta | — | 414 V2-only commits versus 396 on compared dev | Both lines kept moving |
| Executable | opencode | opencode2 | V1 and beta can coexist |
| Distribution | stable/latest | @opencode-ai/cli@next | Explicit beta channel |
| Docs | opencode.ai/docs | v2.opencode.ai | Separate generated reference |

The repository contract declares packages/opencode to be V1 reference code and
routes new implementation to Core, Schema, Protocol, Server, Client, CLI, and
TUI. It fixes dependency direction and durable Session laws in prose beside the
code. [source]

The deployed [migration guide](https://v2.opencode.ai/migrate-v1.md) says the
only intentional breaks are the server and plugin APIs. Existing config,
agents, commands, skills, and other .opencode files should work through
in-memory translation. It also warns beta data may be wiped and APIs may
continue changing. [public]

V2 is therefore a deliberate engine/API break inside a user-artifact
compatibility shell.

## 2. V1 versus V2 at a glance

| Concern | V1 at the first teardown | V2 decision | Lesson |
| --- | --- | --- | --- |
| Engine ownership | Broad packages/opencode runtime plus next packages | packages/opencode is V1-only. Core/Schema/Protocol/Server are authoritative | Isolate legacy ownership |
| Local process | Desktop utility sidecar. Surface-specific lifecycle | Shared background service, standalone server, or remote endpoint | Lifecycle is a product contract |
| API | Legacy plus next routes and two client families | One authoritative Effect HttpApi generating Promise and Effect clients | Generate from the handler contract |
| Embedding | Direct paths could become a second API | Embedded SDK uses the HTTP router through memory transport | In-process is only transport |
| Workspace scope | Project/instance scope with compatibility globals | Location.Ref and LocationServiceMap own runtime services | Ambient cwd is not tenancy |
| Prompt lifecycle | Submission and loop were coupled | Admission, pending inbox, safe promotion, advisory wake | Accept and execute are distinct |
| Mid-run input | Timing-dependent behavior | Explicit steer and queue modes | Delivery belongs in protocol |
| Execution state | Status/events could stand in for liveness | Process-local activity and durable facts are separate | Never persist timeless running |
| Recovery | Session state, SSE, snapshots, migrations | Log, projections, inbox, suspension, staged revert | Failures need distinct recovery |
| Instructions | Loaded prompt/context sources | Typed values, hashes, blobs, deltas, derived rendering | Persist facts, not prompt prose |
| Tools | Several forms and adapters | One opaque type, scoped registration, captured values | Execute what was advertised |
| Plugins | Trusted plugins/hooks | Scoped generations, cleanup, replayable transforms | Hot reload needs ownership |
| Permissions | Nested per-tool shape | Ordered action/resource/effect rules | Precedence should be explainable |
| Undo | Snapshot/revert existed | Stage, inspect, commit/clear, redo baseline | Rollback should be transactional |
| Large catalogs | Direct tool advertisement | Deferred tools plus confined Code Mode | Discovery differs from authority |
| Desktop | Electron app with V1 sidecar | Still bundles V1 at this commit | Engine and product migration differ |

## 3. Package architecture becomes a dependency law

The V2 runtime is split with an enforced direction [source]:

~~~text
@opencode-ai/schema
  ├─ browser-safe wire and durable event contracts
  ▼
@opencode-ai/core              @opencode-ai/protocol
  runtime + persistence          endpoints + middleware placement
              \                  /
               \                /
                @opencode-ai/server
                  handlers + concrete middleware + router
                          │
                          ├─ @opencode-ai/client
                          │    Promise + Effect clients
                          └─ @opencode-ai/sdk-next
                               in-memory host
~~~

Schema owns public values and durable payloads. Protocol owns operations and
transport errors. Core owns runtime behavior and persistence. Server owns
handlers and concrete middleware. Client owns generated transports. SDK Next
owns composition, not another engine.

Client cannot depend on Core or Server at runtime. Its Promise entrypoint has no
Effect runtime dependency. Its Effect entrypoint depends only on Effect,
Schema, and Protocol and is browser-bundle safe. Import-boundary and
contract-identity tests enforce those claims. [source/test]

Migration debt remains visible. SDK Next is transitional, TUI imports both the
new Client and older generated SDK V2 types, V1 schemas remain for
compatibility, and Desktop builds packages/opencode. [source/limitation]

At this commit the V2 runtime/client packages contain roughly 131,700 lines of
TypeScript and 241 test files. Core carries 52 migrations. OpenAPI contains 84
paths, 98 operations, and 322 schemas. Those counts are not proof of quality,
but show this is a broad implementation rather than a sketch. [source]

## 4. From per-client child to managed local service

V2 TUI and CLI normally use a shared background service. The service publishes
a private XDG-state registration with URL, PID, version, and password. Private
service configuration, including a persistent password, is separate.
[source/public]

Discovery:

1. reads and schema-decodes registration.
2. calls /api/health with Basic auth.
3. confirms health PID matches registration.
4. enforces requested exact version. And
5. returns only a verified endpoint.

Start is idempotent: reuse healthy compatible service, authenticate and stop a
mismatch, spawn detached, then poll bounded readiness. Stop authenticates
before signaling the PID, waits, rechecks identity before escalation, and
removes registration. This protects against stale PID reuse. [source/test]

Clients choose:

- **managed** discovery/start.
- **standalone** private server tied to the client. Or
- **explicit server** validated at a supplied URL.

The TUI can restart and reconnect. Diagnostics separate client and server run
IDs and log roles. “OpenCode is broken” becomes client, service, or Location
failure. [public/source]

V1 proved a server boundary. V2 learned server lifecycle must be reusable and
inspectable rather than owned by whichever window opened first. [inferred]

Weaknesses remain:

- the credential is a mode-0600 JSON secret, not an OS capability.
- Basic auth has no client identity, scoped grant, or individual revocation.
- explicit remote version mismatch warns rather than fails closed. And
- only graceful restart continuation is designed.

OpenAgents should adapt the lifecycle record and identity checks, then use
device/process-specific capability credentials and protocol ranges.

## 5. Location is the runtime-context unit

Location.Ref explicitly routes project-sensitive API operations. Directory
identifies local placement. Optional workspaceID is reserved for future
placement. A process-global LocationServiceMap creates, caches, and evicts the
service graph. [source/schema]

Location-scoped services include filesystem, repository/VCS, agents, models,
providers, tools, permissions, plugins, MCP, instructions, skills, commands,
references, PTY, shell context, and Session runner.

A Session stores its Location. Process-global SessionExecution accepts only the
Session ID, loads the durable Session, resolves Location, enters its layer, then
invokes the runner. A caller cannot swap context for an existing Session by
passing a new path. Public debug APIs list and evict loaded Locations.
[source/test]

This is important for OpenAgents: a runtime identity should resolve a stored
typed WorkContext whose services are acquired at execution, not accept an
ambient cwd or directory on every tool call.

V2 does not yet provide distributed placement leases, epochs, or fencing.
Location scoping is the right local abstraction, not cluster safety proof.
[limitation]

## 6. One HttpApi, two clients, one embedded host

The public Effect HttpApi exposes 98 operations covering health, server,
Location, projects, agents, sessions, messages, forms, questions, permissions,
files, VCS, shell, PTY, models, providers, integrations, credentials, MCP,
plugins, skills, references, commands, generation, and events. [schema]

It generates:

- a zero-Effect Promise client with structural values, tagged declared errors,
  and one ClientError for infrastructure failures. And
- a rich Effect client with decoded brands, runtime schemas, typed failures,
  Streams, and an environment-provided HttpClient.

Per-call cancellation and headers stay outside domain input. Streaming returns
lazy AsyncIterable or Effect Stream. Generation-drift, identity, transport,
and import tests protect the seam. [source/test]

The embedded SDK builds the same Server routes, obtains HttpRouter, converts it
to an in-memory handler, and supplies that handler as client fetch. It opens no
socket but traverses routing, middleware, codecs, handlers, and errors. Closing
the Effect Scope disposes router, database, fibers, Locations, and
registrations. [source/test]

Local and remote therefore share executable request semantics, not only types.

V2 separates:

- HTTP query/command.
- volatile instance-wide events.
- replayable per-Session events.
- ticketed PTY WebSocket bytes. And
- embedded memory transport.

That is better than asking one SSE channel to be notification bus, database,
and recovery log.

## 7. Durable admission precedes execution

Session.prompt records a durable session.input.admitted event and pending row
before scheduling. Pending input is not model-visible. At a safe boundary,
promotion publishes session.input.promoted, creates the user-message
projection, and consumes pending atomically. [source/schema/test]

Caller-chosen message IDs define retries:

- exact Session, prompt, and delivery reuse reconciles.
- conflicting reuse fails. And
- already-promoted input reconciles against history and admission.

resume controls scheduling, not durability. False means admit without waking.
This gives mobile/network clients a real retry contract.

Delivery is explicit:

- **steer** promotes at the next safe boundary while the drain continues.
- **queue** waits until the Session would otherwise idle, then promotes one.

Promoted input resets the agent's step allowance. Manual compaction uses the
same pending store as a coalesced barrier, so later prompts cannot overtake it.
[source]

This is the clearest open answer in the audited products to “what does a
follow-up mean while work is active?” OpenAgents should adopt the distinction
even if it chooses different product language.

## 8. Execution liveness is process-local

V2 refuses to persist one overloaded running status.

| State | Authority |
| --- | --- |
| Pending work | durable Session pending rows |
| Visible conversation | projected Session messages |
| Execution history | durable lifecycle events |
| Current activity | process-local SessionRunCoordinator |
| Graceful restart intent | private nullable suspension timestamp |

SessionRunCoordinator owns one fiber per active Session. It joins same-Session
resumes, coalesces wakeups with a doorbell, lets different Sessions run
concurrently, and preserves late wakes arriving during settlement. Interrupt
stops locally owned work and awaits cleanup. Await-idle never starts work.
[source/test]

One execution busy period can contain several drains. A drain can contain
several Steps. Neither is a transcript boundary. The project reserves
“Assistant Turn” for a future durable unit rather than encoding it before a
real requirement exists. [source/history]

This vocabulary matters. V1 and many agents collapse provider request,
assistant response, tool loop, run, and spinner into “turn.” V2 names states
with different retry and durability rules.

## 9. Durable events, projections, and replay

V2 stores durable events per aggregate with unique event ID, aggregate ID,
monotonic sequence, type, schema version, creation time, encoded data, and
optional replay owner. Publishing validates aggregate identity, continuity,
event uniqueness, and exact replay equality. Identical replay reconciles.
divergent payload or ownership fails. Event and projections commit together.
[source/test]

The experimental Session log reads after an exclusive sequence. With follow:

1. subscribe before replay.
2. capture a watermark.
3. page durable events through it.
4. emit one log.synced marker. And
5. tail newly committed durable events.

The instance-wide event subscription is different: current native events are
volatile. Its docs explicitly say a slow consumer can overflow and events
during disconnection are missed. [public/schema]

V2 therefore has three products:

1. a durable log for causal replay.
2. bounded query projections for current state. And
3. volatile events for low-latency UI.

Clients recover through projection and log, then use live events for latency.
They do not pretend the live stream is lossless.

Replay ownership is not clustered execution ownership. V2 has no distributed
lease governing provider/tool dispatch. [limitation]

## 10. Step, attempt, tool, and retry semantics

A Step is one logical LLM call. Most contain one physical provider attempt. A
pre-output context-overflow recovery may rebuild one Step for a second attempt.
Before every Step the runner reloads history, agent, model, instructions, and
captured tools. [source]

Tool identity includes Session, effective agent, assistant-message ID, and call
ID. Local calls may start concurrently, but settlements publish serially.
Before another Step, orphan reconciliation fails calls still projected as
running from an earlier process instead of replaying ambiguous side effects.
[source/test]

Provider retry is narrow:

- only typed rate-limit, provider-internal, and transport failures qualify.
- no durable assistant content or tool evidence may exist.
- initial request plus at most four retries use bounded exponential delay.
- session.retry.scheduled records the next attempt and time. And
- retry history never causes automatic post-crash provider replay.

Retry safety is therefore a property of emitted evidence, not only error class.

## 11. Instructions are values, not persisted prose

V2 treats the model as a replica OpenCode can write but cannot inspect or edit.
Mutable privileged context must update through history without making rendered
prompt text authoritative. [history]

Instruction producers include built-ins, AGENTS.md discovery, selected-agent
skill guidance, references, MCP guidance, and API entries. Each owns:

- stable namespaced key.
- canonical typed value codec.
- read effect.
- initial/change/removal renderers. And
- distinct unavailable and removed semantics.

At a safe boundary the runner reads every source once and concurrently, hashes
canonical values, and publishes one session.instructions.updated delta only
when values changed. The event maps keys to SHA-256 hashes or “removed.”
Canonical JSON bodies live once in instruction_blob. instruction_state is a
rebuildable fold cache, not authority. [source]

Initial instructions and chronological updates are rendered while assembling a
request. Privileged prose is never persisted in the log. Clients display
changed keys. An unavailable initial source blocks the first complete delta and
leaves pending input untouched. Later unavailability retains prior value. An
observed removal can emit removal guidance. [source/test]

Completed compaction advances an instruction epoch at an exact sequence.
Current values become initial. Session movement and committed revert clear the
fold. Forks inherit parent values through a frozen parent sequence.

OpenAgents should persist typed, redacted, content-addressed inputs and
deterministic renderers, not one giant prompt string.

The blob store currently has no GC, hashes are machine-local pointers, and
Session deletion is not erasure. API instruction entries must not contain
secrets. [limitation]

OpenAgents should encrypt sensitive values, tenant-scope storage, and carry
verified bodies—not naked hashes—across Khala Sync.

## 12. Compaction is a durable barrier

V2 estimates system text, history, and tool schemas against context capacity
and output headroom. Successful compaction generates a structured summary and
retains a bounded serialized tail. Older durable messages remain. Active model
history moves to the checkpoint. [public/source]

Manual compaction is an admitted pending input. Repeats coalesce. When busy, it
runs before later prompts at a safe boundary. Success or failure settles the
barrier so work can proceed.

Provider overflow can trigger one recovery compaction only before durable
assistant or tool evidence. It rebuilds the same Step without re-promoting
input. Second overflow or any post-evidence overflow is terminal.

The docs are candid:

- token estimation is heuristic.
- V1-style tool-output pruning is absent.
- full durable history remains.
- no fallback compaction model exists.
- native continuation metadata does not cross the boundary. And
- instructions/tool schemas can dominate the window.

OpenAgents should copy barrier and epoch semantics while keeping summaries
non-authoritative and inspectable beside complete history.

## 13. One opaque tool contract and captured execution

V2 converges on one Tool.make value owning codecs, execution, and optional
model projection. Built-ins, plugins, MCP tools, and deferred tools share it.
[source]

Registration is Location- and Scope-owned:

- name is assigned at registration.
- latest active registration wins.
- closing it reveals the previous generation.
- later mutation of the caller record does nothing. And
- every model request captures the exact tool values it advertised.

The last rule closes a hot-reload race: a provider cannot advertise one
definition and later execute another under the same name.

The registry owns decode, invocation, encode, model projection, generic size
bounding, full-output retention, after-hooks, and settlement. Leaf tools
acquire services and sequence resource authorization. Failure channels remain
distinct: expected ToolFailure, interruption, defect, and invalid/unknown call.
[source/test]

OpenAgents should adopt captured definition and one settlement boundary. It
should also preserve a central compiled authority decision before leaf
execution because cross-runtime policy must be independently auditable.

## 14. Code Mode confines orchestration over deferred tools

V2 adds a private Effect-native Code Mode. It interprets a bounded JavaScript
subset rather than evaluating Node or V8 code. Programs have no ambient
filesystem, process, network, module, or application authority. Only explicit
schema-described tools are callable. [source]

Useful ideas:

- group and defer tools behind one execute dispatcher.
- show a token-budgeted catalog.
- provide deterministic catalog search with exact callable signatures.
- supervise Promise concurrency.
- copy plain data across the boundary.
- retain partial call identities on failure.
- interrupt race losers and fire-and-forget calls.
- compile OpenAPI operations while auth stays host-side. And
- skip unsupported OpenAPI operations instead of lying about them.

Nested calls still execute captured ordinary tools and leaf permissions.
execute controls dispatcher visibility, not nested authority. [source/public]

Two cautions:

1. timeout, tool-call count, and output-byte limits have no library defaults.
2. JSON Schema tools can be descriptive without runtime validation, whereas
   Effect Schema tools validate.

OpenAgents may use this for large catalogs, but should require per-run
timeout/tool/output/spend budgets, Effect Schema at owned boundaries, and one
receipt per nested call.

## 15. Permissions simplify precedence but not containment

V2 uses ordered rules:

~~~json
{
  "permissions": [
    { "action": "*", "resource": "*", "effect": "ask" },
    { "action": "read", "resource": "*", "effect": "allow" },
    { "action": "read", "resource": "*.env", "effect": "deny" },
    { "action": "shell", "resource": "git status *", "effect": "allow" }
  ]
}
~~~

Action and resource have simple wildcards. Last match wins. Global rules come
before agent rules. For multi-resource operations, deny beats ask, which beats
allow. No match asks. [public/source]

Saved “always” approvals are durable and project-scoped. They are appended as
allow rules only after configured deny evaluation, so memory cannot override
authored refusal. Rejecting one request rejects other pending requests in the
same Session. Filesystem leaves separately authorize canonical external
directories and reject relative/symlink escape. [source/test]

Security gaps:

- shell matches raw command text rather than parsed effects.
- external-directory checks cover shell cwd, not every argument path.
- shell retains host-user process, filesystem, and network authority.
- child subagents use their own permission profile, not a parent intersection.
- plugins can introduce actions and execute trusted server code.

V2 has clearer approval, not an OS sandbox. OpenAgents must keep approval,
policy, tool visibility, and effective containment as separate records.

## 16. Subagents, jobs, and human input

The subagent tool creates a child Session with parentID, agent, model
inheritance/override, fresh context, foreground/background job, and a synthetic
completion/error/cancelled message admitted into the parent. [source]

Foreground work can move into background observation. Background completion
notifies the parent without polling. Interruption cancels child execution and
job. The TUI has a subagent tab for family navigation and targeted interrupt.
[source/test]

This is better than returning only a blob from an anonymous task. The graph is
still simple parent IDs, and job state is not a full integration/delivery
ledger. [limitation]

Forms join Questions as structured human-input primitives with Session
ownership, fields, state, reply, cancellation, and Location-scoped pending
queries.

OpenAgents should:

- intersect child authority with parent delegation.
- keep completion separate from integration, commit, push, review, acceptance,
  and payment.
- persist explicit graph edges and receipt refs. And
- unify questions, approvals, and forms under one typed request envelope.

## 17. Snapshots and staged revert

V2 snapshots worktree state before a model Step and after clean completion
using a separate internal Git object database. It does not create repository
commits or intentionally move branches/index. Capture is bounded to the active
directory, tracked and non-ignored untracked files, with file-size exclusions.
[public/source]

Undo is two-phase:

1. **stage** a boundary, hide messages, restore attributed paths, and retain the
   exact pre-undo state as redo baseline.
2. **commit** when new work is admitted, or **clear** to restore messages/files.

Repeated undo widens one staged range. Redo restores the full baseline. Server
APIs expose stage, commit, and clear, so this is engine state, not TUI state.
[source/schema]

It cannot reverse databases, services, network effects, processes, Git
metadata, ignored files, external paths, interrupted steps, or concurrent
edits. Snapshots can contain secrets and undo is not erasure. [public]

OpenAgents should add reversible/irreversible effect lists, conflict detection,
dirty-worktree ownership, checkpoint receipts, and explicit review before
destructive restore.

## 18. Plugins: scoped generations and replayable transforms

V2 intentionally breaks the plugin API. A plugin exports stable ID and
setup/effect. Its context resembles a server client plus narrow transforms,
hooks, reloads, registrations, events, and options. Public Effect plugins do
not receive arbitrary Core services. [public/source]

Lifecycle is Scope-owned:

- local plugins reload from watched config.
- new generation replaces old.
- registrations release automatically.
- plugin resources return cleanup.
- one failure does not block unrelated plugins. And
- disablement replays visible state without manual undo.

The catalog decision record captures a key lesson. The team rejected arbitrary
Config mutation followed by “reload everything,” because it destroys granular
ownership. It selected replayable Location-scoped Catalog transforms:

1. sources register transforms.
2. active transforms replay in order.
3. policy applies after sources.
4. a diff commits. And
5. one updated event tells clients to refetch.

Agents, commands, integrations, references, skills, and tools use related
transform ideas. Runtime hooks intercept model request and tool boundaries.
[history/source]

This is lifecycle isolation, not code isolation. npm/local plugins execute
inside the trusted server. Suppressing install scripts does not sandbox
imported code. There is no publisher signature, capability process, OS
sandbox, or attestation. [limitation]

OpenAgents should copy scope, cleanup, transform replay, and stable IDs while
putting third-party extensions behind signatures, capabilities, isolated
execution, egress/secrets policy, and receipts.

## 19. Compatibility without eternal engine duality

V2 reads V1 global/project config and translates it in memory without rewriting
the source. Native V2 config makes explicit:

- ordered permissions.
- primary/subagent/all modes.
- model variants inside model refs.
- grouped MCP and separate catalog/execution timeouts.
- compaction keep/buffer.
- provider package/settings/headers/body.
- plural agents, commands, plugins, references, snapshots, attachments.

This preserves user assets while letting server and plugin APIs break cleanly.

Docs also mark accepted-but-inert fields:

- sharing is not implemented.
- username is not displayed.
- formatter config does not execute.
- LSP config does not start servers.
- configured instruction paths are not loaded.
- per-agent request overlays are not applied. And
- provider policy is proposed, not implemented.

OpenAgents should copy the honesty, not the inert surface. Compatibility fields
should report unsupported capability status and never imply effect.

## 20. TUI and Desktop migration status

The V2 TUI is a real new-server client. It resolves managed/standalone/remote
endpoints, uses generated clients, reconnects, rehydrates projections, pages
messages, renders pending prompts/permissions/forms/shell/subagents, restarts
the service, and reads durable Session logs. [source]

Migration is incomplete. TUI and CLI still use older SDK V2 types/clients
alongside the new Client. Contributor instructions direct new behavior to the
new client. [source/limitation]

Electron Desktop is not a V2 engine client at this commit. Its build aliases
virtual:opencode-server to packages/opencode/dist/node, starts the V1
Server.listen in a utility process, and health-checks /global/health. [source]

So:

- the earlier Desktop teardown remains correct.
- this file audits the V2 engine/TUI.
- V2 source does not prove V2 Desktop cutover. And
- OpenCode must still reconcile per-window sidecar and shared-service models.

A clean engine is not product architecture until every surface consumes it.

## 21. Security assessment

### Strong choices

- Generated network/embedded contract.
- Private service files and authenticated PID/version health.
- Location-scoped filesystem, tools, permissions, and plugins.
- Canonical path and symlink checks in owned file tools.
- Configured deny stronger than saved approvals.
- Durable admission and replay equality.
- Tool definition captured when advertised.
- Scoped plugin generation/finalization.
- Code Mode without ambient host authority.
- Provider auth hidden from model-visible OpenAPI inputs.
- Volatile and durable streams explicitly distinct.

### Residual risks

- Shell retains host-user authority.
- Raw-string permission policy is not containment.
- Child permissions can widen.
- Plugins are trusted in-process code.
- Shared Basic password is not client-scoped.
- Instruction blobs and snapshots can retain sensitive content.
- No hard-crash exactly-once or clustered fencing.
- Event replay owner is not execution owner.
- Remote mismatch warns.
- Code Mode budgets are optional.
- Promise client does not runtime-validate all valid-JSON shapes.
- Desktop remains on V1.

## 22. Verification and operations

Public seams have focused checks:

- Promise/Effect generation drift and transport equivalence.
- Schema V1 isolation, identifiers, optional encoding, and unsafe-value guards.
- Core admission idempotency, replay, projection, retry, compaction,
  instructions, restart, permissions, tools, subagents, snapshots, VCS, shell,
  and Location lifecycle.
- service discovery, version replacement, readiness, and singleton election.
- TUI reconnect, hydration, permissions, forms, subagents, pending input,
  undo, history, and terminal behavior.

The V2 specs assign authority to Protocol, Schema, Core, canonical vocabulary,
guardrails, generated clients, issues, and history. They explicitly are not a
backlog. That prevents prose from silently outranking code. [source]

The branch also shows heavy recent simplification and correction in Session
lifecycle, instructions, client generation, tool admission, plugins, and
compaction. This is evidence of learning and a warning not to copy beta types
byte-for-byte. [history/limitation]

## 23. What OpenCode learned from V1

These are grounded in docs, source, history, and explicit decisions:

### 23.1 Client/server needs process ownership

V1 established protocol. V2 adds discovery, version, auth, health, election,
standalone mode, restart, logs, and reconnect. An unowned server boundary turns
into sidecar sprawl.

### 23.2 Accept and execute need separate failure boundaries

Network retry, mobile disconnect, and active follow-ups require a durable
inbox. Advisory wake is not authority. Admitted work is.

### 23.3 Live event streams are not databases

Volatile events optimize latency. Durable log establishes causality.
Projection answers current state. Each has distinct retention and reconnect.

### 23.4 Persist minimum facts and derive presentation

Instruction prose, running status, catalog snapshots, and notices are easy to
store but hard to repair. V2 stores values and lifecycle facts, then renders.

### 23.5 Mutable registries require scope and replay

Plugins, tools, models, and config cannot mutate one global object forever.
Registrations are generation-owned and visible state replays current sources.

### 23.6 Embedded must not mean bypass

In-process SDK still traverses the server router. The fastest path is not the
least governed path.

### 23.7 Recovery must name irreversible effects

Snapshots, staged revert, inbox, graceful restart, log, and orphan settlement
cover different failures. V2 declines to claim hard-crash exactly-once.

### 23.8 Compatibility belongs at authored assets

V2 translates config/file layouts but breaks engine/plugin APIs. User
investment survives without forcing new invariants through legacy semantics.

### 23.9 Vocabulary is architecture

Step, Attempt, Turn, Execution, Drain, Settlement, History, Instructions, and
Model Context are non-overlapping. Precision makes retry and persistence
reviewable.

## 24. What OpenAgents should adapt now

These are additive to the first OpenCode teardown.

### A. Durable command admission

Before Runtime Gateway or Pylon work starts, persist client-chosen command ID,
thread/run/work context, typed input/delivery, source client, causal parent,
idempotency, authority-manifest ref, admission sequence, and time. Then
schedule. Exact retry reconciles. Conflict refuses. UI distinguishes admitted,
pending, promoted, executing, and terminal.

### B. Steer and queue

Make mid-run delivery explicit. Steer applies at the next safe boundary. Queue
waits for yield. Never infer semantics from arrival time or spinner state.

### C. Three read surfaces

Define:

1. bounded current projection.
2. replayable per-thread/run log with sequence and sync marker.
3. volatile coalesced live stream with gap/overflow rules.

Desktop and mobile recover from projection/log, then use live for latency.

### D. One request processor

Embedded Effect, local IPC/socket, remote Pylon, mobile Sync, and test transport
must invoke the same handlers, policy, transaction, events, and receipts. Only
transport and credential acquisition differ.

### E. WorkContext service graphs

Compile repo, workspace, account, tool catalog, permissions, provider, MCP,
plugin, filesystem, and containment under one typed WorkContext ref. A run
resolves stored context. Callers cannot swap it with a new path.

### F. Typed instruction value sync

Each instruction source is typed value plus deterministic renderer. Persist
redacted/encrypted content-addressed values and change refs, not opaque prompt
text. Distinguish unavailable, removed, unchanged. Sync verified bodies rather
than naked local hashes.

### G. Scoped extension generations

Every plugin/MCP/provider/tool registration has stable component and generation
ID, owning Scope, declared authority, captured advertised definition, cleanup,
replayable catalog state, and update/rollback/run receipts.

### H. Staged rewind

Rewind has stage, inspect, commit, and clear. Report reversible conversation
and files, irreversible effects, and expected worktree version.

### I. Managed local runtime record

Pylon/Runtime Gateway discovery publishes private authenticated identity,
protocol range, process generation, endpoint, readiness, build hash, and
credential ref. Stop/restart authenticates the instance and avoids stale
PID/socket confusion.

### J. Bounded Code Mode

If OpenAgents exposes model-authored orchestration:

- use a confined language, not ambient Node.
- expose only captured typed tools.
- keep credentials host-side.
- require timeout/tool/output/spend budgets.
- retain nested calls in the parent receipt.
- enforce leaf authority. And
- treat OpenAPI conversion as schema import, not authorization.

## 25. Adapt later

- Cross-machine Location placement after leases, epochs, and fencing.
- Portable log import/export after blob hydration and tenant isolation.
- General plugin hot reload after signed packages and isolated execution.
- Rewind across databases/services only with compensating transactions.
- Code Mode after ordinary typed tools and receipts are reliable.

## 26. Do not copy

- Shared Basic password as long-term client identity.
- Raw shell patterns as containment proof.
- Child authority broader than parent delegation.
- Trusted third-party npm code in the main server.
- Optional budgets for model-authored code.
- Config fields that silently do nothing.
- Durable running status conflating history, owner, and resume intent.
- Volatile events as the only reconnect path.
- Local hashes as portable sync refs without bodies and tenant scope.
- Parallel client families without deletion gates.
- Declaring migration complete while Desktop embeds V1.

## 27. Final assessment

The first OpenCode teardown established the right desktop boundary: sandboxed
local renderer, narrow native IPC, and server-owned workbench. V2 supplies the
deeper engine architecture.

Its best contribution is a coherent separation:

~~~text
durable admission
  ≠ model-visible promotion
  ≠ process-local execution
  ≠ durable lifecycle history
  ≠ current projection
  ≠ volatile UI event
  ≠ delivery/integration acceptance
~~~

OpenAgents has a stronger ambition around Blueprint governance, containment,
receipts, cross-device identity, and economic outcomes. It should take V2's
admission, scoping, replay, generation, and staged-recovery patterns and extend
them through those stronger boundaries.

The target combines:

- OpenCode V2 durable input and Location scoping.
- Codex generated protocol, sandbox, graph, and remote replication.
- Claude Code checkpoint/worktree recovery.
- OpenAgents authority manifests, execution/delivery receipts, Khala Sync, and
  public-safe Blueprint projections.

V2 is most valuable as evidence that local-first agents become easier to
reason about when they store fewer ambiguous states and give each lifecycle
boundary one owner.

## Primary source map

Public documentation reviewed:

- [V2 documentation index](https://v2.opencode.ai/llms.txt)
- [Migrate from V1](https://v2.opencode.ai/migrate-v1.md)
- [Build on OpenCode](https://v2.opencode.ai/build/index.md)
- [Generated client](https://v2.opencode.ai/build/client.md)
- [Embedded SDK](https://v2.opencode.ai/build/sdk.md)
- [HTTP API](https://v2.opencode.ai/api/index.md)
- [Agents and subagents](https://v2.opencode.ai/agents.md)
- [Permissions](https://v2.opencode.ai/permissions.md)
- [Compaction](https://v2.opencode.ai/compaction.md)
- [Undo and snapshots](https://v2.opencode.ai/snapshots.md)
- [Plugins](https://v2.opencode.ai/build/plugins.md)
- [Configuration](https://v2.opencode.ai/config.md)
- [Troubleshooting and service lifecycle](https://v2.opencode.ai/troubleshooting.md)

Commit-pinned source anchors:

- [V2 contributor and runtime laws](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/AGENTS.md)
- [Canonical runtime vocabulary](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/CONTEXT.md)
- [V2 specification authority](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/specs/v2/README.md)
- [Session contract](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/specs/v2/session.md)
- [Managed restart decision](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/specs/v2/session-restart-continuation.md)
- [Instruction sync decision](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/specs/v2/instruction-sync-proposal.md)
- [Catalog/plugin lifecycle decision](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/specs/v2/catalog-config-plugin-lifecycle.md)
- [Generated Client boundary](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/client/README.md)
- [Embedded SDK](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/sdk-next/src/opencode.ts)
- [Managed service discovery](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/client/src/effect/service.ts)
- [Server process and restart](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/server/src/process.ts)
- [Session execution](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/session/execution.ts)
- [Durable event implementation](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/event.ts)
- [Permission evaluator](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/permission.ts)
- [Subagent tool](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/tool/subagent.ts)
- [Tool contract](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/specs/v2/tools.md)
- [Code Mode](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/codemode/README.md)
- [Desktop still embeds V1](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/desktop/electron.vite.config.ts)
