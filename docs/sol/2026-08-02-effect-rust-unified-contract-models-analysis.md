# Unified contract models across Effect and Rust

- Date: 2026-08-02
- Class: historical analysis and proposed contract direction
- Status: recommendation; not implementation or dispatch authority
- Owner: OpenAgents Effect and Rust contract architecture
- Scope: OpenAgents-owned data that crosses an Effect/TypeScript and Rust
boundary, especially the OpenAgents monorepo, Omega, `omega-effectd`, native
helpers, cloud services, receipts, and Nostr-facing projections

## Executive answer

Yes. Reimplementing the same contract independently in Effect and Rust is a
serious correctness problem. The repository has already named this failure:
the [Effect-versus-Rust analysis](../fable/2026-07-17-effect-vs-rust-architecture-analysis.md)
says that a hand-mirrored type means the split has failed. The current code
shows that this is no longer a hypothetical risk.

The answer is **one definition for each cross-runtime wire contract**, with
generated Effect and Rust boundary code. The answer is **not one universal
in-memory model for the whole product**.

The distinction is essential:

- One encoded fact that crosses a process, repository, persistence, or network
  boundary must have one canonical definition.
- Effect services and Rust subsystems can have different native domain models
  behind that boundary.
- The authoritative owner of a domain implements its policy and state
  transitions once. The other runtime receives commands, events, snapshots,
  or receipts; it does not reimplement the authority.
- Code generation owns structural types, codecs, basic constraints, wire names,
  fixtures, and compatibility checks. It does not generate business policy,
  service graphs, GPUI entities, database transactions, or authority decisions.

Protocol Buffers should be evaluated, but it should not become the universal
OpenAgents model language. It is strongest for a closed, high-volume binary
protocol. Most OpenAgents boundaries are JSON-shaped: Nostr events, stdio
frames, HTTP APIs, persisted receipts, public projections, and Effect Schema
codecs. Proto does not naturally preserve several distinctions that already
matter here, including absent versus `null`, Effect refinements and brands,
JavaScript-safe integer encoding, tagged-union conventions, redaction
metadata, and authority semantics.

The recommended target is a small, deliberately restricted **OpenAgents
Contract Profile**. It is a language-neutral contract definition and compiler
for boundary data only. A contract definition produces:

1. Effect Schema codecs and TypeScript types.
2. Rust `serde` types, newtypes, and structural validators.
3. Canonical JSON Schema for inspection, external validation, and OpenAPI use.
4. Positive, negative, forward-compatibility, and canonical-encoding fixtures.
5. A compatibility manifest and content digest.
6. Optionally, a Proto schema and generated binary codecs for a transport that
   has separately selected Protobuf.

Do not build that compiler from intuition. First run a bounded bake-off against
the hardest existing contracts. If a restricted JSON Schema profile can carry
the required semantics without a lossy round trip, use it as the Contract
Profile. If it cannot, use a thin OpenAgents contract IR that emits JSON Schema
and both language targets. In either case, reject hand-maintained mirrors now.

## What the prior decisions actually require

The prior architecture documents make three compatible decisions:

1. The historical
   [Effect-versus-Rust analysis](../fable/2026-07-17-effect-vs-rust-architecture-analysis.md)
   puts product coordination, policy, receipts, and most client-facing
   contracts in Effect. Rust was a small, process-isolated enforcement or
   latency seam. It said that helper types are owned once, in Effect Schema,
   and that drift is a build failure.
2. The later
   [Nostr-first Rust/Effect Desktop analysis](./2026-07-23-nostr-first-rust-effect-desktop-analysis.md)
   accepted a larger Rust application core because Omega is a Zed fork. It
   still assigns current OpenAgents product semantics to the packaged
   Node/Effect service until a bounded, differential, single-authority Rust
   cutover replaces them.
3. The
   [Omega accepted plan](./2026-07-23-omega-zed-primary-surface-accepted-plan.md)
   requires one generated protocol and one owner for each domain. It forbids
   two writable authorities.

The change in product shape matters. When Rust was a count-on-one-hand helper
estate, generating or checking a few Rust mirrors from Effect Schema was
plausible. Omega makes Rust a first-class application runtime with many more
views of shared facts. The original principle remains correct, but “Effect
Schema plus fixtures” is no longer a complete mechanism.

