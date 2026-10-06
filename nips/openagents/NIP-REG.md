# NIP-REG — Curated Plugin Registries

`draft` `optional` — v1, 2026-10-06. **Designed.** The
[shared contracts](contracts.md) and [NIP-EXT](NIP-EXT.md) are normative.

This NIP lets a client subscribe to any operator-selected plugin registry.
A curator publishes a complete catalog of exact extension releases. Nostr
relays, HTTPS servers, and GitHub repositories can deliver the same signed
records and artifact bytes. Registry membership is the curator's recommendation;
the package publisher, client, and executing host retain their own roles.

## Status and scope

Designed. No registry publisher, registry reader, GitHub adapter, or Coder
registry management UI is implemented by this document. Existing EXT validators
do not establish REG conformance. Coder's current OpenRouter BYOK settings
are local configuration for a host-supported adapter, not a downloaded package.

EXT already defines package identity, releases, publisher listings,
revocations, dependency locks, inert installation, and updates. This NIP adds
curator identity, catalog membership, source configuration, refresh, and
transport equivalence. It does not define another plugin runtime, installer,
credential service, or permission system.

## Composition and event kinds

A registry catalog is itself an inert EXT package owned by its curator.
Its ordinary EXT listing points to an immutable catalog release.

| Existing contract | Role here |
| --- | --- |
| EXT `30184` | Curator's registry-package listing and current catalog-release pointer. |
| EXT `3184` | Immutable declaration of one catalog release, or one member package release. |
| EXT `3185`, `30185`, `3186` | Publisher revocation, freshness checkpoints, and explicit namespace migration. Apply independently to the registry package and member packages. |
| [NIP-01](../official/01.md), [NIP-19](../official/19.md) | Signed events, addressable lookup, and shareable pointers. |
| [NIP-94](../official/94.md) `1063` | Optional file locators matching exact artifact hashes and sizes. |
| [NIP-51](../official/51.md) `30063` | Optional artifact set for one catalog release or one plugin release. |
| [NIP-PRG](NIP-PRG.md), [NIP-CAP](NIP-CAP.md), [NIP-POL](NIP-POL.md) | Supported component interfaces, host bindings, and local authority after selection. |

REG allocates no event kinds and changes no EXT event body or tag semantics.
A `30063` set groups the artifacts of one software release; it is not the
registry's membership list. Do not put member `3184` references in it and
claim NIP-51 compatibility. NIP-34 repository announcements can locate source
collaboration, but do not replace a release pin. NIP-89 handler discovery and
NIP-78 application data are not this public registry contract.

## Registry identity and enrollment

The registry ID is the EXT package ID `<curator-pubkey>:<registry-slug>`.
The curator is a lowercase 64-hex Nostr public key; the slug follows the shared
grammar. Its address is `30184:<curator-pubkey>:<registry-slug>`. Display names,
relay URLs, GitHub owners, repository names, branches, and TLS certificates
do not establish the curator's identity.

A client MUST record the operator's decision to trust this exact registry ID
before accepting its recommendations. The operator can enroll multiple
registries and remove any registry. Enrollment does not trust every release
as executable, confer publisher ownership, or enable a component. A built-in
default registry MUST have an explicitly pinned identity and visible source
configuration; it cannot prevent adding independently operated registries.

Accepted enrollment records keep, locally:

- The registry ID and either its changing address or an exact `3184` catalog
  release EventRef. The latter is a frozen snapshot, not an update channel.
- Nostr relay hints and/or HTTPS mirror URLs. A GitHub source additionally
  identifies owner, repository, ref, and directory prefix for the mirror below.
- Finite fetch deadlines, byte/request limits, maximum catalog age, and allowed
  clock skew. These settings belong to the client, not the remote catalog.
- Whether offline browsing and previously pinned use are allowed, and whether
  current membership is required for new installation or admission.

NIP-19 `naddr` can share the address; `nevent` can share an exact catalog
release. Convert them to validated wire identities. Relay hints are optional
locators. Pasting a URL without a previously trusted key requires an explicit
first-use decision about the verified curator key; the serving website cannot
enroll its own key silently. Source changes retain the same remembered registry
identity, revision, and conflicts. Following a new curator key is a separate
trust decision under EXT namespace migration, not an automatic URL redirect.

## The catalog package

The curator publishes an ordinary EXT manifest with these additional profile
requirements:

1. `package` equals the registry ID. The signed `3184` release and `30184`
   listing have that curator as their author and satisfy EXT unchanged.
