# NEEDS_OWNER

## QA Swarm Rate Card Draft

Source issue: OpenAgentsInc/openagents#8061

Resolved for public `/business` package bands by OpenAgentsInc/openagents#8079.
The prices may be quoted only with the operator-assisted caveats and receipt
plans from the page; checkout, self-serve hosted runs, first paid delivery
receipts, and broad hosted availability remain gated.

Modeled package bands from the QA Swarm product plan:

| Package          | Draft modeled band            | Notes before publication                                                                                              |
| ---------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Swarm Audit      | $1,000-$5,000 fixed           | Needs owner-approved scope limits, target-adapter caveats, redaction policy, and deliverable template.                |
| QA-on-every-push | $2,000-$10,000/month retainer | Needs hosted-run receipts, exact accounting, runner-capacity policy, and clear operator-assisted vs. self-serve copy. |
| Swarm Sprint     | $5,000-$15,000                | Needs backlog-size bounds, delivery acceptance criteria, and paid-delivery receipt path.                              |

Remaining owner-gated work: decide when QA Swarm hosted-run receipts,
checkout/intake receipts, and outward-facing third-party report artifacts may be
treated as live delivery evidence.

## QS7 Rhys Sales Motion Owner Gate

Source issue: OpenAgentsInc/openagents#8067

This is an owner-review staging note for the outward-facing Swarm Audit demo PR
against `RhysSullivan/executor`. The external PR must not be opened, commented
on, or otherwise sent publicly until the owner signs off on the exact artifact
packet.

Current state:

- The QS7 acceptance bar depends on QS2 (run-level share URL) and QS8
  (chill-evals variant comparison). Those are prerequisite product surfaces, not
  optional polish.
- No live audit against `RhysSullivan/executor` has been run in this bounded
  checkout.
- No webm, terminal recording, distilled `*.e2e.test.ts`, CONFIRMED/REFUTED
  verdict, share URL, or chill-evals comparison exists for this target yet.
- No external repository PR has been opened.

NEEDS-OWNER: Review
`docs/fable/2026-07-02-qs7-rhys-sales-motion-owner-review.md`, approve or edit
the public-safe artifact packet, and explicitly authorize the outward-facing
`RhysSullivan/executor` PR only after the real audit receipts exist.

## QA Swarm Hosted-Run Engagement Arming

Source issue: OpenAgentsInc/openagents#8070

The QS10 contract now models hosted-run receipts, exact-only metering rows, and
engagement receipts through the existing business quick-win lifecycle. The
settlement/payment seam is intentionally inert unless the owner explicitly arms
it.

NEEDS-OWNER: Decide when QA Swarm hosted-run engagement receipts may evidence
`buyer_paid` / `provider_settled` refs, which checkout/payment source is
approved for those refs, and what operator review is required before any paid
QA Swarm engagement is represented as paid or settled.
