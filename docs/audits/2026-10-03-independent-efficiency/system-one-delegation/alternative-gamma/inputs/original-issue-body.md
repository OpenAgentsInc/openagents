<!-- openagents-audit-2026-09-19:sdk -->

Audit follow-up: A12, A22. Source snapshot: `1843fa6c18a05537bf2b022f69361a9ba3ef12a1`.

[Audit and evidence](https://github.com/OpenAgentsInc/openagents/tree/main/docs/audits/2026-09-19-codebase-audit).

## Problem and evidence

### A12: Validate numeric answer invariants at the SDK boundary

[`SystemOneResponse::decode`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/jev/src/answers.rs#L148) validates
deserialization shape without establishing the numeric contract. A Noul value
of `-2.0` is accepted as a typed answer. Downstream code uses such values as
probabilities and thresholds, so a malformed or incompatible door can produce
meaningless decisions and metrics.

Validate ranges, probability mass within a stated tolerance, selected options,
and score legend bounds. At the request-aware boundary, check answer coverage and
option identity. If raw compatibility is intentional, retain it separately and
require strict validation for Coder and Gym. Add malformed-door fixtures for all
three answer types. The public contract is documented in the
[TypeSafe API reference](https://docs.typesafe.ai/api.md).

### A22: Enforce the advertised whole-call retry budget

[`RetryPolicy::budget`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/jev/src/retry.rs#L65) is documented as the
whole call's budget, including its first attempt. The
[retry loop](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/jev/src/client.rs#L348) checks it only after a failed
attempt, while deciding whether to sleep and retry. A successful attempt can run
past the budget and still return success.

The loopback harness sets a 50-millisecond budget and a one-second attempt
timeout. A server answering after approximately 200 milliseconds succeeds.
Apply a monotonic total deadline to attempts and waits, with each attempt's
timeout capped by the remaining budget. Alternatively rename and document the
setting as a retry-admission budget if that narrower contract is intended.
Test delayed success, body stalls, and a retry with little time remaining.

## Reproduction

Use the retained [Rust harness and run instructions](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-09-19-codebase-audit/verification.md#focused-reproductions). It uses loopback mocks, a private temporary ledger, and harmless temporary markers; it needs no real credential or paid model call. The matching finding names the observed failure.

## Acceptance

- [ ] Typed answers validate finite numeric ranges, probability mass with a documented tolerance, choice/confidence consistency, option identities, and score/legend bounds.
- [ ] Request-aware validation checks required answer coverage; if a raw permissive path remains for compatibility, Coder and Gym use the validated path.
- [ ] Invalid Noul values such as -2, malformed distributions, unknown selected options, and missing answers return structured validation errors rather than usable decisions.
- [ ] The retry budget uses a monotonic deadline across the first attempt, retries, response-body reads, and delays; each attempt is limited by remaining time.
- [ ] The delayed-success reproduction with a 50 ms budget and a 200 ms response does not succeed after the deadline under the documented whole-call contract.
- [ ] Test stalled bodies, delayed successful replies, Retry-After exceeding remaining budget, and async/blocking client behavior with deterministic local mocks.
- [ ] Keep error codes structured and API-key redaction intact; update examples and contract documentation to match.

## Dependencies and scope

Coordinate strict response semantics with A05 and A07 and production thresholds in #9397. Read the TypeSafe skill and current contract before changing SDK behavior. This task does not choose new model thresholds or make paid calls.
