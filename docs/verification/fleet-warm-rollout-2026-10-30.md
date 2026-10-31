# Warm routing deployed for original Elephant

This records the first, intermediate rollout. It reused Debian APT TLS packaging,
which did not satisfy AGENTS' Linux Nix requirement. The subsequent
[Nix replacement](fleet-nix-rollout-2026-10-30.md) records the correction and fresh
final-image observations. The original observations below remain unchanged.

The private Elephant fleet now runs the qualified M0 warm-routing code.
Three normal public calls using the same demo identity and exact paired guard
requests changed from **5901 / 5830 / 5744 ms** to **939 / 887 / 937 ms**.
Median CLI attempt termination time fell from **5830 to 937 ms**, a 4893 ms
reduction (about 84%). All six returned the same configured original checkpoint,
provider and paired decision probabilities. These three paired contexts measure this rollout's warm service path. Inference arithmetic is unchanged.

[Before](data/fleet-warm-rollout-2026-10-30/public-before.jsonl) and
[after](data/fleet-warm-rollout-2026-10-30/public-after.jsonl) retain raw owned
synthetic responses. The unchanged Coax probe includes helper startup, NIP-98
signing, HTTPS, API, fleet, relay, Pylon and process completion. The normal API
quota was used with one owned demo identity, one active call and six-second start
spacing. Its identity is also reused by the separately recorded guide pilot;
no rotation or quota exemption was used.

## Candidate, promotion and rollback

Source: `d6f9d6fd1134e32bae3bf7688fb24ec60e7ce8bd`.
Cloud Build: `ea415392-7ca9-4321-82a6-4988b3b2502c`.
The explicit build timeout was 840 seconds; actual build time was approximately
56 seconds. The source-only context contained no caller or provider credentials.

Image:
`us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/psionic-fleet@sha256:bafefc65363c2f658a385444a663441545908e934638ded8925142a751517f9b`.

Candidate `oa-mesh-elephant-m0warm-20261031a` was first deployed with **zero
production traffic**, behind the private `m0-warm` tag. Unauthenticated access
returned 403. Two IAM-authenticated candidate calls returned valid original
Elephant answers in 531.553 and 312.988 ms. Both passed the actual guard decoder
and exact deployment pins through the offline Coax
[`fleet-answer-check`](../../psionic/tools/fleet-answer-check.coax).

The coordinating agent independently inspected the candidate's concrete proof
before promotion. Only a traffic update then promoted **that same digest and
revision** to 100%. The [candidate](data/fleet-warm-rollout-2026-10-30/candidate-verified.json)
and [promoted](data/fleet-warm-rollout-2026-10-30/promoted-verified.json)
Coax gate outputs record their distinct traffic requirements. The
[three matching private request-log rows](data/fleet-warm-rollout-2026-10-30/promoted-request-metadata.json)
identify the promoted revision, HTTP 200 and 257–325 ms fleet HTTP times for the
public after calls. They include no request bodies or headers.

Rollback retains `oa-mesh-elephant-00002-dgz`, image
`sha256:48c948cef4297f850f8f8a9d4fd0bf5d98e6a2767d4a5fd3bf5c31cd09b33ec8`:

```sh
gcloud run services update-traffic oa-mesh-elephant --project=openagentsgemini --region=us-central1 --to-revisions=oa-mesh-elephant-00002-dgz=100
```

The exact deployment facts are in [deployment.json](data/fleet-warm-rollout-2026-10-30/deployment.json).
The [source context hashes](data/fleet-warm-rollout-2026-10-30/container-SOURCES.sha256),
[compiler hash](data/fleet-warm-rollout-2026-10-30/COAX_SHA256) and authenticated
[private snapshot hashes](data/fleet-warm-rollout-2026-10-30/private-snapshot-SHA256SUMS)
retain input identities. Raw SDK snapshots remain in the private task workspace
for operator review; public artifacts contain selected metadata and synthetic
responses, not gcloud credentials, cookies, auth events or headers.

## Preserved authority and qualification

The deployment preserved the complete requested template spec except its image,
and separately preserved the resolved old/new revision specs except their image.
Template annotations, selected service security annotations, namespace/URLs,
readiness/generation, exact provider/checkpoint pins, private IAM bindings and
IAM version were checked by authored Coax authority policy. The selected service
annotations are ingress, invoker-IAM-disabled and default-URL-disabled; the
image-only claim does not cover all operational metadata fields. Runtime account,
8 HTTP concurrency, 1 CPU/512 MiB, 30-second HTTP timeout, min/max one instance,
always-allocated CPU and native request/attempt/discovery settings remain the
existing values.

API authentication, route configuration and demo quotas were unchanged. The
original GPU model and Pylon services were not restarted or reconfigured.
Checkpoint `elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`
and provider `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7`
remain the route. These are declared checkpoint provenance, not hardware
attestation. **Mammoth remains on its earlier fleet revision; it was not rolled
out by this slice.**

The image/spec/traffic root retains its **120/120** passing mutation result.
The added snapshot-admission root passed **95/95 mutants killed**, with **17
checks including imported baselines**. Native ASan/UBSan passed the actual
candidate and promoted snapshots and refused a promoted-phase check against the
zero-traffic candidate snapshot, without sanitizer findings. Tool usage and exact
scope are in [FLEET-ROLLOUT.md](../../psionic/tools/FLEET-ROLLOUT.md).

The runtime, discovery policy and bounded transport sources exactly match the
prior [M0 qualification](pylon-guard-decisions-2026-10-30.md): current authenticated
EOSE, cold/catch-up and absent-route behavior, reconnect reset, queued withdrawal,
expiry, text/byte/control floods and existing retry/late-result checks. Those
passing native/sanitizer and zero-survivor results were retained rather than
rerunning unchanged model/crypto suites or whole-model CPU inference. Only new
rollout policy and actual deployment paths were qualified here.

The 5000 ms setting remains a bounded discovery window for cold or unavailable
routes; caught-up eligible routes return immediately. It does
not resolve every first-request/reconnect refusal, change existing connect/flush
ceilings, provide independent-provider failover or promote ambient gameplay.
