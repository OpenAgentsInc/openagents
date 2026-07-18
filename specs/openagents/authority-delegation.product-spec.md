---
spec_format_version: "0.1"
title: "Owner-Delegated Autonomous Delivery"
artifact_type: "prd"
spec_revision: 2
author: "OpenAgents"
created_at: "2026-07-18T00:00:00Z"
updated_at: "2026-07-18T00:00:00Z"
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
  openagents_design: "docs/authority/AUTHORITY_DELEGATION_SPEC.md"
  openagents_profile: "AUTHORITY.md revision 2"
  openagents_accepted_plan: "docs/sol/2026-07-18-owner-delegated-autonomy-accepted-plan.md"
  openagents_assurance_level: "cross-resource-action-authority"
---

## Problem

OpenAgents has strong product, proof, learning, issue, claim, runtime, and
release contracts, but operating authority is scattered across current owner
messages, repository prose, credentials, service-account access, UI gates, and
historical owner-action notes. Agents repeatedly treat an inaccessible device,
uncertain login, stale claim, missing automation seam, or old “owner gate” as a
reason to stop even when a safe route already exists.

That behavior burns owner attention and leaves issues open. The opposite
failure is equally dangerous: broad instructions to “be autonomous” can be
misread as unlimited permission to spend, release, extract credentials, move
money, weaken invariants, or make unsupported public claims.

## Hypothesis

If OpenAgents records owner delegation as an exact, typed, revocable authority
profile with action/resource selectors, budgets, independence rules, a
mandatory blocker-resolution ladder, reserved actions, and bounded receipts,
then Full Auto and operating agents can close work continuously through
existing local and Google Cloud authority without either waiting on ordinary
blockers or inventing unlimited company power.

## Scope

```productspec-scope
in:
  - an AuthorityDelegationSpec companion distinct from ProductSpec, AssuranceSpec, and FastFollowSpec
  - one admitted root authority profile with stable identity, revision, lifecycle, precedence, programs, grants, conditions, independence, escalation, reserved actions, and receipts
  - exact action and resource matching with intersection composition, explicit-deny precedence, and no self-amplification
  - a phase-ordered program for Full Auto release, root specs, complete Fast Follow harvest, and transcript-driven user/revenue work
  - repository/GitHub delivery, issue closure, existing provider/device operation, Google Cloud operations, release, and evidence-bound promise-transition grants
  - conservative numeric cloud and external-spend caps with fail-closed exhaustion
  - owner-designated independent AssuranceSpec review that remains distinct from production and evidence generation
  - a mandatory verify, use-existing-access, use-product-surface, substitute, automate, repair, narrow, and irreducible-escalation ladder
  - typed authority-decision receipts that reference private evidence without exposing it
  - current-profile validation and later Effect service, runtime adapter, revocation, budget, and model-checking enforcement
  - autonomous RC impact selection, signed-candidate GitHub publication, linked issue and Forum communication, tester-feedback intake, changelog attribution, and bounded rollback
out:
  - AuthorityDelegationSpec deciding product intent, evidence sufficiency, learning relevance, issue priority, or public truth by itself
  - possession of a credential, cloud role, device, provider session, model output, test result, or upstream source implying additional authority
  - one producer verifying or admitting its own AssuranceSpec obligation
  - silent release, public claim, production mutation, or spend without its exact conditions
  - a second issue queue, roadmap, provider dispatcher, release graph, or promise registry
cut:
  - raw secret, key, mnemonic, token, or credential-store extraction
  - wallet, treasury, custody, payout, settlement, charge, refund, or irreversible financial movement
  - legal contracts, employment decisions, regulatory attestations, or human identity ceremonies
  - destructive customer-data deletion or irreversible production migration
  - paid advertising, bulk unsolicited outbound, or new paid subscriptions in revision 2
  - stable release publication without a current explicit owner direction naming that release
  - unsigned Desktop code overlays or partial ReleaseSet promotion presented as OTA
  - self-modification that increases delegated authority
```

## User Experience

The owner gives one durable direction instead of repeating permission at every
packet. An operating agent can inspect the effective profile and tell exactly
which repo, issue, cloud, local-app, provider, release, or product-promise
action it may take. When a device or credential path fails, the agent visibly
moves through safe alternatives and keeps unrelated packets moving.

The owner sees only irreducible exception actions: a named human identity
ceremony, an exact budget expansion, a reserved legal/custody action, or an
external account boundary no admitted route can cross. Each request says what
was tried, the smallest UI action needed, what work continues, and what receipt
will close it.