This analysis tightens that mechanism:

> Effect remains the semantic owner for Effect-owned domains. Rust remains the
> semantic owner for Rust-owned domains. A language-neutral contract artifact
> owns the encoded boundary between them.

That does not weaken the current Effect authority. It stops the wire shape
from being defined independently in two languages.

## Evidence from the current repository

### 1. A hand-maintained mirror already exists

[`packages/cloud-contract/src/index.ts`](../../packages/cloud-contract/src/index.ts)
opens with this description:

> TypeScript Effect Schema mirrors for OpenAgents Cloud contracts.
> Hand-maintained alongside crates/openagents-cloud-contract.

The Rust source,
[`crates/openagents-cloud-contract/src/lib.rs`](../../crates/openagents-cloud-contract/src/lib.rs),
currently contains dozens of public structs and enums plus handwritten
`validate_contract` functions. The TypeScript package exports only a small
subset of Effect schemas.

Some differences are almost certainly intentional projections. Others can be
drift. The contract system cannot currently tell us which is which. For
example:

- the TypeScript `ComputeLane` accepts `cloud_gcp`, `gcp`, and `cloud_vm`;
- the Rust `ComputeLane` serializes `cloud-gcp` and has a smaller variant set;
- the TypeScript and Rust values carrying the
  `openagents.codex_placement_assignment.v1` identity have substantially
  different fields; and
- Rust contracts use `u64` and `u128` for several counters and timestamps,
  while the corresponding JavaScript boundary often uses `number`.

Those facts do not by themselves prove a production bug. They prove that a
reader cannot infer whether two types are exact wire peers, lossy projections,
legacy aliases, or unrelated domain views. A unified contract system must make
that relationship explicit.

### 2. The current Omega seam is described as generated but is handwritten

Omega's repository README says that Rust and `omega-effectd` use one generated,
versioned local protocol. The current implementation is not yet that:

- TypeScript declares frames and many result types in
  [`packages/omega-effectd/src/protocol/framed.ts`](../../packages/omega-effectd/src/protocol/framed.ts).
