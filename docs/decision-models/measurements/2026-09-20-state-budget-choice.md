# Choice adapter state-budget sweep

This record addresses #9398 with the historical eleven-rung state-budget
comparison. **The completed composite scores 25/64 at the production caps, with no
failed requests at that rung. Base scores 38/64 and hosted Jev scores 32/64.** The original attempt reaches all 704 row keys but contains 68
HTTP 500 requests and fails during controller cleanup. It is retained as a
failed attempt, not evidence of a successful complete measurement.

## Measurement contract

The comparison uses the same 16 development states, four historical questions,
eleven rung labels, caps, and serialized state sizes as the
[base and Jev record](2026-09-20-state-budget.md). Each rung has 16 requests
and 64 question rows. These four questions are correlated within each request.
The `action` truth is always `respond`; 16/16 on that family is a constant-label
result. Three historical questions have since been retired from production.
This is a reproducible historical comparison, not a measurement of today's
production question set or all 40 turn states.

The choice release is `lev-adapted@1`, with eight samples, seed base zero,
pool width four, and no loaded calibration. The retained model card identifies
the actual artifact and base model. Source and binary identities can differ
between attempts and must be reported from actual controller metadata.

## Original failed attempt

The retained file has all 704 unique `(rung, state, family)` keys and matches
both pinned Jev and base references on suite, partition, caps, truth, and
serialized state bytes. Its SHA-256 is
`8fe1d847aafb3d46433636b66d94c91b0f13045f82b185722d25115b051a28aa`.
The runner log reaches `704 rows written`. That establishes checkpoint
coverage, not a successful end-to-end run or the worker's exit status.

The sweep controller's own final result file is absent. The outer queue
records its controller child exiting with status 1 at
2026-09-20T20:14:14.652109Z. The runner exit status remains unknown. Its captured traceback ends
with `PermissionError: [Errno 1] Operation not permitted` in cleanup while
calling `os.killpg(child.pid, signal.SIGKILL)`. Cleanup prevented writing the
final result. The traceback does not prove the origin of the earlier HTTP
500 responses: they occurred during the sweep, before final cleanup.

There are 176 requests, four question rows per request:

| Rung | Answered requests / 16 | HTTP 413 requests | Timeout requests | HTTP 500 requests | Correct rows / 64 |
| --- | --- | --- | --- | --- | --- |
| unbudgeted | 7 | 8 | 1 | 0 | 12 |
| output 512 | 10 | 6 | 0 | 0 | 15 |
| output 256 | 11 | 5 | 0 | 0 | 16 |
| commands 3 | 11 | 5 | 0 | 0 | 16 |
| turns 8 | 16 | 0 | 0 | 0 | 26 |
| turns 6 | 16 | 0 | 0 | 0 | 25 |
| turns 4 | 1 | 0 | 0 | 15 | 2 |
| turns 6, message 1024 | 0 | 0 | 0 | 16 | 0 |
| production: turns 6, message 768 | 0 | 0 | 0 | 16 | 0 |
| turns 6, message 512 | 0 | 0 | 0 | 16 | 0 |
| turns 4, message 512 | 11 | 0 | 0 | 5 | 13 |

The exact recorded categories are 24 `api 413: Other`, one
`the request timed out after 120000ms`, and 68 `api 500: InternalServer`.
That is 96, four, and 272 question rows respectively. All remain incorrect
in this attempt's denominator. None is silently removed.

The first HTTP 500 is request 98, `turns 4` / `1d845d65#10`, with a measured
request latency of 13,177.981 ms. It follows 4,668.967 seconds of summed
request latency from the start of the first request. The last HTTP 500 is
request 168, `turns 4, message 512` / `1d845d65#28`. Eleven requests after
failure onset still returned answers, all in the final rung; plus the first
request of `turns 4` answered before the first 500. The 68 HTTP 500 latencies
have a median of 71.205 ms, minimum 59.138 ms, and maximum 13,371.305 ms.
The much shorter error responses do not measure successful inference latency.

The unbudgeted timeout is request 16, state `9b37638d#7`. Its measured latency
is 120,003.423 ms. It belongs to the retained prefix under the recovery plan
below and must stay a failed request there. The prefix is fully checkpointed,
not failure-free.

