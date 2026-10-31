# 21d. Measure a guard decision

[Chapter 21c](ch021c-a-guard-asks-a-pylon.md) made Bram a shadow consumer.
A valid synthetic answer proves that wiring works. It does not tell us whether
a real Pylon answers in time, whether its answer is worth showing, or whether
quiet local behavior would be better. This chapter makes those questions
separate and measurable. Ambient behavior remains disabled.

## Freeze the question before observing the answer

The corpus is `guard-editorial-v1`, authored before the first live request.
Its [label manifest](../fixtures/guard-evaluation-labels-v1.jsonl) has SHA-256
`099bf141d90a6b85dd2666e7797682054efbcfa9263f3966ebc9f01e8c647141`.
The executable source is
[`guard-evaluation.coax`](../../verse/everglade/guard-evaluation.coax).

Sixteen eligible cases cross four time categories with four weather categories.
Their index is `day * 4 + weather`. Each request uses the product's exact
`ga_request` writer. The model never sees the evaluation masks or a rewritten
prompt that reveals the expected preference.

All three authored options are harmless in these eligible contexts. Editorial
preference is a different label: quiet at night, a greeting during clear
daylight, and a weather remark during other daylight weather. The usefulness
hypothesis includes the preferred nonquiet option during daylight and none at
night. These labels are design preferences, not objective truth, observed
player benefit or calibrated probabilities.

Ten more cases are local refusals: the guard/player is ineligible, or a time/
weather category is outside its range. They must never generate a request.
They are tested locally and excluded from remote-cohort accounting. A local
refusal is not a fabricated failed HTTP request.

## A runnable evaluation example

The [example](../examples/guard-evaluation.coax) uses a synthetic decoder fixture,
not a model call. Its [transcript](../fixtures/guard-evaluation.out) distinguishes
harmlessness, editorial usefulness, deadlines and missing observations.

```shell
coax/bin/coax run psionic/examples/guard-evaluation.coax
```

<!-- source: psionic/examples/guard-evaluation.coax example -->
```jai
main :: () {
    print("eligible authored contexts: %\n",GEV_ELIGIBLE);
    preferred:=gev_evaluate(4,"200\n",GA_RESPONSE,123);
    print("daytime greeting covered: %\n",preferred.covered);
    print("editorial preference matches: %\n",preferred.preferred);
    night:=gev_evaluate(0,"200\n",GA_RESPONSE,123);
    print("night greeting harmless: %\n",night.acceptable);
    print("night greeting editorially useful: %\n",night.useful);
    print("late response covered: %\n",gev_evaluate(4,"200\n",GA_RESPONSE,8000).covered);
    totals:[1]GeTotals;counted:=gev_add(totals[..],preferred);
    print("duplicate record counted: %\n",gev_add(totals[..],preferred));
    body:[1024]u8;refused:=0;
    for i:16..<GEV_CASES {if gev_request(i,body[..])==-1 {refused+=1;}}
    print("local refusal fixtures: %\n",refused);
    print("full remote cohort observed: %\n",gev_complete(totals[0]));
    print("automatic promotion: false\n");
}
```

## A contract is only the first gate

An observation needs the exact pinned response contract and an elapsed time
strictly below eight seconds to count as covered. A late HTTP200 does not become
usable because its option was sensible. An early 503 is an on-time refusal,
not a covered answer. Missing cases remain missing; a duplicate case refuses
the aggregate instead of improving its denominator.

<!-- source: verse/everglade/guard-evaluation.coax score -->
```jai
gev_evaluate :: (index:int,status:string,body:string,elapsed_ms:int)->GeResult #export_c {
    c:=gev_case(index);if !c.valid {return GeResult.{};}
    r:=GeResult.{valid=true,index=index,deadline=elapsed_ms>=0 && elapsed_ms<GA_TIMEOUT,forbidden=c.remote_mask==0};
    if status!="200\n" {return r;}a:=ga_decode(body);r.contract=a.ok;if !a.ok {return r;}r.action=a.action;
    if !r.deadline || r.forbidden {return r;}
    r.covered=true;r.disagree=a.action!=0;
    // Every authored eligible option is harmless; acceptability is not a
    // preference score. Keeping that fact explicit avoids a vacuous mask gate.
    r.acceptable=true;bit:=1<<a.action;r.preferred=(c.preferred_mask&bit)!=0;r.useful=(c.useful_mask&bit)!=0;
    return r;
}
```

Reports keep contract validity, deadline, coverage, disagreement with quiet,
harmlessness, editorial preference and editorial usefulness separate. Local
quiet is acceptable in every eligible context, matches the four night
preferences, and uses zero remote calls. The model must earn its latency and
resource cost. The report always returns `promote: false`; this small corpus
cannot grant game authority.

## Three clocks and one terminal outcome

