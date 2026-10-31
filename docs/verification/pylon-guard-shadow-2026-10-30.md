# Guard shadow measurements — 2026-10-30

## What this slice establishes

This continues M3 of [the Everglade delivery plan](../verse/pylon-guard-decisions.md).
The implementation adds bounded local shadow telemetry, a pre-authored context
corpus and Coax evaluation/recording tools, plus real public-Pylon and normal
cookie-authenticated browser evidence. Ambient application remains disabled.

The warm corpus completed 16/16 within eight seconds, but every answer was
**quiet**. It matched the local quiet baseline, with no observed editorial
usefulness gain. The measurement mechanics are qualified; player benefit,
calibration and broader promotion are not. A first 503 remains separately
retained, so the warm-only result is not presented as unconditional reliability.

## Pre-output labels and actual public requests

[The public probe report](guard-live-public-2026-10-30.md) retains service
identities, raw observations, replay summaries and the narrow failure diagnosis.
The label manifest was frozen before outputs, SHA-256
`099bf141d90a6b85dd2666e7797682054efbcfa9263f3966ebc9f01e8c647141`.
All 16 eligible requests use the product's exact writer. Ten ineligible cases are
local fixtures and never fill gaps in the remote cohort.

| Observation | Result |
| --- | --- |
| First guard request |503 at 6377 ms; no learned answer; cause unresolved |
| Separate published small control |200 at 6092 ms, reference yes probability 0.757197863 |
| New warm guard cohort |16/16 pinned, valid and timely; all quiet |
| CLI attempt termination |minimum 5809 ms, median 5925 ms, p95/maximum 6257 ms |
| Editorial evaluation |acceptable 16, preferred 4, useful 0, disagreement with local quiet 0, promotion false |

CLI timing starts before helper/process creation and includes signing, the
complete service path and process completion. It is not an isolated browser
fetch metric. The exact median retains half milliseconds; p95 uses nearest rank
and equals the maximum for this small cohort. The report separates all-attempt
termination latency from covered-success latency, with counts/null empty groups.
The initial failed cohort is not merged with a duplicate successful case 0.

The deployed API was `oa-api-00068-ktl`/build 32e61639b857 and the private fleet
remained `oa-mesh-elephant-00002-dgz`, image 48c948cef429… . That fleet still waits
5000ms on every discovery. It already has min/max instances 1 and CPU throttling
disabled. Main's M0 warm-routing patch was not deployed by this work. The first
fleet failure took 5.052 s, consistent with but not proof of a discovery failure;
there was no matching job metadata in the inspected narrow journal window.

## Real browser session and cancellation

The browser harness loaded the current local Wasm/host/assets at the real
`https://openagents.com` TLS origin through headless resource interception.
`/api/auth/key`, `/api/auth/logout`, `/api/auth/viewer` and `/api/v1/systemone`
continued to the actual service; unrelated game APIs were suppressed. No public
app deployment or existing owner session was used.

The same single demo Nostr key as the corpus created a normal site login.
The resulting cookie was present, Secure and HttpOnly, unreadable to page JS.
The decision request sent no Authorization header and used the real cookie
path. Records retain only those boolean facts, never the cookie or auth event.

- [Admitted pilot](data/guard-browser-2026-10-30/admitted.json): HTTP 200;
  network 5714 ms, Coax request-to-callback 5718 ms, frame queue 6 ms and total shadow
  admission 5724 ms. One start/callback/terminal/admission; chosen enum quiet.
- [Pending blur pilot](data/guard-browser-2026-10-30/pending-blur.json): real
  fetch cancelled at 131 ms; one start and revoked terminal, no callback sample
  or admission through a 7.5 s observation beyond the measured warm interval.
  This does not prove upstream GPU cancellation.

The admitted browser body exactly matches frozen Coax case 10 (afternoon/rain),
SHA-256 `0d7d397b1b3dbfa46edd698665c10b86eead5aca9f85238cc90ddbfa5c6d4e17`.
All calls reused ONE owned identity under ordinary quotas; no identity rotation,
automatic retries, quota bypass, service restarts or deployments occurred. The
corpus/control record has 18 public attempts and the browser adds 2. Both test
sessions logged out, private browser profiles closed, and the private demo key
was deleted after the pilots. No auth/key/header material enters Git artifacts.

