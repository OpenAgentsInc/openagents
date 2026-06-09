# Nexus Payout Policy And Emergency Pause Runbook

Status: policy evaluator implemented in OpenAgents product surface.

OpenAgents product surface now evaluates payout authority policy inside
`TreasuryPaymentAuthority` before payout intent creation or dispatch can reach
an adapter. The current implementation is code-level policy state, not a full
browser operator UI.

## Policy Inputs

The policy evaluator uses:

- payout target approval refs on the payout intent;
- accepted work refs on the payout intent;
- the amount and spend cap on the payout intent;
- the payout policy snapshot ref;
- wallet readiness;
- payout adapter kind;
- actor ref;
- payout target ref;
- Pylon/job refs; and
- emergency pause state.

The ledger from issue #421 already stores payout target approvals and payout
intents. Issue #425 adds the service-level spend policy and emergency pause
evaluation.

## Emergency Pause Scope

The service can pause:

- the global payment authority;
- an individual payout adapter;
- an individual payout target;
- an individual Pylon/job ref; and
- an individual agent or actor ref.

Use the narrowest pause that stops the unsafe behavior:

| Situation | Pause scope |
| --- | --- |
| Unknown wallet compromise or policy bug | global authority |
| MDK executor failure or daemon issue | adapter |
| Wrong destination or disputed payout target | payout target |
| Misbehaving Pylon or stale wallet reports | Pylon/job ref |
| Misbehaving agent identity | actor ref |

## Large Payout Policy

`TreasuryPaymentAuthoritySpendPolicy` supports a large-payout threshold. If an
intent exceeds that threshold, the intent must contain one of the configured
large-payout approval refs in its metadata or accepted-work refs.

The intent-level spend cap still applies separately. A large-payout approval
does not let an intent exceed its own spend cap.

## Policy Rejection Evidence

Policy rejections create `policy_rejected` payment authority receipt records
when the service has a ledger write boundary available. These receipts include
only bounded refs and a public-safe JSON decision shape:

```json
{
  "policyDecision": "rejected",
  "rawMaterialStored": false,
  "reason": "spend_cap_exceeded"
}
```

Receipts must not include wallet secrets, raw invoices, preimages, raw payment
hashes, private payout targets, raw daemon output, or customer private data.

## Operator Procedure

1. Identify the smallest affected scope: authority, adapter, payout target,
   Pylon/job, or actor.
2. Add the pause state through the approved operator path for the running
   service or test harness.
3. Confirm a new payout preview or intent creation returns the expected bounded
   rejection reason.
4. Confirm a `policy_rejected` receipt exists where a ledger write boundary is
   available.
5. Inspect public/customer projections and verify no raw payment or wallet
   material was exposed.
6. Only resume after the root cause is fixed and a fresh spend-cap/payout
   target policy check passes.

## Resume Procedure

1. Remove the specific pause entry; avoid clearing global pause if a narrower
   pause is enough.
2. Re-run a simulation payout path first.
3. Re-run mocked MDK adapter tests.
4. For live bitcoin, wait for the gated two-wallet smoke issue and use an
   explicit spend cap.
5. Document the receipt refs and policy snapshot refs used for the resumed
   dispatch.

## Verification

Current verification commands:

```bash
bun run --cwd workers/api test -- src/treasury-payment-authority.test.ts src/treasury-payment-simulation-adapter.test.ts src/treasury-payment-mdk-agent-wallet-adapter.test.ts src/nexus-treasury-payout-ledger.test.ts
bun run --cwd workers/api typecheck
```
