# Checks And Gates

Product promises become reliable when claims, evidence, copy, and authority
are checked together. A passing implementation alone is not enough if the
public projection is stale, the copy is broader than the evidence, or the route
does not actually grant the authority implied by the claim.

## Gate Stack

| Gate | Blocks when |
| --- | --- |
| Evidence gate | There is no public-safe endpoint, receipt, artifact, test, screenshot, deployment, commit, or runbook proving the claim. |
| Copy gate | The text implies a green claim while the matching evidence gate is red, yellow, stale, or missing. |
| Staleness gate | Health, counters, heartbeats, run status, or receipt projections are older than the promise's freshness budget. |
| Projection gate | A public or agent-readable projection exposes secrets, raw payment material, sensitive customer data, raw provider payloads, or overbroad authority. |
| Authority gate | A discovery surface implies an action the product route cannot safely authorize. |
| Settlement gate | Copy collapses wallet setup, receive readiness, send readiness, payable state, settlement recorded, and spendable withdrawal into one claim. |
| Release gate | Package, platform, deploy, database, worker, migration, or install smoke evidence does not match the release claim. |
| Route coverage gate | Launch-critical or product-critical routes are missing from `AGENTS.md`, manifest, OpenAPI, or agent instruction sheets. |
| Regression gate | A fixed broken promise has no test, smoke, formal note, or explicit model-boundary exception. |
| Reporting gate | Users and agents have no stable Forum path or in-product Forum-backed flow to report a broken or misleading promise. |

## Red, Yellow, Green

Red blocks public affirmative copy. Red promises may appear in docs only as
blocked or not-live statements.

Yellow allows narrow copy with the limitation included. Examples include
operator-gated, planned, canary-only, no-spend, stale, or partial evidence.

Green allows affirmative copy only for the exact claim proven. Green does not
generalize to adjacent products, platforms, payment modes, providers, training
types, or settlement states.

## Required Checks By Claim Type

| Claim type | Minimum checks |
| --- | --- |
| Install or release | Package version, platform matrix, install smoke, upgrade/fallback behavior, docs link, failure mode. |
| Online or ready state | Fresh heartbeat, stale downgrade, public-safe readiness projection, blocker refs. |
| Assignment or work execution | Lease/assignment source, idempotent accept, progress, artifact/proof ref, closeout, cancellation/stale handling. |
| Payment or payout | Wallet classification, payer/recipient readiness, payment proof, settlement proof, receipt projection, redaction, cap/policy gate. |
| Forum action | Auth/admission, listed/public eligibility, moderation/report path, idempotent writes, redaction, launch status. |
| Site creation or deployment | Order/workroom linkage, generated source/artifact, deployment URL, revision state, review/acceptance state, proof refs. |
| Agent API | Route in `AGENTS.md`, manifest, and OpenAPI; auth requirements; rate limits; allowed methods; explicit denied authority. |
| Provider capacity | Account grant, secret-ref policy, route policy, metering, terms boundary, pricing, assignment, settlement receipts. |
| Training or benchmark | Dataset/task scope, hardware/runtime scope, local versus remote state, unpaid versus paid mode, score/evaluation refs, promotion gate. |
| Data or trace revenue | Consent, redaction, valuation, buyer entitlement, sale proof, payment proof, settlement proof, public-safe projection. |

## Copy Gate Rules

Copy must fail when it:

- says a red or yellow promise is live without limitation;
- uses earning, payout, settlement, training, provider-capacity, or marketplace
  language that is broader than the evidence;
- turns one successful smoke into a general availability claim;
- treats discovery as authority;
- treats receive readiness as send readiness;
- treats payment received as spendable settlement;
- hides stale or manually gated status;
- omits evidence refs for launch-critical claims.

Copy may pass when it:

- states that a claim is planned, gated, blocked, partial, or canary-only;
- names the exact green evidence and the exact scope it proves;
- separates product availability from authority and settlement;
- includes a Forum report path for users or agents who observe a mismatch.

## Verification Outputs

Every gate should output:

- `gateRef`
- `promiseId`
- `state`
- `checkedAt`
- `evidenceRefs`
- `blockerRefs`
- `safeCopy`
- `unsafeCopy`
- `nextCheck`

The launch dashboard already uses this shape. The product-wide version should
reuse it across Autopilot, Pylon, Forum, Sites, payments, provider capacity,
training, and agent-readable surfaces.
