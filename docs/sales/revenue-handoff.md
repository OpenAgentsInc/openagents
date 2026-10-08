# Revenue implementation handoff

The owner requested completion of REV-24, REV-43, REV-54, REV-59, REV-62,
REV-63, and REV-69, followed by this handoff and a stop. Continue from current
`origin/main`. The [roadmap](revenue-roadmap.md) retains the complete inventory;
the [sales-floor contract](agent-sales-floor.md) retains operating requirements.
Use the [revenue project](https://github.com/orgs/OpenAgentsInc/projects/21)
and [workspace project](https://github.com/orgs/OpenAgentsInc/projects/19).
Claim an issue before work and honor another agent's claim.

## Completed scope

| Issue | Completed behavior |
| --- | --- |
| [REV-24 #10831](https://github.com/OpenAgentsInc/openagents/issues/10831) | Joined statements and exports preserve original prices and refuse foreign or stale scope. Commit `e7194e010b`. |
| [REV-43 #10850](https://github.com/OpenAgentsInc/openagents/issues/10850) | Joined access, recovery, limits, and audit export with pinned qualification. Commit `ef22e1eff0`. |
| [REV-54 #10865](https://github.com/OpenAgentsInc/openagents/issues/10865) | Paul uses current owner/controller custody, canonical pipeline reads, bounded research, and measured qualified drafts. |
| [REV-59 #10866](https://github.com/OpenAgentsInc/openagents/issues/10866) | Exact pending meeting slots and written human handoffs. Commit `ca7e0bc245`; integrated with Paul's controls. |
| [REV-62 #10869](https://github.com/OpenAgentsInc/openagents/issues/10869) | Exact owner approvals, durable dispatch intent, bounded SMTP, and measured draft qualification. |
| [REV-63 #10870](https://github.com/OpenAgentsInc/openagents/issues/10870) | Untrusted replies, exact owner review, original contact history, suppression, and bounded follow-ups. |
| [REV-69 #10876](https://github.com/OpenAgentsInc/openagents/issues/10876) | Stable reachable Agora stations and authorized, expiring private sales boards with original proposal hashes. |

Paul, outbox, and reply changes are in `4d4d72b5c4` through `ab9d78b637`;
`db367ef072` verifies their joined CLI controls. The Agora implementation is in
`3820f2a1bd`, with the regenerated combined world tree in `bdb92c9624`. All seven issues
are closed and published on main.

The combined run passed Sales180, CLI units14, installed checks11 across ten
binaries, command-tree regeneration1 and repeat11, Agora boards4, private Verse
transitions2, and Workshop14. The earlier route run passed world-tree8, then
repeated8. Formatting and all 51 source hashes passed. The current-main Verse
recheck passed world-tree8 and repeat8, boards4, private transitions2,
Workshop14, formatting, and all 51 current source pins.

Code qualification does not establish a real customer, production model
quality, delivery, collected revenue, or authority to contact someone.
[Owner steps](../../NEEDS_OWNER.md) record the remaining activation work.

## Next implementation order

1. [REV-64](https://github.com/OpenAgentsInc/openagents/issues/10871): confirm
   exact hires under Paul's original controller binding. Enforce the initial three
   active hires plus Paul and USD 5 daily floor bounds atomically. New hires remain unqualified;
   retirement, reassignments, stop, and suppression preserve original custody.
2. [REV-65](https://github.com/OpenAgentsInc/openagents/issues/10872): report
   private pipeline, cost, complaints, pauses, and weekly owner actions. Keep
   known expenses and unknown reservations distinct. Billing or a cash top-up
   is not earned revenue. Report failures before revenue exists.
3. [REV-66](https://github.com/OpenAgentsInc/openagents/issues/10873): exact
   reviewed batch grants require the issue's four real weeks, 100 independently
   delivered messages, 25 permissioned contacts, clean level-0 operation,
   exact owner-reviewed items/template, and review of the first five batches.
   SMTP acceptance and imported owner reports cannot supply delivery proof.
4. [REV-70](https://github.com/OpenAgentsInc/openagents/issues/10877): extend
   Bob's shared town adapter after REV-64 and REV-69. Bodies and day plans show
   actual recorded work or idle; movement and town time grant no execution or
   outbound authority and cannot reset wall-clock budgets.
5. [REV-71](https://github.com/OpenAgentsInc/openagents/issues/10878): after
   REV-65 and REV-69, ring once for attributed earned settlement and deduplicate
   reversals. Shared boards require separately consented, reviewed, delayed
   aggregates. Private leads, live payment amounts, and deal timing stay private.
6. [REV-72](https://github.com/OpenAgentsInc/openagents/issues/10879): integrate
   Arthur and Vanna after REV-59, REV-62, and REV-64. Narrow research,
   introduction, and referral permissions; reuse accepted partner custody.
   Commission eligibility needs the existing agreement and settlement contracts.
   Research can precede commissions; no agent invents a fee or self-earning.
7. [REV-73](https://github.com/OpenAgentsInc/openagents/issues/10880): after
   measured REV-66 qualification, add disabled-by-default standing follow-ups
   only for explicitly invited existing threads. Exact policy, template,
   cohort, expiry, spacing, caps, and current contact/certification/draft checks
   remain mandatory. No automatic promotion or new messaging platform.

These issues were not started in this completion pass. Read each current issue
and spec before implementation; this sequence does not replace native blockers.

## Scope choices still needed

| Issue | Required concrete choice |
| --- | --- |
| [REV-44](https://github.com/OpenAgentsInc/openagents/issues/10851) | One requested additional client surface carrying a proven commercial flow. |
| [REV-47](https://github.com/OpenAgentsInc/openagents/issues/10854) | A demonstrated paying buyer and task, with exact effect, source, recipient, resource bounds, independent result, fee, and disclosure. |
| [REV-50](https://github.com/OpenAgentsInc/openagents/issues/10857) | A paying customer's SSO provider/protocol, issuer, tenant, audience, account-linking, recovery, and audit requirements. |
| [REV-74](https://github.com/OpenAgentsInc/openagents/issues/10881) | One officially permitted public-reply platform, account, thread, rules, and cohort. |
| [REV-75](https://github.com/OpenAgentsInc/openagents/issues/10882) | A consented booked meeting medium with supervised human start, mute, takeover, end, and recording consent. |
| [REV-76](https://github.com/OpenAgentsInc/openagents/issues/10883) | One new jurisdiction, recipient category, channel, cohort, and current primary regulatory review. |

Do not invent these choices. General permission to ship code or native mobile
exceptions does not select a client, provider, customer, or campaign.

## Existing boundaries to preserve

- Canonical sales state lives in Rust under `coder::task::sales::Store`.
  Authenticate an explicit private credential; never infer sales authority from
  Studio observation, a paired device, proximity, or a caller-provided identity.
- REV-57 qualification pins original calibration, development, and locked
  partitions, frozen maps, practice, exact grades, owner marks, and costs.
  A native identity or an imported model answer does not qualify an employee.
  Weekly suspension survives certificate, key, and assignment changes.
- `qualified_sales_draft` covers the actual body and original content expense.
  Live agent mail currently uses the fixed neutral subject and no attachments.
  Do not add ungraded headers or attachments to a qualified body.
- The default model engine remains unavailable until an admitted Rust host
  enforces input, output, retry, deadline, price, and model identity bounds
  before effects. A generic SDK call cannot prove those bounds.
- Dispatch persists its original intent before network effects. Accepted,
  delivered, failed, and unknown are distinct; uncertain DATA outcomes do not
  permit blind retries. Owner reports and fixtures do not prove delivery.
- Replies are untrusted. Opt-out, bounce, injection, original thread references,
  exact owner review, retention, and bounded follow-ups remain code-controlled.
- Meeting proposals are pending written suggestions. They create no booking,
  message, human acceptance, call, or payment. Phones and headsets remain props.
- Agora private observations expire and clear on failed reads, inactive
  surfaces, and shared-world transitions. Render closed labels, bounded counts,
  and original proposal hashes; never render raw prospect or payment records.

## Verification and activation

Follow `AGENTS.md`: targeted tests for edited crates and formatting, through a
build lease with an external long-lived target directory. Regenerate stale
checked-in world-tree data with its documented command. No ordinary issue needs
a full release gate. Use scratch roots and file keys, not the owner's hosts,
keychain, mailbox, computers, or funds. Required checks run outside GitHub
workflows. Close code-complete issues after main and required deploy; retain
owner-only qualification in `NEEDS_OWNER.md`.

Local evidence is retained under
`~/.openagents/scratch/codex-01a1148f-f33b-7ba2-bee6-9d464d5a2a86/revenue-implementation/`:

- `revenue-final-check.log`, `revenue-final-receipt.json`, and
  `revenue-final-inputs.json`: the complete joined run and source pins.
- `revenue-final-openagents` and `revenue-final-binary.sha256`: copied installed
  CLI, SHA-256 `0497b1796ab1e519b5a3a7ea914deeb958cc2833f146c2ed0146b60ffdcb5636`.
- `rev69-portable-v5.log`: original passing route and portable board checks.
- `revenue-rebase-check.log` and `revenue-rebase-receipt.json`: scoped Verse
  verification after the concurrent Pylon/Meteor changes on main.
- `rev62-ready-proof.json`, `rev63-ready-proof.json`, and
  `rev62-63-handoff.json`: original outbox/reply qualification and boundaries.

REV-24's final joined HTTP export case passed and its 30 pinned sources were
unchanged after integration. REV-43 passed 45 scoped checks plus two checks
against the copied current CLI; `docs/sales/team-qualification.json` retains
its qualified source scope. Their final issue comments retain the results.

The unrelated broad CLI customer fixture still has a missing `team_reports`
context field; it is not evidence against the passing scoped joined-statement
case. Ignored browser/device cases were not qualified. No owner mailbox,
customer campaign, real payment, keychain, or live model run was exercised.
No host deployment is required for these dormant local controls; install or
release them separately when an actual host or device flow is selected.

The previous agent stopped after these seven issues and this handoff. The
remaining epic is incomplete; none of its 13 remaining issues was claimed or
implemented in this completion pass.
