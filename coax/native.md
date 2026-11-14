# Coax Native

The [iOS starter](native/README.md) presents native UIKit controls with optional
signed Updates. Build 1070 replaces the WebKit presentation of 1068–1069.
Coax owns the keyed component tree, state and actions; the host owns native
controls, input, layout, accessibility and effects.

Written 2026-11-13. Status: framework specification with the initial native
iOS account surface implemented by /root on 2026-11-13. Other target and
verification stages below remain proposals, not claims about this release.

The OpenAgents desktop replacement is a planned consumer on **Mac and Linux
first, Windows later**. Its environment service and cross-computer behavior
are specified in the [environment application profile](../docs/protocol/environments-claude-and-world-editing.md#31-one-desktop-product-two-independent-lifetimes).

### Alignment with Exact

Exact's iOS host presents UIKit controls; its web host presents DOM elements.
WebViews are optional content surfaces, not the app renderer. That is also
the Coax Native default. A native app must not substitute a full-screen
WebView merely to make executable OTA delivery convenient.

The initial native-control execution profile compiles Coax through the
existing C/Wasm pipeline and Binaryen's portable JavaScript target. Installed
JavaScriptCore executes that generated artifact. There is no authored JS
business logic, DOM, WebKit, Rust VM or second UI language. The shared
component command decoder is used by browser and native SDK adapters. UIKit
keeps controls by element id, uses system text and keyboard behavior, and
updates after input or a completion. The book's compiled milestone reuses
this same UIKit projection.

This adopts Exact's separation of portable application, keyed presentation
and installed services, not Exact's plan format. Exact's baked first frame,
dependency analysis and complete driver parity remain unfinished work.
Generated-code execution is qualified against the Wasm reference and actual
iOS workloads; it is not a claim of complete language-backend equivalence.

Coax Native is the shared application framework for writing native apps and
websites in Coax. An app defines its state, actions and components once.
Platform hosts present those components, deliver input and execute admitted
effects. The same app can contain ordinary forms, chat, GPU surfaces and Metaverse Engine
realms, and can be operated through one agent interface on every qualified
host.

The relationship to React Native is the product promise: shared application
code with access to real platform capabilities. The architectural patterns
come from [Exact](../docs/research/exact/README.md): stable element identity,
typed effect boundaries, incremental presentation, a controllable clock and
tests that operate the actual app. Coax Native compiles Coax to C11 or Wasm;
it introduces neither JavaScript business logic nor a second UI language.

## Scope and existing contracts

Coax is the language and compiler. Coax Native is the umbrella app framework;
**Coax Native Web** is its browser target. **Coaxial** is deprecated as the
public framework name. Existing `coax/web/` paths, `coaxial_*` exports,
development endpoints and published host contracts retain their identities.
The [web specification](web.md) describes that target. Coax Native defines
the shared app model, development, hosts, packaging and
[signed code delivery](native-updates.md). It assembles the existing
component system and hosts rather than
creating another component library. The Metaverse Engine remains the owner
of worlds, simulation, room sessions, GPU render plans and authored audio.
An ordinary form app need not link the engine, Psionic or an agent engine.

This proposal extends [the components spec](../components/spec.md) and
[the apps roadmap](../docs/apps/2026-11-10-apps-roadmap.md). It preserves
[the context boundaries](../docs/architecture/contexts.md),
[checked state](checked-state.md), and the published
[realm host contract](../metaverse-engine/host/README.md). In particular,
`aethyr-host-v1` retains its name, ABI and behavior. A Coax Native shell does
not qualify a realm adapter merely by embedding it.

The Exact research originally excluded native controls and DOM output for
Coaxial. Browser text surfaces now have a DOM backend, and the iOS wallet
already uses UIKit. This proposal makes presentation an explicit choice per
surface: shared GPU components for custom visuals, DOM for browser text,
native controls for native forms, and system-owned presentations for OS
services. The initial native text/control projection is implemented; this is not a
claim that the GPU component kit runs as UIKit controls.
Realm rendering stays GPU-first.

## Layers and execution ownership

Four owners divide the work. Each layer uses the one below it and never
reaches up.

| Layer | Owns | Does not own |
| --- | --- | --- |
| Coax | The language, the compiler, the standard libraries and the targets (C11, Wasm, shaders) | App composition, product rules |
| Coax Native | App composition: state, actions, routes, the assembly of `components/`, the platform-service boundary, hosts, `coax drive`, packaging and delivery | Product behavior, agent execution, worlds |
| OpenAgents | Product and agent behavior: accounts, conversations, the general agent and its members, the payment rules in `pay/`, and the apps in `apps/` | A second framework or component toolkit |
| Metaverse Engine | Worlds, simulation, room sessions, GPU render plans and authored audio | App navigation and forms |

The application runtime is separate from agent execution. Coax Native runs
the app: it draws state, takes input and admits effects. It does not run
agent tasks on its own. Every agent task has exactly one execution owner at
a time: the device, a paired computer or a hosted service. The task record
names its owner, and a change of owner is an explicit, recorded handoff.

Every other connected client presents that task. It reads the task record,
sends input, approvals and cancellation to the owner, and draws the result.
It does not repeat tool calls, model requests or payments. After a reconnect,
a client resumes from the record; it never executes the task again. Shared
application code, shared account and conversation data, and server-side
agent execution are three different things, and a design names which one it
means.

## Current foundation

The following is the starting inventory on 2026-11-13. Existing modules are
reused; proposals below describe the additional contract they must satisfy.

| Area | Exists | Work required for Coax Native |
| --- | --- | --- |
| Compilation | Coax emits C11, generated headers, Wasm and shaders | A reproducible app package joining these artifacts and their identities |
| Components | Core, layout, paint, skin and backends in `components/` | Shared app-facing semantics across all presentation paths |
| Trees | `core/tree.coax` for browser surfaces; `core/elements.coax` for inspected rows | Reconcile these and the engine HUD trace into one inspection model; they are not interchangeable today |
| Layout | `layout/flex.coax`, a documented flex subset checked against Chrome | Native adoption, platform text measurement and explicit unsupported cases |
| Presentation | GPU, DOM and terminal code; shared Apple, Linux and Android hosts | Native-control projection and evidence per backend and target |
| Driver | `coax/web/drive.coax`; `apps/shared/host/drive.c` and app callbacks | Complete common operations, capabilities and iOS shell integration |
| Time | Web agent clock and pin; shared UI hosts have a partial controlled clock | Common scheduling, pinned environment and asynchronous completion semantics |
| Reload | Web field carry in `coax/web/state-keep.js` | Equivalent native carry and ownership-safe replacement |
| Product rules | Payments in `pay/`; shared Apple login and device keys | Preserve these owners while sharing their app flows |

The OpenAgents iOS app is the first migration consumer. Its replacement
starter presents UIKit controls from the Coax Native semantic program,
executed through generated code in JavaScriptCore; 1068–1069's WebKit profile
is historical. Build 1075 adds installed Metal world producers and signed,
streamed asset inventories through the shared Apple PageHost; see the
[iOS consumer's scope](../apps/openagents-ios/README.md#streamed-worlds-build-1075).
This does not qualify the canonical renderer's other passes or backends.
Account, chat and wallet platform adapters still
have their own installed capability boundaries. This is an initial native
consumer, not evidence that every proposed framework service is implemented.

The shared Apple UI host has iOS support, driver code and accessibility work;
that does not mean `apps/openagents-ios` uses it. Adoption must be established
in the app's actual target and launch path. The older research audit's open
items are historical: its November 11 addendum records desktop driver, clock
and element-table progress, and current web code also has field carry and
the agent pin. None establishes complete native/mobile parity.

### OpenAgents desktop environment consumer

The [desktop/environment plan](../docs/protocol/environments-claude-and-world-editing.md#31-one-desktop-product-two-independent-lifetimes)
replaces the current desktop shell with the same Coax Native application
model used by mobile and web. Reuse the existing Mac AppKit/Metal and Linux
Wayland/Vulkan hosts, components and agent libraries. Shared Coax state/actions
and protobuf profiles are required; one identical executable backend is not.
The UIKit/JavaScriptCore projection is a reference for portable execution,
not an already-qualified desktop executor or a reason to add a WebView.
Windows requires its own host/backend qualification before support is claimed.

Desktop packages include a versioned resident environment service, also
runnable headlessly. The app configures and attaches to that service through
an authenticated local adapter, or connects to another enrolled computer.
Agent turns survive closing the GUI. The CLI, desktop, mobile and web use
the same protocol and authority owners; a UI window never becomes another
execution owner. Client and resident-service updates negotiate compatibility
independently. Native service replacement is not an ordinary UI OTA payload.
Migration preserves desktop identity, credentials, settings and histories;
the linked profile owns the rollout and acceptance gates.

## Architecture

```text
Coax app
  domain state + actions + routes + component composition
       |                      |
       | requests             | semantic elements and surfaces
       v                      v
  admitted effects       components: core / layout / skin / paint
       |                      |
       v                      v
  platform services      native controls | GPU | DOM | terminal
       |                      |
       +------- platform host +-------+
                       |
              actual input and completion events
                       |
                 serialized app dispatch

  coax drive -> the same dispatch, semantic tree and clocks
  realm surface -> existing Metaverse Engine and room contracts
```

The runtime is a library compiled into the app, with caller-owned bounded
storage. It does not execute an Exact plan or embed the compiler's evaluator.
The native profile links compiled Coax procedures. The initial portable app
profile executes Coax-generated JavaScript in JavaScriptCore after the existing
C/Wasm and Binaryen compilation steps, behind the app/service boundary,
so compatible actions and view composition can update over the air while
the native host remains installed. A future direct Wasm executor must reuse/factor the
existing realm-script interpreter and qualify a separate app profile; the
current restricted script VM cannot load full app modules. A portable
description of elements is data consumed by presentation backends.

Each app declares capacities for state, elements, text pools, realized list
rows, queued events/effects and retained payload bytes. Admission precedes
host allocation and copying. Refusal retains the previous committed state;
application-owned, SDK-owned and GPU/driver allocations are measured
separately. A bounded Coax arena does not prove bounded total process memory.

| Responsibility | Owner |
| --- | --- |
| App state, actions, routing, scheduling and request ownership | Shared Coax app code; generic pieces may extend `coax/lib/app/` (proposed) |
| Element semantics, layout, editing, skins and paint | Existing `components/` layers |
| Platform lifecycle, windows, native controls, accessibility bridge and SDK calls | Existing `apps/shared/host/` adapters |
| Web effects and DOM application | Existing `coax/web/` hosts |
| World input, simulation, deeds, room authority and render/audio plans | Existing `metaverse-engine/` modules |
| Account, payments, agents and inference | Their existing contexts and shared libraries |
| App build and drive orchestration | Coax tools under `coax/`, using existing build and driver entry points |

`coax/lib/app/` must remain independent of components and the engine.
Components must not load app product code, a payment context or a model
client. Host code must not implement a second router, text parser, edit
buffer, payment rule or world controller. New reusable behavior extends its
existing owner under [the reuse rule](../docs/reuse.md).

## Application state and dispatch

An app has explicit caller-owned state, a route stack, constructed service
state, bounded event and effect queues, and presentation state. Initialization
uses the compiler's declared-default constructors; zeroed memory alone is
insufficient. Runtime handles and native objects are never durable identity.

Every input, timer, deep link, accessibility action and effect completion
enters serialized dispatch. Platform callbacks enqueue a bounded event and
wake the dispatcher; they never reenter Coax from an executing foreign call.
Workers may do platform work but do not concurrently mutate app state.
Overflow returns a defined refusal or backpressure result, never silent loss
of a payment completion or an unbounded retained callback list.

An action validates its inputs and admits its whole state change and effect
requests before publishing them. A rejected action publishes neither a partial
UI change nor a partial effect batch. Use the existing checked-state
transaction mechanism where applicable; traps retain their documented fatal
behavior. External effects are outside in-memory rollback. A committed
request can fail or have an unknown outcome; state must represent both.

Effect outbox consumption means the host accepted or refused a request, not
that its external operation succeeded. Completions enter as later actions.
An operation ID and the owning service decide idempotency and retry. An
unknown payment or remote write outcome never authorizes automatic retry.

Views derive from committed state and declared environmental facts. They do
not start requests, read clocks, invoke SDKs or alter domain state. Resource
loading is an explicit state machine: absent, pending, ready, failed or
unknown as the domain requires. Invalidation is declared by the app's actions;
it must not arise from rebuilding a view. Coax procedures and typed records
express this model. A stricter bounded-view compiler profile is future work;
the framework must not claim it is enforced before the checker supports it.

## Elements and presentation

### Identity and semantic ownership

Every interactive or meaningful element has a stable ID, role, accessible
label, state, action target and parent. Repeated rows use a domain key, so
inserting a row cannot change the identity, selection or focus of its neighbor.
Duplicate IDs and capacity exhaustion refuse a candidate update before it
becomes interactive. Automatic suffixes or truncated IDs cannot establish
portable identity. Existing element-table behavior must be adapted without
silently changing its current consumers.

Extend the current tree and element modules; do not add an app-private tree.
The tree is the semantic description. The element table and engine trace
are projections of the committed presentation, with actual bounds, clipping,
visibility and focus facts. A host may supply measured layout facts, but it
cannot invent semantic state or action availability. Queries identify the
commit and surface whose facts they describe; mixed old/new layout is not a
valid inspection result.

Native controls may retain platform objects keyed by element ID. GPU surfaces
may rebuild their display list. Both preserve the same keyed component state
and action semantics. Incremental output changes only affected nodes and
bindings; a full rebuild remains the differential reference. No virtual DOM,
new bytecode VM or compiler dependency analysis is required for the first
implementation: use the existing keyed DOM comparison and explicit dirty
state, extending the shared modules where needed.

Long lists separate keyed data/state from the bounded set of realized rows.
Recycling a platform row never transfers focus, selection or pending work
to another key. Inspection distinguishes an offscreen item from an absent
one; targeting it requests realization through the normal scrolling path.
If a backend cannot apply a complete update, it fences input on that surface
and reconciles from committed state before publishing new hit-test facts.
An SDK presentation failure cannot roll back an already completed payment.

### Surface choices

| Surface | Presentation | Required behavior |
| --- | --- | --- |
| Native form or list | Proposed UIKit/AppKit controls; Android controls as separately qualified | Shared actions and route state; native text, scrolling, keyboard and accessibility |
| Custom app UI or realm HUD | Existing component GPU backend and skins | Shared layout, edit buffer, hit testing and accessibility projection |
| Browser text surface | Existing DOM backend and frames kit | Actual browser text input, selection and accessibility; no canvas imitation of these |
| Realm | Existing engine host contract | Preserve room authority, render admission, replay and lifecycle |
| Terminal | Existing terminal backend | Semantic text/actions with declared visual omissions |
| System service | OS-owned picker, passkey, biometric, share or permission UI | Typed request/result, cancellation and ownership; never reimplement the system prompt |

An app selects a surface deliberately; hosts report unsupported capabilities
before presenting it. A default mapping may choose GPU for a control backend
that is unavailable, only if that declared fallback preserves the required
input and accessibility contract. Otherwise the surface refuses explicitly.
Changing presentation at runtime preserves app state and retires old owners.
Terminal support does not imply a terminal can play a realm or scan a camera.

The first native-control set is text, button, field, toggle, scroll and keyed
list. Implement the semantics needed by the first consumer, not the entire
UIKit or CSS API. Native modules such as a map or camera preview have an
explicit element, bounded inputs, typed output events and a concrete lifetime.
An access hatch may configure its own platform object; it cannot mutate app
state or bypass the action path. Arbitrary user-supplied native code is not
made safe by this boundary.

### Layout and text

Use logical pixels for app layout and the existing flex subset and text
measurement interface. Pixel scaling, safe areas, keyboard insets, orientation,
preferred text size and reduced motion are environmental facts. The native
backend must map layout into platform frames without running an independent
application layout algorithm. Platform text measurement can differ; native
scroll dynamics and control appearance can differ too. Record these as
backend behavior rather than claiming identical pixels.

Shared GPU text uses `components/core/edit.coax` and the one wrapper and
Markdown parser. A native field uses the platform text system while editing,
then delivers complete typed edit/composition events to shared state. There
must be one authority for selection and marked text during that edit; a
second buffer must not overwrite an active IME session. Conversion between
UTF-8 and platform UTF-16 offsets uses the existing shared text helpers.

The Chrome oracle applies to the supported portable box-layout subset.
Platform font shaping, accessibility, large text, bidirectional text, IME,
scroll physics and controls require their own actual-host checks. Stable
IDs, labels and actions must reach VoiceOver, TalkBack and desktop bridges.
Accessibility activation dispatches the same action as a person's input.

## Navigation and composed surfaces

The shared app owns typed routes, parameters, tab stacks, modal state and
Back behavior. A route is a destination, not a boolean screen flag. Platform
back gestures, deep links and notification taps request the same transitions.
Parsing and authorization use the domain's existing policy and codec. A link
or decoded route never grants account access or room admission.

A route can own several surfaces: a realm and its HUD, a login modal over a
web view, or a terminal status view. Each surface has one input/focus owner
and explicit coverage. Covering a realm releases its held input and applies
the existing session/audio policy; it does not pause a live room server.
Opening and dismissing a modal restores the intended focus and does not
replay the tap that dismissed it into the surface underneath.

A web view is an explicit embedded surface with an origin/navigation policy,
its own build identity and a narrow typed communication boundary. Native
login and cookies inside the web view are separate credentials until an
existing authenticated protocol joins them. A shared screen or process is
not an implicit token bridge. Page content cannot call privileged native
services without an admitted request under that boundary.

## Services and native modules

The effect boundary is generated from typed Coax declarations and existing
protocol descriptors. Requests identify the operation, allowed capability,
bounded payload and concrete app/surface/service generation. Results carry
that owner back. Host code validates physical spans before forming views or
copying bytes. Borrowed memory lasts only for its declared call; asynchronous
SDK work retains an admitted copy until real completion and disposal.

| Service family | Host effect | Shared decision owner |
| --- | --- | --- |
| Network | HTTP, sockets, TLS and cancellation | Existing transport, route and domain policies |
| Identity | Keychain, Secure Enclave, passkeys and OS prompts | Shared login/device-key code and identity policies |
| Payments | Breez Spark SDK calls | `pay/` authorities; SDK outcome remains explicit |
| Files | Picker, sandbox handles, atomic write and sharing | Existing storage/codec and document owners |
| Media | Capture, output device, permissions and interruptions | Existing voice/audio admission and session policy |
| Clipboard, notices and links | OS calls and callbacks | App action and capability policy |

Capabilities are explicit allowlists for services, origins, file scopes and
device access. The build checks declared requirements against transitive
effects; an unknown effect cannot be accepted as proven harmless. The host
executes an admitted request only if its capability is available and the OS
permission permits it. Compiler inventories, runtime admission and OS consent
are separate checks. Generated Info.plist, entitlement or Android permission
entries are packaging inputs, not proof that the user granted access.

Cancellation invalidates publication immediately. Resources and reservations
remain held until the underlying SDK has completed or joined; a late result
cannot reach a replaced screen, engine or login. Generation exhaustion retires
an owner rather than wrapping. Partial, oversized or malformed typed results
publish no successful prefix. Unsupported service is an explicit result,
never an unconditional success stub.

Keep existing C scalar faces and generated layout assertions. The app contract
is additive and versioned separately from the realm contract. Define exact
widths, units, capacities, borrowing, optionality, errors and callbacks before
an implementation ships. Every new wire or stored shape first gets its schema,
specification, registry row and one owning codec under the root protocol
rules. This proposal reserves no message numbers or schema IDs. In-process
ephemeral views are not automatically wire messages, and must not later be
persisted or transmitted without registration.

## Time and lifecycle

Keep simulation time, UI scheduling time, service deadlines and civil time
distinct. The host reports facts; shared policy determines transitions.
Backgrounding cancels or retains operations according to their declared
lifetime, clears held input and suspends local presentation/audio as required.
Resume starts a fresh local frame interval and never catches up by running a
wall-clock backlog. A realm continues to use the engine's fixed-step budget,
replay gate and lifecycle contract, including its unclamped host delta.

An app reports why it is busy and its next scheduled wakeup. A static screen
can sleep until input, completion, timer or invalidation. A caret, animation,
streaming reply and running world have explicit scheduling needs. Stop GPU
submission when unchanged; skipping application work additionally requires
that no scheduled work is due. A vertex comparison alone does not prove that.

Agent mode freezes UI/simulation scheduling before initialization. The pin
comes from `metaverse-engine/agent/pin.coax` for engine consumers; ordinary
apps use the same values through a shared lower-layer definition, moved or
factored without introducing an app-to-engine dependency. The current pin is
seed 13096, epoch 2026-01-01T00:00:00Z, en-US and UTC. Do not add another set
of literals to each host.

`clock +MS` advances admitted app time and an offline room together.
`clock settle` finishes finite scheduled work to a bound and reports remaining
owners; ambient world movement does not make it infinite. `clock until`
uses a bounded state predicate. `clock data` is a proposed extension for
pending completions, with an independent wall-time deadline and explicit
timeout; it must not fast-forward a real payment or network timeout into a
success. Deterministic tests inject or replay completion events in declared
order. Live-service checks report their real nondeterminism separately.

Live room servers cannot be paused by this clock. Deterministic world tests
use the existing offline room. Pinned randomness is for simulation and fake
services only: real keys, passkey challenges and payment operations must use
their secure platform sources. Deterministic test mode uses isolated fake
credentials and cannot silently reach production signing or spending.

## The agent interface and verification

Extend `coax drive`, not a private iOS driver. The common vocabulary is
`tree`, `screenshot`, `tap`, `type`, `state`, `layout`, `logs`, `clock`,
`prefer` and `perf`. Existing extensions such as `hit`, `taps`, `why`, drag
and realm operations remain available through reported capabilities. An
unsupported operation refuses; hosts must not claim the complete interface
because they implement only screenshot and tap.

Tree and state answers come from committed shared state; layout includes
actual host measurements. `tap ID` and `type ID TEXT` resolve a live target
and exercise the production input/edit path. UI driver operations cannot
shortcut domain actions by directly writing state. Accessibility tests also
exercise the actual platform bridge. Target identities use semantic keys,
never coordinates derived from a previous frame.

Development attachment checks source/build provenance before input. A stale
build refuses with the exact rebuild command; an installed release identifies
its embedded build rather than pretending it matches local sources. The web
surface and native shell report their identities separately. Device transport
may differ from a desktop Unix socket, but it speaks the same registered
operations. Debug endpoints are explicit development capabilities, restricted
to the attached test session, and absent from normal distribution builds.

Inspection uses an allowlist. `state` is not unrestricted memory reflection.
Seeds, private keys, access/refresh tokens, passkey material and recovery
words are excluded from trees, screenshots captured by fixtures, logs and
replay records. Sensitive system screens are tested with isolated fakes and
separately qualified physical-device ceremonies.

Agent tools are a typed view of the same actions. Reuse the CLI MCP and
WebMCP integrations where supported, with availability and consequence
metadata checked against effects. The proposed tool declaration syntax from
the Exact research is not a shipped language feature. Realm tools lower to
deeds and retain room authority. Tool access never bypasses authentication,
device-owner checks or consent; a driver connection is not authorization.

A portable acceptance case is a sequence of these operations with expectations
and declared host requirements. Extend the existing operation-file/replay
work, rather than creating a second evaluator or per-platform test language.
Record input and external answers once; replay performs no real remote side
effects. Native SDK and OS UI checks remain complementary conformance tests.

For example, the same login case should open `account.login`, type into its
email field, request a code, deliver a fake response, enter the code, and
assert the account state and route. Web, UIKit and GPU backends may look
different; their action/result sequence must agree. A separate case must
cover cancellation and a late reply after logout. The payment case quotes,
requests owner confirmation, delivers a fake SDK result, and proves that a
duplicate or late result cannot send again.

## Builds and development workflow

Coax Native provides an Expo-like developer loop over this repository's
toolchain: create from a checked template, diagnose platform prerequisites,
build the smallest changed unit, run, attach the driver, verify and package.
The implementation may expose `coax native new`, `doctor`, `dev`, `build`
and `verify`; these are proposed commands, not commands available today.
Their logic is Coax and reuses the compiler, remote build tools, driver and
platform packaging wrappers. It does not add a parallel dependency manager.

Start with two checked templates: a small form/list app and an app with a
realm surface. Both include a portable acceptance case, declared capabilities
and a minimal startup workload. `doctor` reports the missing compiler, SDK,
host capability or signing prerequisite and its repair command; it does not
silently install or execute untrusted setup scripts. Templates reuse the
shared components and adapters rather than copying their implementations.

An app's typed Coax build description declares its entry, targets, surfaces,
capabilities, asset pins, storage schema and budgets. It generates platform
configuration rather than maintaining independent product manifests by hand.
The emitted package joins source, compiler, generated ABI, shaders, fonts,
assets and host-adapter versions. Embedded web surfaces additionally identify
the served build and required compatibility range. The published shape of
this description/package must be registered before tooling emits it.

Native builds use the pinned Coax compiler and host compiler plus each
platform's required SDK and signing tools. There is no Hermes, Rust or Node
runtime dependency for app logic. Existing native dependencies such as Breez
remain pinned platform adapters. Build tools follow `tools-in-coax.md`;
heavy work runs through the remote build hosts. No downloaded repository
script is executed merely to bootstrap an app.

In development, carry compatible state field by field by extending the
current layout-table machinery. Construct defaults first, preserve only
admitted fields, and report resets. Raw pointers, views, OS handles and
in-flight callbacks cannot be copied into a replacement. Retire their owners
and reattach only through an explicit checked protocol. Source maps remain
bound to the exact generated executable. An iOS dev reload uses a separately
qualified development path or rebuild; desktop dynamic-library support does
not imply equivalent iOS loading support.

Native runtime code ships through platform distribution. Compatible portable
Coax app code and content ship through **Coax Native Updates**, defined in
[native-updates.md](native-updates.md): runtime fingerprints, signed manifests,
content-addressed artifacts, channels/cohorts, atomic staging, safe activation,
trial health and data-compatible rollback. This is a core delivery path, not
limited to data bundles. Code linked into a mobile executable still needs a
native release; code already expressed in an eligible installed Wasm execution
profile can use OTA. New SDKs, entitlements, native APIs or capability ceilings
require a native runtime update.

The maximum portable profile moves changing app composition, actions,
navigation and ordinary rules into updateable Coax modules. Protected native
services retain secrets, permission checks, spend floors and recovery. A
hosted web surface can update through the existing web process; signed,
offline-capable package launch is a separate proposed adoption. Store
eligibility is checked separately from technical execution, especially for
iOS interpreted code and software offered inside the app. A baked first
frame remains a separately qualified startup optimization.

Persistent desktop app data uses the shared `~/.openagents/` helpers, with
the existing engine and model subdirectories. Mobile adapters map the same
logical namespaces into their OS sandbox and secure storage; they cannot
create a literal desktop home directory or duplicate model caches per app.
Cross-app sharing on mobile requires an OS-approved shared container; sandbox
boundaries are not bypassed to emulate desktop paths.
Explicit path overrides remain valid. Account tokens, wallet seeds and
device keys keep their existing storage and recovery rules.

## Qualification and performance

Support is recorded per app, target, backend and service set, with source and
artifact identities. A host that compiles is not a qualified host. The first
native consumer is iOS, with the web reference. The OpenAgents environment
desktop replacement targets Mac and Linux together next; Windows follows
their qualification, with no backend selected yet. Existing Android work
continues independently, but receives no parity claim until its input,
services, lifecycle and real-device checks pass.

| Gate | Required evidence |
| --- | --- |
| Shared rules | Checks written first; properties and zero-survivor mutation for admission/policy authorities |
| State and effects | Refusal atomicity, exhaustion, cancellation, duplicate and stale completions, unknown outcomes and cleanup |
| Components | Same semantic tree/actions; supported flex boxes against Chrome; incremental output against full rebuild |
| Actual host | Real text/IME, focus, accessibility, scrolling, lifecycle, SDK ownership and requested native modules |
| Portable app flow | Same acceptance operations and fake/replayed answers, with semantic expectations before image comparisons |
| Presentation matrix | Light/dark where supported, narrow/wide, largest supported text size, reduced motion and declared locale coverage |
| Realm composition | Existing host-contract, room/replay, GPU and muted audio gates on the actual embedded realm path |
| Release | Exact package provenance, supported compatibility, installed-app launch and preserved login/wallet data |

Use `coax.evidence.v1` and existing receipts for recorded results. Extend their
owning formats by the protocol versioning rules rather than creating another
evidence registry. Declare unavailable checks and device-only evidence as
remaining qualification work. Physical passkey PRF, biometrics, push, audio
routes, camera and GPU recovery cannot be qualified solely with simulator
fakes. Owner actions go in `NEEDS_OWNER.md` when implementation reaches them.

Performance has separate UI-only and realm workloads. Record cold process to
interactive frame, edit to presented frame, input latency, frame CPU/GPU
tails, idle wakeups/energy, peak application-owned bytes, SDK/driver memory
and package size. Device, OS, build, workload and sample counts accompany
percentiles. The initial design objective is 100 ms p50 for a small local
UI's cold interactive frame, excluding OS launch scheduling; it is not an
existing result or a budget for loading an entire realm. Measure complete
user-visible launch as well. Choose release thresholds from physical-device
baselines before a stage claims performance acceptance; preserve established
realm budgets and baseline regression policy.

## Migration and implementation stages

Each stage is an independently reviewable change with a real consumer and
bounded checks. Assign its implementer and date before starting it. The owner
has selected the desktop replacement direction in the linked environment
profile. Its default switch still requires that profile's migration and
consumer gates; this document does not claim a switch has shipped.

| Stage | Consumer and work | Exit condition |
| --- | --- | --- |
| 1. Common app boundary | Extract the iOS shell's inspectable route/action state and connect its existing native and web paths to `coax drive` | One login open/cancel case runs through both launch paths; provenance, redaction and unsupported-operation behavior are checked |
| 2. Controlled time and effects | Add shared scheduling/ownership and fake services to that consumer; reuse existing clock and replay work | The same bounded case gives the same state/actions across web and iOS; cancellation/logout fences a held completion |
| 3. Portable native form | Project the existing component semantics to the small UIKit control set; share one account flow with the web | Shared Coax action/view code, real keyboard and VoiceOver, large-text layout, same semantic assertions |
| 4. OpenAgents desktop replacement | Reuse shared Apple/Linux hosts and components for environment setup, local/remote chat and controls; bundle the independent service, retaining CLI-only operation | Same accepted operations on both desktops and phone; actual input/accessibility, cross-client ownership, GUI-independent turns, migration and update compatibility checked |
| 5. Wallet and realm composition | Migrate wallet presentation/state and a realm route incrementally, preserving the existing domain authorities | No seed replacement, no duplicated send, policy refusals preserved, correct modal/realm lifecycle, physical-device gates recorded |
| 6. Android and packaging | Extend existing Android adapters; finish templates, diagnostics and reproducible packages | Same app acceptance case plus actual Android input/service/device evidence; one documented setup path |
| 7. Later optimizations | Native state carry and baked initial UI | Independent compatibility, ownership and startup evidence for each feature |

The [Updates stages U1–U7](native-updates.md#implementation-and-qualification)
run alongside these app stages. Signed delivery begins with the existing
embedded-web path, followed by the separately qualified native-control app
executor; it does not wait until every host or optimization is built.

For iOS, keep `com.openagents.app`, its App Store record, Keychain services,
wallet seed, backup and authentication semantics. Migrate the shell and one
screen first. Existing UIKit screens can participate as declared native
surfaces while their composition moves to shared components; this is an
explicit migration state, not a second permanent component toolkit. Preserve
their tests until the portable cases cover their behavior and real OS paths.

The web and native Grid remain separate declared presentations until they
qualify the same realm contract. Coax Native must not remove the fallback,
change default rendering or claim Haunted House/web parity as a side effect
of sharing app navigation. Reuse each engine module rather than copying its
logic into the framework.

## Decisions still required before their stage starts

- The public app/service ABI and its versioned descriptors: exact signatures,
  capacities and service-specific errors, based on the first extracted flow.
- The neutral component semantics needed by native controls and the migration
  of today's DOM-shaped tree and inspected element table without breaking
  existing consumers.
- Device driver transport and attachment authorization for iOS and Android.
- The first measured device/OS workloads and acceptable performance thresholds.
- The first store-eligible executable update workload and app Wasm validation/
  execution profile, under the delivery architecture in `native-updates.md`.

These are implementation admission decisions. They do not defer the central
architecture: compiled Coax application logic, shared components, explicit
platform effects and one agent interface over every qualified presentation.

## References

- Exact research: [overview](../docs/research/exact/README.md),
  [architecture](../docs/research/exact/01-what-exact-is.md),
  [Coax comparison](../docs/research/exact/02-coax-comparison.md),
  [adaptation plan](../docs/research/exact/03-what-to-adapt.md) and
  [apps audit with follow-ups](../docs/research/exact/04-apps-audit-2026-11-10.md).
  Exact statements here refer to that pinned research, not fresh performance
  measurements or a new upstream API survey.
- Shared implementation: [components](../components/spec.md),
  [element table](../components/core/elements.coax),
  [tree](../components/core/tree.coax),
  [DOM backend](../components/backend/dom.coax),
  [native driver](../apps/shared/host/drive.c) and
  [Apple UI host](../apps/shared/host/apple/ui-host.m).
- Existing product and tooling: [iOS app](../apps/openagents-ios/README.md),
  [agent clock](../docs/metaverse/agent-clock.md),
  [start here](start-here.md), [effects](EFFECTS.md),
  [protocol registration](../proto/README.md),
  [web field carry](web/state-keep.js) and
  [invariants](../INVARIANTS.md).
- Delivery: [Coax Native Updates](native-updates.md). The cumulative OpenAgents
  book and broader naming/documentation migration is tracked in
  [#13512](https://github.com/OpenAgentsInc/openagents/issues/13512).
