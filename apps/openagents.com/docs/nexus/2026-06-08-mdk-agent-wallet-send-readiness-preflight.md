# MDK Agent-Wallet Send-Readiness Preflight

Issue: `OpenAgentsInc/openagents#555`

## Contract

Any OpenAgents flow that may call `@moneydevkit/agent-wallet send` must pass
the shared MDK send-readiness helper before the send command is issued.

The helper lives in
`workers/api/src/treasury-payment-mdk-agent-wallet-adapter.ts` as
`checkMdkAgentWalletSendReadiness`. The legacy
`checkMdkAgentWalletReadiness` export remains an alias so existing callers use
the same send-readiness contract.

## Guard

Until MDK documents a restore or repair path that proves outbound capacity,
mnemonic-only restore is not send-ready evidence. A live send must use an
explicitly original funded wallet home. Unknown wallet-home mode also blocks.

The helper blocks restore/unknown mode before it runs `balance` and before any
`send` command can execute.

## Classifier

The adapter now distinguishes:

- `insufficient_balance`;
- `insufficient_outbound_capacity`;
- `mnemonic_restore_not_send_ready`;
- `send_readiness_unknown`.

`insufficient_outbound_capacity`, restore-mode blocks, and unknown-home blocks
normalize to the treasury authority rejection reason
`stale_or_absent_wallet_readiness`.

## Public-Safe Refs

The preflight returns only stable refs:

- `wallet_home.mdk_agent_wallet.original_funded_wallet_home`;
- `wallet_home.mdk_agent_wallet.mnemonic_restore`;
- `wallet_home.mdk_agent_wallet.unknown`;
- `balance.mdk_agent_wallet.minimum_satisfied`;
- `balance.mdk_agent_wallet.minimum_not_satisfied`;
- `balance.mdk_agent_wallet.not_checked`;
- `blocker.mdk_agent_wallet.mnemonic_restore_not_send_ready`;
- `blocker.mdk_agent_wallet.original_wallet_home_unverified`.

It never returns raw wallet paths, mnemonics, raw invoices, payment hashes,
preimages, exact balances, or daemon logs.

## Verification

Run:

```sh
bun run --cwd workers/api test -- src/treasury-payment-mdk-agent-wallet-adapter.test.ts src/mdk-agent-wallet-smoke-fixture.test.ts src/treasury-payment-authority.test.ts
bun run typecheck:api
```
