# Verification and conformance

Status: verification plan plus initial specification checks. No engine runtime,
security boundary, performance budget, or browser profile is qualified yet.

## Evidence levels

| Level | Evidence | What it establishes |
| --- | --- | --- |
| D, documentation | Local links, requirement owners, schema/example checks | The design corpus is navigable and its fixtures follow its declared wire slice |
| M, bounded model | Exhaustive exploration of a deliberately small state space | Selected invariant holds within stated abstraction and bounds |
| C, component | Rust/TypeScript/VM/compiler unit, property, fuzz, and differential tests | Implemented component behavior under tested inputs |
| I, integration | Browser → command → document → compiler → runtime → storage | A complete implementation story works, including failure and recovery |
| Q, qualification | Pinned workload/device/browser/network/security/accessibility runs | Declared product profile meets its acceptance contract |

Passing D or M does not establish C, I, or Q. A model cannot prove a shader safe,
a schema cannot prove topology valid, and a screenshot cannot prove collision or
durability. The [invariant ledger](INVARIANTS.md) identifies the required level
and contract owner for each safety property.

## Initial repository check

Run `python3 tools/check_spec.py`. The check uses the standard library and requires
no engine dependencies or network. It verifies:

- Local Markdown targets and anchors, indexes, and requirement owner paths.
- Unique requirement IDs, status values, and milestone links.
- JSON syntax and the schema keywords used by the provided contracts; positive
  fixtures and deliberately invalid fixture variants.
- SHA-256 identities of the canonical synthetic ASCII/integer fixtures.
- Abstract atomic generation activation and channel compare-and-swap models.

The bundled validator deliberately covers only the documented schema subset used
here. It is not a general JSON Schema implementation and must not become a
production validator. Before implementing wire decoding, run the same fixtures
with a full Draft 2020-12 validator and generated Rust/TypeScript decoders. The
schema dialect is [JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12).

The initial design review also ran all five schemas through full Draft 2020-12
meta-schema validation and repeated the positive/negative fixture checks using
`jsonschema` 4.26.0 in an isolated tool environment. This does not replace future
generated-decoder or runtime conformance. See the [initial review](design-review.md).

## Bounded model scopes

**Generation model:** two desired edit revisions; compilation can complete or
fail in either order; desired can advance while a prior candidate is pending.
Only complete current desired candidates activate. Render, collision, navigation,
and selection install one generation atomically. Old activation after newer
failure is forbidden. A deliberately broken activation predicate must yield a
counterexample, establishing that the checker can detect the modeled defect.

**Channel model:** two publishers propose different admitted release digests
against the same previous reference and sequence. Both request orderings are
explored. Only one stale-base proposal succeeds; valid subsequent updates are
monotonic. Missing/unadmitted releases cannot become channel heads. A broken CAS
rule must produce a lost-update counterexample.

These models abstract away cryptography, storage crash consistency, browser/GPU
scheduling, networks, and resource allocation. Their counterexamples motivate
future implementation regressions. They do not prove those omitted systems.

## Component conformance obligations

- **Frontend:** React DOM/Effect Atom/Effect 4 adapter, project-scoped registries,
  pure renders, command/service ownership, bounded summaries, stale result
  rejection, Strict Mode and panel lifetimes. Cover [FE-01–FE-08](frontend.md#acceptance-requirements).
- **World/commands:** UUID/reference stability, exact measurement conversion,
  closed schemas, duplicate-key rejection, topology intersections, shared edges,
  openings/clearance, bounded batches, inverse intents, migrations, fuzzed sizes.
- **Compiler:** deterministic cache keys, exact source/tool/profile identity,
  missing/corrupt dependency rejection, minimal dependency closure, seam agreement,
  fresh-vs-cached equivalence, out-of-order completion, cancellation, peak residency.
- **Kernel/ABI:** finite inputs, ownership/sequence checks, stale/freed slabs,
  integer overflow, untrusted lengths, origin shifts, tick ordering, state snapshots.
- **Renderer:** pipeline diagnostics, bind-layout mismatch, obsolete compilation,
  resource lifetime, resize, image-copy bounds, device loss, optional-feature paths.
- **Behavior VM:** bytecode verification, instruction/size fuel, bounded host calls,
  runaway loops, stack/heap limits, typed state, capability revocation, migration,
  async reply freshness, supported-language differential cases, source locations.
- **Services:** branch-head CAS, exact retries/mismatched IDs, lost acknowledgment,
  outbox replay, authorization changes, merge conflicts, CRDT frontier compaction,
  release signatures, key revocation, channel freshness, garbage collection roots.

## End-to-end matrix

| Story | Required assertions |
| --- | --- |
| Enter and extend a house | Controller traverses visible door; new room support/nav agree; undo never removes active floor silently |
| Shared-wall edit | Either adjacent room resolves the same wall ID; new opening renders and permits movement in one generation |
| Shader/behavior error | Same pose/approved state keeps running; repair works; old candidates cannot win |
| UI/engine independence | With inspectors unsubscribed, panels remounting, React commits delayed, or status consumers slow, ticks/GPU submission have no UI wait dependency; queues stay bounded, receipts remain recoverable, and integrated frame/edit budgets pass |
| Medieval Town rendering | All 18 source images are pinned; 16 scenic recipes pass composition/material/light/atmosphere review and motion checks; overview/composite use their separate gates; private capture evidence records settings, current bake identities, and resource cost |
| Storage/full/crash | Local vs remote save labels are accurate; durable retry produces one result; export remains available |
| Offline conflict | Both source branches survive; merge has explicit structural conflicts; inverse respects other edits |
| Hostile content | Cross-origin message forgery, token theft, network exfiltration, import bombs, infinite script, shader stress are contained or fail closed within the declared threat model |
| Publish/rollback | Complete signed release selected; stale channel update refused; active sessions follow migration policy |
| Lifecycle | Mobile suspend/resume, pointer/audio refusal, resize, device reset, revoked membership, and cache eviction preserve recoverable source |

Every browser run uses its own profile/port and synthetic accounts. Run local
fixtures before production integration. Never reuse an owner's production data
for smokes; test artifacts are cleaned up with recorded identifiers.

## Qualification record

Evidence includes commit, toolchain/lockfiles, exact test command, world/workload
digests, ABI/profile, browser/OS/adapter/driver, viewport/resolution, network/thermal
conditions, seeds, outcomes, raw bounded timing samples, and limitations. Keep
credentials, licensed content, personal data, and private prompts out of public
artifacts. When evidence cannot be redistributed, publish an aggregate report and
an eligible synthetic reproduction instead.

Accessibility acceptance includes keyboard-only journeys, screen-reader hierarchy
and selection, motion/contrast settings, touch targets, error focus, and controls
that do not depend on color or a locked pointer. Follow [creator experience](creator-experience.md)
and evaluate against the declared WCAG scope; automated checks alone do not
establish conformance.

## Regression and decision policy

Run affected component tests and contract consumers for ordinary changes. Run
cross-profile integration/qualification when the relevant boundary changes.
Record unrelated failures separately; do not weaken an invariant to pass a test.
A benchmark improvement must preserve quality, semantics, safety, recovery, and
supported profiles. A meaningful counterexample becomes a fixture or an explicit
documented exclusion before the affected feature can qualify.
