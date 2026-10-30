# Production Pylon mesh, 2026-10-30

The public entry point is `POST https://openagents.com/api/v1/systemone`.
The [canonical quickstart](https://openagents.com/docs/connect-a-pylon)
contains the tested user and agent commands, including native NIP-98 auth.
It is generated from `docs/connect-a-pylon.md`; the page, Markdown twin,
clipboard contents and accessible fallback share that source.

## What is actually connected

1. The existing OpenAgents API admits a browser session or a single-use,
   body-bound NIP-98 event. Its new admission/quota policy is Coax.
2. The API invokes private Google-IAM native Coax fleet services. No user
   header, URL, provider key or checkpoint can replace the deployment's pins.
3. Each fleet exchanges signed, encrypted Nostr jobs with the configured
   Pylon. The Pylon runs the original learned model with Coax-generated
   CUDA kernels. Results must match the configured identity and provider.

Both learned providers currently run on one physical RTX PRO 6000:

| Model | Provider | Checkpoint identity suffix |
|---|---|---|
| `elephant-1` | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` | `682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba` |
| `mammoth-1` | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` | `4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019` |

The stopped L4's earlier proof is retained as dated evidence, not counted
as current serving capacity. Standard GPU stockouts forced this placement;
the active host remains Spot. See the
[cloud deployment evidence](psionic-cloud-pylons-2026-10-30.md) and
[operator runbook](../../psionic/docs/serving/cloud-pylons.md).

## Verification and immutable artifacts

- [Public API production proof](psionic-public-edge-production-2026-10-30.md):
  real Elephant/Mammoth requests in staging and production, an exact
  16,384-byte round trip, and eleven auth/admission refusals in each.
- [Private fleet deployment](data/oct30-public-mesh/private-fleet.json):
  immutable native fleet image, exact provider environment, private IAM
  bindings and unauthenticated HTTP 403 on both services.
- [Cloud worker proof](psionic-cloud-pylons-2026-10-30.md): original model
  authentication, numerical goldens, real relay answers and host placement.

The API source is `9e6d3b76f8`, promoted with the same image digest in both
environments:
`sha256:e3daa959d6c199a39cdaea61f6903828fa1b64486efb9eb3204c22ec4bf6e68f`.
The native fleet source is `da0bbfa057`, with image
`sha256:48c948cef4297f850f8f8a9d4fd0bf5d98e6a2767d4a5fd3bf5c31cd09b33ec8`.
The subsequent model-worker-only fix `730705ed80` selects Mammoth's kind-1
GPU plan; its real startup was the red/green regression, without rerunning
unchanged compiler or model-reference checks.

The new model authority root killed 38/38 mutants. The complete 1,184-row
Mammoth inventory matches the frozen manifest. The generation fix killed
118/118 mutants and passed same-provider restart integration on a retained
fleet. The public-edge policy killed 194/194 mutants, with its native
HTTP/DB and real network evidence recorded separately.

## Managed recovery of the Spot host

The private `pylon-spot-recovery` Cloud Run Job is now triggered every five
minutes by Cloud Scheduler. Its checked Coax decision and keyless runtime
inspect only the fixed RTX VM and request a start only when it is terminated.
The retired L4 is excluded. The added runtime grant contains only instance
get/start; the Scheduler grant invokes only this private job.

[The managed deployment record](data/pylon-spot-recovery-2026-10-30/managed-job.json)
captures actual Scheduler-created dry and active executions. Both observed
the healthy VM and correctly returned `noop`; active execution
`pylon-spot-recovery-57fms` completed successfully at 18:50:14 UTC.
The policy killed 20/20 mutants; native effects and maintenance checks passed.
No healthy GPU VM was stopped to test the controller.

The [recovery runbook](../../psionic/docs/serving/spot-recovery.md) includes
the tested pause/dry/cancel/quiescent-drain procedure required before an
intentional VM stop. Recovery creates no additional VM or GPU and cannot
guarantee Spot stock. An accepted asynchronous start is reported as a
request, rather than prematurely marking the model healthy.

## Confidential execution is a separate lane

The original immutable Clef worker is also running again, on production
Google Confidential Space / Intel TDX. A fresh client verified the vendor
certificate chain, image, release admission and endpoint-key nonce, sealed
its Nostr request, and checked the returned receipt. Its round took 8,404 ms;
changed-image and unbound-key tests refused before input encryption.

[The confidential recovery record](2026-10-30-confidential-pylon.md)
includes signed public records, exact image/release identities and an
optional historical read-only verifier. This is a legacy immutable workload
and test oracle. A current Coax NIP-ATT client and sealed public edge remain
separate work. The ordinary model API authenticates callers and providers;
it makes no confidential-execution or independent-replay claim.

The current public profile is intentionally bounded at 16 KiB per request
and 30 seconds per service call. Native provider context/input errors can
still collapse into the fleet's bounded retry/503 path; they are not claimed
as fully propagated 422 responses. See the
[normative edge profile](../protocol/systemone-public-mesh.md).
