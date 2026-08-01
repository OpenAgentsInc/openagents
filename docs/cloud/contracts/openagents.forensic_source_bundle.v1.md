# OpenAgents forensic source bundle v1

This contract materializes an immutable, private source snapshot for a forensic
run without giving the analysis guest an SCM credential, provider credential,
external IP, or Internet route. The production API is the authenticated,
default-off `POST /api/forensics/source-bundles` endpoint.

## Authority boundary

The source materializer consumes opaque source-object refs created by an
owner-scoped SCM or Forge intake process. Each private object binds:

- owner, tenant, work unit, repository, path, and classification;
- the exact 40-character commit and SHA-256 tree digest;
- a content digest and the exact base64 bytes;
- a submodule path and pinned commit/tree for dependency objects; or
- generator and toolchain refs for generated objects.

SCM access remains outside the guest. A controller stages each bounded object
through the endpoint's authenticated `StageObject` operation; staging strictly
decodes the object, recomputes its byte digest, and writes canonical JSON to an
owner-scoped private-artifact key. An existing ref is accepted only when its
canonical bytes are identical. Raw clone URLs, SCM tokens, provider keys, and
guest service-account credentials are not accepted by the materialization
request and are never written to the guest.

## Materialization

`ForensicSourceMaterializationRequest` binds the target snapshot, declared
submodule pins, expected paths, dependency-manifest digest, optional expected
source digest, retention deadline, sandbox generation, and the exact
`file_write` capability. Decoding is strict and excess fields fail closed.

Every expected path is classified as `target`, `dependency`, `generated`,
`excluded`, or `oversized`. Required absent, excluded, or oversized inputs make
the coverage manifest `incomplete`. In that state no source bundle is stored or
delivered, and the evidence receipt is `inconclusive`. The system therefore
cannot claim it inspected an absent dependency.

For present inputs, materialization recomputes every byte digest and checks:

- target commit and tree equality;
- exact submodule path, commit, and tree equality;
- generated-input target, generator, and toolchain equality; and
- the canonical dependency-manifest and optional bundle digest.

Entries and pins are sorted before canonical JSON and SHA-256 calculation, so
identical inputs produce identical source digests. Complete Coldcard requests
must name the four pins recorded by the benchmark contract:
`external/libngu`, `external/micropython`, `external/ckcc-protocol`, and
`external/mpy-qr`. The generic materializer does not invent missing pins or
tree digests; the Coldcard benchmark manifest owns that completeness rule.

## Private delivery

The canonical bundle is stored in the private artifacts bucket as an
operator-only, no-store object. An existing digest key is reused only if its
bytes still hash to the requested digest. Delivery then resolves the exact
ready managed-sandbox generation and an active, unexpired `file_write`
capability. Each source entry is written beneath `workspace/source/` through
the existing guest-I/O adapter with:

- the admitted file and artifact byte ceilings;
- one process and a bounded operation duration;
- zero admitted network bytes and the deny-all network policy; and
- receipt checks for successful writes, zero network use, denied egress, clean
  secret scan, no symlink traversal, and exact byte length.

The materialization receipt binds owner, tenant, work unit, sandbox,
generation, capability, bundle, coverage, dependency manifest, retention,
artifact, source digest, and every delivery receipt. It states the observed
zero-network and zero-credential posture explicitly.

## Cleanup

Cleanup first deletes the private digest-keyed artifact and verifies that it is
absent. Guest cleanup is satisfied only by the same sandbox generation in the
native `deleted` lifecycle, with deleted filesystem state, complete cleanup,
and no active capability. The cleanup receipt separately records artifact,
source, scratch, and grant deletion. Any missing proof returns
`recovery_required`; it is never treated as success.

## Verification

Focused coverage is in
`apps/openagents.com/workers/api/src/forensic-source-materializer.test.ts` and
includes deterministic complete Coldcard materialization, required-submodule
absence before delivery, submodule/generator/toolchain/byte drift refusal,
cleanup truth, and the default-off live route.
