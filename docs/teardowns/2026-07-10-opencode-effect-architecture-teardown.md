# OpenCode Effect Architecture Teardown — 2026-07-10

Read-only analysis of how OpenCode uses Effect across its legacy V1 runtime
and its V2 replacement architecture.

The V1 evidence is pinned to
[9976269ab1accfc9f9dc98a4a688c516934de422](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422)
and the V2 evidence to
[fe09a2e9b7dde68296a6243e2aaf12ce60410e49](https://github.com/anomalyco/opencode/tree/fe09a2e9b7dde68296a6243e2aaf12ce60410e49).
These are same-day sibling snapshots, not clean before-and-after commits.
The V1 branch already contains much of the new Core work, while the V2 branch
still retains `packages/opencode` as legacy reference code. This audit therefore
compares architectural ownership lanes—legacy `packages/opencode` versus the
V2 Core/Schema/Protocol/Server/Client/SDK/Plugin/CLI packages—not misleading
whole-branch totals.

This document complements the
[OpenCode desktop teardown](./2026-07-10-opencode-desktop-app-teardown.md) and
[OpenCode V2 architecture teardown](./2026-07-10-opencode-v2-architecture-teardown.md).
Those documents ask what the product and V2 engine do. This one asks what
Effect owns, where OpenCode extends or escapes it, what changed between
generations, and which choices OpenAgents should adapt.

Evidence labels:

- **[source]** — observed in the pinned source tree
- **[test]** — encoded in tests or contributor guardrails
- **[history]** — supported by commit history or a checked-in design record
- **[inferred]** — concluded from multiple observations
- **[limitation]** — a boundary on what the evidence proves

No OpenCode source or user data was changed. Counts below are point-in-time
navigation aids, not quality scores.

## TL;DR

OpenCode is not merely an application that calls `Effect.runPromise` around a
few operations. Effect is its application kernel.

It owns:

- service definition and dependency injection;
- resource lifetime, cleanup, and hot replacement;
- structured concurrency and interruption;
- typed failures and defects;
- schema-backed domain, wire, and persistence values;
- HTTP contracts, handlers, clients, and in-memory embedding;
- filesystem, process, HTTP, and database platform services;
- durable and volatile event flows;
- observability;
- deterministic time and service substitution in tests; and
- the native plugin API, with Promise compatibility adapted inward.

The important evolution is not “V1 used Promises and V2 uses Effect.” V1 was
already deeply Effect-based. The change is this:

~~~text
V1
  Effect services + Layers
      wrapped by several lazy ManagedRuntimes
      attached to ambient Instance/Workspace context
      cached per directory
      bridged back into Promise and callback APIs

V2
  Effect services + explicit LayerNode graph
      split into global and Location scopes
      compiled with checked dependencies and replacements
      materialized per canonical Location through LayerMap
      entered through one HttpApi request processor
      exposed natively to Effect and adapted outward to Promise
~~~

V1 proves Effect can incrementally absorb a large Promise application. It also
shows the cost of doing so without first fixing context ownership: ambient
fiber references, `AsyncLocalStorage` fallbacks, multiple managed runtimes,
directory caches, bridge code, and repeated fixes for context loss and cycles.

V2 responds by making topology a first-class internal artifact. OpenCode adds
its own typed `LayerNode` graph over Effect `Layer`, classifies services as
global or Location-scoped, checks dependency completeness and scope legality,
detects cycles, supports graph-aware replacement, and hoists shared global
services out of each Location runtime. This is the most valuable architectural
lesson in the codebase.

Its second-best lesson is boundary discipline. Browser-safe Schema contracts
sit below Core and Protocol. Protocol owns Effect `HttpApi`; Server owns
handlers and middleware implementations. Promise and Effect clients are
generated from the same contract. The embedded SDK installs a memory-backed
`fetch` into the same router instead of calling Core directly. The Effect
plugin API is canonical; the Promise plugin API is a scope-preserving adapter.

OpenAgents should adapt:

1. an explicit service-scope topology—process, WorkContext, request/run, and
   foreign-host lifetimes;
2. one canonical Schema identity per public value;
3. one request processor for embedded, desktop, remote, mobile, and tests;
4. scope-owned registrations and fibers;
5. interruption as cancellation, never an ordinary tool error;
6. native Effect internals with Promise adapters only at compatibility edges;
7. graph-aware test replacements and deterministic clocks; and
8. structured Effect logging and tracing from the application boundary.

OpenAgents should not blindly copy OpenCode's custom framework, Effect beta
pin, frequent `orDie`, trusted in-process plugins, process-global memo map, or
remaining ambient/platform escapes. The lesson is to make topology explicit,
not to make every team maintain a second dependency-injection language.

## 1. Scope and measured footprint

The legacy V1 implementation is `packages/opencode`. The current contributor
contract calls it reference-only and directs new work to Core, Schema,
Protocol, Server, Client, CLI, and related packages. [source]

At the pinned snapshots:

| Measure | V1 legacy `packages/opencode/src` | V2 current package lane |
| --- | ---: | ---: |
| Files directly importing Effect or an `@effect/*` package | 229 | 449 |
| Lines in those directly importing files | about 50,700 | about 50,600 |
| `Context.Service` occurrences | 67 | 94 |
| `Layer.*` occurrences | 138 | 206 |
| `Schema.*` occurrences | 1,785 | 4,045 |
| `Stream.*` occurrences | 88 | 176 |
| `Scope.*` occurrences | 32 | 90 |
| `Effect.runPromise` occurrences | 41 | 11 |

The V2 lane in that table is Core, Schema, Protocol, Server, Client, SDK Next,
Plugin, and CLI. It excludes the retained V1 package. Direct-import counts
understate transitive use and counts of symbol strings overstate distinct
architectural decisions. Their useful signal is directional: V2 widens Effect
across package boundaries while reducing Promise runtime exits. [source]

Both snapshots pin Effect 4 beta (`4.0.0-beta.83`) and use unstable HTTP, SQL,
process, and observability modules. That provides access to a coherent modern
stack but makes framework churn a real dependency risk. [source/limitation]

## 2. Effect is the whole application substrate

OpenCode uses Effect at four layers simultaneously.

### 2.1 Domain algebra

Services expose operations as `Effect.Effect<A, E, R>`. `Schema.Class`,
branded values, and `Schema.TaggedErrorClass` define domain data and failures.
`Effect.fn` names operations for tracing. [source]

### 2.2 Application composition

`Context.Service` declares capabilities. `Layer` constructs implementations.
V1 aggregates them into application runtimes; V2 compiles an explicit
application graph. [source]

### 2.3 Host integration

Effect platform services abstract files, paths, child processes, HTTP, SQLite,
logging, tracing, and server lifecycle. `ManagedRuntime` and platform runtimes
turn the application into CLI, server, SDK, and callback entrypoints. [source]

### 2.4 Protocol and extensions

Effect Schema and `HttpApi` define the public server contract. The current
plugin API itself accepts an Effect program whose registrations require a
`Scope`. Promise consumers are supported by an adapter around that native
model. [source]

This breadth matters. OpenCode gets the largest benefit because cancellation,
cleanup, service substitution, contracts, and observability compose across the
same execution model. Using only Effect Schema or only `Effect.gen` would not
produce the same architecture.

## 3. V1: incremental Effectification of a large application

V1's Effect adoption was substantial and incremental rather than a rewrite.
Commit history from March through June 2026 shows services moving one domain at
a time: files, formatting, skills, snapshots, tools, commands, plugins, PTY,
installation, project state, session processing, child processes, logging,
server routes, database lifecycle, runtime flags, and events. [history]

The migration established several durable practices:

- services are `Context.Service` values rather than imported singletons;
- application operations are named with `Effect.fn`;
- resources install finalizers or use `acquireRelease`;
- background work uses scoped fibers;
- platform services are preferred over direct host calls;
- `Effect.cached` provides in-flight deduplication;
- errors are Schema-tagged where callers can recover; and
- tests run Effects through supplied Layers. [source]

This is a credible brownfield migration strategy. It let OpenCode improve one
boundary at a time while keeping the product moving.

## 4. V1 runtime composition: useful bridge, expensive steady state

The legacy `makeRuntime` helper lazily constructs a `ManagedRuntime`, shares a
memo map, and exposes `runSync`, `runPromise`, `runFork`, and callback exits.
The application builds a large service layer and wraps it in `AppRuntime`.
Additional bootstrap paths create other runtimes for selected services.
[source]

That gives legacy call sites a familiar imperative facade:

~~~text
Promise/callback caller
  → AppRuntime.runPromise(service.use(operation))
  → Effect service graph
  → platform/database/process resources
~~~

As an adoption device, this is good. It centralizes provisioning and keeps
resource initialization lazy. As a permanent architecture, it has costs:

- lifecycle ownership is distributed across multiple runtime wrappers;
- a shared unsafe memo map makes freshness an implicit global concern;
- imperative call sites can repeatedly exit Effect instead of composing;
- services may appear injectable while still depending on ambient context;
- runtime disposal is less visible than an explicit application Scope; and
- callback and Promise bridges become correctness-critical infrastructure.

The V2 reduction from 41 to 11 `runPromise` occurrences in the measured lanes
is evidence of moving the runtime boundary outward. [source/inferred]

## 5. V1's hardest problem: ambient instance and workspace context

Legacy services need to know which project directory and workspace they serve.
V1 carries that identity in Effect fiber references, but compatibility paths
also read workspace state from `AsyncLocalStorage`. `makeRuntime.attach`
inspects the current fiber context, restores `InstanceRef` and `WorkspaceRef`,
and may fall back to ambient workspace context. [source]

`InstanceState` then uses a `ScopedCache` keyed by directory. It initializes one
state instance per directory, registers invalidation on disposal, and again
permits compatibility context fallback. [source]

`EffectBridge` captures fiber references plus ambient workspace context so a
Promise callback or external library callback can re-enter with the expected
identity. Its existence is honest and useful; it also documents the
architectural mismatch. [source]

The result is a split context model:

~~~text
logical request identity
  ├─ Effect FiberRef / service context
  ├─ AsyncLocalStorage workspace context
  ├─ directory-keyed ScopedCache entry
  └─ callback bridge snapshot
~~~

History contains repeated fixes for propagating instance state through fibers,
restoring logger/request/workspace context, removing fallback binds, breaking
cycles, and replacing async facades with services. That is direct evidence that
ambient compatibility is a tax, not a theoretical concern. [history]

## 6. V1 still learned the right resource rules

The context topology was imperfect, but resource handling was already mature.
Services use finalizers for subscriptions, watchers, database handles, and
processes. Background work is generally scoped. The generic Runner helper uses
`SynchronizedRef`, `Deferred`, `Fiber`, and `Scope` to serialize transitions,
coordinate waiters, and cancel active work. [source]

This distinction is important: V2 did not invent structured concurrency. It
made the ownership tree easier to see and compose.

## 7. V2's architectural reset: topology becomes data

V2 introduces `LayerNode`, a project-specific graph representation above
Effect `Layer`. A node records:

- stable name and optional service identity;
- implementation Layer;
- explicit dependency nodes;
- output and error types; and
- a scope tag. [source]

The type system verifies that the declared dependencies provide every service
required by the implementation. Runtime graph traversal deduplicates nodes,
detects cycles, and rejects unbound nodes. [source/test]

This is not ordinary dependency injection sugar. It is an application
intermediate representation:

~~~text
domain service declarations
  → LayerNode dependency graph
  → scope validation and graph rewrites
  → Layer compilation
  → Effect runtime context
~~~

The graph lets OpenCode reason about architecture before constructing Layers.
That is the central V2 Effect decision.

## 8. Why OpenCode did not use raw Layer alone

Effect Layer already models construction and requirements. OpenCode adds a
graph because it needs operations that are awkward once Layers become opaque:

- enumerate and inspect the service topology;
- enforce global-versus-Location dependency laws;
- replace a node together with its rewritten downstream graph;
- reject replacements that add errors or cross scope tags;
- detect cycles introduced through replacements;
- hoist global dependencies from a Location graph; and
- build only the graph required by a selected root. [source/test]

This solves real V1 migration failures. Tests specifically cover cycles through
replacements, conditional map construction, shared top-level services, and
composed graph results. [test]

The cost is also real. OpenCode now owns a second composition vocabulary, type
machinery around `Layer.Any`, graph traversal, identity rules, replacement
semantics, and compiler behavior. Any OpenAgents equivalent should be much
smaller and justified by concrete topology needs.

## 9. Global and Location scopes

V2 defines two node tags:

- **global** — process-wide services such as database, event log, global
  project/session coordination, HTTP platform, plugin runtime registry, and
  observability;
- **Location** — services bound to a canonical directory and optional workspace
  identity, including filesystem, config, agents, providers, plugins, tools,
  permissions, MCP, PTY, shell, instructions, and the session runner. [source]

The tag law allows Location services to depend on global services but prevents
global services from depending on a particular Location. That turns a tenancy
rule into a compile-time composition constraint. [source]

`LocationServiceMap` uses Effect `LayerMap` to materialize a fresh Location
Layer on demand. It canonicalizes the key shape, installs the bound Location
node as a replacement, hoists shared global dependencies, applies `Layer.fresh`
to the local graph, and expires idle entries after 60 minutes. Boot duration is
logged with location attributes. [source]

This is a cleaner successor to directory-keyed `InstanceState`:

~~~text
V1: ambient directory → cached state factory
V2: canonical Location.Ref → compiled service graph → scoped runtime context
~~~

V2 still has a temporary compatibility export for a default
`locationServiceMapLayer`, so migration is not complete. [source/limitation]

## 10. Scope hierarchy is the real application model

OpenCode's most reusable Effect idea is not a particular service. It is the
scope hierarchy:

~~~text
process Scope
  ├─ global services and server
  ├─ Location LayerMap entry
  │    ├─ plugin generation Scope
  │    │    └─ hook/tool registrations
  │    ├─ MCP connection Scopes
  │    ├─ watchers and refresh fibers
  │    └─ session runner services
  ├─ request / stream Scope
  └─ embedded SDK ManagedRuntime Scope
~~~

Resources are attached to the narrowest meaningful owner. Closing a plugin
scope removes its registrations. Invalidating a Location closes its watchers,
PTYs, and local services. Disposing the embedded SDK closes the router,
database, fibers, and locations. Server shutdown closes connections and runs
restart-continuity finalizers in an intentional order. [source]

This is materially stronger than cleanup methods scattered across UI stores.

## 11. Structured concurrency and interruption

V2 uses `forkScoped`, `forkIn`, `FiberSet`, `Deferred`, `Semaphore`, `Queue`,
`PubSub`, `Schedule`, `raceFirst`, and `timeoutOrElse` across core services.
Long-lived refresh loops, watchers, plugin supervisors, MCP connections, title
generation, tool work, and cleanup jobs have explicit owners. [source]

The session runner is especially deliberate:

- it tracks owned tool fibers in a `FiberSet`;
- it clears them when the provider stream is interrupted or fails;
- it uses uninterruptible masks around state settlement, while restoring
  interruptibility around external work;
- user decline becomes interruption rather than a fabricated success; and
- background subagents install interruption handlers that stop the child
  session. [source]

The tool contributor contract explicitly forbids broad cause-catching because
it would convert interruption and defects into ordinary tool failures. [source]

This is a crucial OpenAgents rule: cancellation is control flow with cleanup
semantics. It is not merely another error message to render.

## 12. Effect Schema is a package architecture, not a validation helper

V2 extracts browser-safe contracts into `@opencode-ai/schema`. The package may
define wire and storage values but cannot own services, side effects, or
host-local implementations. [source]

Its rules are unusually strong:

- one canonical exported Schema value per contract;
- Core facades re-export the same identity rather than wrapping it;
- current contracts are unversioned while retained legacy contracts are
  explicitly V1;
- public records are readonly;
- optional fields omit `undefined` unless preserving it is intentional;
- public identifiers and brands are stable and unique;
- `Schema.Any` is excluded from current contracts except documented unsafe
  compatibility boundaries; and
- tests protect identity, optional encoding, identifier uniqueness, and V1
  event exclusion. [source/test]

This avoids a common Effect failure mode: defining slightly different schemas
for database, runtime, HTTP, SDK, and UI, then maintaining conversion glue.

## 13. Package direction preserves browser and host boundaries

The current dependency law is:

~~~text
Schema ───────► Core
   │
   └──────────► Protocol ─────► Server ◄──── Core
                       │
                       └──────► Client

Client + Core + Server ───────► SDK Next composition
~~~

Schema and Protocol remain browser-safe. Server injects concrete Core service
identities into middleware positions declared by Protocol. Client cannot
depend on Core or Server at runtime. [source/test]

This lets OpenCode offer a rich Effect client without shipping the host engine
into a browser bundle. The Promise client can avoid an Effect runtime entirely.
Effect is therefore an internal architecture without becoming a mandatory
runtime tax for every consumer.

## 14. One HttpApi contract, several transports

Protocol builds the server with Effect `HttpApi`, endpoint schemas,
middleware placement, errors, and OpenAPI annotations. Server supplies
`HttpApiBuilder` handlers and concrete authorization, schema-error, Location,
Form, and Session middleware. [source]

Generated outputs include:

- a Promise client;
- a rich Effect client; and
- OpenAPI/contract metadata. [source]

The server compiles its application service graph, installs handlers and
middleware, and produces an Effect `HttpRouter`. The Node process serves that
router. The embedded SDK instead converts the same router to a Web handler,
installs that function as `fetch`, and creates the generated client against a
local synthetic URL. [source]

No alternate direct Core SDK is introduced:

~~~text
network client ─► HTTP transport ─┐
                                 ├─► same router/middleware/handler graph
embedded SDK ──► memory fetch ───┘
~~~

This preserves authentication shape, Location resolution, codecs, typed
errors, events, and cleanup across deployment modes.

## 15. ManagedRuntime is retained at the true host edge

V2 does not eliminate `ManagedRuntime`. It narrows its role.

The embedded SDK creates one runtime for the composed application router with
`Effect.acquireRelease`, extracts its router and plugin registry, and disposes
the runtime through the surrounding Scope. The remaining runtime helper also
serves narrow imperative integration points. [source]

That is the right use of `ManagedRuntime`: host an Effect application at a
foreign boundary. It should not be the normal way Effect modules call one
another.

## 16. Persistence: Effect all the way through SQLite

V2's Database service is built from an in-repo Effect/Drizzle SQLite adapter
over the generic Effect SQL client. It applies WAL, synchronous, busy-timeout,
cache, foreign-key, and checkpoint pragmas before migrations. The database
path is resolved lazily so tests and embedders can override it. [source]

The adapter preserves the Effect transaction context. Nested transaction work
discovers the current reserved connection through services; transaction scope
and cleanup are explicit, including rollback after failed deferred constraints.
[source]

Node and Bun SQLite implementations expose the same SQL service, serialize
connection acquisition with semaphores, and close native handles with
finalizers. Tests substitute in-memory SQLite Layers. [source/test]

The weakness is error policy. Database initialization and many operational
queries use `Effect.orDie`. In the measured current lane, 49 files contain
`orDie`. Some operations genuinely treat storage failure as an invariant or
fatal infrastructure defect; others collapse recoverable operational detail.
OpenAgents should define this boundary deliberately rather than inherit the
habit. [source/inferred]

## 17. Durable and volatile events share an Effect interface

`EventV2` combines:

- durable per-aggregate SQLite events and sequence reservation;
- atomic optional projection work during publication;
- replay and follow streams with a synchronization marker;
- volatile live `PubSub` channels;
- typed per-event subscriptions; and
- bounded queue adapters that fail on subscriber overflow. [source]

Finalizers shut down every PubSub and subscription. `Stream` is the public
consumption model. Durable reads page through the database; live streams are
explicitly documented as lossy across disconnects. [source]

Effect makes these mechanisms composable, but the architecture does not confuse
them. This reinforces the V2 teardown's three-surface rule: current projection,
durable replay log, and volatile live stream have different authorities.

## 18. Plugins: Effect-native inside, Promise-compatible outside

The V2 plugin API has an Effect-native contract:

~~~text
plugin.effect(host): Effect<void, never, Scope>
~~~

Plugin setup installs domain transforms and runtime hooks. Each registration is
owned by the current plugin Scope and can be disposed early. Activating a new
plugin generation serializes changes, closes replaced scopes, batches state
rebuilds, restores the previous generation if replacement fails, and emits
plugin lifecycle events. [source]

Internal built-ins—agents, provider integrations, tools, skills, and config
projections—use the same public Effect plugin model. That is an excellent
dogfood property: the extension API is not a decorative external wrapper.
[source]

The Promise API is adapted into the Effect loader. The adapter captures the
Effect context, runs registrations in the plugin Scope, converts Streams to
`AsyncIterable`, maps Effect values to wire-friendly values, wraps Promise
tools as Effect tools, and installs returned cleanup as a finalizer. [source]

The direction is important:

~~~text
Promise plugin ─► adapter ─► canonical Effect plugin runtime
Effect plugin ────────────────────────────────┘
~~~

OpenAgents should use the same direction for JavaScript compatibility. The
application core should not become Promise-first because external developers
prefer `async`/`await`.

## 19. Plugin design records reveal the future direction

The checked-in V2 plugin plan says internal and external plugins should share
one public API; Core types should remain private; registrations should be
scope-owned, ordered, independently disposable, and snapshot-based; domain
transforms should replay into fresh state; and Promise support should wrap the
same capabilities. [history]

Much of that target is already implemented. The plan is partially stale—the
Promise wrapper it describes as later work now exists—but it still reveals the
direction:

- domain state is rebuilt from ordered transforms rather than mutated forever;
- registration changes coalesce rebuilds;
- in-flight hook invocations use captured snapshots;
- post-commit events publish only after new state is visible;
- cross-domain transactions are intentionally absent; and
- plugin contexts expose purpose-built capabilities, not unrestricted Core
  objects. [source/history]

This is a useful model for catalogs, policies, renderer hosts, and runtime
adapters beyond plugins.

## 20. Tools are scoped Effect values

V2 converges built-in and plugin tools on one opaque tool value. Tool
construction captures its dependencies and schemas. Registration requires a
Scope. A model request captures the exact tool values advertised for that
request, so later registry reload cannot silently change execution semantics.
[source]

The registry owns input decoding, output encoding, bounding, persistence, and
settlement. Individual tools retain their own ordering of permission checks and
side effects, and may translate only expected typed failures. Interruption and
defects remain visible to the runner. [source]

This is Effect used as an authority-preserving execution algebra, not only an
implementation convenience.

## 21. Platform abstraction is pragmatic, not pure

V2 defines global nodes for Effect FileSystem, Path, and HTTP client and uses
Effect child-process and SQL services in central paths. That enables tests to
replace HTTP, files, and databases without global monkey-patching. [source/test]

The code also uses Bun and Node APIs directly for performance or missing
abstractions: native SQLite, selected filesystem operations, HTTP server
creation, dynamic module import, environment variables, and platform-specific
process behavior. [source]

This is a healthy lesson if made explicit. “Effect as the whole-app
foundation” does not require pretending the host does not exist. Direct host
calls should sit behind an owned service or a narrow construction boundary,
especially when they affect tests, security, or portability.

## 22. Observability is installed as a Layer

OpenCode builds structured file logging with Effect Logger, a per-process run
ID, minimum-level configuration, optional stderr output, and flattened
annotations/spans. Optional OTLP logging and tracing add deployment, version,
client, run, and service-instance attributes. [source]

Observability is provided underneath route service graphs and ordinary runtime
graphs. If OTLP setup fails, the application falls back to local logging. Heavy
OpenTelemetry modules are dynamically imported only when configured. [source]

`Effect.fn` names and `withSpan` calls then attach application operations to
that substrate. The current instrumentation is not exhaustive, but the
composition point is correct: observability is a service Layer, not logging
globals threaded manually through every domain.

## 23. Testing is where the architecture pays rent

Core's `testEffect` helper runs every test inside `Effect.scoped`, logs pretty
causes, supplies TestConsole, and chooses TestClock or live time explicitly.
At the pinned V2 snapshot, Core's test tree contains 184 files, including 159
`*.test.ts`/`*.test.tsx` files; 70 test files use `AppNodeBuilder.build`, and 51
install custom `Layer.succeed` or `Layer.effect` implementations. [source/test]

Tests replace graph nodes rather than monkey-patch modules. Examples substitute
HTTP clients, filesystem services, credentials, permissions, tool registries,
database Layers, plugin runtimes, and Location identity. `TestClock.adjust`
drives retries, expiry, debouncing, and runner timing deterministically.
[test]

The graph tests themselves are architectural verification: dependency
replacement, cycle detection, conditional construction, and scope partitioning
are executable contracts.

OpenAgents should treat this as a primary reason to use Effect. If services are
not replaceable and time/resource tests remain flaky, the architecture is not
capturing the intended boundaries.

## 24. Error model: strong types, uneven fatality policy

OpenCode distinguishes:

- expected domain failures as Schema-tagged errors;
- transport and schema failures in HttpApi contracts;
- interruption as runtime control flow;
- defects for violated invariants or unavailable internal facilities; and
- startup infrastructure failures often collapsed with `orDie`. [source]

The strong parts are typed endpoint errors, operation-level `catchTag`, defect
preservation through tools, and explicit cause logging. The weaker parts are
frequent `orDie`, selective broad `catchCause` in background supervisors, and
some raw `Error` failures in graph or process setup. [source]

A useful OpenAgents rule is:

| Failure class | Treatment |
| --- | --- |
| User/action/domain refusal | typed recoverable error and durable outcome |
| Dependency unavailable | typed operational error with retry policy |
| Cancellation | interruption with guaranteed cleanup |
| Invariant violation | defect, diagnostic, fail closed |
| Optional telemetry failure | degrade to local observability |

The table should be encoded in service contracts and tests, not only style
guidance.

## 25. Module and startup discipline

Effect-heavy TypeScript can produce slow type checking, import cycles, and
large eager graphs. OpenCode responds with contributor rules and history that
favor:

- flat modules instead of broad barrels;
- self-exported namespaces rather than alias webs;
- dynamic imports for heavy optional modules;
- branch-local imports in startup-sensitive paths;
- explicit application nodes instead of exported default Layers everywhere;
- lazily resolved database paths and service maps; and
- measured boot-duration logging. [source/history]

Several June refactors removed domain Layer exports, converted tests to nodes,
and built runtimes from the node graph. Those are signs that unrestricted Layer
composition had become difficult to govern at scale. [history/inferred]

## 26. What changed from V1 to V2

| Concern | V1 Effect architecture | V2 Effect architecture | Lesson learned |
| --- | --- | --- | --- |
| Service ownership | Large service graph behind runtime facades | Package-directed Core graph | Make ownership architectural |
| Context | FiberRefs plus ALS compatibility | Explicit Location service context | Ambient context does not scale |
| Per-project state | Directory-keyed ScopedCache | Canonical `Location.Ref` LayerMap | Scope the whole graph, not isolated stores |
| Composition | Layers assembled into app/bootstrap runtimes | Typed LayerNode IR compiled to Layers | Keep topology inspectable |
| Scope law | Conventional | Global/Location tags checked | Encode tenancy direction |
| Replacement | Layer/test-specific | Graph-aware, typed, cycle-checked | Replacement is an architectural operation |
| Runtime exits | Promise/callback facades common | Mostly at host and compatibility edges | Move `runPromise` outward |
| HTTP | Effect HttpApi grew inside legacy server | Protocol/Server/Client package law | Contracts need an owner below handlers |
| Embed | Potential direct-service paths | Memory transport through same router | In-process must not bypass policy |
| Schema | Broad in-package use | Canonical browser-safe Schema package | One identity per public value |
| Plugins | Hooks adapted into service graph | Effect-native scoped API, Promise wrapper | Native inside, compatibility outside |
| Registries | Mutable state plus migration bridges | Generation scopes and replayable transforms | Hot state needs ownership and replay |
| Tests | Effect tests and instance fixtures | Graph replacements plus deterministic services | Testability validates topology |
| Observability | Migrated from legacy logger | Effect Layer under all app routes | Install once at composition root |

## 27. Evolution timeline

The commit history supports a coherent sequence rather than a sudden rewrite:

1. **March 2026:** individual legacy domains become Effect services; scoped
   per-instance state appears.
2. **Late March–April:** instance/workspace propagation, session processing,
   child process, callback bridges, unified app runtime, Effect logging, and
   observability are added.
3. **Late April–May:** Core package extraction, Effect HttpApi, typed errors,
   SQLite lifecycle, runtime flags, and Effect-native events reduce legacy
   facades.
4. **Late May–early June:** database ownership moves to Core; Location-scoped
   config and an embedded V2 session/tool foundation appear.
5. **June 9:** a typed application Layer graph lands.
6. **June 24–29:** Schema and Protocol boundaries, tiered Layer nodes,
   Location hoisting, graph tests, replacement refinement, and node-built
   runtimes establish the current topology.
7. **Late June–July:** tools move into internal plugins; Effect and Promise
   plugin APIs, scoped generations, reload, restoration, and durable event
   refinements continue on V2. [history]

The future direction is therefore visible: less ambient compatibility, fewer
exported construction Layers, more canonical contracts, more scope-owned
generations, and one current unversioned API after V1 retirement.

## 28. Clearly incomplete or transitional directions

The pinned code also identifies what has not fully converged:

- `packages/opencode` still exists and Desktop still embeds it;
- default Location service-map exports remain for compatibility;
- current namespaces still contain some V2 naming that the Schema contract
  intends to remove;
- V1 provider/config types remain inside a current provider compatibility
  path;
- the plugin design plan and implementation have drifted as Promise support
  landed;
- several HttpApi and Effect modules are still marked unstable upstream;
- plugin code remains trusted in-process code;
- platform-specific direct APIs remain mixed with injected services; and
- cross-machine Location placement is reserved rather than implemented.
  [source/limitation]

OpenCode's direction is credible, but “Effect-native” does not mean the
migration is finished.

## 29. What OpenCode gets especially right

### 29.1 It fixes context topology instead of adding more helpers

V2 does not merely improve the bridge. It replaces ambient instance lookup with
an explicit Location graph.

### 29.2 It keeps embedded and network execution semantically identical

The memory transport traverses the same request processor. This prevents an
in-process SDK from becoming a privileged second API.

### 29.3 It makes resource lifetime part of extension semantics

Plugin and tool registrations are owned by Scope. Hot replacement is therefore
cleanup plus replay, not mutation plus hope.

### 29.4 It treats cancellation as a first-class semantic

Tool, shell, subagent, stream, and runner paths deliberately preserve
interruption.

### 29.5 It uses Schema identity to control package drift

Public values are canonical across Core, Protocol, Client, and SDK.

### 29.6 It tests the dependency graph itself

Composition rules and replacement behavior are not left to convention.

## 30. What to treat cautiously

### 30.1 Custom LayerNode is powerful internal framework code

It is justified by OpenCode's migration and scope partitioning, but it adds a
new type and graph compiler every contributor must understand.

### 30.2 Effect beta and unstable modules enlarge upgrade risk

HTTP, SQL, process, and observability are load-bearing. A beta upgrade can
affect much more than syntax.

### 30.3 `orDie` can erase operational distinctions

Fatal startup configuration and transient database failure should not
automatically share a defect path.

### 30.4 In-process plugins share full server containment

Scope guarantees cleanup, not security isolation. Typed capabilities reduce
accidental coupling but do not sandbox malicious npm code.

### 30.5 Process-global memoization needs strict freshness rules

Shared memo maps improve Layer reuse but can leak stale or test-coupled state
without explicit `Layer.fresh`, scoped maps, and replacement discipline.

### 30.6 Host abstraction remains uneven

Direct Bun/Node access is practical, but every unowned escape weakens
portability and deterministic substitution.

## 31. What OpenAgents should adapt now

### A. Publish one explicit scope topology

Define and document:

~~~text
process
  ├─ identity, database, Sync, Blueprint, observability
  ├─ WorkContext(account, repository, workspace, placement)
  │    ├─ policy, tools, providers, MCP, filesystem, containment
  │    ├─ runtime/plugin generation
  │    └─ conversation/run scopes
  ├─ request/command scope
  └─ renderer foreign-host scope
~~~

For every service, specify the owning scope, allowed upstream scopes, cache key,
freshness rule, and disposal proof.

### B. Keep WorkContext explicit

Do not use cwd, ambient `AsyncLocalStorage`, renderer-selected paths, or hidden
global mutable state as runtime authority. A stored typed WorkContext reference
resolves the service graph. Compatibility adapters may restore context only at
named edges and must have deletion gates.

### C. Keep Schema below runtime behavior

One canonical Effect Schema value should define each public command, event,
projection, authority manifest, and receipt. Core and UI facades should
re-export identity, not recreate it. V1 compatibility belongs in explicit
namespaces.

### D. Use one Effect request processor

Desktop IPC, local socket, embedded runtime, remote Pylon, mobile Sync, browser,
and tests should vary transport and credential acquisition only. They must
share decoding, WorkContext resolution, policy, handler, transaction, event,
and receipt behavior.

### E. Make generations scope-owned

Provider, model, tool, plugin, MCP, foreign-host, and policy registrations need
stable generation IDs, captured advertised values, an owning Scope, ordered
replay, cleanup, and replacement receipts.

### F. Preserve interruption across every adapter

Promise callbacks, Electron IPC, Web streams, provider SDKs, Pylon engines, and
mobile steering must map cancellation to Effect interruption and run
finalizers. No broad cause conversion may turn user cancellation into a tool
failure or success.

### G. Test the graph and clock

Provide graph-level replacement for filesystem, process, transport, identity,
policy, provider, database, and Sync services. Use deterministic time for
leases, retries, approval expiry, reconnect, debounce, and cleanup. Add tests
for forbidden scope dependencies and cycles.

### H. Install observability at the composition root

Every command/run should inherit trace, owner, WorkContext, authority-manifest,
runtime generation, and receipt correlation without services passing logger
objects manually. Optional exporters may fail open to local logs; authority
and receipt persistence must not.

## 32. Effect Native consequences

The OpenCode audit sharpens what “Effect Native” must mean for OpenAgents.

It should not mean only React components that accept Effects. It should mean:

- one application service graph beneath every renderer;
- renderer components consume narrow services and emit typed intents;
- foreign hosts are acquired resources with Scopes and finalizers;
- platform implementations are replaceable Layers;
- WorkContext services are separate from process-global and view-local state;
- embedded composition substitutes transport into the canonical request
  processor;
- native callbacks re-enter a captured Effect context only at explicit bridge
  modules; and
- tests can replace host services without Electron, React Native, or browser
  globals.

OpenCode's global/Location split should become a slightly richer OpenAgents
split:

| Scope | OpenAgents examples |
| --- | --- |
| Process | identity, encrypted storage, Sync client, telemetry, component ledger |
| WorkContext | repository, Blueprint program/action, provider catalog, policy, containment, Pylon target |
| Conversation/run | model stream, tool snapshot, child topology, budgets, settlement |
| Request/command | decode, approval, idempotency, transaction, receipt |
| Foreign host/view | PTY, editor, diff, browser preview, canvas, native capture |

Do not copy `LayerNode` wholesale before these concrete scopes demand graph
rewrites. Start with ordinary Layers, explicit node metadata, and a small
architecture test. Add a compiler only when replacement, hoisting, or scope
validation cannot remain clear with native Effect primitives.

## 33. What OpenAgents should not copy

- multiple application runtimes as ordinary internal call paths;
- ambient workspace recovery as a permanent context model;
- a process-global unsafe memo map without ownership rules;
- trusted third-party plugins in the main authority process;
- `orDie` as a convenient substitute for failure design;
- a custom graph DSL broader than the topology problem requires;
- direct host APIs scattered through domain modules;
- separate embedded and network business logic;
- Promise-first internals with Effect wrappers; or
- beta framework upgrades without contract, typecheck, startup, and resource
  regression gates.

## 34. Recommended adoption sequence

1. Inventory current OpenAgents services and assign each to process,
   WorkContext, run, request, or foreign-host scope.
2. Freeze canonical Schema identities for command, event, projection,
   authority, and receipt contracts.
3. Make embedded and remote clients enter one request processor.
4. Convert long-lived registries and background fibers to scope-owned
   generations.
5. Add graph replacement, cycle, forbidden-dependency, interruption, and
   deterministic-time tests.
6. Move Promise and callback bridges to explicit perimeter modules.
7. Install tracing/log annotations from the application Layer.
8. Only then decide whether OpenAgents needs a small Layer graph IR.

## 35. Final assessment

OpenCode's deepest Effect lesson is not that generators are nicer than chained
Promises. It is that an agent product is a hierarchy of capabilities and
lifetimes.

V1 used Effect to make individual services safer while context remained partly
ambient. That brought real value but left bridge code responsible for restoring
the world around each operation. V2 makes the world explicit: global services,
Location services, request middleware, plugin generations, tool snapshots,
streams, fibers, and native resources all have owners.

The result is not pure or complete. It contains a custom Layer compiler,
unstable framework dependencies, trusted plugins, fatalized errors, and legacy
compatibility. But it is the strongest open evidence available that Effect can
serve as the entire architecture of a serious local-first agent application.

OpenAgents should adapt the explicit topology, canonical schemas, single
request processor, scope-owned generations, interruption discipline, and
substitutable tests. It should keep its own graph smaller, its authority model
stronger, its plugins isolated, and its failure taxonomy more deliberate.

## Primary source map

V1 anchors:

- [V1 Effect contributor rules](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/AGENTS.md)
- [Legacy runtime helper](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/effect/run-service.ts)
- [Legacy application runtime](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/effect/app-runtime.ts)
- [Per-instance scoped state](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/effect/instance-state.ts)
- [Promise/callback bridge](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/effect/bridge.ts)
- [Legacy Effect Runner helper](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/effect/runner.ts)
- [V1 Effect HttpApi server](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/server/routes/instance/httpapi/server.ts)

V2 anchors:

- [V2 contributor and package laws](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/AGENTS.md)
- [LayerNode graph](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/effect/layer-node.ts)
- [Application node builder](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/effect/app-node-builder.ts)
- [Global and Location tags](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/effect/app-node.ts)
- [Location service graph](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/location-services.ts)
- [Canonical Schema package rules](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/schema/AGENTS.md)
- [Protocol HttpApi](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/protocol/src/api.ts)
- [Server composition](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/server/src/routes.ts)
- [Managed server resource](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/server/src/process.ts)
- [Embedded SDK memory transport](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/sdk-next/src/opencode.ts)
- [Database service](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/database/database.ts)
- [Effect/Drizzle transaction adapter](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/effect-drizzle-sqlite/src/effect-sqlite/session.ts)
- [Durable and volatile event service](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/event.ts)
- [Effect-native plugin runtime](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/plugin.ts)
- [Promise plugin adapter](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/plugin/promise.ts)
- [Public Effect plugin API](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/plugin/src/v2/effect/plugin.ts)
- [Plugin target design](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/plugin/src/v2/effect/PLAN.md)
- [Observability Layer](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/src/observability.ts)
- [Effect test harness](https://github.com/anomalyco/opencode/blob/fe09a2e9b7dde68296a6243e2aaf12ce60410e49/packages/core/test/lib/effect.ts)
