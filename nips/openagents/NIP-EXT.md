# NIP-EXT — Extension distribution

`draft` `optional` — v1. Proposed protocol; publication and installation
services are not implemented by this document. The [shared contracts](contracts.md)
are normative. [Architecture](../../docs/extensions/architecture.md) explains
why programs, plugins, skills, and execution bindings remain different things.

This NIP distributes immutable component packages with signed provenance.
It does not grant execution, synchronize private machine inventories, or
require the catalog to be online for every local invocation.

Packages can specialize any supported agent domain. A domain profile is an
assembly of existing schemas, sources, operations, programs, guidance, and
evaluations plus host-owned bindings and policy. It is not a new component kind
or permission mechanism. Coding packages add repository-specific behavior;
document, research, or business packages use the same distribution contract.

## Kinds

These are OpenAgents draft assignments, not upstream registrations.

| Kind | Class | Record |
| --- | --- | --- |
| `30184` | Addressable | Package listing/current-release pointer. |
| `3184` | Regular | Immutable release declaration. |
| `3185` | Regular | Irreversible revocation of a release. |
| `3186` | Regular | Namespace migration attestation. |
| `30185` | Addressable | Signed revocation checkpoint. |

Public bodies have `v: 1`, `requires`, `type`, and optional `meta`. Indexed
`t` is `oa:ext:<type>:v1`; type is `listing`, `release`, `revocation`,
`migration`, or `checkpoint`. Body/tag disagreement refuses. Public package
ID is `<root-pubkey>:<slug>`. The root signer owns that namespace; a familiar
display name, administrator listing, or transport login does not confer it.

## Release and package manifest

A `3184` body has `package`, `version` (human-readable string), and
`manifest: ArtifactRef` with schema `openagents.package.v1`. Signer MUST equal
the package root. A release is identified by its event ID and manifest digest,
not its version label. Different events rebinding a package/version to different
manifests are publisher equivocation: clients MUST report a conflict and MUST
NOT choose by timestamp. A verified installed pin remains explicit.

A manifest contains `v: "openagents.package.v1"`, `requires`, `package`,
`version`, `license`, `provenance`, `components`, `files`, `dependencies`, and
optional `meta`:

- `components`: distinct component slugs, each with `kind`,
  `definition: ArtifactRef`, and `descriptor: ArtifactRef`. Descriptor may be
  omitted for inert `guidance` and `schema` assets.
- `files`: distinct normalized relative POSIX paths with digest, exact byte
  length, and media type. Directory entries are unnecessary. Links are forbidden.
- `dependencies`: exact release EventRefs with manifest ArtifactRefs. Version
  ranges are a resolver input outside the immutable release, never a run pin.
- `provenance`: source reference, build/fixture receipt ArtifactRefs, and
  explicit unknowns. Local builds and independently verified prebuilt releases
  have distinct provenance; neither is automatically hermetic or attested.

Manifest package/version MUST match the release. Component IDs derive from
package root/slug plus component slug. All definition, descriptor, schema,
guidance, and binary references must resolve to listed files or explicit locked
dependencies. Reject cycles, duplicate/conflicting IDs, unsupported semantics,
path traversal, absolute paths, normalized/case-colliding paths, unlisted
executable bytes, and excessive total size or expansion. Check limits before
allocation. This initial package transport is a manifest plus individually
addressed files; arbitrary archive unpacking is not required for interoperability.

NIP-94 `1063` events can locate bytes by MIME type, size, and hash; a locator
must match the ArtifactRef. NIP-51 release-artifact sets may curate these
locators. They are discovery aids, not substitutes for this immutable release,
dependency closure, or authority contract. A replaceable set is never an
execution pin by itself.

## Component types and operation descriptors

