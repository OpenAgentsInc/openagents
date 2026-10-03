# Static review: aea05ee2-d29f-4c2c-b824-453847b66afe

Two material validation gaps are visible statically. The Score gap concerns standalone/per-answer decoding; its checked method enforces the requested upper bound.

## Method

Canonical payload/change identity and original file hashes verified. Full unified diffs and affected control flow inspected. Candidate and checker code were not executed. No Cargo, models, or remote jobs. No plan, arm mapping, native transcript, run result, or check result inspected. Native executions had ended when these packets arrived; separate diagnostic outcomes were not consulted.

Candidate manifest: `305ac37bfab1c4f2f0dbe186a91dadfd2908e09788c854586feb6517bd62afa5`. Source: `c427943a5c84ba5938a3549f24b27de551812a37`.

## P2: Empty Score metadata uses u32::MAX as the accepted upper bound

`crates/jev/src/answers.rs:540`. Classification: `demonstrated_static_contract_gap`.

When legend and probabilities are both empty, check_score uses low=0 and high=u32::MAX. The pinned SDK supports only two to ten Score levels (questions.rs:20–24 and README.md:74–75), so an arbitrary integer-map maximum is not a valid fallback Score range.

Call public SystemOneResponse::decode on {"model":"m","answers":{"q":{"type":"score","score":100,"confidence":0.5,"legend":{}}}}. The plain system_one path uses this same decoder.

The finite score=100 falls within 0..u32::MAX and returns as a typed Score. The new system_one_checked catches a matching two-level Score request later in check_levels (277–295), so this example is not a claim that its requested Score bound is also missing. An extra Score answer not requested receives only per-answer decoding.

The task calls for numeric and Score legend bounds before returning typed decisions. Legacy omission of probabilities should not turn the strict decoder into an unrestricted unsigned-integer range; a caller that needs raw compatibility already has a separate path.

Static inference only; not executed.


## P2: Request-aware validation uses the pre-override question set

`crates/jev/src/client.rs:273`. Classification: `demonstrated_static_contract_gap`.

SystemOneRequest documents that extra_body replaces SDK-written fields (56–58). body() serializes self.questions (135–138), then merges extra_body last (140–142). The checked entrypoint sends that prepared body but validates the response against request.questions, which can differ from the wire questions.

Construct a valid request with question q of type Noul, then supply extra_body.questions.q as a valid Choice over a and b. Return a valid Choice answer for q.

The checked API rejects the valid Choice reply because the original field still says Noul. Conversely, a valid Noul response is accepted although the actual sent question was Choice. The blocking wrapper shares the same path.

The task requires request-aware answer type, coverage, and identity checks. The supported last-write body override must either be accounted for by validation or explicitly rejected before sending at this strict boundary.

Static source inference only; no HTTP or candidate execution.

## Existing tests and scope

No existing assertions were removed or relaxed. The blocking fixture adds the third Choice option and Score level already present in its served response. Seven new integration tests cover numeric tables, request misfits in async/blocking checked calls, raw bypass, and valid legacy/calibrated shapes. New mock listeners are detached loops without a shutdown handle or socket read timeout, so they are not lifecycle-bounded as the task requested; no observed hang is claimed.

Six changed/added files stay under crates/jev, including the README error description and public API inventory. No dependencies, unrelated packages, deletions, or existing-file modes changed. New validation.rs uses mode 0600.

## Limits

Request-aware validation is opt-in through system_one_checked; system_one intentionally validates individual answers only. This policy choice is disclosed, not counted separately as a demonstrated defect. The checked Score range includes both the nonnegative per-answer lower bound and request upper bound. The documented mass tolerance grows with rounded entries and is not labeled a defect merely for being permissive. Static findings do not change primary scores or incorporate post-panel diagnostic results.
