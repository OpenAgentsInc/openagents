# Omega Agent Computer runner in omega-effectd (AC-01)

- Date: 2026-07-24
- Packet: `OMEGA-AC-01`
- Omega issue: [OpenAgentsInc/omega#28](https://github.com/OpenAgentsInc/omega/issues/28)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `54de828f1e17d4cef5524255a0ad78e800d86d25f1cc4964ee934ebe7ac57855`
- Freeze: [2026-07-24-omega-agent-computer-contract-freeze.md](./2026-07-24-omega-agent-computer-contract-freeze.md)
- Environment: `HarnessEnvironment.openagents_cloud`
- Runner: `@openagentsinc/agent-harness-environment` `makeOpenAgentsCloudHarnessEnvironmentRunner`

## Result

Omega can start and observe an Agent Computer turn through `omega-effectd`:

- Framed methods: `start_agent_computer_session`,
  `refresh_agent_computer_session`, `run_agent_computer_turn`,
  `get_agent_computer_session`, `list_agent_computer_sessions`
- Durable public-safe sessions live under
  `{dataRoot}/agent-computer/sessions.json`
- Objective stores as SHA-256 digest only
- Bearer tokens stay runtime-only on the request and never enter disk or
  list projections
- Rust supervisor only forwards framed methods and generation fences
- No Rust call to `oa-codex-control`, GCE, or Firecracker

## Verification

- `pnpm --filter @openagentsinc/omega-effectd test` — 191 passed
- `cargo test -p omega_effectd --lib` — includes
  `ac01_agent_computer_session_survives_restart_without_bearer`

## Next

- `OMEGA-AC-02` minimal native launch surface
- `OMEGA-AC-03` live Omega Firecracker proof
