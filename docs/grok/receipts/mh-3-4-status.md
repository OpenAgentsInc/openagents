# MH-3 / MH-4 status receipt (Grok lane)

Date: 2026-07-09  
Issues: #8589 (MH-3), #8590 (MH-4)

## Landed (this wave + prior)

### MH-3 Axis A

| Piece | Status |
| --- | --- |
| `@openagentsinc/grok-harness` mock ACP + runtime | green |
| Desktop `GrokDesktopChatRuntime` adapter | green (unit) |
| `grok_runtime` in RpcRuntimeMode / harness setting / env | wired |
| `selectRoleRuntime` branch for grok | wired |
| `submitChatMessage` / codingStatus for grok | wired |
| Model role harness `grok` | wired |
| Live ACP smoke | `GROK_ACP_LIVE=1 bun packages/grok-harness/scripts/live-acp-smoke.ts` |

### MH-4 Axis B + RL

| Piece | Status |
| --- | --- |
| Worker executor port (pylon-core-shaped) | green (unit) |
| RL-1 concurrency probes | measured ≥48 concurrent tiny `-p` |
| Free-window auto preference helper | `buildFreeWindowAutoPreference` (MH-8 input) |
| Typecheck | `packages/grok-harness` clean |

## Live RL-1 (cli_session, free grok-4.5)

maxFullSuccessConcurrency **≥ 48** (no 429s). Soft fleet derate: **24** (0.5×).

## How to select Grok harness (desktop)

```bash
export KHALA_CODE_DESKTOP_RUNTIME=grok_runtime
# or harnessSettingWrite mode: "grok_runtime"
# or model role registry: { role: "coder", harness: "grok" }
```

## Not done

- UI harness pill chrome (MH-7 / EN)
- Grok fleet dispatch (MH-4 full / MH-5 already has workerKind=grok enum; executor still throws at narrowToDelegate for real dispatch until wired)
- Agent computers (MH-9)
- RL-4 tool/worktree concurrency