[`guard-metrics.coax`](../../verse/everglade/guard-metrics.coax) owns a six-entry
local ledger, matching the visit's attempt cap. Each entry contains a local
sequence, monotonic times and an enum outcome. It stores no request/answer
content, user text or remote identity. A duplicate or late callback cannot
create another terminal outcome or latency sample.

It separates request-to-callback time, callback-to-frame-admission time and
request-to-admission time. Rejected terminal callbacks still measure arrival;
a locally cancelled request with no callback has no arrival sample. Admission
histograms contain admitted proposals only. A zero timing is meaningful only
when its corresponding sample count is nonzero. Last callback and last
admission fields can refer to different attempts; do not sum those scalars
as if they were one paired observation.

There are eleven upper-exclusive histogram buckets: below 10, 50, 100, 250, 500,
1000, 2000, 4000, 8000, 30000 ms, then at least 30000 ms. Last-duration scalars cap at
30000 ms and expose clipping flags. Small aggregate counts and fixed storage
make an interrupted or duplicate callback stream unable to grow telemetry.

The existing `coaxial.guardProbe(0..6)` fields remain stable. Additional fields:

| Fields | Values |
| --- | --- |
| 7 | metrics version 1 |
| 8–12 | starts, callbacks, terminals, last terminal reason, unique late callbacks for already-ended known tickets |
| 13–15 | last callback, frame-queue and total admission milliseconds |
| 16 | clipping bits: callback 1, queue 2, total 4 |
| 20–28 | counts by terminal reason1–9 |
| 40–50 | request-to-callback histogram |
| 60–70 | callback-to-admission histogram |
| 80–90 | request-to-admission histogram |

Terminal reasons are admitted 1, authentication 401=2, quota 429=3, no transport 4,
timeout 5, malformed successful body 6, revoked eligibility/lifecycle 7, stale
context/ticket 8 and other HTTP/transport failure 9. At an observed expired lease,
timeout takes precedence over a simultaneous frame/lifecycle change. An owned
401/429 callback still disables the visit and retains its specific reason.
A callback quarantined by the generic HTTP host never reaches this ledger.

## Capture a real cohort without inventing successes

The [Coax CLI and capture schema](../tests/guard-live/README.md) turn observations
into reports. Its C companion provides only byte I/O and argv; Coax owns case
selection, request writing, validation and scoring.

```shell
psionic/tests/guard-live/build.sh /tmp/guard-evaluation
/tmp/guard-evaluation/guard-evaluation selfcheck
/tmp/guard-evaluation/guard-evaluation manifest
/tmp/guard-evaluation/guard-evaluation request 0
/tmp/guard-evaluation/guard-evaluation summary < observations.jsonl
```

One observation records case, full-response elapsed monotonic milliseconds,
HTTP status and bounded response body. The capture schema refuses malformed,
duplicate, oversized or ambiguous records. A report separately states expected
eligible cases, observed cases and completeness. The ten negative local
fixtures never fill gaps in those sixteen observations.

Use one owned test identity and normal public quotas. Preserve a failed cohort
before starting a clearly identified new one. A known-small diagnostic control
is separate from guard quality. Do not rotate identities or silently retry
until the preferred result appears. Auth events, cookies and private keys
never belong in a measurement artifact.

The browser pilot loads the current local Wasm/host/assets at the real public
TLS origin through headless interception. It signs in normally using the same
owned test key and receives the real Secure/HttpOnly session. Auth and SystemOne
requests continue untouched; unrelated game APIs are suppressed. This exercises
the normal cookie path without deploying experimental guard behavior. The
native Coax policy remains the admission authority.

## Interpret the measurement at its actual scope

The observed warm cohort returned 16/16 valid, timely replies, all quiet. It
matched local quiet with no editorial usefulness gain. The first 503 remains
separate. CLI attempt timing had median 5925 ms and p95/maximum 6257 ms including
startup/signing. A normal cookie-authenticated browser request admitted in
5724ms, and a pending-blur pilot revoked before a late reply could apply.
These are different clocks and populations. Ambient remains disabled.

The [dated record](../../docs/verification/pylon-guard-shadow-2026-10-30.md)
retains actual outcomes, timings, identities, failures and qualification scope.
The public deployment can differ from main: the warm-routing implementation on
main does not imply that a deployed fleet uses it. Record both.

A small categorical cohort gives a useful integration measurement. It does not
establish population calibration, availability under load, independently hosted
failover, or what players enjoy. Latency percentiles need their sample counts;
nearest-rank p95 is simply the maximum with sixteen or fewer samples. Broader
activation still needs judged player outcomes and a measured useful advantage.
Keep the local quiet fallback and ambient mode disabled until that evidence
supports promotion in the [delivery plan](../../docs/verse/pylon-guard-decisions.md).
