# Public Claim Projection Records

Date: 2026-06-06

Roadmap: OPENAGENTS-060 / GitHub #308

Status: implemented as a typed projection contract, ready for later D1
persistence.

## Purpose

Public pages should not read private workroom state, raw runner state, provider
state, or wallet state to decide what they can claim. They should read a
purpose-built public claim projection record that already carries state,
evidence refs, caveat refs, source refs, and audience-specific redaction.

`workers/api/src/public-claim-projections.ts` adds that boundary.

## Record Shape

`PublicClaimProjectionRecord` includes:

- `claimId` and `claimRef`;
- `claimKind`;
- `surface`, such as `site`, `forum`, `autopilot`, `pylon`, `provider`,
  `public_agent`, `launch`, `order`, or `workroom`;
- `subjectRef`;
- `desiredState`, interpreted through the public claim-state contract;
- `titleRef`;
- `caveatRefs`;
- `evidenceRefs`;
- `sourceRefs`;
- `customerRefs`;
- `teamRefs`;
- `operatorRefs`;
- `updatedAt`.

This is not a public payload by itself. It is the storage-ready source record
for audience projections.

## Audience Projection

`projectPublicClaimRecord(record, audience)` emits:

| Audience | Visible refs |
| --- | --- |
| `public` | claim, subject, source, evidence, caveat, state, and copy-rule refs |
| `customer` | public refs plus customer refs |
| `team` | public refs plus customer and team refs |
| `operator` | public refs plus customer, team, and operator refs |

All audiences receive the same claim-state projection and copy-rule object.
If evidence is missing, stronger desired states are lowered by the claim-state
contract. `blocked` and `prohibited` remain terminal.

## Redaction

Projection records fail closed when any ref contains private or secret-shaped
material, including:

- raw runner payloads, prompts, run logs, or source archives;
- provider account/grant/payload/token refs;
- raw invoices, payment hashes, preimages, wallet state, private keys, or
  mnemonics;
- customer private data or email addresses;
- bearer tokens, OAuth material, cookies, checkout query state, or secrets.

This means public claim projection is a narrow ref-and-state boundary. It is
not an evidence store, log store, provider diagnostics store, or wallet
projection.

## Verification

Regression coverage lives in `workers/api/src/public-claim-projections.test.ts`
and covers:

- public/customer/team/operator projection splits;
- claim-state lowering and copy-rule integration;
- blocked/prohibited terminal states;
- rejection of private workroom, provider, wallet, raw payment, and customer
  refs.