- Omega independently declares the envelope, method, error, and result types
  in
  [`crates/omega_effectd/src/protocol.rs`](https://github.com/OpenAgentsInc/omega/blob/main/crates/omega_effectd/src/protocol.rs).
- both sides use open `unknown`/`serde_json::Value` payloads for substantial
  parts of the protocol;
- TypeScript applies some handwritten guards, while Rust relies on `serde`
  decoding and per-call result decoding; and
- method and error enums must be updated on both sides.

The generation-fenced transport is good. The schema source is still duplicated.
This seam is the best first proof target because the protocol is local,
versioned, bounded to 64 KiB, already tested in both repositories, and central
to the intended architecture.

### 3. Existing fixture parity is transitional, not sufficient

The
[`Rust And Native Contract Conformance Fixtures`](../../apps/openagents.com/docs/pylon/2026-06-06-rust-native-contract-conformance-fixtures.md)
note says native repositories should copy TypeScript-owned payloads or generate
equivalent JSON. It also says this stays in place until a stable Rust fixture
package exists.

Fixtures are valuable, but a few examples cannot prove the equivalence of two
independently authored type systems. They frequently miss:

- an untested optional field;
- an enum added on one side;
- `null` versus absent;
- integer overflow or precision loss;
- a renamed encoded field;
- unknown-field handling;
- a default applied by only one decoder; or
- a constraint that exists in only one handwritten validator.

Fixtures should be generated from the canonical definition and supplemented by
property and compatibility tests. They should not be the definition.

### 4. The repository already knows how to enforce generated drift

Two packages demonstrate most of the required build discipline:

- [`packages/codex-app-server-protocol`](../../packages/codex-app-server-protocol/README.md)
  generates Effect schemas and method maps from pinned upstream schema
  identities, emits runtime JSON Schema, and fails on generated drift.
- [`packages/agent-client-protocol`](../../packages/agent-client-protocol/README.md)
  vendors a digest-bound JSON Schema, generates stable TypeScript artifacts,
  regenerates into a temporary directory, and byte-compares the result.

The missing capability is not deterministic generation as a practice. It is a
reviewed OpenAgents schema profile and a Rust output target.

### 5. Structural schemas do not contain every invariant

[`packages/portable-session-contract`](../../packages/portable-session-contract/src/index.ts)
contains Effect schemas for targets, agent graphs, generations, attachments,
leases, commands, and projections. Its audits also check relationships such as
graph integrity, generation consistency, and lease scope.

This is an important limit on any IDL proposal. A format can generate a
bounded integer, a branded reference, or a tagged variant. It cannot cheaply
become a full policy and temporal-logic language without recreating a
programming language badly.

The unified model must therefore separate structural conformance from semantic
authority.

## “One model” has four layers

The phrase “one data model” is too ambiguous to be safe. The design needs four
explicit layers.

| Layer | What it contains | Source and owner |
| --- | --- | --- |
| Domain model | Native state, behavior, indices, services, entities, and transactions | The one authoritative Effect or Rust subsystem |
| Boundary contract | Commands, events, snapshots, receipts, errors, and their exact encoded form | One language-neutral contract definition |
| Generated binding | Effect Schema/types and Rust serde types/validators | Generated; never hand-edited |
| Semantic adapter | Mapping between native state and boundary DTOs; cross-record and policy checks | Handwritten beside the authoritative domain |

The flow is:

```text
Effect domain                         Rust domain
services, state, policy               GPUI/project/native state
        |                                      |
  handwritten adapter                   handwritten adapter
        |                                      |
generated Effect codec  <--- one --->  generated Rust codec
                         contract
                         definition
```

This avoids two opposite mistakes:

- **Mirror everything.** Both languages grow copies of the entire product
  ontology, and every change becomes a coordination hazard.
- **Share one runtime model.** Product code is forced into the lowest common
  denominator of an IDL, even when the two runtimes need different ownership,
  memory, concurrency, and UI representations.

The contract DTO is intentionally less powerful than either native model.

## What should be unified

Generate both language bindings when a value crosses one of these boundaries:

- Rust process to Effect process;
- OpenAgents repository to Omega repository;
- durable bytes decoded by both runtimes;
- command, event, response, error, or receipt observed by both runtimes;
- signed or hashed data whose canonical bytes must agree;
- a public or mobile projection produced by one runtime and consumed by the
  other; or
- a protocol capability or version negotiated by both.

Do not unify a type merely because both sides use the same noun. For example,
an authoritative Effect `WorkItem` can project an `OmegaWorkItemSummary`; it
does not require Omega to ingest every private field or reproduce the complete
Effect aggregate. The relationship must be declared as a projection, not
misnamed as a mirror.

Every cross-runtime relationship should have one of these classifications:

| Relationship | Meaning |
| --- | --- |
| `wire_identity` | Both sides encode and decode exactly the same value |
| `projection` | The contract is an intentional public-safe or UI-safe subset |
| `command` | Receiver validates a request; the authoritative receiver decides it |
| `event` | Producer states an occurred fact; consumer must not reinterpret it as authority |
| `receipt` | Producer reports bounded evidence; separate rules decide admission or attestation |
| `adapter_only` | Similar concepts exist on both sides but no shared encoded identity exists |
| `local_only` | No cross-runtime contract should be generated |

This classification is as important as field generation. It prevents a
projection from silently becoming an authority-bearing aggregate.

## Why not make Proto the universal source

Protocol Buffers are credible, but they solve only part of this problem.

### What Proto does well

- mature code generation and a strong Rust path;
- compact binary messages and fast decoding;
- stable numbered fields and explicit reserved numbers/names;
- useful additive-evolution rules;
- services and streaming when gRPC is actually the selected transport; and
- a familiar compatibility toolchain.

Omega already inherits Protobuf machinery from Zed's remote-collaboration
protocol. Reusing that machinery for a measured binary-heavy Omega seam could
be sensible.

### Where Proto is a poor universal fit

1. **OpenAgents is predominantly JSON at its interoperability boundaries.**
   Nostr, HTTP, JSON lines, persisted receipts, ACP, MCP, and public artifacts
   all need stable JSON semantics. A Proto source plus Proto JSON mapping would
   add translation rules to almost every important boundary.
2. **Presence is subtle.** Effect distinguishes an absent optional key from a
   present value, a present `null`, and transformation defaults. Proto presence
   can model some of this, but not as the natural form of the existing
   contracts. A generator could easily collapse distinctions.
3. **Refinements and brands matter.** A `WorkItemRef` is not only a string.
   Length, pattern, privacy class, and non-interchangeability are part of its
   contract. Proto custom options could carry these facts, but then the real
   contract is the OpenAgents option vocabulary and compiler, not Proto alone.
4. **JavaScript integers are a wire decision.** Rust `u64`/`u128` values cannot
   automatically become TypeScript `number`. The contract must select a safe
   integer, decimal string, fixed-width bytes, or bounded number explicitly.
5. **Tagged unions differ.** Effect's discriminated unions, Rust's serde enum
   representations, and Proto `oneof` do not have identical JSON encodings.
6. **Unknown variants and fields are policy.** Commands can need strict
   rejection while passive projections may retain or ignore future fields.
   The correct behavior is per contract and per direction.
7. **Canonical signed bytes are separate.** Protobuf wire bytes are not a
   substitute for Nostr canonical event serialization or canonical JSON
   required by an existing digest contract.
8. **Authority is not a message feature.** Proto can describe a field named
   `approved`; it cannot establish who was authorized to set it or make both
   runtimes settle the same action.

Proto therefore belongs in the toolbox, not at the top of the ontology.

### When Proto should be selected

Select it for a particular protocol only when measurement and topology show
that all of these are true:

- the protocol is closed between controlled binaries;
- binary size or decode cost materially matters;
- the contract is not itself a Nostr, HTTP/JSON, or public artifact format;
- generated Effect runtime validation is still provided at the boundary;
- the canonical JSON/debug representation is specified if operators need one;
- presence, integer, and unknown-variant rules pass the shared corpus; and
- the protocol has its own version and capability handshake.

Even then, the Proto file is a generated transport projection of the canonical
contract unless the contract family explicitly declares Proto as its one
authoring source.

## Candidate source formats

| Candidate | Strength | Failure mode | Disposition |
| --- | --- | --- | --- |
| Effect Schema as the only source | Matches current product semantics and Effect validators | Rust generation depends on a complete, stable export of Effect constructs; transformations and handwritten checks can be lossy | Keep as the transition source for current Effect-owned contracts; test in the bake-off |
| Proto/Buf as the only source | Strong evolution and Rust generation | JSON/Nostr mismatch; Effect validators and OpenAgents metadata become custom sidecars/options | Reject as the universal source; admit per transport |
| Unrestricted JSON Schema | Matches current JSON boundaries and many tools | Dialect and generator differences; overlapping unions and advanced keywords can generate incompatible code | Reject unrestricted authoring |
| Restricted JSON Schema profile | Small standard substrate, inspectable artifacts, close to current wire | Needs strict linting and OpenAgents extensions for brands, integer encodings, sensitivity, and evolution | Preferred first experiment |
| Thin OpenAgents contract IR | Can model exactly the supported cross-language subset and emit every target | We own a compiler and must prevent it from becoming a second programming language | Preferred fallback if the JSON Schema profile is lossy |

The practical recommendation is not “invent a giant new IDL.” It is “define
the smallest portable type algebra OpenAgents actually needs.” JSON Schema can
be the serialized form of that algebra if it survives the hard corpus.

## Proposed OpenAgents Contract Profile

The profile should support only reviewed, deterministic constructs.

### Required type algebra

- records with explicit encoded field names;
- required keys, optional keys, and nullable values as distinct forms;
- closed literals and enums;
- tagged unions with one required discriminator and non-overlapping variants;
- arrays, bounded maps, and explicit recursive references;
- strings with length, pattern, and format constraints;
- booleans;
- safe integers with explicit signedness and width;
- large integers with an explicit encoded representation;
- byte strings with an explicit base64 or hex encoding;
- timestamps and durations with one declared unit and encoding;
- opaque JSON only where a named boundary deliberately permits it; and
- named brands/newtypes for non-interchangeable identifiers.

### Required contract metadata

Each contract must also declare:

- stable contract reference and version;
- owning domain and authoritative runtime;
- relationship class: wire identity, projection, command, event, or receipt;
- canonical encoding and hashing rules;
- maximum encoded size and collection bounds;
- unknown-field and unknown-variant behavior by direction;
- sensitivity/redaction class for each field that can carry private data;
- capability and minimum protocol version;
- compatibility policy;
- replacement/supersession identity; and
- named semantic checks that remain outside structural generation.

The authority metadata is descriptive and lintable. It never grants authority.
It helps reviewers detect a projection that has accidentally gained a secret,
command, or settlement field.

### Generated outputs

For Effect, generate:

- `Schema.Struct`, `Schema.Literals`, and tagged unions;
- branded/refined scalar schemas;
- `Schema.Type` interfaces or aliases;
- boundary `decodeUnknown` and encode functions;
- typed failure classifications; and
- an explicit generated-file marker.

For Rust, generate:

- `serde` structs and enums with explicit rename attributes;
- newtypes for branded identifiers;
- checked constructors or structural `validate` methods;
- safe numeric representations selected by the contract;
- unknown-variant behavior selected by the contract; and
- an explicit generated-file marker.

For both, generate:

- canonical valid fixtures;
- one negative fixture per constraint;
- old-writer/new-reader and new-writer/old-reader compatibility cases;
- canonical-byte and digest vectors where applicable;
- fuzz/property-test seeds;
- a manifest of every type, method, direction, capability, and digest; and
- a machine-readable compatibility report against the previous release.

Generated code must be committed or deterministically produced in both
repositories. CI regenerates it in a clean temporary directory and byte-compares
the output. Manual edits in a generated zone fail the build.

## The invariants generation must preserve

The generator is useful only if the following distinctions survive exactly.

### Optionality and nullability

These are four different contracts:

```text
field is required and non-null
field is required and nullable
field is optional and non-null when present
field is optional and nullable when present
```

No target generator may infer one from another. Defaults are decode behavior,
not documentation, and must be explicit.

### Integer range and representation

Every numeric field must declare its exact wire range. A Rust `u128` must not
silently become a JavaScript `number`. Suitable cross-language encodings are:

- JSON number restricted to `Number.MAX_SAFE_INTEGER` when that range is
  sufficient;
- canonical base-10 string for large unsigned counters and monetary units;
- fixed-width bytes for cryptographic or binary-native values; or
- a transport-specific integer only when no JSON consumer exists.

Money, token counts, sequence numbers, timestamps, and durations must not use
an unspecified generic `number`.

### Tagged variants and future variants

Each union needs one canonical discriminator and one encoding. The contract
must say whether an unknown future variant is:

- rejected because it is a command or authority-bearing input;
- retained as opaque private diagnostic data;
- ignored by a passive projection; or
- represented as a typed `unknown` variant that cannot mutate state.

This decision cannot be left to whichever default a generator happens to use.

### Unknown fields

Use stricter defaults for mutation than for observation:

- commands, grants, policies, and settlement inputs reject unknown fields
  unless a reviewed extension point exists;
- events and projections can accept additive fields only when ignoring them
  cannot change the meaning of the known fields; and
- signed/hashed documents follow their canonical encoding rule, not generic
  decoder permissiveness.

### Canonical bytes

If a digest or signature covers a value, the definition must specify exactly
which bytes are covered. “Equivalent JSON object” is not sufficient. Field
order, Unicode handling, number format, omitted fields, `null`, and whitespace
must be fixed by an existing protocol rule or a canonical encoder.

## What remains handwritten

Some duplication is unavoidable, and some is desirable. The goal is to remove
accidental duplication, not all independent checking.

### Write once in the authoritative runtime

- product admission and approval;
- work lifecycle transitions;
- provider and model selection;
- receipt acceptance and public-claim decisions;
- Nostr event-to-product interpretation;
- lease, deadline, and retry policy;
- graph-wide and temporal invariants;
- database transactions; and
- domain-specific state recovery.

The other runtime consumes the result as a typed fact. It must not recreate the
decision algorithm and become a second authority.

### Implement independently when defense in depth requires it

Some constraints must be enforced on both sides of a trust boundary: maximum
frame size, path traversal rejection, generation fencing, digest shape,
credential redaction, or sandbox-envelope limits. These implementations can be
generated when the rule is structural. A complex enforcement rule can be
handwritten twice only when the AssuranceSpec requires independent enforcement.

That deliberate duplicate needs:

- one normative rule;
- shared valid and invalid vectors;
- differential tests;
- an explicit statement that neither implementation grants the other broader
  authority; and
- a named owner for resolving divergence.

This is defense in depth, not two product models.

## Versioning and compatibility law

The Contract Profile should make these rules mechanical:

1. A released field name, discriminator, enum encoding, and integer encoding
   do not change in place.
2. Deleted field numbers or names stay reserved in any numbered transport.
3. A same-major change is additive only when old readers can safely ignore the
   addition and the field is optional for old writers.
4. Adding an enum/union variant is breaking for any receiver whose declared
   behavior is reject-unknown.
5. Changing absent to `null`, or adding a decoder default, is a semantic change.
6. Tightening a constraint is breaking for existing writers unless a corpus
   proves no admitted value is excluded.
7. Loosening a command constraint can be security-relevant and requires an
   authority review even when wire-compatible.
8. Generated bindings identify the contract digest they implement.
9. A process handshake negotiates protocol version and capabilities; it does
   not infer compatibility from package version alone.
10. One release manifest binds the OpenAgents commit, Omega commit, contract
    digest, generated artifacts, binaries, and rollback-compatible predecessor.

## The bake-off should use hard contracts, not toy examples

Before selecting the source format, build the same corpus through three lanes:

1. Effect Schema to normalized JSON Schema to Rust.
2. Proto/Buf to Rust and generated Effect runtime schemas.
3. A restricted neutral Contract Profile to both targets.

The corpus should include:

- the `omega-effectd` request/response/host-request envelopes;
- one fully typed method family with command, result, and typed errors;
- a branded public reference with bounds and a forbidden-secret rule;
- optional, nullable, and optional-nullable fields side by side;
- a tagged union with an unknown future variant;
- `u64` and `u128`-class values with canonical JSON encodings;
- a recursive portable agent graph;
- a receipt with canonical digest vectors;
- a redacted public projection derived from a private aggregate;
- a map and a bounded byte payload;
- a Nostr event projection whose signature bytes remain governed by Nostr; and
- a cross-record invariant that intentionally stays outside the IDL.

Each lane must pass the same acceptance matrix:

| Gate | Required result |
| --- | --- |
| Type coverage | No construct silently degrades to `unknown`, `Value`, `any`, or an unbranded string |
| Encode/decode cross-product | Effect encode → Rust decode and Rust encode → Effect decode agree for every fixture |
| Negative parity | Both decoders reject every structurally invalid fixture for the same classified reason family |
| Canonical bytes | Both encoders produce identical bytes wherever the contract claims canonical encoding |
| Evolution | Old/new reader-writer combinations match the declared compatibility rules |
| Unknown behavior | Unknown fields and variants follow the contract's direction-specific policy |
| Determinism | Clean regeneration is byte-identical and offline |
| Reviewability | Generated diffs clearly expose wire changes and do not bury them in unrelated output |
| Runtime cost | Compile size, startup, frame size, and decode cost fit the actual boundary budget |
| Escape hatches | Every opaque JSON or handwritten adapter is visible in the manifest and justified |

If no candidate passes without custom metadata, that metadata defines the thin
OpenAgents IR. Do not choose Proto merely because its generator compiles, and
do not choose Effect export merely because it preserves the TypeScript side.

## Recommended first migration

Use `openagents.omega.effectd.v2` as the first complete generated seam rather
than trying to retrofit every cloud and product contract at once.

The packet should:

1. Inventory every v1 method, direction, parameter, result, error, capability,
   frame limit, and unknown-field rule.
2. Replace `params: unknown` and `serde_json::Value` with named method payloads
   where v2 supports the method.
3. Author the v2 contract once and generate the TypeScript and Rust bindings.
4. Preserve v1 as the rollback path during a bounded compatibility window.
5. Run both implementations against the same golden, negative, fuzz, restart,
   generation, timeout, and oversized-frame corpus.
6. Bind both artifacts and both repository commits in the component manifest.
7. Cut over atomically, then delete handwritten v2 declarations from both
   repositories.

Why this seam first:

- both implementations already exist;
- it has a clear process boundary and no network migration;
- it exposes the hardest practical problems: open payloads, method/result
  association, errors, capabilities, generations, size bounds, and restart;
- it can prove the cross-repository release workflow; and
- failure can roll back to the pinned v1 component without changing product
  domain storage.

After that proof, classify `cloud-contract` rather than mechanically generating
all of it. Separate exact wire identities from product-safe projections and
local Rust-only types. Migrate one contract family at a time and delete each
handwritten mirror only after cross-language proof.

## Repository shape if the experiment succeeds

This analysis does not authorize these paths, but a likely shape is:

```text
packages/cross-runtime-contracts/
  definitions/
    omega-effectd-v2.contract.json
    cloud-placement-v2.contract.json
  schema/
    openagents-contract-profile.schema.json
  generated/
    effect/
    json-schema/
    fixtures/
    manifests/
  compiler/

crates/openagents-contracts-generated/
  src/
  fixtures/
  manifests/
```

Omega should consume the Rust crate and manifest as immutable, digest-bound
artifacts from an OpenAgents contract release, or regenerate from the exact
pinned definition with a byte-identical compiler. Omega-specific domain
adapters remain in Omega. Product semantics remain in the OpenAgents package or
service that owns them.

The definition source, compiler version, generated outputs, and fixtures must
all be pinned. “Latest schema” is not a release identity.

## Immediate rules before a generator exists

The project does not need to wait for the bake-off to stop making the problem
worse.

1. Do not add a new hand-maintained cross-language mirror without a dated
   exception that names the deletion packet.
2. Label every existing similar type as exact mirror, projection, adapter, or
   local-only.
3. Give every exact mirror one canonical source and generate fixtures from it.
4. Require explicit wire names. Do not rely on independent camel-case,
   snake-case, or kebab-case conventions.
5. Forbid unconstrained JS `number` for values whose Rust range exceeds safe
   integer precision.
6. Forbid new `unknown`/`Value` payloads in stable method families unless the
   field is an explicit extension envelope.
7. Add generated-file headers and clean-regeneration checks to every generated
   zone.
8. Keep business validation in its one authoritative domain; do not port it
   merely to make generated structs look self-contained.
9. Treat cross-language differential tests as release evidence, not optional
   unit-test coverage.

## Failure modes to resist

### “Generate everything from the database schema”

Persistence, process protocol, public projection, and domain behavior are
different contracts. A database row is rarely the right command or Nostr
event. This approach leaks storage concerns and private fields across
boundaries.

### “Effect types are enough because TypeScript compiles”

TypeScript types disappear at runtime. Effect Schema provides runtime decoding,
but Rust still needs an exact generated representation and both sides need
encoding and compatibility law.

### “Serde derives mean Rust has a schema”

Serde describes encoding behavior attached to Rust types. It does not by
itself publish a complete, language-neutral contract, semantic validation,
compatibility policy, or generated Effect decoder.

### “Golden fixtures prove parity”

Golden fixtures prove only the examples they contain. They are one layer of a
system that also needs generated schemas, negative cases, property tests,
compatibility matrices, and drift checks.

### “One IDL means one authority”

A shared type definition does not decide who may mutate state. Authority must
remain explicit in the domain architecture. The generated model is a message
shape, not an authorization grant.

### “A giant shared Work object is simpler”

It is simpler only initially. It couples UI state, private execution context,
public projections, Nostr events, database rows, and receipts. Prefer small
purpose-specific commands, events, snapshots, and receipts connected by stable
refs.

## Decision and next action

The architectural decision should be:

> OpenAgents defines every cross-runtime wire contract once and generates its
> Effect and Rust structural bindings. Native domain models and semantic
> authority remain owned by one runtime. Protocol Buffers may be selected for
> a measured transport, but it is not the universal product model.

The next admitted work should be a contract-tooling spike, not a bulk model
rewrite. Its deliverable is the hard-corpus comparison, a selected restricted
profile, generated `omega-effectd.v2` bindings, and a cross-repository drift
gate. The spike fails if it requires hand-editing either generated target, loses
any required semantic distinction, or makes both runtimes authoritative for
the same domain.

This gives OpenAgents the useful part of “one definition”: one wire truth,
automatic language bindings, and mechanical drift detection. It avoids the
dangerous part: forcing Effect and Rust to pretend they have the same runtime,
state model, or authority.