Rows carry latency but no per-request UTC timestamp. Summing latencies omits
checkpoint and scheduling overhead; no exact UTC failure onset is known.
The run log's filesystem birth time is 18:52:34 UTC and final modification is
20:14:14 UTC; these are filesystem observations, not request timestamps.
The load log independently records a peak one-minute load of 98.294 at
20:11:52.026963 UTC, and 14.320/18.680/11.505 at its final sample,
20:14:12.611876 UTC. High load is observed; causation is not established.

The startup model card reports current policy with 16,563 seconds remaining.
The entire measured attempt is about 82 minutes, substantially less than
that lifetime. Policy expiration is not supported by these observations;
its mapped HTTP status would also be 503, not 500. The 500 status alone
cannot distinguish `unsupported_guide`, `decoding_failure`, or `bridge_error`.
The SDK categorized it `InternalServer`, and response bodies were not
retained. Do not assign a more specific cause from these rows.

## Recovery boundary

The recovery retains exact original lines 1–384: the first six complete rungs
through `turns 6`. It repeats all 80 requests in the five remaining rungs,
starting with `turns 4`, including their 12 originally answered requests.
The boundary is the rung before the first HTTP 500, not a selection by answer
correctness. The retained prefix keeps its timeout and 24 HTTP 413 requests
in the denominator.

The validated final grid is a two-attempt composite: the exact retained
prefix plus all 320 fresh suffix rows. The complete failed 704-row original
remains separately available. This differs from the base record's resumption
of uncheckpointed work: choice repeats an entire failed suffix.

The recovery runner adds stderr diagnostics with the response body, HTTP
status, rung, and state ID for API errors. The retained source patch changes
logging only; questions, caps, request timeout, retries, answers, and row
serialization remain unchanged. New response bodies cannot establish the
cause of the original attempt's unrecorded response bodies.

## Recovery results

The recovery completes on 2026-09-20 at 21:16:40 UTC with actual runner exit
zero and no cleanup errors. Its 80 fresh requests all return answers. The
composite retains 24 HTTP 413 requests and one timeout from the original
prefix; no HTTP 500 remains in the composite. These 25 failed requests still
contribute 100 incorrect rows. The original 68 HTTP 500 requests remain
visible in the separately retained failed attempt.

| Rung | Median B | Largest B | Pooled | `action` | `needs_code` | `progress` | `risk` | Refused requests |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| unbudgeted | 11,914 | 20,053 | 12/64 | 2/16 | 3/16 | 3/16 | 4/16 | 9 |
| output 512 | 10,064 | 15,561 | 15/64 | 2/16 | 3/16 | 2/16 | 8/16 | 6 |
| output 256 | 9,240 | 14,781 | 16/64 | 2/16 | 3/16 | 2/16 | 9/16 | 5 |
| commands 3 | 9,240 | 14,781 | 16/64 | 2/16 | 3/16 | 2/16 | 9/16 | 5 |
| turns 8 | 6,766 | 9,945 | 26/64 | 6/16 | 6/16 | 2/16 | 12/16 | 0 |
| turns 6 | 5,649 | 8,021 | 25/64 | 7/16 | 5/16 | 3/16 | 10/16 | 0 |
| turns 4 | 3,546 | 5,473 | 28/64 | 10/16 | 6/16 | 2/16 | 10/16 | 0 |
| turns 6, message 1024 | 3,631 | 4,879 | 24/64 | 6/16 | 6/16 | 3/16 | 9/16 | 0 |
| **production: turns 6, message 768** | 3,106 | 4,100 | 25/64 | 7/16 | 6/16 | 4/16 | 8/16 | 0 |
| turns 6, message 512 | 2,717 | 3,309 | 24/64 | 7/16 | 6/16 | 4/16 | 7/16 | 0 |
| turns 4, message 512 | 1,806 | 2,312 | 24/64 | 9/16 | 6/16 | 3/16 | 6/16 | 0 |

Refused requests counts SDK request failures, each contributing four incorrect
rows. Exact error categories are retained in the JSON summary; an SDK `Other`
category does not identify a more specific server cause.

