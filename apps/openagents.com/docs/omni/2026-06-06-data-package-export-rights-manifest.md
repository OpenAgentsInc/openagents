# Data Package Export And Rights Manifest

Status: implemented for issue #371 / `OPENAGENTS-LATE-011`.

## Purpose

Accepted work should be able to produce portable data packages without
confusing package metadata with raw source archives or payment/provider data.
This contract records provenance, schema, rights, redaction, digests, receipts,
and review state as a read-only export projection.

Implementation:

- `workers/api/src/omni-data-package-exports.ts`
- `workers/api/src/omni-data-package-exports.test.ts`

## Package State

Supported states:

- `draft`;
- `package_ready`;
- `reviewed`;
- `published`; and
- `revoked`.

State labels are projected directly, and published claims are allowed only when
the package is published, rights are allowed, receipt refs exist, and redaction
is shareable.

Revoked packages cannot be presented as ready for sharing.

## Manifest Parts

Each package contains:

- artifact digest records;
- schema manifest;
- rights manifest;
- redaction summary;
- provenance manifest;
- receipt refs;
- caveat refs; and
- optional review state ref.

Artifact digests carry artifact kind, artifact ref, digest algorithm, digest
ref, and size in bytes. The contract records digest refs only; it does not host
files or create download URLs.

## Rights

The rights manifest records:

- rights policy ref;
- rights state;
- allowed audience refs;
- license refs;
- usage caveat refs; and
- optional expiry ref.

License refs and usage caveat refs are required. Revoked rights require revoked
package state.

## Redaction

The redaction summary records:

- redaction state;
- redaction policy refs;
- removed field refs;
- retained field refs;
- reviewer refs; and
- blocked reason refs.

Redacted packages require both policy refs and removed field refs. Blocked
packages require blocked reason refs.

## Provenance

The provenance manifest records source bundle refs, source refs, span refs,
generation refs, and review refs. Source bundle and source refs are required so
packages remain tied back to the knowledge source layer.

## Authority Boundaries

Data package exports are read-only. They cannot:

- mutate download state;
- mutate file hosting;
- spend wallets;
- upgrade public claims;
- mutate receipts; or
- mutate rights.

Those actions require later approval-gated routes.

## Projection Audiences

The first projections are:

- `public`;
- `team`; and
- `operator`.

Public projections redact private package, schema, rights, artifact, digest,
receipt, provenance, redaction, source, span, and title refs. Team projections
can retain more internal metadata but still remove private digest, license,
receipt, rights, source, and span refs. Operator projections can see the full
safe ref set.

## Tests

Coverage includes:

- published package projection;
- package-ready, reviewed, published, and revoked state separation;
- public redaction;
- artifact digest, schema, rights, redaction, provenance, receipt, review, and
  revocation requirements; and
- false authority, invalid digest size, raw source archive, private repo,
  payment, wallet, provider, and timestamp rejection.
