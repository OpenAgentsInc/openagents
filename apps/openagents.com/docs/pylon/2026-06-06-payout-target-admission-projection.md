# Pylon Payout Target Admission Projection

Issue #350 / `OPENAGENTS-L-002` adds the first read-only payout target admission
projection for Pylon and LDK settlement readiness.

The implementation lives in
`workers/api/src/pylon-payout-target-admission.ts`.

## Purpose

This contract separates a Pylon heartbeat from paid-work settlement readiness.
A node can be online or recently heartbeating without having a registered,
wallet-owned, LDK-compatible payout target. The admission projection lets OpenAgents product surface
show that distinction without disclosing payout target material or granting
settlement authority.

The record captures:

- owner and provider refs;
- admission state;
- target kind;
- target ownership;
- safe target fingerprint refs;
- heartbeat refs;
- registration refs;
- target verification refs;
- rejection, revocation, and stale refs;
- caveats, blockers, evidence refs, and source refs.

The record carries only safe refs such as target hashes or registration refs,
not raw BOLT offers, BOLT invoices, BIP353 names, LNURL strings, payout
addresses, or wallet-owned secrets.

Issue #451 adds OpenAgents product surface's typed payment destination classifier in
`workers/api/src/payment-destination-input.ts`. That classifier can normalize
the class of a pasted BOLT11, BOLT12, LNURL, Lightning Address, BIP353-style
name, or `bitcoin:` URI input before admission, but it is not an admission
decision. A parsed destination still needs wallet-owned registration,
verification refs, and this read-only admission projection before Pylon can be
shown as payout-target ready.

## Admission States

`PylonPayoutTargetAdmissionState` is:

- `missing`;
- `heartbeat_hint_only`;
- `pending_registration`;
- `registered`;
- `rejected`;
- `stale`;
- `revoked`.

Registered payout target claims are allowed only when the state is
`registered`, the target kind is supported, the target is wallet-owned, a safe
target fingerprint exists, registration refs exist, and target verification
refs exist.

Heartbeat-only state does not imply paid-work eligibility.

## Target Kinds

The supported v0.2 target kinds are:

- `bolt12_offer`;
- `bolt11_invoice`;
- `bip353_name`;
- `lnurl_pay`.

`bolt12_offer` is the preferred durable target. `bolt11_invoice` is treated as
a compatibility target. `unknown`, `none`, and `unsupported` target kinds do
not allow a registered payout target claim.

## Authority Boundary

`PYLON_PAYOUT_TARGET_ADMISSION_READ_ONLY_AUTHORITY` is the only valid authority
shape. It explicitly denies:

- live wallet spend;
- payout dispatch;
- payout target disclosure;
- payout target mutation;
- provider eligibility mutation;
- settlement mutation.

This contract is admission evidence only. It does not register a target, update
Nexus, change provider eligibility, dispatch payout, or settle provider work.

## Redaction

Public, customer, team, and agent projections redact private owner, provider,
registration, target, verification, rejection, revocation, stale, source, and
heartbeat refs according to audience.

All projections reject raw wallet material, private payout identifiers, raw
BOLT offers, raw BOLT invoices, raw LNURL or BIP353 strings, payout addresses,
channel monitor state, provider secrets, bearer/API credentials, customer data,
and raw timestamps.

Payment destination classifier projections are acceptable inputs only when they
carry redacted destination refs and `payoutAuthorityCreated: false`. Do not
copy raw classifier input into Pylon heartbeats, target admission records,
Nexus events, Forum posts, issue comments, or public receipts.

Operator projections can show safe internal refs, but never raw payout target
material or wallet/payment secrets.

## Tests

`workers/api/src/pylon-payout-target-admission.test.ts` covers:

- fixture decoding;
- read-only authority;
- registered wallet-owned target claims;
- heartbeat-only non-eligibility;
- public redaction of private owner, provider, registration, and verification
  refs;
- state-specific evidence requirements; and
- rejection of raw payout target, wallet, channel, provider, credential, and
  timestamp material.
