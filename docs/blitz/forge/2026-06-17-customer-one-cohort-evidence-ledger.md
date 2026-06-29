# Customer #1 Cohort Evidence Ledger

Date: 2026-06-17
Scope: #5200, #5098, Epic D / customer #1 dogfood (#5104).

## Purpose

D3 is not complete just because the internal Forge loop exists. It is complete
only when 3-5 small teams have moved through the same dogfood loop with
public-safe evidence that an operator can inspect without exposing private team
material.

This ledger defines the evidence shape for those teams. It is a documentation
and planning contract only; it does not onboard a team by itself.

## Cohort Target

The target cohort is 3-5 small teams. A team counts toward #5098 only after it
has a completed dogfood-loop bundle with every required proof ref listed below.

Until at least three completed bundles exist, #5098 and #5104 remain open.

## State Model

| State               | Meaning                                                  | Minimum evidence                                                                               |
| ------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `candidate`         | A team is a possible fit for the Customer #1 loop.       | `candidateRef`, vertical/ref category, fit rationale ref.                                      |
| `invited`           | The team has received a bounded invite.                  | `inviteRef`, invite channel ref, expiry/refusal policy ref.                                    |
| `workspace_seeded`  | Forge has a safe workspace for the team.                 | `workspaceRef`, template ref, public/private access mode, authority blocker refs.              |
| `first_run_started` | The team has at least one work item in the dogfood loop. | `runRef`, work-order ref, placement/routing ref, budget or no-spend policy ref.                |
| `delivery_reviewed` | A human has reviewed the output.                         | `reviewRef`, change/artifact refs, verifier refs, accepted/rejected/deferred state.            |
| `loop_completed`    | The team has completed one end-to-end loop.              | `completionBundleRef`, acceptance/closeout refs, blocker resolution refs, outcome caveat refs. |

## Completion Bundle

Each completed team bundle must carry these public-safe refs:

- `teamCohortRef`: opaque team handle, not a person or company name.
- `workspaceRef`: Forge workspace or invite workspace ref.
- `templateRef`: selected vertical/template path.
- `runRef`: primary dogfood run or work-order ref.
- `routingRef`: owned-node, fallback-lane, or blocked-routing evidence.
- `reviewRef`: human review result and reviewer authority ref.
- `artifactRef`: public-safe artifact, delivery, or deferred-output ref.
- `verificationRef`: test, smoke, manual QA, or documented blocked-verification ref.
- `completionBundleRef`: final bundle tying the refs together.
- `privacyReviewRef`: confirmation that private data was omitted or redacted.

The bundle may record blocker refs. A blocked team does not count as completed
unless the blocker is explicitly resolved or accepted as a documented deferral
that still satisfies the team's scoped dogfood objective.

## Redaction Boundary

The ledger and any roadmap, issue comment, or public-safe projection must not
include:

- real team, company, or person names unless the owner explicitly approves
  public attribution;
- raw prompts, private repo content, shell logs, stack traces, provider payloads,
  invoices, wallet material, access tokens, OAuth material, local paths, or
  customer-private data;
- private acceptance notes or commercial details beyond opaque refs and safe
  state labels.

Allowed public-safe values are opaque refs, enum-like state labels, counts,
template refs, route refs, policy refs, issue refs, caveat refs, and short
operator-safe summaries.

## Authority Boundary

This ledger is evidence only. It does not grant:

- deployment authority;
- merge or GitHub write authority;
- accepted-work authority;
- payout, settlement, or wallet spend authority;
- provider-account mutation authority;
- public product-promise promotion authority.

Those authorities remain with their existing receipt-backed paths.

## #5098 Closure Criteria

#5098 can close only after:

1. At least three `loop_completed` bundles exist.
2. Every counted bundle has the required public-safe refs.
3. Every counted bundle has a privacy review ref.
4. The active roadmap names the completed cohort count and remaining caveats.
5. #5104 is updated to show D3 complete, or to name any accepted deferral if the
   owner deliberately closes the epic before five teams.

Fewer than three completed bundles means D3 is incomplete.

## Next Slices

- Render this state model in the Forge customer #1 panel without showing private
  team material.
- Add a private/operator data source for team cohort rows.
- Record the first real team completion bundle, then repeat until at least
  three bundles exist.
