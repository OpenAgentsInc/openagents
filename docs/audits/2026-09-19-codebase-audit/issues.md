# Open issue map

The audit reviewed the bodies and available comments of all 27 issues returned by
`gh issue list --state open --limit 500` on September 19, 2026, Central time
(September 20 UTC). The source snapshot is `1843fa6c18`.
Issue state and titles can change after this record. A roadmap's older description
of missing functionality is not treated as proof that it remains missing.

The [audit findings](README.md#priorities) use A01–A25. “Related” means the issue
provides context or depends on the fix; it does not mean that the issue already
covers the defect. This table preserves the original review snapshot.

The subsequent implementation issues and explicit roadmap blockers are listed
in the [remediation register](remediation.md). The follow-up also checks closed
#9376 and #9384 because A05 and A07 affect the interpretation of their measurements.

## Agent and program roadmap

| Issue | State of the work at review | Relationship to this audit |
| --- | --- | --- |
| [#9402: main is not rustfmt-clean, and it costs every agent that notices](https://github.com/OpenAgentsInc/openagents/issues/9402) | Existing cleanup work; reproduced on Rust 1.95.0 across 51 files. | A23 confirms it. Keep formatting separate from behavior fixes. |
| [#9403: CoderBench: our own goldens, and the first one is the Devin fan-out](https://github.com/OpenAgentsInc/openagents/issues/9403) | The task, restored golden, and provenance sidecar exist. The implementation comment supersedes the original missing-artifact description. | A04 concerns grading correctness. The staged golden's presence does not demonstrate a successful Coder episode. |
| [#9404: [Roadmap] Coder runs the golden's path, over the relay, driven from a script](https://github.com/OpenAgentsInc/openagents/issues/9404) | Headless turns, delegation, and registry/probe support have landed. Program execution and the full observed episode remain open. | Follow the latest comment: replace the staged golden with the actual Coder recording when the runtime works. Complete A01–A10 and A17–A18 as applicable before claiming the end-to-end path is proven. |
| [#9406: [2] coderbench run: drive Coder, capture the trace, judge it](https://github.com/OpenAgentsInc/openagents/issues/9406) | The driver and preflight checks landed during the audit; the issue is now closed. Retained here to explain the delivered work. | A04 and A13 still affect its grade; retain failed and incomplete runs. |
| [#9409: [5] A program runtime that runs delegate-fan-out from its definition](https://github.com/OpenAgentsInc/openagents/issues/9409) | A registry can parse definitions; it does not yet execute them. | Require proven admission, trust, write isolation, deadlines, and resource bounds from A01–A03 and A17–A18. Parsing a definition is not execution conformance. |
| [#9410: [6] Prove the relay transport under a recorded episode](https://github.com/OpenAgentsInc/openagents/issues/9410) | The client exists; a comparable recorded episode remains planned. | Add replay, subscription cleanup, reconnect, and deadline cases from A08–A11. A successful episode alone misses these defects. |
| [#9411: [7] The tuning loop, with the episode's own noise floor measured first](https://github.com/OpenAgentsInc/openagents/issues/9411) | Control-versus-control measurement is explicitly required before tuning. | Preserve that requirement. Repair A04–A07 before treating a favorable grade or calibrated statistic as an improvement. |
| [#9413: [9] Use the fan-out to clear the backlog, and break the system doing it](https://github.com/OpenAgentsInc/openagents/issues/9413) | Intended operational use and stress test. | Run only after execution ownership and enforced bounds are proven; test interference and actual workspace changes, not just task declarations. |

The delegation and capability-probe issues, #9407 and #9408, were open at the
start of the audit and closed as their implementations landed. The final source
review includes both implementations and the subsequently landed #9406 driver. Their absence is therefore not reported
as a finding. The headless and golden-provenance changes are also included.

## Decision contracts and production behavior

| Issue | State of the work at review | Relationship to this audit |
| --- | --- | --- |
| [#9383: [Decision models] Should a model be able to say it does not know?](https://github.com/OpenAgentsInc/openagents/issues/9383) | An explicit abstention/unknown contract is under discussion. | Preserve unknown separately from false, failed, and refused. A04, A12, and A18 show why collapsing these states is already consequential. |
| [#9394: [Lev] Our Score contract promises a position and our numbers report an argmax](https://github.com/OpenAgentsInc/openagents/issues/9394) | Score semantics are already identified as inconsistent. | Do not create a duplicate issue for this known problem. Coordinate the corrected contract with A05's served-choice/metric alignment. |
| [#9395: The production question set does not discriminate on production traffic](https://github.com/OpenAgentsInc/openagents/issues/9395) | Production question quality is an acknowledged gap. | Improve and measure the questions, while independently fixing A01's host execution policy. Better questions cannot enforce a permission boundary. |
| [#9396: retry is a shell verdict Agent::turn cannot act on](https://github.com/OpenAgentsInc/openagents/issues/9396) | The missing retry transition is already tracked. | Define a typed transition and test what changes on retry; pair it with A02–A03 so a retry cannot overlap work that is supposedly finished. |
| [#9397: damage >= 0.7 means something different on every door](https://github.com/OpenAgentsInc/openagents/issues/9397) | Cross-door threshold meaning needs calibration and contract work. | A05 and A12 identify implementation prerequisites. Do not choose a replacement threshold from this audit's synthetic examples. |
| [#9398: A quarter of real turns do not fit on device, and that decided the door](https://github.com/OpenAgentsInc/openagents/issues/9398) | Real-turn capacity and refusal behavior need to drive routing. | A07 must keep refusals visible in comparisons; A14–A15 add deadline and memory admission requirements. |
| [#9414: A door confidently denies a fact its own state asserts](https://github.com/OpenAgentsInc/openagents/issues/9414) | Newly opened during the audit: a repeated confidently wrong read-only judgment needs controlled cross-door investigation. | Related to A18, but distinct: measuring a model's judgment does not enforce filesystem permissions. Preserve the issue's separation of hypotheses and held-out evaluation. |

## Measurement, admission, and training

| Issue | State of the work at review | Relationship to this audit |
| --- | --- | --- |
| [#9345: [Lev] Apple FM decision-model roadmap and tracking](https://github.com/OpenAgentsInc/openagents/issues/9345) | Ongoing umbrella roadmap with substantial implementation already present. | Preserve completed bridge, estimator, and admission work; prioritize A05 and A14 alongside remaining experiments. |
| [#9363: [Lev 11] Train the first adapter and prove it beats the base](https://github.com/OpenAgentsInc/openagents/issues/9363) | Adapters and measurements exist. Later comments qualify earlier improvement and flip-rate claims after variance and definition checks. | Do not repeat superseded headline claims. Fix A05 and retain paired, comparable evidence before interpreting another calibrated result. |
| [#9364: [Gym] The measurement and control plane for decision models](https://github.com/OpenAgentsInc/openagents/issues/9364) | The Gym, receipt store, gates, and terminal are implemented; the issue remains an umbrella. | A05–A07 target current correctness, not an absent control plane. |
| [#9377: [Gym 13] Add the cheap baseline we skipped: frozen embeddings + logistic regression](https://github.com/OpenAgentsInc/openagents/issues/9377) | The comments report baseline experiments. A registered door, calibration, and associated documentation remain. | Keep encoder/revision and regularization provenance. Use A07's shared refusal contract when adding the door. |
| [#9380: [Gym 15] Measure the adapters out of domain](https://github.com/OpenAgentsInc/openagents/issues/9380) | Generalization work remains planned. | Necessary after correcting implementation and data-partition integrity; passing an in-domain gate is not generalization evidence. |
| [#9381: [Gym 16] Validate the instrument, not just the measurement](https://github.com/OpenAgentsInc/openagents/issues/9381) | Label quality, agreement, partition discrimination, and baseline headroom are in scope. A comment adds the headroom-versus-floor check. | A05–A07 are distinct instrument defects. Report measurement capacity before spending another experiment. |
| [#9382: [Gym 17] Put cost and latency in the gate](https://github.com/OpenAgentsInc/openagents/issues/9382) | `deployment-v1` exists. A defensible quiet-machine latency floor and completion of its acceptance criteria remain. Later comments correct earlier comparison claims. | Do not report cost/latency gating as wholly absent. Keep unmeasured bounds explicit and avoid using this audit's contended test timings as a floor. |
| [#9387: Measure what our 98 labels bought, against a zero-label compile](https://github.com/OpenAgentsInc/openagents/issues/9387) | A controlled baseline comparison is planned. | Retain fixed question, suite, model, and gate identities; address A05–A07 before drawing conclusions. |
| [#9389: Replace static admission with a behavioral floor](https://github.com/OpenAgentsInc/openagents/issues/9389) | Behavioral admission is planned. | A floor needs reliable outcomes and capacity handling. This does not replace structural admission for host permissions or resource bounds. |
| [#9391: The gate digest covers its own prose](https://github.com/OpenAgentsInc/openagents/issues/9391) | The issue proposes separating structured basis from explanatory prose. Comments note that a pending bound's prose currently contains its only justification. | Preserve substantive provenance when separating wording. A structured pending-measurement basis must remain inside the digest; removing all prose mechanically would lose meaning. |
| [#9393: [Gym 19] Finish the four-way on support-v2-three-way: hosted Jev, and latency on a quiet machine](https://github.com/OpenAgentsInc/openagents/issues/9393) | Later comments report the hosted Jev comparison completed. The quiet-machine latency work remains. | Do not duplicate the hosted run or treat absent shell credentials as proof that the service is unavailable. This audit makes no new paid comparison calls. |
| [#9399: Half the locked partition is training data for every adapter](https://github.com/OpenAgentsInc/openagents/issues/9399) | Training/evaluation overlap is already tracked. | A06 is a separate concurrent-spend defect. Both must be fixed: an atomic ledger cannot make contaminated data held out. |
| [#9401: The confident-error floor refuses an unchanged door half the time](https://github.com/OpenAgentsInc/openagents/issues/9401) | Gate variance and false refusal are already tracked. | Fix A05's correctness alignment first, then estimate an appropriate control-versus-control floor with independent, comparable data. |

## Backlog additions suggested by the audit

The existing issues already cover the main feature direction and many statistical
questions. Add focused implementation work for the findings rather than another
umbrella roadmap. Useful groupings are subprocess supervision (A02–A03), execution
admission (A01, A17–A18), evidence integrity (A04–A07, A12–A13), transport lifecycle
(A08–A11, A22), model serving (A14–A16), and relay operations (A19–A21).

Each fix should include the negative case described in the finding and a clear
consumer-level acceptance test. A23–A25 can be handled as separate repository
maintenance changes so they do not obscure behavioral review.
