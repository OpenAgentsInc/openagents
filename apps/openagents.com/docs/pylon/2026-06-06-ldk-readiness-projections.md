# LDK Readiness Projections

Issue #349 / `OPENAGENTS-L-001` adds the first read-only
Nexus/Treasury/LDK readiness projection contract for OpenAgents product surface.

The implementation lives in
`workers/api/src/pylon-ldk-readiness-projections.ts`.

## Purpose

The projection lets OpenAgents inspect whether a provider-facing settlement
rail appears ready without giving OpenAgents product surface, an agent, or a public page authority
to spend bitcoin, mutate Nexus or Treasury, open channels, dispatch payouts,
change payout targets, or claim settlement.

The record captures:

- provider refs;
- settlement rail refs and rail kind;
- readiness state;
- balance evidence refs;
- channel posture refs;
- failed-route and no-route counts;
- failed-route refs;
- operator action refs;
- caveats, blockers, evidence refs, and source refs.

## States

`PylonLdkSettlementReadinessState` is:

- `ready`;
- `degraded`;
- `attention_required`;
- `blocked`;
- `stale`;
- `unknown`.

Ready states require balance, channel, evidence, and source refs. Blocked
states require blocker refs. Degraded, stale, or attention-required states need
caveat or operator-action refs. No-route counts require failed-route refs.

## Authority Boundary

`PYLON_LDK_READINESS_READ_ONLY_AUTHORITY` is the only valid authority shape for
this contract. It explicitly denies:

- buyer charge mutation;
- live wallet spend;
- channel-open mutation;
- Nexus mutation;
- Treasury mutation;
- payout dispatch;
- payout target disclosure;
- payout target mutation;
- settlement mutation.

`pylonLdkReadinessCanMutateSettlement` returns false for conforming records.
This is a projection and evidence layer only.

## Redaction

Public, customer, team, and agent projections hide private provider refs,
private rail refs, private balance refs, private channel refs, private failed
route refs, and operator action refs according to audience.

All projections reject raw wallet material, recovery phrases, entropy, private
keys, raw channel monitor state, invoices, preimages, raw payment material,
payout targets, provider secrets, raw provider payloads, bearer tokens, private
customer data, raw logs, and raw timestamps.

Operator projections can show safe internal refs such as `provider.private.*`
or `rail.private.*`, but still cannot include secrets or raw wallet/payment
material.

## Tests

`workers/api/src/pylon-ldk-readiness-projections.test.ts` covers:

- fixture decoding;
- read-only authority and no-spend/no-mutation flags;
- operator projection shape;
- public redaction of private provider, rail, balance, channel, failed-route,
  and operator refs;
- evidence requirements for ready, blocked, degraded, stale, and
  attention-required states;
- route count validation; and
- rejection of raw wallet, payment, payout, channel, provider, and timestamp
  material.
