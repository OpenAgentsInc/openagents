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
2. A temporary project installs that tarball with `bun add`.
3. `bunx pylon bootstrap --json` creates isolated `PYLON_HOME` state.
4. `bunx pylon presence register` registers the Pylon against OpenAgents.
5. `bunx pylon presence heartbeat` sends a fresh heartbeat.
6. `bunx pylon wallet report-readiness` classifies the local MDK wallet and
   reports only public-safe readiness refs.
7. `bunx pylon wallet request-payout-target-admission` requests admission for a
   redacted payout-target ref.
8. The wrapper reads `GET /api/public/pylon-stats` and
   `GET /api/public/pylon-capacity-funnel` to capture projection evidence.

Exit codes:

- `0`: packaged install, registration, heartbeat, wallet readiness,
  payout-target admission, stats read, and capacity-funnel read passed, and the
  local wallet classified as receive-ready.
- `2`: the network path ran but the local wallet was not receive-ready.
- `1`: install, credentials, route, or package execution failed.

The output is JSON and redacts bearer tokens. It must not contain raw wallet
material, mnemonics, invoices, preimages, private payment destinations, or
provider credentials.
