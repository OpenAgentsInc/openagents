---
spec_format_version: "0.1"
title: "Done-Condition Verification for Autonomous Runs"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-20T00:00:00Z"
updated_at: "2026-07-20T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "9104"
  openagents_lane: "VSE-04 (#9109)"
  openagents_assurance_level: "verification-governance"
  openagents_origin: "docs/fable/2026-07-20-verifiable-software-engine-gap-analysis-and-roadmap.md gap G4"
  openagents_supersedes_cut: "CUT-FA-04 in specs/desktop/full-auto.product-spec.md (automatic done-condition verification, explicitly deferred there)"
  openagents_admission_status: "candidate specification; not owner-admitted. Design may proceed now; activation depends on Full Auto Wave 0 closure (#8978, #8979). This document declares the contract; it flips no promise and admits nothing."
  openagents_first_class: "repository objectives adjudicable by the assure-repo sweep machinery"
  openagents_prototype: "packages/assure-repo/src/done-condition.ts (fail-closed verdict library and tests land with this spec)"
---

## Problem

Full Auto's ProductSpec explicitly cuts automatic done-condition verification
(CUT-FA-04). "Completed" is a self-reported, owner-reviewable disposition
backed by the provider's own turn output, never a verified-truth claim. That
cut is honest, and it is also the engine's acceptance stage failing open at the
flagship: an autonomous run that works while nobody watches ends in a state
nobody verified. Provider prose cannot prove completion, yet nothing beside the
provider disposition adjudicates whether the objective was actually met. The
gap analysis names this G4. The fix is not to trust the provider harder. It is
to add, for the objective classes that admit one, a verdict issued by an oracle
rather than by prose, recorded as a distinct fact beside the self-reported
disposition.

## Hypothesis

If autonomous runs carry a typed done-condition verdict issued by a
machine-checkable oracle for the objective classes that admit one, starting
with repository objectives the assure-repo sweep already adjudicates, and if
that verdict is recorded as a distinct fact that never merges into the provider
disposition and fails closed to `unverified` or `unavailable` whenever the
oracle is absent, errors, or is fed stale evidence, then a run can terminate
with machine-checked objective completion where it is possible and an honest
self-reported label where it is not, without ever presenting provider prose as
verified truth.

## Scope

```productspec-scope
in:
  - a typed done-condition verdict distinct from the provider disposition: objective class, verdict state (`verified` / `unverified` / `unavailable`), a source-bound evidence ref, a reason, and a timestamp
  - an objective-class model naming which classes are machine-adjudicable and which are not; an objective with no oracle is `unsupported` and yields `unavailable`, never `verified`
  - a first machine-adjudicable class — repository objectives (named tests pass at a pinned commit, a drift-clean document set, an inventory-row transition) decided by the assure-repo sweep machinery
  - fail-closed rules: an absent oracle yields `unavailable`; an oracle error yields `unverified`; evidence that is stale, unpinned, or not bound to the run's commit yields `unverified`; only fresh, source-bound, matching evidence yields `verified`
  - the recording rule: the verdict is stored beside the provider disposition, and a `completed` disposition never implies a `verified` verdict
  - exact-replay verification as a second class where the work admits it (the `docs/tassadar/` route), gated to replayable work only
out:
  - a claim that all objectives are machine-checkable; natural-language or open-ended objectives stay `unsupported` and keep the honest self-reported label
  - any change to Full Auto's current honest labeling before this contract is separately admitted
  - autonomous provider selection, fleet scheduling, or any expansion of the Full Auto run authority beyond adding a verdict fact
  - replacing owner acceptance; an oracle verdict informs the owner, it does not stand in for owner review where the AssuranceSpec requires it
cut:
  - self-amplification: a `verified` verdict grants no release, promise-flip, spend, or public-claim authority on its own
  - inferring a verdict from provider prose, a timeout, a screenshot, or a completed turn
  - a `verified` verdict from evidence the run cannot reproduce or that is not bound to the run's exact commit
```

## User Experience

An owner launches an autonomous run with a repository objective — for example,
"the named test command passes at HEAD and the governed docs are drift-clean."
When the run terminates, the run report shows two distinct facts: the provider
disposition (what the harness said it did) and the done-condition verdict (what
the oracle observed). If the objective is machine-adjudicable and its evidence
is fresh and commit-bound, the verdict reads `verified`. If the oracle could
not run, the verdict reads `unavailable`. If the evidence is stale or the oracle
errored, it reads `unverified`. A natural-language objective shows
`unavailable` with `unsupported` as its class, and the honest self-reported
disposition stands alone. The owner never sees "completed" rendered as
"verified."

## Acceptance Criteria

