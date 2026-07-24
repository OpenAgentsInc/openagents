# Omega Full Auto routing and liveness (FA-04)

- Date: 2026-07-24
- Packet: `OMEGA-FA-04`
- Omega issue: [OpenAgentsInc/omega#23](https://github.com/OpenAgentsInc/omega/issues/23)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `d4d8ea7035c37d9d03ef6b90ce129a71118f55280d6af08a2a4cee5d3aa5d93b`
- Protocol: `openagents.omega.effectd.v1` methods `get_capacity` and `decide_attention`
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)

## Result

The supervised framed protocol now projects routing and liveness truth:

- `get_capacity` — per-lane capacity ledger, active-run cap, non-overridable
  guardrail list, and the FA-H10 enabled-thread eviction rule
- `decide_attention` — redacted title/state notification decision (never
  objective or transcript text)
- `get_run` — continues to settle stall cause and exactly one recovery action
  (`retry_now` / `stop_only` / `none`)

Non-overridable guardrails stay structural. `FullAutoGuardrailsSchema` has no
fields for `workspace_binding`, `own_capacity_only`, or
`no_rate_limit_reset_triggering`. Unknown keys are dropped at decode.

A missing host thread (vanished registry record while the run still names a
`threadRef`) settles to `stalled` with `stallCause: host_thread_missing` and
`recoveryAction: stop_only`. It does not silently stop the run.

Provider prose still cannot close a run. Only typed outcomes close a run.

Stop remains legal from every non-terminal state even after the selected
provider loses authentication or becomes unavailable. The framed Stop path does
not refresh lane readiness or host evidence before committing the local terminal
transition, so an unavailable provider cannot strand a paused run.

## Verification

- `pnpm --filter @openagentsinc/omega-effectd test` — 181 passed

## Next

- Omega supervisor and GPUI surfaces for capacity and stall recovery
- FA-05 reports, Sync, and mobile control intents