An independent reviewer can reproduce a producer's Full Auto assurance proof
from a clean identity and admit it only when the bound AssuranceSpec explicitly
accepts an owner-designated reviewer. A release operator can promote only a
signed candidate whose exact assurance and release gates are green. Authority
never changes the visible evidence tier.

## Acceptance Criteria

- **AD-AC-01:** The root profile has stable format, profile, revision,
  lifecycle, admission, effective-time, and expiry/revocation fields. Unknown,
  inactive, malformed, expired, or unresolved profiles fail closed.
- **AD-AC-02:** Every grant names unique identity, roles, actions, resources,
  programs, and condition refs. Every ref resolves exactly once, and no grant
  overlaps a mandatory reserved category.
- **AD-AC-03:** Effective authority is the intersection of current system and
  owner instructions, law/platform terms, repository invariants, resource
  policy/runtime gates, the profile, and target-local contracts. Explicit deny
  wins; the delegate cannot amplify its own authority.
- **AD-AC-04:** Full Auto release, root-spec completion, complete Fast Follow,
  and promise/growth/revenue programs have a durable order and honest advance
  condition. Parallel non-colliding capacity may pull later admitted work, but
  it cannot demote the Full Auto critical path.
- **AD-AC-05:** Repository and GitHub delivery authority covers claim, edit,
  test, build, commit, push to main, issue update/closure with evidence, and
  release-candidate construction without requiring repeated owner ceremony.
- **AD-AC-06:** Existing authenticated local app/provider/device state may be
  operated through visible UI and typed APIs without exporting secrets.
  Process mutation is exact-owner scoped, and absent automation becomes an
  implementation packet rather than an owner wait.
- **AD-AC-07:** The documented automation identity may inspect, start, stop,
  repair, provision bounded ephemeral resources, run candidates, deploy
  staging, and roll back inside the admitted Google Cloud project under exact
  daily and recurring spend caps.
- **AD-AC-08:** A production or release action resolves authority again and
  requires exact admitted assurance, signed artifacts where applicable,
  platform receipts, clean release gates, rollback, and no blocking invariant
  failure. Access alone cannot promote.
- **AD-AC-09:** An evidence-bound product-promise transition may proceed under
  delegated authority only through the existing typed registry when every
  named verification gate is green. No public copy bypasses that registry.
- **AD-AC-10:** A designated independent reviewer has a separate execution
  identity, claim, and reproduction receipt from the producer. Producer,
  evidence producer, verifier, admitter, and release roles cannot collapse
  where the AssuranceSpec requires separation.
- **AD-AC-11:** The blocker-resolution ladder is ordered and mandatory. Only an
  irreducible reserved action, inaccessible human account identity, or budget
  expansion can settle as `needs_owner`; all other blockers route, automate,
  repair, narrow, or yield another admitted packet.
- **AD-AC-12:** `NEEDS_OWNER.md` entries identify reserved category, exact
  target, attempted ladder steps, smallest UI action, continuing work, and
  closure receipt. A generic waiting request is invalid.
- **AD-AC-13:** Authority receipts bind profile revision, program, grant, role,
  action, target, condition results, timestamps, outcome, and evidence refs.
  Receipts contain no raw secrets or unbounded private evidence.
- **AD-AC-14:** Revocation, profile supersession, budget exhaustion, or a
  security/invariant failure stops new actions immediately and moves in-flight
  work to the safest bounded checkpoint with a receipt.
- **AD-AC-15:** Growth/revenue authority in revision 1 permits research,
  priority decisions, product/onboarding/pricing/instrumentation work, and
  zero-spend reversible experiments, while paid outreach, contracts, customer
  charges, and financial movement remain refused.
- **AD-AC-16:** Deterministic repository tests validate the first admitted
  profile's required blocks, IDs, refs, order, anti-amplification literal,
  mandatory reserved categories, spend caps, independence, and links from
  repository law.
- **AD-AC-17:** Revision 2 authorizes a release operator to publish a strictly
  newer verified RC and its non-authoritative GitHub mirror, promote an
  otherwise-admitted signed RC, deploy eligible web/mobile OTA lanes, publish
  `/changelog`, and roll back the bounded service path without repeating an
  owner ceremony. Stable publication retains an exact current-owner gate.
- **AD-AC-18:** Release impact selection prevents web-only, mobile-only, docs,
  or release-infrastructure work from manufacturing a Desktop version or
  Windows build. Desktop renderer/host/native/shared-closure/lockfile changes
  retain the complete five-target requirement until a separately signed,
  compatibility-bound, first-launch-checked, rollback-capable renderer OTA is
  admitted.