The two opt-in pilot commands use explicit artifact, owned-key, signer and
private-output paths. `EVERGLADE_GUARD_LIVE=1` enables the admitted pilot;
`EVERGLADE_GUARD_LIVE_REVOKE=1` additionally enables pending blur. Select each
named test separately and respect the shared identity's normal rate/active
limits. These are not default tests or a reason to recreate identities.

## Bounded telemetry and focused qualification

`guard-metrics.coax` owns a six-entry ledger, nine terminal reason counters and
three 11-bucket histograms. Each known sequence records one callback and one
terminal at most. Unknown or duplicate inputs cannot consume capacity. The
metrics never grant admission. Last-duration scalars clip at 30000 ms with flags;
callback and last-admission timings can refer to different attempts and must
not be summed as if paired. Histograms separate those populations.

| Changed authority root | Focused result |
| --- | --- |
| `guard-metrics.coax` |8 checks;279/279 mutants killed |
| Integrated `guard-decision.coax` |62 checks including imported baselines;574/574 mutants killed |
| `guard-evaluation.coax` |201/201 mutants killed |
| `guard-evaluation-wire.coax` |166/166 mutants killed |
| `guard-latency.coax` |54/54 mutants killed |
| `public-probe-policy.coax` |74 checks with imports;39/39 mutants killed |
| `token-capacity-policy.coax` |1 check;2/2 mutants killed |

All 1315 selected sites were killed, with zero survivors and no equivalent
markings. Mutations targeted owned roots; unchanged crypto, model, generic HTTP
and shared ticket roots reused the prior 7536d48c1d qualification. No full-model
CPU inference or broad release gate was run.

`verse/test_guard_ai.sh` passes 58 independent emitted-C oracle cases in debug,
release and ASan/UBSan. The Coax evaluation CLI has 80 baseline checks and passes
its native selfcheck and complete real-cohort replay in debug/release ASan/UBSan.
It rejects duplicate/ineligible/malformed/oversized records, preserves missing
observations and reports editorial preference separately from harmlessness.
The runnable book example matches its pre-authored transcript in debug/release.

Actual realm Wasm builds cleanly. The six synthetic-edge browser scenarios pass
39 assertions: default/ambientoff, admitted shadow, malformed/oversized reply,
401/429 disable, pending blur, and golden/clip suppression. The admitted real
pilot passes 8 assertions and live revoke 7. All Chrome runs were muted/headless.
The live revoke extension reused the same admission path and added no game
policy. After the unused main/source-marker cleanup, the realm was rebuilt;
its byte identity changed. A targeted final-artifact admission/revoke smoke
passes 2 cases/17 assertions. The earlier actual pilot artifact and final
artifact hashes are both retained, rather than claiming byte identity.
Pure replay/default checks perform no live calls.

The optional offline capacity probe uses the pinned 8.28 MB tokenizer only,
with no model execution. All 16 contexts render to 133 tokens (65 prefix + 68 suffix),
387–393 JSON bytes and 601–607 rendered bytes. Actual provider capacity 512 leaves
379 tokens of headroom. Source default 128 would be insufficient; no capacity or
prompt change was made. Its file-size admission is independently qualified.

An unused empty policy main was removed to allow a normal book example main;
it adds/removes no admission behavior or mutable site. Later listing markers, trailing-source whitespace
and documentation edits are metadata. The [source manifest](pylon-guard-shadow-sources-2026-10-30.json)
records final source/artifact identities and the original tokenizer pin.

## Integration with concurrent main changes

Rebased onto main's new Coax JSON/args/file/process/base64 tooling. Our measured
public artifacts retain their earlier source/host identities. The rebase added
general file APIs and an optional quiet-process mode to the two native effects
used by the public probe; existing default behavior was preserved. A rebuilt
ASan/UBSan probe passes invalid invocation and a missing-key child refusal with
no network/model call. No real cohort was repeated for this integration.

Book structure passes: 94 programs, 374 source-linked listings and local links.
The probe/source manifest distinguishes pre-rebase effects from final sources.

## What follows

The current decision is safe but has no demonstrated advantage over local quiet.
Do not enable ambient behavior from a successful response, a model confidence,
or a prompt tuned to the observed 16 labels. A useful next candidate needs a
clear player-facing purpose, a local baseline and fresh held-out contexts.

Separately, stage and qualify the already-implemented warm-fleet change against
real current-connection/withdrawal/cold cases before any production rollout;
retain the unresolved 503 and compare full public/cookie latency after promotion.
Independent-provider recovery, scheduling and service-side cancellation remain
M5. This slice claims neither general availability nor calibrated player value.