- A run terminal state carries a done-condition verdict as a field distinct
  from the provider disposition. The two are never merged into one indicator,
  and a `completed` disposition does not set the verdict.
- The verdict state is exactly one of `verified`, `unverified`, or
  `unavailable`. There is no permissive `pending` or `warning` state.
- For a repository objective, `verified` is returned only when the named
  evidence is fresh, bound to the run's exact commit, and matches the
  objective. Any mismatch, absence, or staleness yields `unverified` or
  `unavailable`.
- An absent oracle yields `unavailable`. An oracle error yields `unverified`.
  Stale or unpinned evidence yields `unverified`. None of these yields
  `verified`, and none blocks the honest self-reported disposition path.
- An objective with no registered oracle is classed `unsupported` and yields
  `unavailable`. It is never silently treated as met.
- A `verified` verdict grants no authority on its own: it cannot flip a
  promise, authorize a release, or stand in for owner acceptance where the
  governing AssuranceSpec requires review.

## Success Metrics

```productspec-success-metrics
- id: verdict_disposition_separation
  metric: terminal_run_states_recording_verdict_and_provider_disposition_as_distinct_fields
  target: "100%; zero states where completed is rendered as verified"
  window: every autonomous run once this contract is activated
  segment: all Full Auto terminal states
  source: run_report_verdict_records
- id: fail_closed_correctness
  metric: absent_error_or_stale_evidence_cases_that_yield_verified
  target: "0"
  window: every deterministic done-condition fixture suite and every live run
  segment: repository-objective and unsupported-objective adjudications
  source: done_condition_oracle_fixtures_and_receipts
- id: repository_objective_soundness
  metric: verified_verdicts_whose_evidence_is_fresh_commit_bound_and_matching
  target: "100% of verified verdicts"
  window: every repository-objective adjudication
  segment: runs with a repository objective
  source: verdict_evidence_binding_audit
```

## Risks

- A verdict field that can be set from the provider disposition would recreate
  exactly the false green this contract exists to remove. The verdict must be
  computed only by an oracle and never assigned from harness prose.
- An oracle that reads unpinned or ambient evidence can report `verified` for
  a stale state. Evidence must be bound to the run's exact commit, and
  staleness must fail closed.
- Expanding the machine-adjudicable set too fast risks classing open-ended
  objectives as adjudicable and manufacturing verdicts. The `unsupported` class
  must remain the default for anything without a sound oracle.
- A `verified` verdict is a strong signal and an attractive shortcut. It must
  not become release, promise, or acceptance authority by adjacency.

## Solution

The verdict is a small typed fact with three states and a source-bound evidence
ref, computed by a fail-closed adjudicator. The first adjudicable class reuses
the assure-repo sweep machinery: a repository objective names a test command, a
governed-document set, or an inventory transition, and the adjudicator returns
`verified` only when the sweep receipt is fresh and bound to the run's commit.
Absence, error, and staleness map to `unavailable` / `unverified` by
construction. A prototype library
(`packages/assure-repo/src/done-condition.ts`) lands with this spec to prove
the fail-closed semantics under test before any Full Auto activation. Activation
— wiring the adjudicator into the Full Auto terminal state so a real run carries
the verdict — depends on Full Auto Wave 0 closure (#8978, #8979) and is a
separate admitted step, not part of this contract's landing.

## Related Artifacts

- Origin gap analysis:
  `docs/fable/2026-07-20-verifiable-software-engine-gap-analysis-and-roadmap.md`
- The cut this supersedes: `specs/desktop/full-auto.product-spec.md` (CUT-FA-04)
- Full Auto assurance and release gates: #8978 and #8979
- Repository-objective adjudication machinery: `packages/assure-repo`
- Exact-replay verification route: `docs/tassadar/`
- Fail-closed prototype: `packages/assure-repo/src/done-condition.ts`

## Owner Gates

- Activation — wiring the adjudicator into a real Full Auto run — is an owner
  action gated on Full Auto Wave 0 closure. The prototype library and its
  fixtures do not authorize activation.
- Any public claim that autonomous runs are machine-verified. A landed spec
  and passing fixtures alone cannot authorize that statement.

## Receipts

- Done-condition verdict records binding objective class, verdict state,
  evidence ref, reason, and timestamp, stored beside the provider disposition.
- Fail-closed fixture receipts for the absent-oracle, oracle-error, and
  stale-evidence cases.
- Repository-objective adjudication receipts binding a `verified` verdict to a
  fresh, commit-bound sweep receipt.

## Promise Links

- No promise flips to green from this spec. Autonomous-run verification
  promises stay non-green until a real run carries an oracle-issued verdict
  after activation.
