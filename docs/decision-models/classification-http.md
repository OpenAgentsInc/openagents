# Classify through the gateway

The gateway exposes `POST /v1/classify` with schema
`openagents.classify.v1`. It supports single-label Choice questions,
independent multi-label Noul questions, binary filtering as one Noul per
input, rubric scoring as one Score per input, and named dimensions using
those modes. This is a partial implementation of #9482. Count and review
helpers, policy exclusions, and richer item metadata remain required.

A configured door must explicitly declare `classify` limits in its gateway
configuration. The fields are `max_inputs`, `max_labels`, `max_dimensions`,
`max_judgments`, `max_id_chars`, `max_input_bytes`, `max_instructions_bytes`,
`max_label_bytes`, and `max_levels`. Missing declarations refuse
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

The route shares authentication, tenant authorization, backend identity
checks, quota reservations, and receipt recording with `/v1/systemone`. It
forwards one native request per input serially under the configured
concurrency permits. The configured forward timeout bounds the
classification execution across inputs; remaining inputs are unattempted
after the deadline or a transport failure. It does not provide packed
inference or durable jobs.

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
declared-subset and ranking outputs, undeclared-limit and invalid-rubric
refusals, partial outcomes, incomplete usage, model/distribution
rejection, and bounded reads of an unfinished chunked response. They do
not establish model quality, production throughput, or the remaining
classification features.
