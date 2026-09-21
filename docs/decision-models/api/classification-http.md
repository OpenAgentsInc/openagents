# Classify through the gateway

The gateway exposes `POST /v1/classify` with schema
`openagents.classify.v1`. It supports single-label Choice questions,
independent multi-label Noul questions, binary filtering as one Noul per
input, rubric scoring as one Score per input, and named dimensions using
those modes. Review and fallback policies use separately admitted native calls;
see the policy sections below.

A configured door must explicitly declare `classify` limits in its gateway
configuration. The fields are `max_inputs`, `max_labels`, `max_dimensions`,
`max_judgments`, `max_id_chars`, `max_input_bytes`, `max_instructions_bytes`,
`max_label_bytes`, `max_levels`, and `max_forward_bytes`. Missing declarations refuse
classification. A `max_levels` below two admits no rubric at all and is
refused as `unsupported_limits`. Use measured backend limits; the product
maxima are not evidence of backend support.

Requests name a model, capacity lane, versioned policy, ordered inputs, and
either labels plus a mode, levels plus the `score` mode, or named
dimensions. Each input has an ID and exactly one of `text` or `record`. A
unit carries `labels` under `single-label`, `multi-label`, and `binary`
modes — `binary` requires exactly one — and carries `levels` under `score`
mode: an ordered rubric of two to ten nonempty level descriptions, level 0
first, that the forward sends as the Score question's `criteria`. Mixing
`labels` and `levels` on one unit is refused.

The policy schema is `openagents.classify-policy.v1`; its `select` object
declares `single_label`, `multi_label`, `binary`, and `score` rules, and a
request that plans a mode the policy does not declare is refused.
Single-label rules declare `ties` (`first-declared` or `no-match`) and
`no_match` (`{"kind":"null"}` or a designated label). Multi-label rules
declare `threshold`, `ties` (`include-all` or `truncate`), and `no_match`
(`empty` or `null`), with optional `top_n`. A binary rule declares
`threshold` — the Noul at or above which an input joins the selected
subset. A score rule declares `order` (`descending` or `ascending`) for the
call's ranking and an optional `top_n` cap on it. Thresholds and ordering
belong to caller policy and require caller evaluation; the contract assigns
no universal confidence threshold.

Every rule also accepts an optional `uncertain_below` cut — a probability
the caller declares, validated like the others, never defaulted or
invented. When a unit's rule declares it, an answered unit carries
`"uncertain": true` when its answered evidence falls below the cut: the
distribution's top probability for single-label, any label's winning side
(`max(noul, 1 - noul)`) for multi-label, the Noul's winning side for
binary, and the top level's probability for score. A score answer that
carries no distribution is never flagged. The flag changes no selection —
an abstained input can be flagged and a flagged input still counts
normally — and when no rule declares a cut, no unit reports `uncertain`
and no aggregate lists one.

An answered unit likewise carries `"no_match": true` when its `selected`
is the no-match outcome: a single-label unit routed by `min_probability`
abstention or the `no-match` tie rule — including when `selected` shows
the designated label — a multi-label unit whose `selected` is empty or
null, a binary unit the threshold declined, and a score unit with no
categorical level to report. The marker distinguishes a routed no-match
from a designated label that won the distribution outright; both can show
the same `selected` value.

The route shares authentication, tenant authorization, backend identity
checks, quota reservations, and receipt recording with `/v1/systemone`. It
forwards one native request per input through a bounded scheduler. A
door's `classify_item_concurrency` declaration — one unless the operator
configures more — bounds how many of a call's forwards run at once, and
every in-flight forward additionally holds the binding's declared
`capacity.concurrency` slot and the process's `max_in_flight` slot, so a
configured bound never multiplies capacity the deployment did not
declare. Items that cannot start queue inside the call's deadline: the
configured forward timeout bounds the classification execution across
inputs, including the wait for a slot. Inputs still queued when the
deadline passes, or after a transport failure halts the call, report
`unattempted` with the bound that stopped them. Results reassemble in
input order however forwards complete. This does not provide packed
inference or durable jobs; `batch-execution.md` covers the scheduling
contract and its measured fixture.

Results preserve input order and IDs. Each unit reports its outcome, raw
answer, and policy-selected output: the label or label list for the
categorical modes, the label or null for a binary unit, and the categorical
level for a score unit — the estimator's `selected` when the door reports
one, else the highest level among the distribution's maxima. A score
unit's `raw` keeps the weighted position, confidence, legend, and
distribution so a caller can re-rank without rerunning inference.

The response's `selections` array carries the corpus-level views the
binary and score modes produce. A binary unit reports `selected` — the
input IDs its threshold admitted, in input order — beside its label. A
score unit reports `ranking` — the answered inputs ordered by weighted
position under the declared direction, equal positions keeping input
order. Both report `unevaluated`, the inputs no judgment answered: an
input that was refused or unavailable is neither selected nor ranked, and
is named rather than dropped.

