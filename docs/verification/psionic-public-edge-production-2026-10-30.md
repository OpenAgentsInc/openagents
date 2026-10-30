# Authenticated public mesh — production proof, 2026-10-30

Live endpoint: **`POST https://openagents.com/api/v1/systemone`**.
[The canonical quickstart](https://openagents.com/docs/connect-a-pylon) is the
user/agent entry point; [the technical profile](../protocol/systemone-public-mesh.md)
defines admission. This record captures actual managed-cloud calls, separate
from the [local boundary qualification](psionic-public-edge-2026-10-30.md).
The [machine-readable record](psionic-public-edge-production-2026-10-30.json)
contains exact image identities, health replies, synthetic model responses and
refusal checks. It contains no keys, cookies, authorization tokens or user data.

## Immutable staging-to-production promotion

API source: `9e6d3b76f8cd986c717f7916f7b530ae15d8385c`.
Cloud Build: `e46d1398-8a8f-46ea-9b61-f633d1f18c6f`.
Image: `us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-api@sha256:e3daa959d6c199a39cdaea61f6903828fa1b64486efb9eb3204c22ec4bf6e68f`.

The standard `api_verified_rollout` guard checked authenticated service/revision
snapshots, the immutable digest, exclusive traffic and uncached routed health.
Staging `oa-api-staging-00087-9ww` passed before the **same digest** was promoted
to production `oa-api-00064-52b`, each serving 100% of its service's traffic.
Both routed health responses report source 9e6d3b76f8 and migration
`0068_mesh_systemone.sql` in the correct staging/production database.

Private IAM fleet image:
`us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/psionic-fleet@sha256:48c948cef4297f850f8f8a9d4fd0bf5d98e6a2767d4a5fd3bf5c31cd09b33ec8`.
Elephant revision `oa-mesh-elephant-00002-dgz`; Mammoth revision
`oa-mesh-mammoth-00001-4gv`. Both private fleet URLs return 403 without IAM
credentials. The public API obtains its own service-account audience token;
client Nostr credentials are not forwarded to those fleets or to a Pylon.

## Actual learned-model calls

Each test created a disposable NIP-98 identity with the published client helper.
Requests used the synthetic "Water is wet" choice from the quickstart. Each
response had to name the exact requested model, checkpoint and configured
provider, identify a `pylon:` door, and return finite normalized yes/no
probabilities matching the retained learned-model reference within 0.001.

| Environment | Model/request | Wall time | Yes probability | Result |
| --- | --- | ---: | ---: | --- |
| Staging | Elephant,167 bytes | 5689ms | 0.757197863 | 200 |
| Staging | Mammoth,166 bytes | 5864ms | 0.944614404 | 200 |
| Staging | Elephant,16384 bytes | 5599ms | 0.757197863 | 200 |
| Production | Elephant,167 bytes | 5641ms | 0.757197863 | 200 |
| Production | Mammoth,166 bytes | 5987ms | 0.944614404 | 200 |
| Production | Elephant,16384 bytes | 5551ms | 0.757197863 | 200 |

The exact 16384-byte request adds an ignored JSON padding field while preserving
the actual typed question. It proves the complete public HTTP→IAM fleet→NIP-44
job envelope→Pylon→GPU→verified result path fits the published admission bound.
It is not merely a successful parser fixture or a loopback model response.

Elephant provider:
`59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7`,
slug `oct30-gcloud-elephant-rtx`, checkpoint
`elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`.

Mammoth provider:
`d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829`,
slug `oct30-gcloud-mammoth`, checkpoint
`mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019`.

The two learned-model Pylons share one 96 GB RTX PRO VM; they are distinct
provider identities, not independently attested physical machines. The old
L4 provider 335ddc… is stopped after repeated GCloud preemption and is not in
the hosted route configuration. Native model execution is much shorter than
these wall times; the deployed fleet deliberately includes a five-second
discovery window. No claim of subsecond public-endpoint latency is made.

## Real refusals and qualification scope

Eleven real checks passed in **each** environment: fresh disposable identity
reaches model admission, anonymous request 401, changed signed bytes 401, signed
unknown-model 400, replay 401, missing payload tag 401, authenticated foreign
Origin 403,16385-byte request 413 with `limit_exceeded`, wrong method 405 and
wrong content type 415. These refusal checks requested no model work.

The public edge admits exactly 16 KiB requests and bounds responses to 256 KiB.
This matches the existing native fleet/Pylon request capacity and stays below
the NIP-44 compatibility envelope after framing. It refuses oversized work
before authentication, quota reservation or sealing. Native remote provider
errors still trigger retry/refusal 503; this deployment does not claim remote
context errors propagate as 422. An actual upstream HTTP 422 is retained as
`unsupported_input`, separately covered by the boundary harness.

The required existing API gate ran once: 1106 named checks, no failed checks or
sanitizer findings. After the changed public body bound was identified, only
that affected qualification was repeated: 34 Coax checks, 194/194 policy mutants
killed, 41 scoped ASan/UBSan HTTP/database/upstream checks and the client boundary
checks. Unrelated gates were not rerun for remote commits. The staged image
was reused directly for production, with existing production configuration
preserved and operator-owned route-secret references updated.

These are free demo decisions. No sats were spent, no inference credit was
created, and no model correctness/confidential hardware attestation is implied
by a valid provider signature. The separate receipt/checker and attestation
profiles retain their own qualification and trust requirements.
