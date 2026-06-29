# MDK Payout Mode Gate

Date: 2026-06-08

## Summary

OpenAgents product surface now exposes an explicit MDK payout-mode gate before Pylon, Site, Forum,
or Artanis surfaces make payout claims.

The public modes are:

- `hosted_mdk_direct_payout`: hosted Money Dev Kit programmatic payout.
- `local_mdk_agent_wallet_bridge`: local `@moneydevkit/agent-wallet` bridge.
- `disabled`: no live payout mode is claimable.

## Current Launch State

Hosted MDK direct programmatic payout is blocked by
`blocker.mdk.hosted_programmatic_payouts_disabled` unless both conditions are
true:

- hosted programmatic payouts are enabled for the app;
- a funded hosted payout key is verified.

The current Pylon settlement evidence is therefore scoped to
`local_mdk_agent_wallet_bridge`, not hosted direct payout.

## Required Guards

The local bridge can be claimable only when the gate records:

- send-readiness preflight passed;
- original funded wallet home is in use;
- live bridge authority is recorded;
- payment-material redaction is checked.

Hosted sandbox verification is not live payout authority. It projects as
`sandbox_ready` and keeps `livePayoutClaimAllowed:false`.

## Projection Points

- Site payment manifests include `payoutModeGate`.
- MDK agent-wallet smoke fixtures include `payoutModeGate`.
- Forum tip smoke inherits the agent-wallet mode gate through
  `agentWalletSmoke`.
- Artanis public reports expose the mode gate through
  `pylonOpenAgents product surfaceReleaseGate.payoutModeGate`.
- The web Artanis dashboard renders the declared payment mode alongside the
  release gate.

## Regression Coverage

- `workers/api/src/mdk-payout-mode-gate.test.ts`
- `workers/api/src/site-payment-manifest.test.ts`
- `workers/api/src/mdk-agent-wallet-smoke-fixture.test.ts`
- `workers/api/src/pylon-v02-openagents-release-gate.test.ts`
- `workers/api/src/artanis-public-report.test.ts`
- `apps/web/src/docs-blog-route.test.ts`