The response's `aggregates` array carries one tally entry per unit, in
plan order, keyed by `dimension` when the unit is one. Each entry reports
`mode`, the unit's own `outcomes` counts across the inputs — answered,
refused, unavailable, and unattempted sum to the input count — and
`no_match`, the answered inputs whose selection resolved to the no-match
outcome. A label unit's `labels` maps every declared label id to the count
of answered inputs whose policy-selected output includes it; a score
unit's `levels` does the same per rubric level index. Counts are
selections that happened, so a label a `top_n` cap excluded is not
counted, a designated no-match label counts under `no_match` only when the
policy routed the input there, and a multi-label input counts under every
label it selected — the label counts are independent judgments, not shares
of a distribution, and need not sum to `answered`. A score `top_n` trims
the `selections` ranking only; the `levels` tally still counts every
answered input's level. An entry's `uncertain` lists the flagged input ids
in input order and is present only when the unit's rule declares
`uncertain_below`. Inputs that were refused, unavailable, or unattempted
appear under the unit's `outcomes` — and under `unevaluated` on the binary
and score `selections` entries — never as a zero, a label, or a no-match
they did not earn.

Native SDK validation checks answer types and probability distributions;
a score answer is additionally held to the declared rubric, so a legend or
a distribution that names other levels fails. The returned model must
match the admitted artifact. A mixed result retains failed and
unattempted items rather than dropping them. Per-input usage is retained
where available. Aggregate counters appear only when every dispatched
input reports them without arithmetic overflow; completeness flags
distinguish unknown totals from zero. Review status is `not-reviewed`.

Quota settlement counts dispatched questions and options; a rubric's
levels count as options the way a categorical set's labels do. Input-byte
accounting uses the admitted classification envelope size. These quota
units are not monetary prices or provider token charges. Response bytes
are bounded while reading, including responses without a content length.
Receipts record the overall attempt; per-item receipt identities and
complete commercial settlement remain integration work.

The HTTP tests use local fixture backends. They prove ordering, the
declared-subset and ranking outputs, the per-unit tallies and the
declared-cut uncertainty flags — overlapping label counts, routed versus
genuine no-match labels, and unevaluated inputs counted under their
outcomes — undeclared-limit and invalid-rubric refusals, partial
outcomes, incomplete usage, model/distribution rejection, and bounded
reads of an unfinished chunked response. The scheduling tests prove the
serial default, the configured item bound at the stub's observed peak,
input order under reordered completions, a binding's declared
concurrency shared across tenants, a deadline's partial coverage with
its settled units, and a halted call's unattempted queue — plus a
reproducible serial-versus-concurrent fixture measurement in
`batch-execution.md`. They do not establish model quality, production
throughput, caller-declared label exclusions, per-item receipt
identities, semantic review of flagged inputs, or the durable-job
surface #9484 owns.

## Discover configured classification bounds

Each authorized `GET /v1/models` card includes a `classification` object with
schema `openagents.classify-discovery.v1`. `configured` publishes the request
and policy schemas, supported modes, declared backend limits, gateway body
and admitted-input ceilings, membership-header requirement, execution timeout,
and effective per-item concurrency. The input limit is narrowed by global and
per-tenant admitted-input ceilings; available capacity can still be lower while
other requests are active. Independent multi-label work counts every input-label
judgment, not only the number of inputs.

`unsupported` means the configured door declares no classification limits;
`unavailable` means no backend is configured; `invalid-limits` means its bounds
cannot be served. These states publish no invented limits. A configured card is
operator-declared support, not a live probe, artifact attestation, or throughput
guarantee. `context_tokens: null` explicitly leaves token-context support unknown;
byte, count, and wire limits are not token estimates. Execution remains native
per-input forwarding with `model_packing: false`. Discovery has the same tenant
and optional workspace admission as the decision routes.

## Complete context bounds and disconnects

`max_forward_bytes` bounds the serialized UTF-8 JSON bytes in each complete
native request, including state, expanded questions, model identity, and JSON
framing. Its product ceiling is 1,048,576 bytes. Operators must declare a lower
bound appropriate to the backend, with headroom for its prompt framing and
context window. Existing configuration documents must add this required field.
`GET /v1/models` advertises it with the other classification limits. This is a
byte bound, not a tokenizer estimate: `context_tokens` remains null because the
gateway cannot establish that backend-specific value.

The gateway constructs and checks every primary envelope before queue admission
or quota reservation. An oversized envelope returns HTTP 422 `context_limit`;
it never truncates input. Review and fallback envelopes are checked against their
own door's classification bound, when declared, before reservation or dispatch.
Native-only reviewer doors retain the native API contract. Per-field limits still
apply independently. These declarations are operator configuration, not evidence
that a model achieves a particular classification quality or supports all product
maxima.

Request cleanup belongs to the gateway even when the caller disconnects. A
disconnect stops queued classification work and prevents subsequent review or
fallback dispatch. Cleanup records a sealed receipt and settles attempted usage.
A forwarded parent records `caller_disconnected`; a call canceled before dispatch
records `cancelled`. A hold is released only when inference provably did
not dispatch; an interrupted dispatch retains unknown monetary liability for
reconciliation. Canceling the HTTP request cannot prove that an upstream model
stopped computing. The disconnected caller receives no fabricated JSON response.

## Classification contract verification, September 21, 2026

The manual `./scripts/verify-rust.sh` gate passed all 12 required phases,
including default and runtime-feature workspace tests, strict Clippy, minimum
compiler checks, dependency policy, and disposable PostgreSQL acceptance.
Both test configurations passed all 79 gateway HTTP tests. The three Oak
classification-corpus tests checked planner fixtures, tighter backend refusal,
and decoding answered primitives through the existing Jev SDK.

The separate JSON Schema check passed seven valid requests, ten response
fixtures, seven rejected response mutations, and the disconnect fixture's request.
These tests establish the bounded synchronous contract in #9482. They do not
establish model quality, token capacity, efficient model packing (#9483), durable
jobs (#9484), Metal inference, or long-running soak behavior.
