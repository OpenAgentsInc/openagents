# Classify through the gateway

The gateway exposes `POST /v1/classify` with schema
`openagents.classify.v1`. It supports single-label Choice questions,
independent multi-label Noul questions, and named dimensions using those modes.
This is partial implementation of #9482. Binary filtering, Score-based ranking
and rubrics, count/review helpers, and policy exclusions remain required.

A configured door must explicitly declare `classify` limits in its gateway
configuration. The fields are `max_inputs`, `max_labels`, `max_dimensions`,
`max_judgments`, `max_id_chars`, `max_input_bytes`, `max_instructions_bytes`,
and `max_label_bytes`. Missing declarations refuse classification. Use measured
backend limits; the product maxima are not evidence of backend support.

Requests name a model, capacity lane, versioned policy, ordered inputs, and
either labels plus a mode or named dimensions. Each input has an ID and exactly
one of `text` or `record`. The policy schema is
`openagents.classify-policy.v1`; its `select` object declares `single_label`
and/or `multi_label` rules for the modes used. Single-label rules declare
`ties` (`first-declared` or `no-match`) and `no_match` (`{"kind":"null"}`
or a designated label). Multi-label rules declare `threshold`, `ties`
(`include-all` or `truncate`), and `no_match` (`empty` or `null`), with optional
`top_n`. Thresholds belong to caller policy and require caller evaluation.

The route shares authentication, tenant authorization, backend identity checks,
quota reservations, and receipt recording with `/v1/systemone`. It forwards one
native request per input serially under the configured concurrency permits.
The configured forward timeout bounds the classification execution across inputs;
remaining inputs are unattempted after the deadline or a transport failure.
It does not provide packed inference or durable jobs.

Results preserve input order and IDs. Each unit reports its outcome, raw answer,
and policy-selected output. Native SDK validation checks answer types and
probability distributions; the returned model must match the admitted artifact.
A mixed result retains failed and unattempted items rather than dropping them.
Per-input usage is retained where available. Aggregate counters appear only when
every dispatched input reports them without arithmetic overflow; completeness
flags distinguish unknown totals from zero. Review status is `not-reviewed`.

Quota settlement counts dispatched questions and options. Input-byte accounting
uses the admitted classification envelope size. These quota units are not monetary
prices or provider token charges. Response bytes are bounded while reading,
including responses without a content length. Receipts record the overall attempt;
per-item receipt identities and complete commercial settlement remain integration
work.

The HTTP tests use local fixture backends. They prove ordering, undeclared-limit
refusal, partial outcomes, incomplete usage, model/distribution rejection, and
bounded reads of an unfinished chunked response. They do not establish model
quality, production throughput, or the remaining classification modes.
