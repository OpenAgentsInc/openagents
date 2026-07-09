# MH-3 / MH-4 status receipt (Grok lane)

Date: 2026-07-09  
Issues: #8589 (MH-3, closed), #8590 (MH-4)

## Landed

### MH-3 Axis A — closed

| Piece | Status |
| --- | --- |
| `@openagentsinc/grok-harness` mock ACP + runtime | green |
| Desktop `GrokDesktopChatRuntime` adapter | green |
| Live ACP smoke | env-armed path |
| Harness conformance `grok_cli` proven | green (`fixtures/grok.ts`) |

### MH-4 Axis B + RL matrix

| Piece | Status |
| --- | --- |
| Worker executor (pylon-core-shaped) | green (unit) |
| Readiness probe | green |
| Fleet dispatch `workerKind=grok` | green |
| `codex_spawn worker_kind=grok` | green (never silent codex sub) |
| Free-window auto preference | `buildFreeWindowAutoPreference` |
| **RL-1** concurrency | ≥48 concurrent tiny `-p`, 0×429 |
| **RL-2** metering | `not_measured` (no token headers) |
| **RL-3** multi-account | planned; skip without ≥2 logins (`planRl3MultiAccountProbe`) |
| **RL-4** worktree + tools | full success at concurrency **4** |
| **RL-5** calendar quota | not observed on free cli_session (honest negative) |
| **RL-6** free-window death | flip helper + alert (`evaluateFreeWindowDeath`) |
| Extended matrix receipt | `docs/grok/receipts/rl-extended-matrix-2026-07-09.json` |

## Measured floors (live)

| Probe | Floor | Soft derate |
| --- | --- | --- |
| RL-1 | maxFullSuccessConcurrency ≥ 48 | 24 (0.5×) for chat fan-out |
| RL-4 | maxFullSuccessConcurrency = 4 | tool-using fleet soft cap |

## How to refresh receipts

```bash
# RL-1 / RL-2
bun packages/grok-harness/scripts/rl-probe.ts

# RL-4
bun packages/grok-harness/scripts/rl4-worktree-probe.ts --concurrency 1,2,4

# RL-3/5/6 matrix (policy + measured floors; RL-3 needs GROK_RL3_ACCOUNT_IDS=a,b)
bun packages/grok-harness/scripts/rl-extended-receipt.ts
```

## Exit (#8590)

Issue exit: **executor fixture green** + **RL-1/RL-2 receipts set concurrency ceiling** — both met.

Deferred live soaks (not exit blockers): RL-3 multi-login curve when a second free identity exists; RL-5 after free ends; RL-6 live flip when free window actually dies; higher RL-4 bands (8+).

## Out of scope here

- UI harness pill chrome (EN-5 / MH-7)
- Agent computers MH-9 (after CX-3)