- **AD-AC-19:** Candidate and publication communication is bounded and
  idempotent across linked GitHub issues, explicitly requested tester handles,
  and the Forum `release-candidates` topic. A structured tester PASS is
  receipted; BLOCKED or unstructured feedback becomes a linked Full Auto issue
  even when the source issue is already closed.
- **AD-AC-20:** Every public changelog row records trigger kind, triggering
  actor, release actor, source feedback, release URL, authority profile
  revision, program, and grant. Historical releases identify their historical
  owner direction and never borrow revision-2 authority retroactively.

## Success Metrics

```productspec-success-metrics
- id: owner_wait_reduction
  metric: actionable_issue_blockers_settled_as_generic_owner_wait_without_exhausting_the_resolution_ladder
  target: "0"
  window: every autonomous program iteration
  segment: Full Auto, root specs, Fast Follow, and growth programs
  source: authority_decision_and_issue_receipts
- id: delegated_issue_throughput
  metric: admitted_actionable_issues_closed_with_evidence_without_repeated_owner_permission
  target: ">= 90%"
  window: rolling 30 days after profile admission
  segment: actions within exact revision-1 grants
  source: github_closeout_and_authority_receipts
- id: authority_safety
  metric: secret_extraction_reserved_action_execution_budget_overrun_self_amplification_or_unsupported_public_claim_events
  target: "0"
  window: lifetime of each authority profile revision
  segment: all delegated actions
  source: authority_refusal_security_and_promise_audits
- id: full_auto_autonomy
  metric: full_auto_release_chain_gates_completed_without_generic_owner_wait
  target: "100% except irreducible human identity ceremonies"
  window: program.full_auto_release
  segment: open Full Auto issues and release obligations
  source: full_auto_assurance_release_and_authority_receipts
```

## Risks

- A prose-only profile can be applied inconsistently until runtime adapters
  enforce it; deterministic validation and exact receipts are the bootstrap
  defense, not the final control plane.
- Over-broad resource labels can hide privilege expansion. Each adapter must
  resolve them into exact project, repository, release channel, device,
  provider, or product-promise targets before execution.
- Independence can become ceremonial if a second session merely trusts the
  producer's summary. Review must reproduce evidence and record its own claim.
- Conservative budgets can still create waste through repeated small actions;
  cumulative measurement and stop-before-cap behavior are required.
- “Grow revenue” can be misread as permission to contact or charge people.
  Revision 1 deliberately limits that phase to product work and zero-spend
  reversible experiments.

## Owner Gates

- **RESOLVED FOR REVISION 2 BY CURRENT OWNER DIRECTION.** The owner explicitly
  directed OpenAgents to formalize broad delegated authority, stop waiting on
  ordinary owner/device blockers, use existing Google Cloud access, rapidly
  close issues, prioritize Full Auto, complete root specs and all Fast Follow,
  and then pursue transcript-backed user and revenue outcomes. The owner then
  explicitly directed agents to publish release builds, automate the
  RC17–RC20 tester/issue/update loop, avoid unnecessary Windows builds, use
  OTA where safely admitted, communicate through GitHub and Forum, and expose
  actor/authority attribution on `/changelog`. `AUTHORITY.md` revision 2 is
  the admitted profile for those directions.
- Any increase to the numeric spend caps, permission for paid outreach or new
  subscriptions, financial movement, legal commitments, destructive customer
  data action, raw secret access, or weakening of a mandatory invariant needs
  a current owner direction and profile revision.
- A platform flow that legally or technically requires the natural account
  holder remains an exact owner action; its existence does not stop unrelated
  admitted work.

## Receipts

- `openagents.authority_decision_receipt.v1` for each action/refusal/rollback.
- Profile validation receipt binding exact document bytes and revision.
- Independent assurance review and admission receipt with producer/reviewer
  separation.
- GCP operation and budget receipts with redacted target identity.
- Signed release and product-promise transition receipts from their existing
  systems, referenced rather than duplicated.
- Revocation/supersession receipt for every profile lifecycle change.

## Promise Links

- Full Auto release and availability promises remain controlled by their
  existing product-promise entries and #8978/#8979 proof gates.
- Transcript-derived growth or revenue promises become actionable only after
  reconciliation into the promise registry or a target-owned ProductSpec.
- Authority delegation improves delivery throughput; it is not itself a public
  product claim.
