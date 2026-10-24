# Ownership, integrations, and distribution

Status: selected boundaries and proposed adapters. Aethyrforge is a standalone
repository with no mandatory sibling checkout. This design reuses public ideas;
it does not copy OpenAgents implementation or licensed art into the engine.

## What Aethyrforge owns

The engine owns semantic schemas, typed commands, compilation and cache contracts,
simulation, browser renderer, behavior VM, creator experience, project export,
and release format. It owns enough implementation to improve any critical path
without waiting for a scene-engine vendor's roadmap.

Ownership means documented replaceable boundaries, reproducible outputs, source
control, compatibility policy, and freedom to host independently. It does not
require inventing a browser, cryptographic primitive, database, codec, or language
parser. Keep those dependencies behind explicit interfaces and qualification.

| Dependency class | Initial choice or candidate | Boundary |
| --- | --- | --- |
| Editor components | React DOM, TypeScript, CSS | Owned accessible components; no ownership of engine frames or world authority |
| Reactive UI state | Effect Atom, `@effect/atom-react` | Replaceable editor-state adapter; bounded views of services and selected runtime data |
| Browser orchestration | Effect major 4 | Trusted editor services; no required behavior VM environment |
| Browser language/tooling | TypeScript, Vite | Editor builds and behavior parsing; portable project formats independent of bundler |
| CPU implementation | Stable Rust, Wasm toolchain | Kernel/compiler/VM/service crates with pinned lockfiles at implementation |
| Graphics | Standard WebGPU API: browser WebGPU on the web, Dawn natively; Slang compiled to WGSL | Owned Rust render graph and assets; each platform profile separately qualified |
| Hosted persistence | PostgreSQL plus immutable blob storage | Transactional metadata separated from bulk artifacts |
| Concurrent text | Loro through an adapter | Source/comments only; frozen frontier on source commit |
| Observability | OpenTelemetry-compatible exports | Bounded private traces; no mandatory telemetry vendor |
| Physics/codecs/fonts | Reviewed libraries or owned modules | Explicit licenses, version locks, conformance, and replaceability |

Concrete versions are pinned at the first implementation milestone after checking
official APIs, licenses, supported targets, advisories, and reproducible builds.
This specification does not invent a lockfile for code that does not exist.
Pin React/React DOM, Effect 4, and its compatible Atom React binding together.
The [frontend contract](frontend.md) defines the adapter and lifecycle tests;
changes in the Atom API cannot redefine engine state or scheduling.
Changing a dependency must preserve protocol and semantic compatibility or use an
explicit migration. No library's presence establishes a performance advantage.

## OpenAgents ecosystem adapters

| Adapter | Allowed role | Must not own |
| --- | --- | --- |
| Kiln | Inspect granted project/context, propose typed commands, stage source/assets, validate candidate, read diagnostics, request captures | Independent world state, hidden publish authority, editor account credentials |
| Jev/other decision model | Select supported intent or candidates with uncertainty and a bounded request budget | Exact IDs/coordinates invented without evidence, authorization, execution policy |
| Pylon/remote worker | Execute declared build/generation jobs with frozen inputs and scoped artifact outputs | Arbitrary project-wide writes or production release-channel mutation |
| OpenAgents identity/session | Authenticate principals and adapt existing room/grant services | A second incompatible meaning for engine transactions or runtime state |
| Verse/native host | Mount a release and implement qualified input/GPU/audio/session adapters | Reinterpret content versions or silently reduce gameplay semantics |

Each adapter has contract tests using synthetic identities and local fixtures.
The core can run without these services. Authentication and transport details
belong to an adapter/host; model-provider credentials stay on the service side.
Browser-local models can use a separately admitted resource profile and remain
subject to the same action evaluation and trust policy.

Kiln's ordinary object editing uses the command API. Filesystem/source access is
a separate explicit grant limited to project sources. A successful file write
does not activate code, approve a model proposal, or publish a release. Worker
outputs identify inputs, toolchain, model/job version where relevant, and hashes.
Missing or invalid results never replace active resources.

## Import and rights

Accept standard source formats through isolated bounded importers. glTF is the
initial mesh/animation interchange; optimized geometry, collision, and textures
are generated per target. Import does not infer permission to redistribute.

Each source asset records source URL/provider or local origin, author, license
identifier/text reference, modification history, permitted distribution modes,
and evidence of the publisher's rights. An asset may allow inclusion in a compiled
game while forbidding a raw reusable-source export. The package checker enforces
that distinction. Private vendor geometry and captures stay in licensed storage,
not this public repository or test fixtures.

The medieval kit and Haunted House prototypes are motivating scenarios, not
bundled engine dependencies. Conformance worlds use original or clearly
redistributable art. A “walkable prefab” needs semantic structural/traversal data;
an opaque licensed facade cannot gain that capability merely from its filename.

## Portable projects and self-hosting

A complete export contains the project revisions selected by its owner, permitted
source blobs, behavior sources/bytecode, component schemas, dependencies, rights
inventory, required engine profiles, and content-addressed artifacts. A separate
export receipt lists omissions and why. If legal source redistribution is not
allowed, offer a dependency/reconstruction report and an eligible compiled player
package; do not label an incomplete source archive self-contained.

Run a release through a minimal static player for single-player/offline games.
Multiplayer and external-capability games declare their service requirements.
Self-hosted authoring uses the same contracts and migrations as hosted authoring.
Engine-specific telemetry, identity, inference, or storage providers cannot be
mandatory for opening a valid local project or exporting owned data.

## Repository licensing and contribution gate

The engine moved into the OpenAgents repository on 2026-10-24
([ADR 0002](../../adr/0002-everything-in-openagents.md)); the license of the
imported engine code is an open owner decision
([merge plan](../merge.md#decisions-for-the-owner)). That decision covers the
code license, contribution terms, trademark policy, and dependency-license
policy. Apache-2.0
is the proposed engine-code option because of its explicit patent grant; this is
a proposal, not an adopted license. Creator content retains its chosen rights.

The licensing gate does not prevent specification or synthetic conformance work.
It does prevent advertising an open-source grant or shipping unreviewed vendor
content before the policy exists. See [delivery](delivery.md) and [risks](risks-and-experiments.md).
