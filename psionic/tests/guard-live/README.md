# Guard context evaluation

Corpus `guard-editorial-v1` was authored before observing live model outputs.
Its frozen [label manifest](../../fixtures/guard-evaluation-labels-v1.jsonl) has
SHA-256 `099bf141d90a6b85dd2666e7797682054efbcfa9263f3966ebc9f01e8c647141`.
Case identities, masks, request bytes, response admission and metrics are owned
by `verse/everglade/guard-evaluation*.coax`. No label is sent to the model.

The first 16 cases exhaust four time categories crossed with four weather
categories, ordered `case = day * 4 + weather`. They all describe the same calm,
healthy, visible guard and nearby peaceful living player. The request writer
is the product's `ga_request`; there is no evaluation-only prompt rewrite.

All three authored choices are harmless in these eligible contexts. Their
acceptable mask is 7: quiet=1, greeting=2, weather=4. Editorial preference is
quiet at night, greeting during clear daylight, and a weather remark during
other daylight weather. The editorial useful mask includes that preferred
nonquiet action during daylight and is zero at night. These are hypotheses
about utility, **not ground truth, calibrated probabilities or measured player
benefit**. Quiet may remain the better choice once latency, cost and annoyance
are considered. Never promote ambient behavior automatically from this cohort.

Cases 16–25 are a separate local refusal suite. One of calm/alive/near/visible/
safe/playable is false (16–21), or a day/weather category is -1 or 4 (22–25).
Their remote mask is zero and local fallback mask is quiet=1. `gev_request`
refuses every one. They are checked locally and never queried. Do not fabricate
HTTP errors for these cases to fill a remote cohort: aggregation rejects
ineligible case IDs.

Build the Coax CLI with native ASan/UBSan effects:

```shell
psionic/tests/guard-live/build.sh /tmp/guard-evaluation
/tmp/guard-evaluation/guard-evaluation selfcheck
/tmp/guard-evaluation/guard-evaluation manifest
/tmp/guard-evaluation/guard-evaluation request 0
```

`build.sh DIR --release` selects optimized Coax emission. The shell script is
only a compiler wrapper; the C file provides byte I/O, argv and entry effects.

Capture each actual eligible HTTP attempt as one JSONL observation:

```json
{"case":0,"elapsed_ms":6377,"status":503,"body":"{\"error\":\"example\"}"}
```

Measure elapsed monotonic milliseconds through receipt of the entire response;
round up to avoid advertising fractional deadline overruns as successes. The
record adapter requires integer case/elapsed/status fields, unique required
keys, a string body of at most 8192 decoded bytes, elapsed 0..2^53-1 and status
0..599. A record outside that capture schema fails loudly; preserve it as
unscorable evidence rather than silently dropping it. The pure evaluator also
accepts direct status/body inputs and independently rejects oversized content.

```shell
/tmp/guard-evaluation/guard-evaluation observation < observation-case0.json
/tmp/guard-evaluation/guard-evaluation summary < observations.jsonl
```

`observation` accepts exactly one JSON object; `summary` accepts distinct JSONL
case IDs 0–15, with an optional trailing newline. Blank rows, duplicate cases,
malformed records and ineligible case IDs refuse the report. Missing cases do
not become successes or fabricated failures. The report exposes expected=16,
observed counts and `all_eligible_observed` separately.

Contract validity uses the actual guard decoder: exact model/checkpoint/service,
one question, all options, selected argmax, numeric bounds and normalization.
Deadline success means elapsed is strictly below 8000ms. Coverage requires both
contract and deadline. Disagreement counts covered nonquiet actions relative
to local quiet. Acceptable/preferred/editorial-useful coverage are separate
counts. Local quiet is acceptable in all eligible contexts, prefers four night
contexts editorially, and requires zero remote calls.

The report separates all observed attempt termination times from covered-success
times. Each has a sample count, minimum, exact median, nearest-rank p95 and
maximum. An even-count median is the mean of the two middle observations and
may end in `.5` milliseconds. Nearest rank is `ceil(0.95 * n)`, one-based;
for at most 16 observations it equals the maximum. Empty cohorts emit count
zero with null ranks. These descriptive samples do not estimate production
tail latency; failed attempts are never called successful timing samples.
The public Coax probe starts its clock before spawning the published mesh
client, so its timings include client/process startup and NIP-98 signing as
well as response receipt and process completion. They do not isolate browser
fetch or cookie authentication latency. A browser pilot is separate evidence.