2. `files` includes `registry.json` with media type `application/json` and
   `registry.schema.json` with media type `application/schema+json`. These are
   ordinary EXT file rows: path, digest, size, and media type. Construct the
   catalog ArtifactRef from its row with schema `openagents.registry.v1`;
   the schema file supplies the catalog's shared JSON Schema 2020-12 SchemaRef.
3. A component with slug `catalog` and kind `schema` names that SchemaRef as
   its `definition`. Only `schema` and inert `guidance` components are allowed;
   no executable component or package dependency is allowed in this catalog
   package. Any additional schema/guidance files remain in its verified closure.
4. `version` is `registry-<revision>`, using the catalog's positive decimal
   revision without leading zeros. Reusing that version for different bytes
   is the EXT publisher-equivocation case.

The published schema MUST enforce this NIP's fields, bounds, and reference
shapes. Clients also enforce its semantic rules; a curator cannot weaken them
by publishing a permissive schema. Remote schema loading is forbidden.
An ordinary package named "plugins" is not a registry without this complete
profile. Opening or following a registry need not install its catalog package.

Member entries are inert catalog data, not dependencies of the catalog package.
Reading one registry MUST NOT download or install every member's executable
closure. Resolve a selected member's closure separately under EXT.

## Catalog document

`registry.json` is a closed object, except for optional inert `meta`.
The common encoding, unsupported-feature, and exact-byte digest rules apply.

| Field | Contract |
| --- | --- |
| `v` | `"openagents.registry.v1"`. |
| `requires` | Empty array for this initial profile. Unknown features refuse. |
| `registry` | Registry ID; MUST match the enclosing package and curator. |
| `revision` | Positive shared integer, increased on every catalog change. |
| `as_of` | Unix seconds: when the curator states this membership. |
| `valid_until` | Unix seconds greater than `as_of`; bounds the curator's freshness claim. |
| `title` | Plain text, 1–128 Unicode scalar values. |
| `description` | Plain text, at most 2,048 Unicode scalar values. |
| `entries` | Complete membership, at most 1,000 entries, sorted by `package`; no duplicate package IDs. Empty is valid. |
| `meta` | Optional inert annotations. No trust, authority, or update rules. |

The catalog is at most 1,048,576 bytes; clients MAY impose lower documented
limits. `as_of` MUST NOT exceed the enclosing release's `created_at` beyond
the client's allowed skew. Clients reject future-dated releases and catalogs
outside that skew. Local maximum age can be stricter than `valid_until`.

An entry is a closed object with:

| Field | Contract |
| --- | --- |
| `package` | Publisher-qualified EXT package ID. The publisher may differ from the curator. |
| `release` | Exact EventRef of its `3184` release; signer MUST equal that package's root. No mutable coordinate or version range can substitute for the ID. |
| `manifest` | ArtifactRef with schema `openagents.package.v1`; MUST equal the release's manifest reference in identity, size, media type, and schema. Locator hints can differ without changing identity. |
| `title` | Plain text, 1–128 Unicode scalar values. |
| `summary` | Plain text, at most 2,048 Unicode scalar values. |
| `configuration` | Optional SchemaRef for local settings, already in the selected package's verified file/dependency closure. It grants no access or runtime behavior. |
| `evaluation` | Optional array, at most 32 exact EVAL `3189` EventRefs or ArtifactRefs with schema `openagents.eval-report.v1`. Evidence is checked under EVAL, not inferred from inclusion. |
| `meta` | Optional inert annotations. |

The curator signs the catalog package, not the member publisher's declaration.
Clients MUST verify member signatures, identities, manifests, closures, and
revocations independently. A title such as "OpenRouter BYOK" is display text,
not the package ID. Unavailable bytes or an unsupported component remain an
explicit unavailable state, not permission to fetch an unpinned alternative.

Before presenting an evaluation as evidence for a member, resolve and verify
the report's subject and lock against that member's exact component closure
under EVAL. A report about another version, task, or component is not evidence
for this entry. Unavailable evidence remains unverified.

Catalog prose, schemas, URLs, and evaluation summaries are untrusted data.
Render display text inertly, escaping terminal controls and ANSI sequences.
They cannot change client instructions, request credentials during browsing,
hide failures, or approve installation, network access, or execution.

## Nostr publication and lookup

Publish the catalog files and manifest before the curator-signed `3184`
release, then update the normal `30184` listing to that exact release.
Use the EXT `oa:ext:release:v1` and `oa:ext:listing:v1` tags without alteration.
Readers look up the configured address with a bounded filter:

