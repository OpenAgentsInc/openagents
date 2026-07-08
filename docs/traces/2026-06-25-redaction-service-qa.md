# Trace Redaction QA And Metrics

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Status: implemented for #6219/#6297 on 2026-06-25.

## Safety Posture

Default-on trace capture treats redaction as the primary scrubber and
`atifTraceTripwire` as the fail-closed backstop. The redactor intentionally
over-redacts high-risk shapes: provider keys, bearer/auth material, OpenAgents
agent tokens, cloud tokens, private keys, mnemonics, wallet/payment material,
emails, local paths, internal IPs, owner identifiers, X verification codes, and
long opaque blobs. Known public false positives are fenced first and restored:
`openagents/khala`, public OpenAgents/GitHub URLs, and issue refs.

The tradeoff is explicit:

- False negatives are the unacceptable failure mode. If a value resembles a
  secret or PII, the scrubber replaces it with a typed placeholder.
- False positives are tolerated when they preserve safety. The corpus keeps
  important public context, such as model ids, issue refs, and public URLs,
  useful despite conservative scrubbing.
- A post-redaction tripwire finding means the trace is dropped, not stored, and
  the completion remains unaffected.

## Observable Metrics

The capture path emits only public-safe metrics:

- `redactionTotal`
- `redactionCounts` by placeholder category
- `residualTripwireCount`
- outcome reason (`emitted` or a bounded failure reason such as
  `redaction_residual_drop`)

Production wiring records these as the structured Worker event
`khala_trace_redaction_metrics`. It never includes prompt text, completion text,
secret values, local paths, emails, or tripwire offending values. A healthy
default-on rollout should keep `residualTripwireCount` at zero. Any non-zero
residual is a corpus-expansion bug: add the slipped shape to
`packages/atif/src/redaction.test.ts`, tune the scrubber, and keep the
post-redaction tripwire test green before widening capture.

## Test Corpus

The canonical corpus is `packages/atif/src/redaction.test.ts`. It verifies that
known secret and PII shapes are scrubbed to typed placeholders, allowlisted
public values survive, numeric token metrics remain untouched, bare usernames
derived from local paths are removed, and a redacted ATIF trajectory passes
`atifTraceTripwire`.
