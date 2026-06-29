# Packaged Live Network Smoke

This smoke verifies the non-GEPA v0.3 Pylon network path from a fresh packaged
install. It is narrower than assignment settlement and does not claim paid work,
GEPA execution, payout approval, or bitcoin settlement.

Run it from `apps/pylon` on a real macOS or Linux machine with a registered
OpenAgents agent token:

```sh
OPENAGENTS_AGENT_TOKEN="oa_agent_..." \
bun run smoke:packaged-network
```

Optional inputs:

- `OPENAGENTS_BASE_URL`: defaults to `https://openagents.com`.
- `PYLON_PACKAGED_NETWORK_SMOKE_PYLON_REF`: override the generated public Pylon
  ref.
- `PYLON_PACKAGED_NETWORK_SMOKE_PAYOUT_TARGET_REF`: override the redacted
  payout-target ref. The value must be a public-safe
  `payout.bolt12.*` ref, not a raw offer, invoice, mnemonic, or wallet secret.

The smoke performs these steps through the freshly installed package:

1. `bun pm pack` builds the local release tarball.
2. `bun pm pack` builds the local `@openagentsinc/nip90` protocol tarball so the
   smoke uses the same `nostr-effect`-backed NIP-90 package as the workspace.
3. `bun pm pack` builds the local `@openagentsinc/tassadar-executor` tarball so
   the installed artifact carries the same exact-replay executor as the
   workspace.
4. A temporary project installs the Pylon tarball with local
   `@openagentsinc/nip90` and `@openagentsinc/tassadar-executor` overrides.
5. `bunx pylon bootstrap --json` creates isolated `PYLON_HOME` state.
6. `bunx pylon presence register` registers the Pylon against OpenAgents.
7. `bunx pylon presence heartbeat` sends a fresh heartbeat.
8. `bunx pylon wallet report-readiness` classifies the local MDK wallet and
   reports only public-safe readiness refs.
9. `bunx pylon wallet request-payout-target-admission` requests admission for a
   redacted payout-target ref.
10. The wrapper reads `GET /api/public/pylon-stats` and
   `GET /api/public/pylon-capacity-funnel` to capture projection evidence.
11. The installed package replays
    `@openagentsinc/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json` and
    requires a verified exact-replay verdict.

Exit codes:

- `0`: packaged install, registration, heartbeat, wallet readiness,
  payout-target admission, stats read, capacity-funnel read, and packaged
  executor-trace replay passed, and the local wallet classified as
  receive-ready.
- `2`: the network path ran but wallet readiness, public stats visibility, or
  capacity-funnel non-dark eligibility did not meet the live smoke criteria.
- `1`: install, credentials, route, or package execution failed.

The output is JSON and redacts bearer tokens. It must not contain raw wallet
material, mnemonics, invoices, preimages, private payment destinations, or
provider credentials.

## Evidence

Production run on 2026-06-10:

- Command: `bun run smoke:packaged-network` from `apps/pylon` with
  `OPENAGENTS_AGENT_TOKEN` sourced from the local ignored agent env file.
- Status: `passed`, with `blockerRefs: []`.
- Public stats summary: `pylonsOnlineNow: 2`, `pylonsWalletReadyNow: 2`,
  `pylonsAssignmentReadyNow: 2`, `sellablePylonsOnlineNow: 2`.
- Capacity funnel summary: `eligibleCount: 2`, `darkCount: 46`,
  `totalCount: 48`.
- Wallet readiness summary: `receiveReady: true`,
  `readiness: send-ready-blocked`.
- Registered capability refs included
  `capability.tassadar_poc.numeric_model_executor` (the 2026-06-10
  configuration-asserted default). Since W4.1 (openagents#4750) the
  executor capability is published only after `pylon provider go-online`
  runs the digest-pinned self-test and mints
  `receipt.tassadar_executor.self_test.v1.<digest16>`; an unreceipted
  claim is stripped client-side at registration and refused by the
  Worker (`refusal.public.pylon_capability.tassadar_executor_unreceipted`).
- Executor replay summary: `fixtureId: tassadar-poc-loop-sum-v1`,
  `verified: true`.
