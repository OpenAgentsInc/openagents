# Static review: 3a4d5159-4758-467f-b4b4-69b17471e97d

Two material response-validation gaps are visible statically. No existing assertions were weakened.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. Candidate and checker code were not executed. No Cargo, models, or remote jobs. No plan, arm mapping, native transcript, run result, or check result inspected. Review completed during the unchanged panel; not supplied to executors.

Candidate manifest: `75a76be9ec9fd49c87aae644d811efb6f1d9a5d0fcab6db47f3979faee1b2b16`. Source: `c427943a5c84ba5938a3549f24b27de551812a37`.

## P2: An empty legend bypasses Score bounds even in the checked client

`crates/jev/src/answers.rs:525`. Classification: `demonstrated_static_contract_gap`.

check_score derives bounds from legend or probabilities, but skips range checking when both are empty. It then requires only score.is_finite(). validate_for checks existing legend/probability keys against the requested number of levels (260–271), not the numeric score itself.

Ask one valid two-level Score q. Return {"model":"m","answers":{"q":{"type":"score","score":-1,"confidence":0.5,"legend":{}}}}. A score of 100 likewise follows this path.

Deserialization accepts the required but empty legend and defaults omitted probabilities to an empty map. The range guard has no bounds, and both request-aware key loops are empty, so system_one_checked returns an out-of-range typed Score.

The task explicitly requires numeric ranges and score legend bounds before returning typed decisions. Legacy omission of probabilities must remain valid with a usable rubric; it must not disable the known request bounds.

Static inference through deserialize, check_score, and validate_for; not executed.


## P2: Request-aware validation uses the pre-override question set

`crates/jev/src/client.rs:268`. Classification: `demonstrated_static_contract_gap`.

SystemOneRequest documents that extra_body replaces SDK-written fields (56–58). body() serializes self.questions (135–138), then merges extra_body last (140–142). The checked entrypoint sends that prepared body but validates the response against request.questions, which can differ from the wire questions.

Construct a valid request with question q of type Noul, then supply extra_body.questions.q as a valid Choice over a and b. Return a valid Choice answer for q.

The checked API rejects the valid Choice reply because the original field still says Noul. Conversely, a valid Noul response is accepted although the actual sent question was Choice. The blocking wrapper shares the same path.

The task requires request-aware answer type, coverage, and identity checks. The supported last-write body override must either be accounted for by validation or explicitly rejected before sending at this strict boundary.

Static source inference only; no HTTP or candidate execution.

## Existing tests and scope

No existing tests or assertions were removed or changed. Six new integration tests cover malformed values, valid values, rounded mass, and request-aware coverage/type/identity through async and blocking paths. The new mock listener loops indefinitely and has no socket read deadline (tests/validation.rs:14–45), so its lifecycle is not bounded as the task requested; detached listeners remain until the test process exits. This is a test-harness limitation, separate from the production gaps.

Four changed/added files stay within crates/jev. No dependencies, unrelated product files, deletions, or existing-file modes changed. The added integration test is mode 0600.

## Limits

Request-aware checks are opt-in through the new system_one_checked; the existing system_one intentionally retains per-answer-only validation. This API choice is recorded separately rather than counted as another demonstrated defect, because the task does not name a required method. No claim is made that calibrated selection must equal argmax or that confidence must equal the selected probability. Frozen acceptance scores are unchanged.
