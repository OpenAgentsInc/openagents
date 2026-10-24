# Publishing and release distribution

Status: proposed specification. Release formats, signing, and deployment services
described here are requirements, not shipped features.

Publishing distributes a complete immutable world generation and moves a small
channel reference. A scene edit must not require rebuilding the engine or
redeploying a service container. Compiler and engine changes remain separate
deployments. See [architecture](architecture.md), [asset compilation](asset-compilation.md),
and the normative payloads in [contracts](contracts.md).

## Release identity

**PUB-001.** A release manifest identifies the project and source revision,
compiler/toolchain fingerprint, document schema, runtime and script ABI,
simulation protocol, entry points, capability requirements, asset-rights summary,
and complete dependency closure. It pins exact engine/runtime build artifacts and
behavior-compiler artifact digests alongside their ABIs. ABI compatibility alone
does not preserve simulation or language semantics. Updating any pinned runtime
creates a new release; a loader cannot silently substitute a newer build.
Every artifact entry contains a SHA-256 digest,
byte length, media type, and role. The manifest also identifies startup-critical
content separately from streamable content. A release contains no credentials,
mutable dependency aliases, local filesystem paths, or compulsory private
service addresses.

Use the [JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785.html)
for manifest body bytes. The release ID is the SHA-256 digest of those bytes;
detached signatures do not participate in their own digest. Reject duplicate
JSON keys, non-finite numbers, ambiguous encodings, and unknown required
extensions. Store large integer counters as bounded decimal strings where JSON
number precision would be unsafe. Binary artifacts are hashed as exact bytes;
HTTP compression does not change their identity.

