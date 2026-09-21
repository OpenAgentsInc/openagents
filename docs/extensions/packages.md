# Packages and distribution

Status: target specification. Local capability and program registries exist;
this portable package, catalog, and lifecycle contract is proposed.

## Package contents

An extension package is an immutable distribution bundle. It can contain
programs, Wasm plugins, operation descriptors, question sets and decision
function definitions, skills/guidance, schemas, fixtures, and documentation.
It can declare dependencies on host adapters but cannot install an executable
adapter into a trust store by shipping a manifest that approves itself.

The package manifest binds its publisher-qualified identity, human-readable
version, component identities, file byte lengths and digests, compatibility,
license/provenance, dependency requirements, required effects, and evidence
references. The finalized versioned format must define canonical manifest
encoding, digest algorithms, signing payload, path normalization, and size
limits with test vectors. No package identity may depend on incidental
archive ordering or an unsigned display name.

Validate the entire archive before activation. Reject absolute paths, parent
traversal, escaping links, duplicate normalized paths, case collisions on the
destination filesystem, unlisted executable payloads, excessive expansion,
and unsupported required semantics. Documentation and fixtures are untrusted
content too. Installation must not execute post-install scripts, build code,
run capability probes, start services, or contact a model provider.

Component types retain their own admission rules. A package with one useful
pure plugin does not gain the permissions of its bundled executor descriptor.
Expose per-component enablement and make unsatisfied dependencies explicit.

## Identity, dependencies, and locks

Distinguish these objects:

| Object | Identity and mutability |
| --- | --- |
| Publisher | A verified signing identity or explicitly trusted local identity; display text is not ownership. |
| Package | Publisher-qualified name with documented ownership/transfer rules. |
| Release | Immutable manifest and content digests; a version label cannot be rebound to different bytes. |
| Listing | Mutable description, discoverability, review information, and release pointers. |
| Installation lock | Exact resolved component/dependency digests and source provenance for one installed revision. |
| Run lock | Installation lock plus host bindings, policies, grants, schemas, question sets, and execution configuration used by an attempt. |

Resolve dependency ranges into exact digests before activation. Reject cycles,
incompatible interfaces, conflicting component identities, ambiguous source
precedence, and missing transitive content. Bound resolution depth, total
components, archive size, and fetch attempts. Verify every dependency using
the same rules as the root package; trust does not transfer from a trusted
root to an arbitrary transitive publisher.

Existing local registries use first-definition-wins slug lookup. Preserve that
behavior for legacy runs and record the resolved digest. Portable packages
must use qualified identities and explicit bindings so installing another
package cannot shadow a previously selected component. Migration produces a
new lock; it does not rewrite earlier execution records.

Locks contain portable identities and public dependency locators, not private
absolute paths, access tokens, host credentials, or machine inventories.
Local bindings supply secrets and machine-specific paths outside the package.
If a declared destination cannot be reached under host policy, report an
unavailable dependency rather than substitute a different provider silently.

## Trust is several independent decisions

Content verification answers which bytes arrived. Publisher verification
answers who signed them. A catalog review reports what was reviewed. None of
these answers whether the component may execute on this host for this task.

Track these states independently:

1. Known metadata: a local index or catalog describes a release.
2. Verified content: all required bytes and identities validate.
3. Installed: an atomic lock references retained verified content.
4. Enabled: host policy allows discovery or specified roles.
5. Granted: required access has been approved outside the checkout.
6. Admitted: this invocation satisfies task scope, current policy, compatibility,
   revocation, input validation, and resource bounds.

Disabling a component prevents new invocation without deleting its history.
Installing a package must not create program grants, approve executable
probes, or widen an existing turn's permit. Repository-provided configuration
is a proposal; it cannot override operator policy. Follow the existing
[program authority](../coder/guides/program-authority.md) and
[capability trust](../programs.md) boundaries.

## Installation and updates

Use one installation service across terminal, headless, CLI, and automation:

1. Inspect an inert manifest and produce a dependency/permission change plan.
2. Resolve bounded fetches, verify content, and stage an immutable closure.
3. Check compatibility and any required current trust/revocation information.
4. Atomically commit the new installation lock. Interrupted staging leaves
   the previous complete lock usable.
