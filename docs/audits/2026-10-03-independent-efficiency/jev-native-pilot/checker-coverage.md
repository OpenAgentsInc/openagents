# Native pilot checker coverage

Checker audit, October 3, 2026. This reviews the previously qualified beta and gamma checkers and their calibration receipts. These checkers stayed outside executor inputs. The separate retrospective diagnostic adds cases after coding; it does not change these original checks or their scores.

## What acceptance establishes

A candidate passes the pilot gate only when its captured final patch passes scope, formatting, ordinary package tests, and the unchanged independent checker under the pinned Linux toolchain, with validated identity, known accounting, and closed execution. Native session completion is reported separately: a budget-ended session can leave a passing patch. Passing these finite checks is evidence for the tested requirements; it does not establish general code quality, exhaustive correctness, stability, or production readiness. Candidate-owned ordinary tests can be changed, so removed or weakened assertions still need source review.

Both checkers were calibrated once per final base/reference/control on Linux x86_64, Rust 1.97.1, with development/test debug information disabled and a 240-second total check deadline. These are feasibility and sensitivity observations, not a flakiness estimate. The newly resumed pilot rebuilds seed copies; prior qualification timings are not guaranteed future timings.

## Beta: trace recovery and grading integrity (#9425)

Source: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`. Final checker SHA-256: `ee58a16d8975ee63b1f7a6911ee63e990a1e0355d0456c9b83da7febfea1ef01`. Ordinary tests cover both `atif` and `coderbench`.

Three independent tests:

- `clean_round_trip_and_writer_closure`: a clean two-step trace ends, has no unreadable lines, and grades Passed through the public consumer. Repeated `finish` leaves bytes unchanged; append after closure returns an error and leaves bytes unchanged. This positive control prevents a blanket reject-everything implementation from passing.
- `every_suffix_byte_keeps_the_valid_prefix_and_interruption_visible`: truncate one final JSON step containing `café` and a rocket character at every byte. The previous valid step must survive, any retained new step must have the exact original message, incomplete damaged suffixes must be visible, no variant is ended, and the strict consumer must never grade it Passed. A parseable final JSON value without its newline may be retained or dropped; the checker deliberately accepts either policy.
- `damaged_or_incomplete_records_never_become_successful_grades`: repeated header, repeated end, step after end, malformed JSON interior, invalid UTF-8 interior, and missing end cannot become a successful consumer grade. A reader error is allowed for these malformed/lifecycle cases. Where interior corruption is recovered, its unreadable-line count must be positive.

Calibration with the final checker: the base passed ordinary tests but failed independent acceptance; the historical reference passed both (17.900 seconds total). Both planted controls compiled and failed ordinary and independent tests: permitting append after closure, and rejecting the whole trace on any invalid UTF-8.

The final checker corrected an earlier overstrict grade expectation before qualification: the public task permits rejection or a non-Passed verdict. The pinned consumer can classify unfinished evidence as Failed, so requiring only Unverifiable would have rejected a valid implementation. Earlier attempts remain retained.

Limits: the byte sweep covers one representative non-ASCII final record, not arbitrary trace contents, versions, sizes, or concurrent writes. Lifecycle damage need not use one particular diagnostic field. The independent consumer check uses a simple trace-grade task with zero writes; it does not exercise every grading mode or consumer. Broad ATIF compatibility also relies on the ordinary fixture tests and source review.

## Gamma: typed SDK response validation (#9424 A12)

Source: `c427943a5c84ba5938a3549f24b27de551812a37`. Final checker SHA-256: `575a283ba8d4e09005b75ad8d81977eb532cc6c0bf2d47eaf80bca0bb62fbb9f`. Ordinary and independent checks explicitly enable `jev/blocking`; no live provider feature is enabled.

Four independent tests:

- `numeric_contract_rejects_invalid_answers_and_keeps_valid_calibrated_values`: Noul endpoints 0 and 1 pass; valid modern and legacy responses pass. Fifteen malformed examples cover Noul range, Choice confidence/range/mass/empty distribution/selected identity, and Score range/confidence/empty legend/negative or wrong-mass probabilities/legend-key mismatch. Rejection must use `ResponseValidation`, `MissingAnswer`, or `AnswerType`.
- `async_typed_calls_validate_the_requested_answer_and_option_set`: bounded local HTTP mocks test modern and legacy success, missing answer, wrong answer type, unexpected Choice option, and a Score legend with the wrong requested size through `Client::system_one`.
- `blocking_typed_calls_apply_the_same_request_contract`: the same positive and request-mismatch fixtures through `BlockingClient::system_one`.
- `raw_transport_keeps_the_unvalidated_response_available`: the async raw call preserves a successful HTTP response containing an invalid Noul value for callers that need raw compatibility.

The positive fixtures intentionally allow selected answers that differ from the probability argmax and confidence values that differ from the selected probability. A legacy Score may omit both selected and probabilities. These are required compatibility cases, not acceptance loopholes to tighten retrospectively.

Calibration: the base passed ordinary tests but failed independent acceptance; the historical reference passed both (11.688 seconds total). Omitting the Noul range guard compiled and failed both gates. Omitting request-aware coverage/option checks compiled and passed ordinary tests but failed the independent checker, demonstrating coverage beyond that ordinary suite.

Limits: this is a small fixed malformed-response panel. It does not sweep tolerance boundaries, every malformed legend/selected representation, all unexpected answer combinations, or large responses. JSON fixtures do not directly inject IEEE NaN/Infinity values. Only the async raw path is independently checked. A22 retry/deadline behavior is already present in the source and is outside this task; no live-provider, broader Coder/Gym/CoderBench, performance, or concurrency result follows from this gate.

## Evidence

The native evidence archive retains the exact checker files at
`inputs/alternative-beta/checker.rs` and `inputs/alternative-gamma/checker.rs`.
The earlier calibration summaries are
[beta](../system-one-delegation/eligibility-beta.json) and
[gamma](../system-one-delegation/eligibility-gamma.json). Use the final hashes
above: beta's earlier freeze-v2 record predates the retained calibration
correction and names a superseded checker.