Kinds are `program`, `plugin`, `capability`, `decision-function`, `skill`,
`source`, `checker`, `operation`, `guidance`, and `schema`. Executable native
bindings remain host-owned; their declarations cannot self-install adapters.
Program/plugin/capability definitions follow their NIPs. Source, checker, and
operation definitions have `v: "openagents.binding.v1"`, `requires`, `id`,
`kind`, `capability: DefinitionRef`, `operation` (slug), input/output
SchemaRefs, `effects`, and `minimum`. They resolve only to host-supported
bindings and pinned schemas. Guidance is an inert text artifact; schema assets
follow the shared SchemaRef dialect. Unknown definition kinds refuse.

An operation descriptor contains `v: "openagents.operation.v1"`, `requires`,
`id`, `kind`, `definition` (ArtifactRef), `summary`, `input`, `output`,
`preconditions`, `effects`, `minimum`, `guidance`, and optional `evaluation`
and `meta`. Input/output are SchemaRefs; effects/bounds use common contracts;
guidance is an array of ArtifactRefs; evaluation is an array of signed result
EventRefs or ArtifactRefs with explicit provenance. Preconditions contain
supported format IDs and required evidence schema IDs. No free-form text is
an executable condition. The descriptor must agree with its definition.

Descriptors support bounded inert indexing, mechanical filtering, shortlist
retrieval, and optional semantic selection. Full schemas/manuals load after
selection. Their wording is digested: editing a summary creates a new release
and evaluation identity. Selection does not install, enable, grant, or dispatch.
Record candidates and omissions; absence from a shortlist is not proof that
an operation cannot help. The ranking algorithm is host policy. Descriptions
and manuals remain untrusted content: they cannot change selector rules,
request credentials, suppress errors, or override mandatory instructions.

### Decision functions

A function definition has `v: "openagents.function.v1"`, `requires`, `id`,
input/output SchemaRefs, `state_builder` (registered operation DefinitionRef),
`questions` (ArtifactRef), `policy` (DefinitionRef), `model_requirements`
(`apis` and `schemas` arrays plus boolean `requires_scorable_answer`),
`bounds`, and
`evaluation` references. State/options actually served are recorded at call
time. The host separately admits a model/artifact and recipient; a package
cannot authorize a provider. Question wording remains a pinned artifact.

Policies consume typed results under explicit abstention/refusal rules.
Confidence is not permission or proof of calibration. A function cannot call
itself through a hidden guest network path or redirect a local-only request
to a hosted reviewer. Every attempt uses the shared accounting/receipt path.

### Skills and bounded hooks

A skill definition has `v: "openagents.skill.v1"`, `requires`, `id`,
`description`, `body` (ArtifactRef), `applicability` (format/task-class IDs),
`allowed_operations` (qualified IDs), `lifetime` (`operation`, `task`, or
`session`), and `hooks` (possibly empty). Optional bodies are progressively
loaded and revision-checked. Mandatory host/user/scope constraints are
resolved by scope and precedence independently of semantic relevance.

A hook has `event`, `operation: DefinitionRef`, typed `input`, `bounds`,
`on_error` (`stop` or `continue`), and `destination` (`evidence` or `view`).
Supported initial event IDs are `context.requested`, `operation.completed`,
and `task.closed`; their payload schemas are pinned in the definition.
Host activation requires a compatible event binding, scope, and authority.
Unknown events refuse. No shell scripts, arbitrary new event names, recursive
hook triggering, or lifetime extension are allowed. Allowed-operation lists
only narrow authority. Explicit host choice is required for session lifetime.

## Listings, updates, and installation

A `30184` listing has `d` equal to the package slug; signer equals root.
Body contains `package`, `state` (`published` or `hidden`), `release` (exact
EventRef or null for hidden), and bounded display `title`/`description`.
Draft is a local preparation state; publishing the listing is an external
effect. A hidden listing does not revoke a known release. A failed/new invalid
release must not replace the last usable installed lock.

Installation resolves and verifies the whole closure in inert staging, then
atomically commits one lock. Installation MUST NOT run builds, post-install
scripts, probes, services, inference, or publication. Enablement, grants, and
invocation admission are separate operations. A component can be installed
but unavailable. Updates show changed identities, effects, dependencies,
schemas, and selector wording, and require explicit operator policy.

