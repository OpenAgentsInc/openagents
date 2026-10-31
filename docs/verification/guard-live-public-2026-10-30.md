# Public Elephant guard shadow observations, 2026-10-30

The exact current guard request now has real public-path evidence. One initial
attempt returned 503; a separately authorized warm cohort completed all 16
contexts within eight seconds. **Every learned choice was quiet.** This matched
the local quiet baseline and supplied no observed editorial benefit. Ambient
application remains disabled.

The labels were frozen before model outputs in
[`guard-evaluation-labels-v1.jsonl`](../../psionic/fixtures/guard-evaluation-labels-v1.jsonl),
SHA-256 `099bf141d90a6b85dd2666e7797682054efbcfa9263f3966ebc9f01e8c647141`.
All live requests came from the unchanged product `ga_request` through
`gev_request`, with no editorial labels added to the prompt. The contexts are
owned synthetic day/weather categories, not player speech or user history.

## Actual service and retained attempts

The public route was `POST https://openagents.com/api/v1/systemone`, using ONE
fresh demo NIP-98 identity under the normal quota. The whole record contains
18 model attempts: 17 guard attempts and one separately authorized published
control. There was no identity rotation, automatic retry, service change,
production deployment or model CPU execution. The later browser pilot uses the
same identity and has its own record.

| Retained observation | Result | CLI attempt termination time |
| --- | --- | ---: |
| [Initial guard case 0](data/guard-live-public-2026-10-30/initial-guard.jsonl) | HTTP 503 `mesh_unavailable`; no learned answer | 6377 ms |
| [Published Water-is-wet control](data/guard-live-public-2026-10-30/control.jsonl) | HTTP 200, pinned original Elephant; yes probability 0.757197863 | 6092 ms |
| [New warm guard cohort](data/guard-live-public-2026-10-30/warm-cohort.jsonl) | 16/16 HTTP 200, exact ambient contract/provider/checkpoint | 5809–6257 ms |

The initial refusal is retained rather than overwritten. Its private fleet
request log returned 503 after 5.052135034 seconds, approximately the configured
five-second discovery wait. A narrow Pylon/model journal window contained no
matching job/answer/refusal metadata. The cause remains unresolved. The control
and identical warm guard request subsequently succeeded; this does not establish
a deterministic input failure or a proven cold-start cause. Across all retained
guard attempts, 16 of 17 completed; the warm-only 16/16 result is conditional.

The successful control's provider-reported backend time was 91 ms; the first
successful guard reported 140 ms. These are separate from the client timings.
The CLI clock starts before published-helper creation and ends after response
and process completion, including NIP-98 signing, network, edge, fleet, relay
and Pylon. It is not an isolated cookie-auth browser-fetch measurement.

The [read-only status snapshot](data/guard-live-public-2026-10-30/status.json)
pins API revision `oa-api-00068-ktl` (build `32e61639b857…`) and native fleet
revision `oa-mesh-elephant-00002-dgz`, with its unchanged image
`sha256:48c948cef4297f850f8f8a9d4fd0bf5d98e6a2767d4a5fd3bf5c31cd09b33ec8`.
The fleet still has the old unconditional 5000 ms discovery wait; M0 was not
rolled out by this work. Its total/attempt bounds are 25000/20000 ms. It already
has min/max instances 1 and CPU throttling disabled.

All 16 successful guard responses named:

- Model/checkpoint: `elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`.
- Provider: `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7`.
- Door: `pylon:oct30-gcloud-elephant-rtx`.

The provider's Spot VM and backend/Pylon services were active; the actual
backend capacity flag was 512 tokens. These are observations and declared
checkpoint provenance, not remote hardware or model-correctness attestation.

## Coax replay and scope

The [Coax replay summary](data/guard-live-public-2026-10-30/warm-summary.json)
records 16 valid contracts, 16 responses within the consumer deadline and
16 covered contexts. Attempt termination latency was minimum 5809 ms, median
5925 ms and nearest-rank p95/max 6257 ms. With only 16 samples, nearest-rank p95
is the maximum; this is not a tail-SLO estimate.

All 16 outputs were harmless according to the predeclared three-option editorial
set. Four matched the authored preferred label; zero had authored additional
utility; zero differed from the local quiet baseline. Local quiet was also
acceptable in all 16 cases, preferred in four, and required no remote calls.
The labels express editorial hypotheses, not calibrated ground truth or observed
player benefit. Confidence numbers do not establish usefulness. The summary
therefore retains `promote:false`.

The [initial-attempt summary](data/guard-live-public-2026-10-30/initial-summary.json)
remains incomplete with zero coverage. Ineligible cases were checked locally,
never dispatched remotely. The control is excluded from guard quality totals.

New probe admission/recording code is Coax in
[`public-probe-policy.coax`](../../psionic/tests/guard-live/public-probe-policy.coax)
and [`public-probe.coax`](../../psionic/tests/guard-live/public-probe.coax), with
only argv/clock/sleep/file bindings in the C host. It reuses existing process and
file effects and the published signing client. The
[probe instructions](../../psionic/tests/guard-live/PUBLIC-PROBE.md) include exact
build and explicit live commands. Qualification passed five authored root
checks (74 including imported baselines), **39/39 root mutants killed**, and all
18 actual network attempts under ASan/UBSan without sanitizer diagnostics.
No frozen model or crypto mutation suite was repeated.

[SHA-256 records](data/guard-live-public-2026-10-30/SHA256SUMS) retain direct tool,
fixture, observation and tested-binary identities. Records contain only synthetic
bodies and public metadata; credentials and authorization headers were not saved.
Further work is to diagnose first-request availability, separately qualify the
M0 deployment, and define a useful shadow objective before any ambient promotion.
