# Public Claim State Copy Boundary

Date: 2026-06-06

Roadmap: OPENAGENTS-059 / GitHub #307

Status: implemented.

## Purpose

OpenAgents public surfaces need to say what is actually true without turning a
planned feature, model estimate, operational measurement, or buyer-side payment
event into a stronger public proof claim.

`workers/api/src/public-claim-state.ts` is the shared claim-state and copy-rule
boundary for Sites, Forum, Autopilot activity, Pylon/provider projections, and
launch pages.

## Claim States

| State | Meaning |
| --- | --- |
| `planned` | Intended work or capability that is not yet evidenced. |
| `modeled` | Estimated or inferred from a bounded model. |
| `measured` | Observed by OpenAgents records, but not independently verified. |
| `verified` | Backed by an OpenAgents receipt, deployment, or approved evidence record. |
| `settled` | Backed by settlement or payment evidence. |
| `blocked` | Waiting on missing evidence, approval, or reachable authority. |
| `prohibited` | Must not be made on public surfaces. |

`verified` and `settled` are intentionally not synonyms. A Site deployment,
customer-visible revision, or receipt can verify a narrow fact. Accepted-work
settlement and contributor payout claims require settlement evidence and must
not be inferred from buyer checkout evidence.

## Copy Rules

`publicClaimCopyRuleForState(state)` returns the allowed public verb and
disallowed claim refs for each state. Public copy should use the state label and
caveats from `publicClaimStateProjection(...)` instead of hand-written stronger
phrases.

The module rejects copy that implies unsupported stronger claims, including
guarantees, live provider settlement, settled payout claims, unsupported
verified-proof language, and secret-shaped provider/auth material.

## Evidence Refs

Evidence refs are refs, not payloads. They can point to public-safe receipts,
deployments, Site URLs, proof bundles, or settlement records, but they cannot
contain:

- email addresses or customer private data;
- provider account/grant/payload/token refs;
- raw runner payloads, prompts, logs, or source archives;
- raw invoices, payment hashes, preimages, wallet state, or private keys;
- bearer tokens, OAuth material, cookies, or secret-shaped values.

Unsafe evidence refs fail closed with `PublicClaimCopyUnsafe`.

## Verification

Regression coverage lives in `workers/api/src/public-claim-state.test.ts` and
covers:

- planned claims without evidence;
- lowering verified claims when evidence is missing;
- verified and settled evidence requirements;
- blocked and prohibited terminal states;
- copy-rule contracts;
- unsafe copy and evidence-ref rejection.