Sixteen categorical contexts and one response per context cannot establish
calibration, reliability in production or general game quality. An unavailable
service yields no model-quality observation. Record actual failures and stop
bounded probes instead of escalating retries or bypassing normal quotas.

Focused authority checks:

```shell
coax/bin/coax test verse/everglade/guard-evaluation.coax --library
coax/bin/coax mutate verse/everglade/guard-evaluation.coax --root-only --jobs 4
coax/bin/coax test verse/everglade/guard-evaluation-wire.coax --library
coax/bin/coax mutate verse/everglade/guard-evaluation-wire.coax --root-only --jobs 4
coax/bin/coax test verse/everglade/guard-latency.coax --library
coax/bin/coax mutate verse/everglade/guard-latency.coax --root-only --jobs 4
coax/bin/coax test verse/everglade/guard-latency.coax --library
coax/bin/coax mutate verse/everglade/guard-latency.coax --root-only --jobs 4
```

An optional offline preparation probe measures every categorical prompt using
only the pinned original tokenizer table. It loads no model and makes no
network requests:

```shell
psionic/tests/guard-live/build-token-capacity.sh /tmp/guard-token-capacity
/tmp/guard-token-capacity/guard-token-capacity psionic/fixtures/text/ch05/qwen35.pst
```

The size gate lives in the independently checked authority root
[`token-capacity-policy.coax`](token-capacity-policy.coax). The probe additionally
authenticates the exact tokenizer SHA-256 through `pt_admit`, then calls the
product request writer, public request decoder, prompt renderer and tokenizer.
It prints each prompt's input bytes, rendered bytes, full/prefix/suffix token
counts and the observed context range. This is context-capacity evidence,
not model inference or quality evidence.

```shell
coax/bin/coax test psionic/tests/guard-live/token-capacity-policy.coax
coax/bin/coax mutate psionic/tests/guard-live/token-capacity-policy.coax --root-only --jobs 4
```

## API-only browser cookie probe

[`cookie-probe.coax`](cookie-probe.coax) measures a real browser fetch at the
production TLS origin without loading a game module. It requires explicit
`--live`, one already-owned demo identity and signer, a bounded request file and
a new writable output path. Missing opt-in or malformed outer request shape
refuses before signing/network. Keep one caller active and respect normal
quotas; do not rotate identities after refusal.

```shell
coax/web/tool.sh psionic/tests/guard-live/cookie-probe.coax --live --key /private/owned-demo.key --signer /private/sign --request psionic/fixtures/guard-guide-requests-v1/case0.json --case 0 --out /private/new-case0.json
coax/web/tool.sh psionic/tests/guard-guide/guard-guide-evaluate.coax observation /private/new-case0.json
```

The headless browser is muted and has its own disposable profile. NIP-98 login
uses the signer output in memory; the decision fetch supplies only the normal
session cookie. The tool checks HttpOnly/Secure and script unreadability,
streams at most 8192 response bytes, uses an 8000 ms abort timer and retains
bounded HTTP/transport failures. It logs out and removes its browser profile.
Only whitelisted synthetic response metadata is written; auth events, signatures,
headers, cookie values and raw DevTools messages are never captured.

The probe checks the outer request shape for the two known model names, not
frozen-case identity or model quality. It hashes the exact supplied request.
The guide evaluator must match that SHA to the selected case before any result
counts. Extra probe metadata does not expand model admission. Timings use
`performance.now()` around fetch/body completion and exclude login/startup and
frame queueing. Use the existing actual-page test for game-frame measurements.

```shell
coax/bin/coax test psionic/tests/guard-live/cookie-probe-policy.coax --library
coax/bin/coax mutate psionic/tests/guard-live/cookie-probe-policy.coax --root-only --jobs 4
```

The first [directions record](../../../docs/verification/guard-directions-2026-10-30.md)
contains the five frozen captures, strong local reference and explicit scope.
This probe does not enable a game feature or grant ambient rollout.