| Rung | Choice | Base | Jev | Choice − base | Choice − Jev |
| --- | --- | --- | --- | --- | --- |
| unbudgeted | 12/64 | 21/64 | 33/64 | -9/64 | -21/64 |
| output 512 | 15/64 | 25/64 | 32/64 | -10/64 | -17/64 |
| output 256 | 16/64 | 26/64 | 33/64 | -10/64 | -17/64 |
| commands 3 | 16/64 | 26/64 | 36/64 | -10/64 | -20/64 |
| turns 8 | 26/64 | 39/64 | 36/64 | -13/64 | -10/64 |
| turns 6 | 25/64 | 41/64 | 35/64 | -16/64 | -10/64 |
| turns 4 | 28/64 | 38/64 | 34/64 | -10/64 | -6/64 |
| turns 6, message 1024 | 24/64 | 37/64 | 34/64 | -13/64 | -10/64 |
| production: turns 6, message 768 | 25/64 | 38/64 | 32/64 | -13/64 | -7/64 |
| turns 6, message 512 | 24/64 | 40/64 | 33/64 | -16/64 | -9/64 |
| turns 4, message 512 | 24/64 | 35/64 | 36/64 | -11/64 | -12/64 |

At production caps, choice scores 25/64 (39.1%), compared with base 38/64
(59.4%) and Jev 32/64 (50.0%). Choice trails both references at every rung.
Its best observed pooled score is 28/64 at `turns 4`; this small historical
sample does not justify selecting new production caps after seeing the scores.
Shrinking state removes observed request failures but does not establish that
choice is a better coding router. Keep the existing caps and routing
disposition; this record does not broaden the adapter's support-routing
calibration grant to coding. The current production question set requires
separate evidence before changing its door.

The recovery runs on Apple M5 Max, macOS 26.4 (25E246), source
`19f57190b2851a2ee0a211174d9d832aca5aaf9a`. It starts at 20:42:28 UTC
and takes 2,051.973 seconds including startup and checkpointing. Load changes
from 6.75/5.36/5.50 to 4.35/4.41/4.34. The original attempt uses source
`4edf4f1d16573801411a6f4b02d62e7e3d607045`. These are accuracy runs,
not quiet latency measurements. Each request's latency repeats across four
rows and must be counted once. The server binary changes between attempts;
retained hashes identify what ran. The artifact, base signature, eight samples,
seed zero, and pool width four match both model cards.

The composite SHA-256 is
`776409b53c017d24ff07951de82d5f8bf3fb3505642398c66905418925b8070e`.
Its first 384 raw lines equal the original prefix byte for byte.

## Retained evidence and reproduction

The [evidence directory](../2026-09-20-state-budget-choice/) retains the original
rows, model card, progress, admission evidence, load samples, runner and server
logs, controller traceback, outer queue result, source, and offline analyzers.
[`SHA256SUMS`](../2026-09-20-state-budget-choice/SHA256SUMS) pins these bytes.
The forensic JSON is analysis metadata, not a manufactured controller result.
No original sweep terminal result exists. The outer queue's observed status 1
is retained independently; it does not establish the inner runner's status.

The composite analyzer requires exact original bytes and prefix, all 704
paired keys, unchanged truth/caps/state-byte fields, the declared 80-request
suffix, successful real recovery metadata, matching artifact/base identities,
and estimator settings. It rejects changed boundaries and fabricated original
completion. It checks final cleanup separately before producing publication
tables. The recovery metadata's historical `original_controller_exit` string
means the sweep's own terminal evidence is missing; the outer queue does
establish that its controller child failed.

Reproduce the offline validation from the repository root, choosing an output
directory that does not exist:

```sh
record=docs/decision-models/2026-09-20-state-budget-choice
python3 "$record/analyze-choice-state-recovery.py" \
  --reference-dir docs/decision-models \
  --original-rows "$record/state-budget-lev-choice.jsonl" \
  --choice-rows "$record/state-budget-lev-choice-recovery.jsonl" \
  --measurement-result "$record/state-sweep-choice-recovery-measurement-result.json" \
  --controller-result "$record/state-sweep-choice-recovery-result.json" \
  --model-card "$record/state-sweep-choice-recovery-models.json" \
  --original-model-card "$record/state-sweep-choice-models.json" \
  --output-dir /tmp/choice-state-reproduction
```

The retained Rust sources and Cargo files identify both runners. To rebuild,
copy a runner's `main.rs` into `src/main.rs` in an external directory, adjust
local dependency and suite paths for the recorded checkout, and use its
lockfile with a separate Cargo target. The runner provenance JSON is a preparation snapshot: its `not compiled`
field predates the run. The terminal result records the built runner hash
and actual exit zero. The controllers are retained provenance,
not portable launch scripts: their absolute paths and process ownership checks
are specific to this machine. No new inference is needed to reproduce the
tables above.
