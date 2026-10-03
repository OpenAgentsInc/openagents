# Static review: 05a0357e-2307-40b6-a747-3d633ea22388

Two material response-validation gaps are visible statically. Existing assertions remain, with two transport cases moved to the raw API.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. Candidate and checker code were not executed. No Cargo, models, or remote jobs. No plan, arm mapping, native transcript, run result, or check result inspected. Native executions had ended when these packets arrived; separate diagnostic outcomes were not consulted.

Candidate manifest: `a4ad5cbbb491f3afc8fe498794eb2e22a29cac294c72eee948b263f9840681f1`. Source: `c427943a5c84ba5938a3549f24b27de551812a37`.

## P2: An empty legend disables the numeric Score bounds

`crates/jev/src/answers.rs:628`. Classification: `demonstrated_static_contract_gap`.

check_answer tests the score against legend endpoints only when the legend is nonempty. Otherwise it requires only a finite score. validate_for checks response legend/probability keys against the requested count (245–253) but never checks the score against that count.

Ask a valid two-level Score q. Return {"model":"m","answers":{"q":{"type":"score","score":-1,"confidence":0.5,"legend":{}}}}; score=100 follows the same path.

The empty legend and defaulted empty probabilities leave no keys or endpoints to reject. The existing typed system_one entrypoint returns this out-of-range Score, and the blocking wrapper uses the same path.

The task requires finite numeric ranges and Score bounds at the typed request-aware boundary. The new valid-compatibility test intentionally accepts an empty legend with score=1, but that must not disable the request’s known 0–1 range.

Static inference only; not executed.


## P2: Request-aware validation uses the pre-override question set

`crates/jev/src/client.rs:255`. Classification: `demonstrated_static_contract_gap`.

SystemOneRequest documents that extra_body replaces SDK-written fields (56–58). body() serializes self.questions (135–138), then merges extra_body last (140–142). The checked entrypoint sends that prepared body but validates the response against request.questions, which can differ from the wire questions.

Construct a valid request with question q of type Noul, then supply extra_body.questions.q as a valid Choice over a and b. Return a valid Choice answer for q.

The checked API rejects the valid Choice reply because the original field still says Noul. Conversely, a valid Noul response is accepted although the actual sent question was Choice. The blocking wrapper shares the same path.

The task requires request-aware answer type, coverage, and identity checks. The supported last-write body override must either be accounted for by validation or explicitly rejected before sending at this strict boundary.

Static source inference only; no HTTP or candidate execution.

## Existing tests and scope

No material assertion removal or relaxation found. Existing fixtures align request IDs/options/level counts with their served response. Two array/null-state cases now use system_one_raw, preserving request-body assertions but no longer directly covering the typed API. Five new integration tests cover malformed answers through both clients, a valid blocking response, compatible rounded/empty-legend shapes, and raw bypass. The new mock loops over connections indefinitely without shutdown/join or socket read timeout; this is a test-lifecycle limitation, not evidence that a test hung.

Five changed/added files stay under crates/jev. No dependency changes, unrelated packages, deletions, or existing-file mode changes. The new validation.rs is mode 0600.

## Limits

The existing typed entrypoint does invoke request-aware validation; the findings concern missing numeric enforcement and incorrect request binding, not absence of the call. Non-argmax selection and confidence independent of selected probability remain allowed. Static findings do not change primary scores or incorporate post-panel diagnostic results.