```json
{"kinds":[30184],"authors":["<curator-pubkey>"],"#d":["<registry-slug>"],"limit":1}
```

The placeholder strings above describe a filter, not a signed fixture.
After validating the listing, request its release by exact event ID and
resolve the pinned manifest and catalog bytes. Check every fetched event's
NIP-01 ID, signature, kind, author, and supplied references. NIP-01 timestamp
replacement selects a discovery head; it does not authorize registry rollback.
`EOSE`, relay acknowledgments, and an empty result cannot prove that a catalog
or revocation checkpoint is current or complete.

Normal EXT revocation/checkpoint rules apply to the catalog package as well
as to selected member packages. A relay needs only its supported EXT roles
to transport these records; a relay serving them does not become a curator,
registry reader, plugin installer, or executor.

## GitHub and HTTPS mirror

A mirror uses this layout beneath an enrolled directory or HTTPS base URL:

```text
registry.json                 complete signed 30184 listing event
events/<event-id>.json         complete signed NIP-01 events, addressed by ID
heads/<kind>/<pubkey>/<slug>.json  complete signed current addressable event
artifacts/sha256/<hex-digest>   exact manifest, catalog, schema, or package-file bytes
```

The mirror's `registry.json` is the discovery event. The catalog package's
`registry.json` is a different object fetched through its ArtifactRef and
digest path. The mirror MUST retain the catalog release and catalog closure;
it SHOULD retain selected member releases and the objects needed by its
documented availability and revocation-freshness policy. Missing member bytes
can be fetched from other admitted sources, but never under a different pin.

The mirror's `registry.json` and its enrolled registry's `heads/30184/…` path
serve the same listing. Readers can discover publisher `30185` checkpoints by
their package address through `heads/30185/<publisher>/<package-slug>.json`,
then fetch the referenced revocations by ID. They validate signatures,
checkpoint revision/freshness, and the union of previously verified revocations
under EXT independently of catalog revision. Mirrors supporting strict fresh
installation MUST expose the required catalog and member checkpoint heads;
missing or stale heads refuse that operation, not imply an empty revocation set.
Other admitted mirrors or relays may supply those same signed objects.

A GitHub adapter maps owner/repository/ref/directory to bounded HTTPS reads of
this layout. A moving branch is allowed for discovery. Artifacts and events
remain pinned even if that branch changes between requests. A Git commit can
be recorded as transport provenance but is not a substitute for Nostr signatures.
Plain HTTPS uses the same layout. Both paths can verify events locally without
a live relay connection. GitHub need not hold or use a Nostr secret key when
serving a catalog signed elsewhere.

Preserve signed `content` strings and tag order; do not parse and reserialize
an inner body, strip signatures, or sign member releases as the curator.
JSON object formatting outside signed field values may change only if the
reconstructed event still passes the original NIP-01 ID/signature check.
Artifact bytes cannot change at all. NIP-94 and ArtifactRef locator hints
remain optional alternative sources checked against the same expected digest.

Stage a new mirror's immutable objects before switching its discovery file.
An interrupted mirror publish or changed branch may cause `content_unavailable`;
it cannot authorize partial catalog acceptance. A repository containing only
an unsigned plugin list, a foreign `plugin.json`, or a mutable source tree is
not a REG mirror. Foreign packages require EXT's separately admitted import
and retained provenance/losses before they can become eligible member releases.

All fetches, redirects, URLs, credentials, deadlines, and expansion limits use
the client's admitted transport policy. Browsing does not run repository hooks,
build scripts, package managers, plugin probes, or installation commands.

### Curation through a repository

A contributor can propose exact publisher release references in a GitHub pull
request. The curator reviews the package, provenance, supported bindings, and
any evaluation evidence, then publishes a new catalog revision under its Nostr
key. A merge alone is not a signed recommendation. The repository can retain
the resulting signed events and digest-addressed bytes and serve them immediately
through the mirror layout; the same events may also be published to relays.
Switching from GitHub-only distribution to Nostr requires no package re-signing,
new registry identity, or replacement of the user's installed pins.

## Refresh, conflicts, and removal

Remember the newest observed, signature-valid EXT listing for the enrolled
address, including `hidden` listings, separately from the accepted catalog.
NIP-01 ordering uses greatest `created_at`, then lowest lexicographic event ID
for equal timestamps. Refuse future-dated heads beyond the configured skew and
older heads after a newer one has been observed. A valid newer hidden head
withdraws current membership. A newer published head whose catalog cannot be
verified leaves the prior snapshot as a cache with an explicit error; replaying
its older listing cannot make it current again. Mirror aliases and different
sources obey the same head watermark.

