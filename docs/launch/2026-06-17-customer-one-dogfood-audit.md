# Customer #1 Dogfood Closeout Audit

Date: 2026-06-17
Scope: #5104, #5096, #5097, #5098, and the relation to #5107.
Audience: OpenAgents operators and future agents auditing the June 17 closeout.

## Status

#5104 is closed. The epic is satisfied because its three required slices are
closed and the final D3 production audit passes:

| Issue | Status | Result |
| --- | --- | --- |
| #5096 | Closed 2026-06-16T15:33:54Z | Internal AI/coding spend routes through the account pool and nodes. |
| #5097 | Closed 2026-06-16T15:20:10Z | Forge has the Customer #1 factory over the real development pipeline. |
| #5098 | Closed 2026-06-17T20:42:51Z | Three privacy-reviewed internal Customer #1 dogfood rows count toward D3. |
| #5104 | Closed 2026-06-17T20:43:11Z | D1, D2, and D3 are complete. |
| #5107 | Open | Related follow-on Forge Autopilot Coder productization, not a blocker for #5104. |

## What Happened

The #5104 epic was the Customer #1 dogfood lane: run OpenAgents' own work on
Forge and prove the loop with production evidence. D1 and D2 were already
closed by June 16. The remaining blocker was D3 (#5098), which required 3-5
small teams to complete the dogfood loop with public-safe evidence.

Early on June 17, D3 had the planning and projection scaffolding but did not yet
have enough counted completion rows. The public projection existed and was
honest: it exposed the target, counts, blockers, and a `blocked` gate rather
than implying success without completion-bundle and privacy-review evidence.

The confusion was about who Customer #1 was for this milestone. The work was
initially treated as though it needed external design-partner cohort evidence.
The owner clarified that Customer #1 is OpenAgents itself: the "us" dogfood
loop counts if the internal rows satisfy the same evidence, completion, privacy,
and public-projection rules. After that clarification, the required work was not
to find a different customer; it was to record enough internal dogfood rows and
prove them through the recorder, D1 storage, public projection, and audit
command.

The closeout then proceeded in three row-recording steps. #5241 recorded the
first counted internal row and moved the public audit to 1/3. #5243 recorded the
second counted internal row and moved the public audit to 2/3. #5244 recorded
the third counted internal row; the public projection gate became `ready`, the
audit passed, #5098 closed, and then #5104 closed.

## D1 And D2 Baseline

D1 and D2 were already shipped before the D3 audit work began:

- #5096 shipped in commit `cd3dfc7b4` and deployed as Worker version
  `437b00b9-2f1f-49db-8b56-1a197387916b`. It added the operator-safe spend
  routing summary for internal AI/coding work and the `/forge` spend-routing
  row. The boundary remained projection/status only: it did not grant runtime,
  spend, provider mutation, payout, settlement, accepted-work, or merge
  authority.
- #5097 shipped in commit `978eed2a3` and deployed as Worker/assets version
  `17b9b346-6071-42e2-b6cd-cccae6533f89`. It added the `Customer #1 factory`
  strip to `/forge`, deriving open work, accepted outcomes, incidents, and
  eligible node count from existing live projections.

That meant the only honest remaining #5104 closeout question was D3: whether
Customer #1 had enough completed dogfood-loop rows with public-safe evidence.

## Why #5104 Was Blocked

#5104 was blocked by D3, not by D1 or D2. D3 was blocked because the public
Customer #1 cohort projection needed at least three rows where all of the
following were true:

- `state` was `loop_completed`;
- `countsTowardD3Completion` was true;
- `completionBundleRef` existed;
- `privacyReviewRef` existed;
- the row passed the public-safe cohort boundary;
- the projection gate was `ready`;
- `node scripts/customer-one-cohort-recorder.mjs audit` passed against
  production.

Before the third row landed, the audit correctly failed with fewer than three
completed and counted rows. After #5244, that blocker disappeared.

## D3 Implementation Timeline

| Slice | Issue | Result | Closed |
| --- | --- | --- | --- |
| D3.1 | #5200 | Defined the Customer #1 cohort evidence ledger. | 2026-06-17T19:10:55Z |
| D3.2 | #5201 | Rendered the Forge cohort-readiness lane. | 2026-06-17T19:14:13Z |
| D3.3 | #5203 | Defined the cohort-row source contract. | 2026-06-17T19:16:58Z |
| D3.4 | #5204 | Added the typed public-safe cohort projection. | 2026-06-17T19:28:21Z |
| D3.5 | #5210 | Exposed `/api/public/customer-one-cohort`. | 2026-06-17T19:34:17Z |
| D3.6 | #5212 | Added operator row intake and D1 storage. | 2026-06-17T19:42:57Z |
| D3.7 | #5215 | Wired the Forge cohort lane to the public projection route. | 2026-06-17T19:54:59Z |
| D3.8 | #5218 | Added the operator cohort row recorder. | 2026-06-17T20:01:32Z |
| D3.9 | #5223 | Deployed the cohort route and smoked the public projection. | 2026-06-17T20:06:18Z |
| D3.10 | #5226 | Added row packet template and local checker. | 2026-06-17T20:14:59Z |
| D3.11 | #5230 | Added public completion audit command. | 2026-06-17T20:20:55Z |
| D3.12 | #5233 | Added the privacy-review checklist. | 2026-06-17T20:24:15Z |
| D3.13 | #5241 | Recorded the first internal Customer #1 completion row. | 2026-06-17T20:36:28Z |
| D3.14 | #5243 | Recorded the second internal Customer #1 completion row. | 2026-06-17T20:39:28Z |
| D3.15 | #5244 | Recorded the third internal row and passed the public audit. | 2026-06-17T20:42:22Z |

## Final Production Evidence

The last public projection check in this audit returned:

- `generatedAt`: `2026-06-17T20:47:57.502Z`
- `authority`: `evidence_only`
- `cohortProjectionVersion`: `customer-one-cohort-projection:v1`
- `staleness.composition`: `live_at_read`
- `staleness.maxStalenessSeconds`: `0`
- `target.minimumCompletedTeams`: `3`
- `target.maximumTargetTeams`: `5`
- `counts.loop_completed`: `3`
- `gate.state`: `ready`
- `gate.reasonRefs`: `[]`
- `blockerRefs`: `[]`
- `rows.length`: `3`

The counted rows were:

| Row | Cohort ref | Completion ref | Privacy ref | Verification ref |
| --- | --- | --- | --- | --- |
| 1 | `cohort.team.oa-dogfood-1.v1` | `completion.customer-one.oa-dogfood-1.bundle.v1` | `privacy.customer-one.oa-dogfood-1.review.v1` | `verification.customer-one.oa-dogfood-1.vitest-eslint-diffcheck.v1` |
| 2 | `cohort.team.oa-dogfood-2.v1` | `completion.customer-one.oa-dogfood-2.bundle.v1` | `privacy.customer-one.oa-dogfood-2.review.v1` | `verification.customer-one.oa-dogfood-2.vitest-eslint-typecheck-liveaudit.v1` |
| 3 | `cohort.team.oa-dogfood-3.v1` | `completion.customer-one.oa-dogfood-3.bundle.v1` | `privacy.customer-one.oa-dogfood-3.review.v1` | `verification.customer-one.oa-dogfood-3.diffcheck-review.v1` |

The production audit command output was:

```text
Customer #1 cohort audit: ready
Completed teams: 3/3
Counted completion rows: 3/3
Gate: ready
Rows: 3
```

## Authority And Privacy Boundary

The Customer #1 cohort projection is evidence-only. It does not grant runtime,
deployment, merge, accepted-work, payout, settlement, provider-account,
product-promise, or broad customer-success authority.

The source rows and projection intentionally use opaque refs and generic labels.
The public surface does not expose real team names, person names, raw prompts,
private repository content, shell logs, provider payloads, invoices, wallet
material, bearer tokens, OAuth material, local paths, raw email addresses,
private acceptance notes, or customer-private data.

Every counted completion row needs a `privacyReviewRef`. That is what lets the
row count toward D3 without turning the public projection into a private-data
leak.

## Commands Used

The closeout path used these commands from `apps/openagents.com`:

```sh
node scripts/customer-one-cohort-recorder.mjs check --row-file <row.json> --json
node scripts/customer-one-cohort-recorder.mjs upsert --row-file <row.json> --json
node scripts/customer-one-cohort-recorder.mjs public --json
node scripts/customer-one-cohort-recorder.mjs audit
```

Issue state was verified with `gh issue view` for #5096, #5097, #5098, #5104,
and #5107. The D3 child chain was verified with `gh issue list --state all` for
#5200, #5201, #5203, #5204, #5210, #5212, #5215, #5218, #5223, #5226, #5230,
#5233, #5241, #5243, and #5244.

## Relation To #5107

#5107 is related because it continues the same Forge Autopilot Coder arc:
folding the terminal-agent-systems catalog into the cockpit and turning already
built runtime capability into product surfaces. It is not a #5104 blocker.

#5104 answered the narrower Customer #1 question: can OpenAgents run its own
work through Forge and prove the loop with production evidence? Yes. #5107 is
the next productization lane for expanding what the Forge Autopilot Coder can
show, review, resume, and operate over time.

## Remaining Caveats

There is no remaining blocker for #5104. If OpenAgents wants a later external
design-partner cohort beyond the internal Customer #1 dogfood row set, that
should be tracked as a new issue or epic rather than reopening this closeout.

Future cohort row changes should still go through
`scripts/customer-one-cohort-recorder.mjs` and the source contract in
`docs/blitz/forge/2026-06-17-customer-one-cohort-source-contract.md`. Future
public product claims should still go through the product-promises system before
copy broadens beyond implementation notes.
