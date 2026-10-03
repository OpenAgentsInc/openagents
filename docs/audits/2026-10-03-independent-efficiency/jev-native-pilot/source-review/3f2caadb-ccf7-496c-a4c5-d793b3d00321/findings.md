# Static review: 6c7040bd-1bba-42d4-b592-512cce884302

Two material response-validation gaps are visible statically. No existing assertions were weakened.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. Candidate and checker code were not executed. No Cargo, models, or remote jobs. No plan, arm mapping, native transcript, run result, or check result inspected. Review completed during the unchanged panel; not supplied to executors.

Candidate manifest: `81f9f790f86e6339fe388fdd272ba61429bc0dfe7cb51ffa46cbd7dbd59bc58a`. Source: `c427943a5c84ba5938a3549f24b27de551812a37`.

## P2: An empty legend allows a negative Score through the checked client

`crates/jev/src/answers.rs:461`. Classification: `demonstrated_static_contract_gap`.

check_score skips range checking when both legend and probabilities are empty, requiring only a finite score. validate_for checks requested key bounds and only an upper score bound (249–261); it does not check the lower bound when there are no response levels.

Ask one valid two-level Score q. Return {"model":"m","answers":{"q":{"type":"score","score":-1,"confidence":0.5,"legend":{}}}}.

The required empty legend deserializes, omitted probabilities default to an empty map, and the score is finite. The request-aware upper bound accepts -1 <= 1. The checked async and blocking APIs therefore return a negative typed position on levels beginning at zero.

The task requires finite numeric ranges and score legend bounds. The pinned Score question explicitly starts levels at zero. Legacy omission of probabilities must not bypass the known request range.

Static inference through deserialize, check_score, and validate_for; not executed.


## P2: Request-aware validation uses the pre-override question set

`crates/jev/src/client.rs:274`. Classification: `demonstrated_static_contract_gap`.

SystemOneRequest documents that extra_body replaces SDK-written fields (56–58). body() serializes self.questions (135–138), then merges extra_body last (140–142). The checked entrypoint sends that prepared body but validates the response against request.questions, which can differ from the wire questions.

Construct a valid request with question q of type Noul, then supply extra_body.questions.q as a valid Choice over a and b. Return a valid Choice answer for q.

The checked API rejects the valid Choice reply because the original field still says Noul. Conversely, a valid Noul response is accepted although the actual sent question was Choice. The blocking wrapper shares the same path.

The task requires request-aware answer type, coverage, and identity checks. The supported last-write body override must either be accounted for by validation or explicitly rejected before sending at this strict boundary.

Static source inference only; no HTTP or candidate execution.

## Existing tests and scope

No existing tests or assertions were removed or changed. Five new integration tests cover malformed answer tables, valid/raw compatibility, and blocking paths. The mock serves one connection and bounds socket reads and client timeout at two seconds. Missing response coverage is deliberately accepted by system_one in one new test but refused by system_one_checked.

Three changed/added files stay within crates/jev. No dependencies, unrelated product files, deletions, or existing-file modes changed. The new integration test is mode 0600.

## Limits

Request-aware checks are opt-in through system_one_checked; the existing system_one intentionally keeps per-answer-only validation. This API choice is recorded separately rather than counted as another demonstrated defect, because the task does not name a required method. No claim is made that calibrated selection must equal argmax or that confidence must equal the selected probability. Frozen acceptance scores are unchanged.