Build/package commands do not imply publish authorization. Build provenance
binds relevant sources, local dependencies, lockfile, hidden configuration,
toolchain, and artifact. Source changes during/after a build invalidate its
freshness claim; failed/pending receipts cannot certify a release. Human and
model-facing authoring surfaces use one bounded authorization service.

Concurrent updates use revision checks and an atomic lock commit. Running
attempts retain exact pins; there is no hot swap. Rollback selects a previously
verified, still-eligible lock. Uninstall first disables new activation and
removes the installation reference, then cleans unreferenced bytes. Cleanup
failure creates a retryable tombstone and MUST NOT reactivate the package.
Active runs and user retention/deletion policy govern retained evidence.

## Revocation and freshness

A `3185` body has `package`, `release: EventRef`, `reason`, and `effective_at`.
Signer MUST equal the release's package root. An `e` tag names the release.
`effective_at` must not exceed event `created_at`; verified revocation applies
immediately, and the timestamp records attribution rather than deferred action.
Revocation is monotone and irreversible for that exact release identity;
republishing a listing cannot undo it. Third-party assessments are advisories,
not publisher revocations. Hosts may impose stronger local deny policies.

A `30185` checkpoint has package slug `d`, `package`, monotone `revision`,
`as_of`, `valid_until`, and `revocations`, the complete sorted list of exact
revocation EventRefs, sorted by event ID with no duplicates, for that package
as of the checkpoint. If the list exceeds
the wire bound, `revocations` is an ArtifactRef to that sorted list instead;
clients must resolve it completely. Signer equals root. Equal revision with
different contents, or omission of a previously observed revocation, is a
conflict. Clients retain the union of verified revocations and highest accepted
revision. Timestamp replacement alone cannot roll it back.

Host policy determines required freshness and whether explicitly pinned offline
use is permitted. When current revocation knowledge is required, verify the
checkpoint, referenced revocations, `as_of` age, and expiration against a
trusted clock. Refuse future-dated `as_of` beyond the configured skew window
and require `valid_until` to follow `as_of`. A query returning no revocations
proves nothing. Missing/stale
checkpoints refuse under strict policy; they do not become fresh because a
relay answered. No universal freshness duration is specified.

Revocation stops new admission/dispatch. Active work follows cancellation
policy, retaining unknown outcomes when stop cannot be confirmed. Completed
effects and historical records are not erased. Reinstatement requires a new
release and explicit policy; it is not deletion of the revocation event.

## Namespace migration

Root public-key namespaces never silently transfer. A `3186` body contains
`from`, `to`, `migration` (random 64-hex ID), and `role` (`offer` or `accept`).
An offer is signed by the old root; acceptance by the new root additionally
contains `offer: EventRef` and an `e` tag naming it. Package names may differ.
Verify both events and matching fields. This establishes consent to migration,
not mutation of old release identity or authority to sign as the old key.

Clients require explicit approval to follow the new namespace, re-resolve locks,
and recheck grants. A compromised/lost old key cannot establish this two-party
proof; out-of-band recovery is an explicit new trust decision. Never infer
ownership from whichever migration event a relay delivers last.

## Relay and client conformance

Relays validate public body/tag syntax, signature/namespace alignment, and
reference shapes. Clients additionally resolve closure and verify referenced
ownership, schemas, revocations, and host authority. Regular releases and
revocations are retained under a published policy; replicas may mirror signed
bytes without becoming publishers. Removal from one relay is neither global
erasure nor revocation. Private releases require a future defined profile;
this public catalog must not carry private evidence or credentials.

Advertise `nip-ext-v1` only after publication, lookup, replacement, retention,
and malformed/conflict paths are fixture-backed. Required client cases include
tampering, unavailable historical bytes, cycles, source replacement, label
equivocation, stale/withheld checkpoints, namespace spoofing, interrupted
installation, uninstall failure, rollback, and preservation of active pins.
