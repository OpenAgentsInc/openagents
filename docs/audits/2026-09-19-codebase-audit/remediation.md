# Remediation register

This register turns the audit into implementation work after review by the
author of the affected code. A01–A08 each have an individual P1 issue. Related
P2 findings share bounded acceptance lists; A23–A25 remain separate maintenance
issues. Two additional issues cover the unnumbered terminal and protocol-testing
recommendations. Issue creation does not mean a finding is fixed.

The report's original source and open-issue snapshots remain historical records.
The implementation issues link that evidence and specify the negative cases
required to close each finding. Partial implementation changes that arrived
during this follow-up are identified below; they do not close a finding without
its remaining acceptance evidence.

## Implementation work

Created 19 issues with source evidence, acceptance criteria, and dependencies:

| Issue | Priority | Coverage |
| --- | --- | --- |
| [#9415: [P1][Audit A01] Require host execution intent before accepting a shell plan](https://github.com/OpenAgentsInc/openagents/issues/9415) | P1 | A01 |
| [#9416: [P1][Audit A02] Terminate and reap subprocess trees on timeout or cancellation](https://github.com/OpenAgentsInc/openagents/issues/9416) | P1 | A02 |
| [#9417: [P1][Audit A03] Bound subprocess output while it is captured](https://github.com/OpenAgentsInc/openagents/issues/9417) | P1 | A03 |
| [#9418: [P1][Audit A04] Make CoderBench grades require verified outcomes and complete evidence](https://github.com/OpenAgentsInc/openagents/issues/9418) | P1 | A04 |
| [#9419: [P1][Audit A05] Align calibrated choices with labels and re-derive dependent measurements](https://github.com/OpenAgentsInc/openagents/issues/9419) | P1 | A05 |
| [#9420: [P1][Audit A06] Serialize and durably record locked-partition reads](https://github.com/OpenAgentsInc/openagents/issues/9420) | P1 | A06 |
| [#9421: [P1][Audit A07] Count Kev refusals correctly and reconcile the #9384 evidence](https://github.com/OpenAgentsInc/openagents/issues/9421) | P1 | A07 |
| [#9422: [P1][Audit A08] Bind relay feedback and results to the signed current job](https://github.com/OpenAgentsInc/openagents/issues/9422) | P1 | A08 |
| [#9423: [P2][Audit A09–A11] Make streaming, relay sessions, and identity persistence reliable](https://github.com/OpenAgentsInc/openagents/issues/9423) | P2 | A09, A10, A11 |
| [#9424: [P2][Audit A12, A22] Validate decision responses and enforce whole-call SDK deadlines](https://github.com/OpenAgentsInc/openagents/issues/9424) | P2 | A12, A22 |
| [#9425: [P2][Audit A13] Recover torn traces while enforcing strict grading integrity](https://github.com/OpenAgentsInc/openagents/issues/9425) | P2 | A13 |
| [#9426: [P2][Audit A14–A16] Bound model serving and unify advertised door behavior](https://github.com/OpenAgentsInc/openagents/issues/9426) | P2 | A14, A15, A16 |
| [#9427: [P2][Audit A17–A18] Require trusted probes and verified enforcement before delegation](https://github.com/OpenAgentsInc/openagents/issues/9427) | P2 | A17, A18 |
| [#9428: [P2][Audit A19–A21] Align relay search and harden media admission and restore consistency](https://github.com/OpenAgentsInc/openagents/issues/9428) | P2 | A19, A20, A21 |
| [#9429: [P2][Audit A23] Define and restore the workspace Rust verification baseline](https://github.com/OpenAgentsInc/openagents/issues/9429) | P2 | A23 |
| [#9430: [P2][Audit A24] Remove generated Swift and Python artifacts from tracked source](https://github.com/OpenAgentsInc/openagents/issues/9430) | P2 | A24 |
| [#9431: [P2][Audit A25] Adopt a dependency policy and resolve the paste maintenance advisory](https://github.com/OpenAgentsInc/openagents/issues/9431) | P2 | A25 |
| [#9432: [P2][Audit] Restore terminal state reliably and preserve execution events under load](https://github.com/OpenAgentsInc/openagents/issues/9432) | P2 | Terminal lifecycle, event delivery, and long-session behavior |
| [#9433: [P2][Audit] Verify Nostr parsing and cryptographic primitives with independent tests](https://github.com/OpenAgentsInc/openagents/issues/9433) | P2 | Independent protocol/cryptographic validation and fuzzing |

## Required sequencing

- **#9413 is blocked on A01, A17, and A18.** A plan requires host execution
  intent; executable probes require host trust; admission requires verified
  enforcement. A01 has a verified fix in `25f0b54e4a`, recorded in the
  [verification record](verification.md#a01-after-the-fix); A17 and A18 remain
  open. A model's independence/read-only judgment and a successful staged
  golden cannot waive these requirements. Process-tree cancellation and capture
  bounds from A02–A03 also precede unattended work; both have a verified fix in
  `a96845c40d`, recorded in the
  [verification record](verification.md#a02-and-a03-after-the-fix).
- **#9409 delivered part of A18.** Commit `1eb60eccea` refuses unknown bounds and
  supports worktrees when configured. Admission still derives executor
  enforcement from the manifest's declaration. Verified executor enforcement,
  trusted executable probes, and read-only protection remain requirements of
  #9427; worktrees alone do not establish them.
- **A02 superseded the partial delegate deadline fix.** The `kill_on_drop(true)`
  the audit found killed the direct delegate and left its grandchildren running,
  and the shell runner lacked even that. Both paths now run through one
  subprocess supervisor that owns the process group, so the finding is closed by
  ownership rather than by a second call-site patch.
- **A05 owns the post-fix #9376 re-derivation.** Review raw and mapped provenance,
  regenerate affected measurements, and state which numbers change. The quoted
  raw standard deviations did not pass through `mapped_observations`; unchanged
  values remain acceptable when independently re-derived under the chosen
  contract. Update gate digests when semantics or adopted values change.
- **A07 owns the post-fix #9384 reconciliation.** The old classifier cannot prove
  proper refusal accounting. The retained row sets independently cover all 157
  expected open items per Kev variant; verify receipt/run provenance and failure
  evidence before deciding whether any numerical claim changes.
- **#9411 requires sound grades and metrics.** Do not optimize against the known
  false pass in A04 or claim a mapped-metric improvement before A05 is resolved.
  #9410's transport proof includes A08 and the client lifetime/error cases.

## Implementation arriving during this follow-up

[#9409](https://github.com/OpenAgentsInc/openagents/issues/9409) closed with
[commit `1eb60eccea`](https://github.com/OpenAgentsInc/openagents/commit/1eb60eccea31873af59fe2df65e16ce46b99b928).
The runtime distinguishes host, executor, ignored, and unknown enforcement and
refuses unknown bounds. Worktree delegation now creates separate checkouts when
a repository is configured and still refuses the request when none is available.
The [verification record](verification.md#runtime-admission-and-worktrees)
identifies the targeted tests for this partial progress.

The runtime's `check` still labels a bound as executor-enforced solely because
`manifest.enforces` names it. That is a claim, not independently verified
enforcement. Likewise, a separate checkout does not prohibit filesystem writes.
The historical A18 reproduction remains evidence for its pinned snapshot;
#9427 and the #9413 blockers remain open for the unmet trust and enforcement
criteria. This update does not claim a full audit of the newly landed runtime.

## Implementation that has landed

**A04 is fixed.** `Task::judge` answers with `gym::gate::Verdict`, so a run is
`passed`, `unverifiable`, or `failed` rather than clean or faulted.
`delegations_correct` is enforced against delegations the trace records as
completed, correct, and holding an answer; an unknown correctness value is a
named fault and never counts. Observation keeps each call's outcome, the
answers a decision returned, the order the steps came in, the end record, and
the unreadable-line count. A task states the terminal outcomes it allows and
the decision predicates the run gates on, and the driver makes the exit code a
grading fault rather than a printed line. A task that forbids writes is judged
against the checkout read before and after the run, so an absent `wrote` field
is unknown rather than proof.

`crates/coderbench/tests/negative.rs` holds the runs that must not grade clean,
starting with the constructed run this finding was reproduced with.
`coderbench diff` on the staged golden now exits `4`: a trace carries neither
the exit code nor the workspace. The golden's provenance is unchanged, and
[#9404](https://github.com/OpenAgentsInc/openagents/issues/9404) still owns
replacing it with a run Coder drove.

## Recorded dependencies and follow-ups

GitHub records #9413 as blocked by [#9415](https://github.com/OpenAgentsInc/openagents/issues/9415),
[#9416](https://github.com/OpenAgentsInc/openagents/issues/9416),
[#9417](https://github.com/OpenAgentsInc/openagents/issues/9417), and
[#9427](https://github.com/OpenAgentsInc/openagents/issues/9427). Its body also names
A01, A17, and A18 separately as mandatory acceptance criteria.

- [#9376 re-derivation follow-up](https://github.com/OpenAgentsInc/openagents/issues/9376#issuecomment-5747232502) is owned by #9419.
- [#9384 evidence-reconciliation follow-up](https://github.com/OpenAgentsInc/openagents/issues/9384#issuecomment-5747232576) is owned by #9421.
- [#9409 admission coordination](https://github.com/OpenAgentsInc/openagents/issues/9409#issuecomment-5747232652) links the narrow runtime change to #9427 without treating the remaining trust/isolation work as complete.

## Coverage of additional recommendations

| Recommendation | Implementation owner |
| --- | --- |
| Validated plan versions and host execution permissions | A01 |
| Process ownership, blocking work, and resource admission | A02–A03 and A14–A16 |
| Probability invariants, response coverage, and structured SDK errors | A12 and A22 |
| Supported serving-state constructors and encoding invariants | A14–A16 |
| Checked duration/concurrency bounds and explicit unknown enforcement | A17–A18 |
| Trace lifecycle, persistent evidence integrity, and recovery policy | A04, A06, and A13 |
| Terminal RAII, reliable control events, bounded scrollback, and resize/editing tests | [#9432](https://github.com/OpenAgentsInc/openagents/issues/9432) |
| Differential byte/parser tests and independent cryptographic vectors | [#9433](https://github.com/OpenAgentsInc/openagents/issues/9433), A09, A13, and A19 |
| Toolchain/feature matrix, crate/lint policy, README and historical-plan reconciliation, and Python-tooling exception | A23, with existing #9402 for formatting |
| Generated artifact removal and reproducible helper builds | A24 |
| Dependency/source/license policy and public reimplementation provenance | A25 |
| Shared turn runner and terminal design system | A01 and [#9432](https://github.com/OpenAgentsInc/openagents/issues/9432) acceptance criteria |
| Preserved transcripts, explicit test skips, and manual/non-GitHub checks | Relevant trace, serving, and maintenance issue acceptance criteria |

Existing statistical, question-quality, and feature work remains in the
[original issue map](issues.md). These remediation issues add implementation
acceptance criteria and targeted rechecks without duplicating that roadmap.
