# Omega Agent Computer live proof attempt (AC-03)

- Date: 2026-07-24
- Packet: `OMEGA-AC-03`
- Omega issue: [OpenAgentsInc/omega#30](https://github.com/OpenAgentsInc/omega/issues/30)
- Freeze: [2026-07-24-omega-agent-computer-contract-freeze.md](./2026-07-24-omega-agent-computer-contract-freeze.md)
- Runner: [2026-07-24-omega-agent-computer-runner.md](./2026-07-24-omega-agent-computer-runner.md)
- Surface: [2026-07-24-omega-agent-computer-launch-surface.md](./2026-07-24-omega-agent-computer-launch-surface.md)
- Machine evidence: [2026-07-24-omega-ac03-live-proof.json](./2026-07-24-omega-ac03-live-proof.json)

## Result

This packet records an honest live-proof attempt from the Omega path.
It does **not** close `OMEGA-AC-03`.
It does **not** reopen closed openagents `#9190` or `#9191`.

## Gate matrix

| Gate | Status | Evidence |
| --- | --- | --- |
| Call through `omega-effectd` / `openagents_cloud` | green (code) | AC-01 runner + AC-02 panel |
| Operator-reachable surface | green (code) | Agent menu → Agent Computer |
| Live `POST /v1/cloud-coding-sessions` | blocked | `401 unauthorized` with current agent token class |
| Live Firecracker microVM | blocked | No admitted session |
| Staged change + verifier + writeback | blocked | Depends on live session |
| Exact usage + teardown receipts | blocked | Depends on live session |
| Same capacity authority as Sarah `#9191` | design green | Shared Worker route. No Omega placement plane. |

## Attempt evidence

- Driver: `packages/omega-effectd/scripts/ac03-live-proof.ts`
- Control plane: `https://openagents.com`
- Repo: `OpenAgentsInc/openagents`
- Adapter: `claude_agent`
- Engine path: `runAgentComputerTurn` → `HarnessTurnError.failureClass=unauthorized`
- Direct HTTP probe: `POST /v1/cloud-coding-sessions` → `401` `{ "error": "unauthorized" }`
- Machine receipt: `ok=false`, `error.code=turn_failed`, `error.message=unauthorized`
- Durable disk never retained bearer or objective text
- Tip at attempt: openagents `21f30837d5` (pre-land)
- Post-land pack SHA-256: `32aeccbeb21d16ef46b73e52880c4c36f538714d2bac6e188a05a9a36a1f46af`

## Owner blockers (smallest irreducible)

1. Provide a runtime bearer that the cloud coding-session admission gate accepts
   (mobile/user credit-bearing session — not a bare `oa_a*` agent registry token).
2. Confirm Pool B credit and live Agent Computer capacity remain available.
3. Re-run the AC-03 driver or the Agent Computer panel Start action and capture
   public-safe session/placement/artifact/usage/teardown refs.

## Next

- Keep `#30` open until a live Firecracker turn from Omega is proven
- Do not treat fixture/mock runner green as AC-03 close