Verify the complete catalog package in staging before atomically accepting a
snapshot. Remember the highest accepted revision and its catalog-release ID,
manifest digest, and catalog digest per registry. A lower revision is a rollback;
the same revision with different release or artifact identity is a conflict.
Neither later event timestamps nor changing transport URLs can clear that state.
Reject the candidate and retain the last usable snapshot with an explicit error.
Resetting remembered trust/revision state requires an explicit operator decision.

Refresh can skip revisions; it does not require downloading unbounded history.
Conflict detection covers records the reader has observed. A first-use reader
cannot prove that no withheld fork or newer catalog exists. Show source,
accepted revision, freshness, and verification limits. Cached browsing may be
allowed by local policy; a stale cache MUST NOT be presented as current.
New installation/admission obeys separately configured membership and EXT
revocation-freshness requirements. Offline use of an existing pin does not
establish that its registry or publisher is still recommending it.

When multiple registries recommend the same exact release and manifest,
clients may deduplicate the package while retaining every curation origin.
Different pins for the same publisher-qualified package remain visible choices;
an explicitly configured per-package source policy may choose one. Do not
choose by display-name equality, registry iteration order, or another
publisher's higher version label. Conflicting publisher declarations remain
the EXT equivocation case even if different curators recommend them.

The latest verified catalog's complete membership replaces the previous
membership. Removing an entry, hiding a registry listing, or unenrolling a
registry withdraws its recommendation; none revokes a member release, uninstalls
a package, transfers ownership, or changes an active run's lock. A host may
require current curation for new admissions under its explicit policy.
Publisher revocation remains monotone under EXT and is not cleared by any
registry update. Expiration or removal cannot manufacture publisher revocation.

Record the accepted registry identity, exact catalog release and artifact
digests, selected member release/manifest, fetch time, and source provenance
with installation/update decisions. Retain the signed records and bytes needed
to explain that decision. Installed-generation checks and atomic lock updates
remain EXT's responsibility. Registry trust and private installed inventory
stay local unless separately disclosed under the shared private-artifact rules.

## Local settings and OpenRouter BYOK

A registry can describe a package that exposes an OpenRouter BYOK configuration
surface. Its `configuration` schema can describe the optional model and required
credential field, but users' settings values, secret defaults, API keys, and
private account references MUST NOT appear in public catalog or package metadata.
A host keeps values and secret references in its private store, scoped to the
selected package/component and account. Copying a title from another package
does not inherit that configuration or credential.

The selected package must describe its supported host binding through EXT/CAP.
A configuration schema cannot install a native adapter, invent an endpoint,
broaden credential recipients, or grant networking. A host without that binding
shows the component as unavailable. PRG's initial `pure` and `snapshot-read`
Wasm profiles continue to deny network, credentials, and model calls; downloading
a guest does not give it direct OpenRouter access. A trusted host adapter may
perform the separately admitted request with its scoped key.

Selection, installation, enablement, configuration, admission, and actual
activity remain distinct. Adding a registry or installing metadata does not
turn a plugin on, test its key, send a chat message, or invoke any service.
Updates changing settings schemas, host bindings, destinations, effects, or
credential use require host-supported compatibility and operator policy;
they cannot silently move a stored key to another recipient.

## Conformance

Implementations name tested publisher, mirror, or reader roles. A REG reader
must cover at least:

- Enrolling an independent curator and rejecting a substituted key or namespace.
- Equal Nostr/GitHub/HTTPS results from the same signed events and artifact bytes.
- Signature/body tampering, altered artifact bytes, malformed/unsupported catalogs,
  duplicate entries, excessive bounds, and unavailable member closures.
- Independent member-publisher verification and catalog/member revocation checks.
- Rollback, equal-revision conflict, hidden-head replay, stale/withheld heads,
  clock skew, and source changes preserving remembered state.
- Checkpoint discovery from a mirror without relay connectivity, independent
  checkpoint watermarks, and preservation of known publisher revocations.
- Atomic acceptance after interrupted publication/fetch and preservation of the
  last usable snapshot and installed pins.
- Multi-registry deduplication and conflicting recommendations without identity
  confusion or silent source selection.
- Removal without publisher revocation, uninstall, activation, or active-lock changes.
- Browsing without code execution, key collection, provider/model calls, or inventory
  disclosure; unsupported host bindings and foreign import semantics refuse.

Advertise `nip-reg-v1` only for configured, fixture-backed roles. Serving ordinary
EXT events or a GitHub directory alone does not establish registry conformance.
