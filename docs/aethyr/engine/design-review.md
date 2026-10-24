# Initial specification review

Status: design review, 2026-10-22. This report covers the initial documentation
and synthetic contract fixtures. It does not qualify an engine implementation.

## Review scope

Parallel reviews examined creator experience and world semantics; runtime,
rendering, compilation, and behavior execution; collaboration, publishing,
security, and operations. Integration review compared their shared contracts,
wire fixtures, acceptance obligations, and ownership boundaries.

## Resolved findings

| Finding | Resolution and owner |
| --- | --- |
| Worker ownership could imply a full-world copy every frame | Baseline simulation and GPU encoding share one preview worker; independent compilation and guest behavior work remain bounded. [Architecture](architecture.md) |
| A saved edit could be mistaken for a safely running edit | Document acceptance and generation activation are separate; unsafe activation holds the previous generation without undoing shared source. [Contracts](contracts.md) |
| Rebuilding topology could create new authoritative IDs | Authoring transactions persist semantic IDs and remapping; compilers reuse source identities. [World model](world-model.md) |
| A camera movement could invalidate an otherwise clear relative command | Resolve against captured view; selection, target, catalog, and document dependencies determine freshness. [AI authoring](ai-authoring.md) |
| ABI labels and replay storage were ambiguous | Map portable kernel/VM IDs to binary versions; retain compact replay outcomes instead of maximum-sized response slabs. [Runtime ABI](runtime-abi.md), [bytecode](behavior-bytecode.md) |
| Release/channel prose exceeded their closed wire schemas | Pin exact compiler/runtime identities, startup closure, features, offline policy, signer, times, and trust version in the fixtures. [Contracts](contracts.md#releases-and-signatures) |
| Restoring an old authoring database could revive revoked publication authority | Separate quorum-backed publication/trust control, preserve admission pins, and use new recovery branch identities when a current head cannot be established. [Operations](operations.md) |
| Offline or cached signatures could be mistaken for current authorization | Distinguish cached trust from live-quorum resolution; check nonce, current key state, and sequence/digest/trust high-water marks. [Publishing](publishing.md) |
| An uncertain portal transfer could duplicate session ownership | Keep a durable pending transfer until its coordinator decision is recovered. [Collaboration](collaboration.md) |
| Full engine scope could collapse into a house-placement demo | Specify game systems and complete-engine gates, with separate Haunted House and Medieval Town projects. [Product](product.md), [delivery](delivery.md) |

## Checks completed

- `python3 tools/check_spec.py`: local links and anchors, requirement ownership,
  strict JSON, schema/example consistency, synthetic digests, structural fixture
  references, and bounded abstract models pass.
- Full Draft 2020-12 validation with `jsonschema` 4.26.0: all five schema
  meta-schemas, five positive examples, and eleven invalid fixture variants pass
  their expected acceptance or rejection.
- Bounded activation exploration covers 21 states; channel race exploration covers
  six schedules. Deliberately broken rules produce counterexamples. These are
  small abstract models, not proofs of browser or distributed implementation.
- `git diff --check`: whitespace validation passes for the staged specification.

## Evidence still required

There is no runtime, browser test, performance benchmark, security audit,
accessibility study, disaster drill, or production deployment in this commit.
The [experiments](risks-and-experiments.md) and [qualification plan](verification.md)
define those obligations. Component/operation registries and service protocol
profiles must be frozen at their [delivery gates](delivery.md), with cross-language
fixtures. The owner must select a repository implementation license before
distributing engine code; this specification does not grant one.

The architecture is selected, while its performance and product claims remain
falsifiable. A failed experiment changes the design or supported profile before
the affected capability is advertised as qualified.
