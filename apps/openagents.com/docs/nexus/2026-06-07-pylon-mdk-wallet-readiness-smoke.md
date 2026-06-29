# Pylon MDK Wallet Readiness Smoke

Date: 2026-06-07
Related issue: #501
Repositories:

- `OpenAgentsInc/openagents`
- `OpenAgentsInc/openagents`

## Summary

#501 adds the source-controlled Pylon launcher path for MDK-backed wallet
readiness. A registered Pylon can now opt into local MoneyDevKit agent-wallet
setup, create a receive-readiness invoice locally, and submit only redacted
wallet and payout-target refs to OpenAgents product surface.

This is not a payout proof. It proves wallet and payout-target readiness intake
only. Assignment, accepted-work closeout, bitcoin payout, public receipts,
multi-host repeated jobs, and release movement remain blocked by #502 through
#505.

## Source Change

OpenAgents commit:

```text
OpenAgentsInc/openagents@6983d0512
```

The Pylon launcher now accepts:

- `--setup-mdk-wallet`;
- `--mdk-wallet-home`;
- `--mdk-wallet-port`;
- `--mdk-receive-amount-sats`.

The launcher uses `npx @moneydevkit/agent-wallet@latest` locally. It may call
`init --show`, `init`, `balance`, and `receive`, then posts:

- `POST /api/pylons/{pylonRef}/wallet-readiness`;
- `POST /api/pylons/{pylonRef}/payout-target-admission`.

The submitted values are public-safe refs:

- `wallet.public.mdk_agent_wallet.<digest>`;
- `receive.redacted.mdk_agent_wallet.<digest>`;
- `payout_target.public.mdk_agent_wallet.<digest>`;
- `balance.mdk_agent_wallet.minimum_satisfied` or
  `balance.mdk_agent_wallet.minimum_not_satisfied`;
- `admission.public.requested`;
- `policy.public.operator_review_required`.

The launcher must not submit or print an MDK mnemonic, wallet config, raw
receive invoice, payment hash, preimage, exact wallet balance, wallet home
path, or private destination material.

## Local Tests

OpenAgents launcher tests:

```bash
bun test packages/pylon-bootstrap/test/cli.test.js \
  packages/pylon-bootstrap/test/bootstrap.test.js
```

Result:

```text
46 pass
0 fail
```

OpenAgents product surface route/projection tests:

```bash
bun run --cwd workers/api test -- \
  src/pylon-api-routes.test.ts \
  src/pylon-wallet-liquidity-readiness.test.ts
```

Result:

```text
13 pass
0 fail
```

The OpenAgents product surface route coverage now includes payout-target admission statuses:

- `pending`;
- `approved`;
- `revoked`;
- `blocked`;
- `stale`.

The redaction coverage rejects raw payment and wallet material, exact wallet
balance refs, duplicate idempotency misuse, and non-owner Pylon updates.

## Production Smoke

Smoke ref:

```text
pylon.issue501.local.20260608023035
```

The smoke used a fresh temporary launcher home, Pylon home, install root, and
isolated MDK wallet home. The MDK wallet daemon used an isolated port and was
stopped after the smoke. The temporary wallet state was removed after the
public-safe readback was captured.

Public-safe launcher summary:

```json
{
  "pylonRef": "pylon.issue501.local.20260608023035",
  "walletReady": true,
  "walletRef": "wallet.public.mdk_agent_wallet.5576e12d29e80167",
  "receiveRef": "receive.redacted.mdk_agent_wallet.9f170f4267df0f25",
  "payoutTargetRef": "payout_target.public.mdk_agent_wallet.dc554a80344157af",
  "balanceReadinessRef": "balance.mdk_agent_wallet.minimum_satisfied",
  "releaseVersion": "0.2.4",
  "releaseTag": "pylon-v0.2.4",
  "installMethod": "release_asset"
}
```

Production API readback:

```json
{
  "pylonRef": "pylon.issue501.local.20260608023035",
  "walletReady": true,
  "events": [
    { "kind": "payout_target_admission", "status": "requested" },
    { "kind": "wallet_readiness", "status": "ready" },
    { "kind": "heartbeat", "status": "online" },
    { "kind": "registration", "status": "active" }
  ]
}
```

## Boundary

#501 closes only the MDK wallet readiness and payout-target admission setup
lane. Payout-target approval is still operator policy. Wallet readiness is not
payment evidence. No new Pylon release, npm `latest` movement, broad earning
claim, or autonomous paid-work claim is authorized by this smoke.