**PUB-002.** An organization-controlled release key signs a domain-separated
signature statement with exactly `schema`, `purpose`, `projectId`, `subjectDigest`,
and `keyId`. The statement uses `aethyrforge.signature.v1` and purpose `release`; channel
signatures use purpose `channel`. Ed25519 signs the exact UTF-8 prefix
`AETHYRFORGE-SIGNATURE-V1` followed by a newline and the statement's JCS bytes.
The detached 64-byte signature uses unpadded base64url. The normative field names,
byte sequence, and encoding are in [contracts](contracts.md#releases-and-signatures).
Use audited
Ed25519 implementations following [RFC 8032](https://www.rfc-editor.org/rfc/rfc8032.html).
Online verification uses a configured trust root and the key registry's authorized
role and current revocation state; accepting
an arbitrary key embedded in a manifest proves nothing about its publisher.
Signatures establish publisher attribution and byte integrity, not game safety,
performance, or asset ownership. Offline verification uses a retained trusted
registry snapshot under the release's offline policy; it cannot claim knowledge
of subsequent revocations. A release that requires fresh authorization cannot
start offline.

## Publish transaction

**PUB-003.** Publishing follows this ordered protocol:

1. Freeze an accepted project revision and all source frontiers. Select the
   target device profile and compiler/runtime versions.
2. Compile missing outputs and run required structural, script, resource, and
   rights checks. A failed check preserves the previous published channel.
3. Ask the artifact service which digests are missing within this project's
   authorized namespace; global deduplication must not disclose other tenants'
   holdings. Upload only those blobs
   with resumable, bounded transfers. Verify length and digest server-side.
4. Upload and sign the immutable release manifest. Verify that every referenced
   blob exists in the release's authorized distribution scope.
5. Replicate the admitted release's complete artifacts and private pinned source
   snapshot to at least two independent regions, verifying length and digest in
   each. The source snapshot remains private and obeys asset rights; public
   release access never grants source access. Record admission evidence.
6. Compare and swap the signed channel reference against its expected previous
   value in the publication/trust-control ledger. Commit its audit event and
   publication receipt in the same ledger entry, acknowledged by a two-of-three
   region quorum. Only that commit establishes **Published**.

Publication states are **building**, **uploading**, **ready**, **published**, or
**failed**. Retry identifiers prevent duplicate charges and ambiguous channel
updates. A disconnected publisher queries the attempt's result before retrying.
CDN warming may follow origin readiness, but the UI distinguishes ready-at-origin
from geographically warmed delivery.

## Channels and promotion

**PUB-004.** A channel reference binds project, channel, release digest,
monotonically increasing channel sequence, previous-reference digest, key ID,
issue/expiry times, and a signed trust version. Development, preview, staging,
and production are policy names for channels, not separate world formats.
Promotion signs a new reference to the already verified manifest; it does not
recompile it. Production updates require an explicitly authorized publisher
capability and the configured approval policy. `channelRefDigest` is the SHA-256
digest of the JCS channel body, excluding detached signatures. The signature
statement binds that digest and purpose `channel`; see [contracts](contracts.md).
The initial expiry interval is at most 24 hours. Renewal advances the channel
sequence even when the release is unchanged, preserving the previous-reference
chain. Retiring an old signing key does not erase audit history; if current policy
rejects its signatures, a reviewed historical manifest needs a new authorized
detached signature before reuse. Its unchanged body retains the same release ID.

The three-region publication/trust-control consensus ledger owns channel
sequences, publisher grants, authorized key roles, and their revocations. A commit
requires two regions and survives loss of any one region without losing an
acknowledged sequence or revocation. Without a quorum, publishing and trust changes
pause; an authoring database restored from backup cannot replace this authority.
See [operations](operations.md#recovery-and-disaster-handling).

The control service's ledger comparison is authoritative. An HTTP adapter
may expose the same condition through `If-Match`; the standard defines the
precondition semantics, while Aethyrforge owns the signing protocol.
[HTTP conditional requests](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-match).

Clients persist the last trusted `(sequence, channelRefDigest, trustVersion)`.
They reject an older sequence or trust version and reject a different digest at
the same sequence as a fork. The verified statement's project, purpose, digest,
and key ID must match its body and authorized registry entry.
First-time online resolution obtains a fresh authenticated channel response and
checks expiry; a signature alone cannot prove freshness. Its request carries a
new random nonce; the control service performs a linearizable read of the live
control quorum's current reference and key/trust state
and echoes the nonce in a non-cacheable TLS response. The client rejects a wrong
nonce or a response outside its bounded request deadline. A partitioned region
without current quorum authority cannot issue a fresh resolution. Static/CDN-only
resolution cannot establish currentness and is labeled cached. An offline client may
open its last verified release and must label it as cached. Clock errors,
trust-root rotation, revoked keys, and an expired channel response have explicit
recovery flows. Key rotation uses overlapping authorized keys and a separately
protected trust-root update; a compromised signing key is revoked without
deleting historical release records.

## Loading, cache, and access

**PUB-005.** The player resolves a channel, verifies its signature and manifest,
checks compatibility, loads the startup set, then starts the session on that
release. Artifact fetching checks digest and length before activation. A complete
startup generation becomes active atomically; mixed release files never render
as one generation. Later streaming follows the same verified manifest.

Immutable blobs use long-lived caching. Channel and revocation responses use
bounded freshness and revalidation. Private artifacts require authorization at
fetch time and isolated cache keys; knowing a digest is not access permission.
Publicly released bytes cannot be withdrawn from copies already downloaded.
Asset privacy and release signatures are independent controls.

## Rollback and live sessions

**PUB-006.** Rollback creates a higher-sequence channel reference to an earlier
compatible release. It never lowers the channel sequence or mutates an old
manifest. The publication record includes the reason and destination release.
Rolling back distribution does not roll back player databases, payments, or
existing simulation state.

The default session policy is **pin until exit**. New sessions receive the new
channel target; existing sessions retain their verified release. Optional live
migration requires a declared source-to-target state migration, simulation
protocol compatibility, candidate compilation on every relevant authority, and a
checkpoint at a tick boundary. A bounded preparation deadline aborts a migration
that cannot make progress. Connected clients negotiate the target protocol;
incompatible clients reconnect or remain in a separately pinned session. The
authority pauses input acceptance, transforms
a copy of session state, validates player placement and references, and commits
the generation/state pair together. A failure resumes the old pair. Buffered
inputs carry a generation identity and are discarded or translated explicitly.

An incompatible release drains sessions or restarts them from a declared safe
checkpoint. Shared persistent-data schema changes need reversible or
expand-and-contract migrations before rollback can be offered as safe.

## Verification and retention

Publishing tests cover missing blobs, tampered manifests, partial uploads,
concurrent promotion, replayed channel responses, revoked keys, private-cache
isolation, crash between readiness and channel update, and failed live migration.
See [verification](verification.md). Garbage collection retains every referenced
release, active session pin, legal retention hold, and rollback window before
removing unreferenced blobs. Export includes manifests, signatures, trust history,
and permitted artifacts so [self-hosted players](operations.md) can verify a
release without depending on the original hosting vendor.