5. Apply separately authorized enablement and grants. Report components that
   are installed but cannot run.

Updates are explicit. Show changed components, permissions, dependencies,
schemas, selection wording, and evaluation identities. Reuse grants only if
their own policy covers the exact new binding; a new artifact cannot inherit
digest-pinned executable approval. A catalog's current-release pointer may
suggest an update but cannot alter an installed or running revision.

Pin all code and definitions at run admission. No hot swap during a program
step, tool call, or plugin instance. Recheck revocation and mutable authority
at each dispatch. A revocation can stop queued work and trigger cancellation
of active work according to host policy; it cannot make an already completed
effect disappear. Preserve an unknown outcome when cancellation cannot be
confirmed.

Rollback selects a previously verified, still-eligible lock. It does not
restore revoked authority or overwrite another run's artifacts. Concurrent
install/update/uninstall operations use revision checks and one atomic commit
boundary so a stale operation cannot replace a newer lock.

## Uninstall, retention, and cleanup

Uninstall first disables new activation and removes the installation reference
atomically. Active runs keep their pinned content until completion or explicit
cancellation. Delete unreferenced content afterward. A cleanup failure leaves
a visible tombstone and a retryable cleanup operation; it must not restore the
installation reference and make the package appear enabled again.

Content retention accounts for other installations, run locks, replay records,
and user retention settings. Evidence deletion is a separate scoped operation.
Deleting evidence can make a replay unavailable; record that explicitly.
Retention is not a reason to ignore a user's deletion request. Garbage
collection never executes package code and must not follow package-supplied
paths outside its managed store.

## Catalog and publication

The catalog distributes metadata and locators. It is not an inference service,
a grant service, or a new private backend that the runtime must call before
every local operation. Start with local packages and verified static sources;
add remote publication only through an explicitly versioned public contract.

Prefer the repository's existing Nostr identity and relay model when remote
distribution is introduced. Keep existing event meanings intact:

- NIP-CAP `30180` describes a capability; `30181` describes operator policy.
- NIP-PRG `30182` describes a program; `30183` locates a module whose bytes
  must match the hash required by the program.
- [NIP-EXT](../../nips/openagents/NIP-EXT.md) defines releases, listings,
  revocation checkpoints, and namespace migration. The module announcement
  remains a locator and does not implement this catalog.

Implement and verify NIP-EXT ownership proofs, signed payloads, explicit
namespace migration, immutable release validation, revocation ordering, and
relay policy before publication is enabled.
It must not import the reference service's private endpoints or authentication
implementation. A verified transport session does not by itself establish
ownership of every package name.

Use separate catalog states for draft, published, hidden, and revoked.
Publishing makes an immutable release discoverable; hiding removes ordinary
discovery without rewriting its identity; revocation records a reason that
clients evaluate under policy. Retain prior release identities and audit
history. A listing update or rejected release must leave the prior valid
release pointer intact. Concurrent publication uses explicit revision checks.

Publication is a separately authorized external effect. Build/package commands
may prepare a reviewable release without publishing it. A model-facing surface
must use the same authorization and validation service as the human surface.
Author descriptions, downloads, catalog placement, and review badges are not
proof of safety or task performance.

## Offline and freshness policy

An already verified pure local operation should not need the catalog on each
invocation. Every source records origin, signer, fetched time, content version,
and revocation information. Host policy specifies the maximum age and required
availability of trust/revocation information for each component class.

When required information is stale or unavailable, report that condition and
follow the configured policy. A strict policy refuses new dispatch; an
explicit offline policy may allow a pinned release within its stated scope.
No clock, network failure, or absent record may silently manufacture a fresh
verification. There is no universal freshness duration in this specification.
Revoked identities remain distinguishable from merely unavailable sources.

## User-facing surfaces

Programs are presented by the workflows they perform. Extensions show the
packages and component types installed, enabled, unavailable, or awaiting
authority. Plugin details show the actual Wasm access mode and effective
limits. Task records show selection, invocation, fallback, and verification.

Inspection, installation, enablement, grant management, update, rollback,
uninstall, and publication must have shared semantics across the terminal and
headless interfaces. Use explicit typed results for partial failure. Do not
make the model navigate UI state to manage packages or interpret a prose
success message as an execution receipt.
