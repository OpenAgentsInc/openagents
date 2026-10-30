# C API deployment identity

This independently scoped change hardens the existing C-image deployment. It
does not connect the Coax probe, complete #f96d, or alter phone/NIP-98 routes.
The Dockerfile remains unchanged. A successful deploy captures the committed
HEAD in a clean detached worktree, runs that snapshot's API checks, and stages
its C sources, headers, migrations and Dockerfile. The build's single successful
image result is resolved to an immutable digest; staging and production use
that same digest. A failed deploy command stops immediately.

For each environment the verifier checks authenticated `gcloud` snapshots
collected before deployment, before health and after health: project namespace,
region, service UID, new ready revision and 100% ordinary traffic; it also
checks that the ready revision resolves to the expected immutable image. The
HTTPS request has no redirect and requests no caching. The health response
includes the intended build commit and fresh release ID, plus Cloud Run's
service and revision identity. The verifier requires all four to match the
rollout. Snapshot JSON is read with a 1 MiB bound and duplicate fields are
refused. The output still depends on the collector and TLS/provider trust; the
offline verifier does not authenticate arbitrary supplied files.

The commit and release fields are deployment configuration and prove that the
expected configured revision answered. They are not an authenticated build
attestation. Debian base and package versions are not pinned by this change;
no claim is made that a digest, host provenance record, or build response alone
authenticates source, toolchain or a Coax compiler artifact.

## Evidence

- The focused offline verifier tests cover image/build substitution, stale
  generation, split traffic, changed UID, wrong project/region, malformed
  JSON/traffic/conditions, duplicate fields, bounded huge-file reads, changed
  health service/revision/build/release and failed command/status cases.
- Fake-provider tests exercise staging success followed by promotion of the
  identical digest, and verify that failed deployment, stale routed health,
  mismatched revisions/digests and changed traffic stop before promotion.
  A real temporary Git fixture runs the actual deployment script against its
  committed detached snapshot and rejects dirty API source. The existing
  deployment command failure fixture is retained.
- A read-only current Cloud Run v1 staging snapshot matched the tested field
  types, namespace, location, revision ownership and revision digest. The
  current service template uses a tag; it is correctly refused by the digest
  guard. A digest-template substitution was tested only in memory.
- Build-only Cloud Build `ed26e078-3a76-41cf-a90a-c47e88178994` succeeded using
  the unchanged C Dockerfile and a frozen 74-file source context (manifest
  SHA-256 `2c748752c0ab02f40e291ede01d05380d279d7423e21eaaa0bd3a3538fc3248d`).
  `gcloud builds submit --format=json --suppress-logs` returned one successful
  image, resolved by the same parser to digest
  `sha256:ccd616c56f3381494a20d581bbb189208df3f21b0b7dd3c5d7b487cc9e7a648a`.
  The built `api.c` SHA-256 is
  `a7e9c4c27183056af4452a410543f8ddba25b60ec5a599857ac8e5efc68bd5e4`.
  The verified context archive, manifest and sanitized result are retained
  locally under `~/.codex/handoffs/nostr-api-c-deploy-20261030/` (archive
  SHA-256 `e87482a401f3b411e42601d1dcc157423b3632f5243b035e217b5b4694d8d5ce`).
  This is not a remote backup.

The final frozen `coax/web/api/test.sh` completed successfully: the offline
verifier, rollout and real-Git snapshot groups passed, and the service suite
reported 954 successful checks, including the exact health identity and the
existing phone/NIP-98 routes. It ran on macOS with native ASan/UBSan. The
suite log SHA-256 is `1328268c2850f0f92c62bec5750b92392a4c57ccc77b0d21bb8f043e70fcdf47`. An earlier run was invalid because its
shell script was edited while running; it is not counted as a pass.

No actual digest-based Cloud Run rollout, routed staging response, or rollback
has been tested by this change. An initial live rollout must verify the real
provider response shape and LB route before production promotion. #f96d remains
open for the Coax-linked service, a demonstrated source-to-image build,
reviewed artifact provenance and staging acceptance.
