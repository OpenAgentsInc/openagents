# Static review: e03c48e5-b2cc-4e5b-a1db-b073bc6388f2

Two material request-validation gaps are visible statically. Existing assertions are preserved with fixture ID corrections.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. Candidate and checker code were not executed. No Cargo, models, or remote jobs. No plan, arm mapping, native transcript, run result, or check result inspected. Native executions had ended when these packets arrived; separate diagnostic outcomes were not consulted.

Candidate manifest: `8a33cd32bb4b0db9db6434a73c45214aa4e87b3e3651da5dca72f05f3614adf6`. Source: `c427943a5c84ba5938a3549f24b27de551812a37`.

A second reviewer independently inspected only this anonymized packet and pinned public source, without outcome data.

## P2: Request-aware checks omit unselected Choice options and Score rubric identities

`crates/jev/src/answers.rs:248`. Classification: `demonstrated_static_contract_gap`.

validate_against checks only the selected Choice against question.criteria, leaving the distribution keys unchecked. Its Score branch checks only answer type and ignores the request rubric. check_answer checks Score against the response’s own legend, not the requested levels (650–663).

For a Choice over a and b, return choice=a, confidence=.5 and probabilities={a:.5,foreign:.5}. For a two-level Score, return score=9, selected="9", legend={"9":"foreign"}, probabilities={"9":1.0}, confidence=1.0.

Both responses satisfy per-answer numeric checks and pass validate_against despite naming options or levels the request did not offer. The existing typed async method and its blocking wrapper return those answers. Empty legend plus omitted probabilities also permits an arbitrarily large finite nonnegative Score at 650–654, with no later request range check.

The task explicitly requires request-aware option identities and Score legend bounds. Self-consistency of a response cannot establish that it answers the supplied rubric.

Static inference only; not executed.


## P2: Request-aware validation uses the pre-override question set

`crates/jev/src/client.rs:254`. Classification: `demonstrated_static_contract_gap`.

SystemOneRequest documents that extra_body replaces SDK-written fields (56–58). body() serializes self.questions (135–138), then merges extra_body last (140–142). The checked entrypoint sends that prepared body but validates the response against request.questions, which can differ from the wire questions.

Construct a valid request with question q of type Noul, then supply extra_body.questions.q as a valid Choice over a and b. Return a valid Choice answer for q.

The checked API rejects the valid Choice reply because the original field still says Noul. Conversely, a valid Noul response is accepted although the actual sent question was Choice. The blocking wrapper shares the same path.

The task requires request-aware answer type, coverage, and identity checks. The supported last-write body override must either be accounted for by validation or explicitly rejected before sending at this strict boundary.

Static source inference only; no HTTP or candidate execution.

## Existing tests and scope

No material assertion removal or relaxation found. Existing request IDs and assertions change together to match the response fixture, including array/null-state cases that remain on the typed API. Four integration tests are added for malformed responses, compatibility/selected-answer shapes, and raw behavior through async/blocking clients. They do not cover the counterexamples above. New validation mocks create detached accept loops with no shutdown/join or socket read timeout; this is a test-lifecycle limitation, not an observed execution failure.

Five changed/added files stay under crates/jev. No dependencies, unrelated files, deletions, or existing-file mode changes. New validation.rs uses mode 0600.

## Limits

Non-argmax selected answers and confidence distinct from a selected probability remain correctly permitted. The request binding finding applies even when the typed response fields are numerically valid. No candidate/checker code or outcome evidence was used to derive these findings, and primary scores remain unchanged.
