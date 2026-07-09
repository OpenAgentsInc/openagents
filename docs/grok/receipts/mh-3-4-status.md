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
| RL-4 worktree + always-approve tool probes | measured full success at concurrency **4** (see `rl4-worktree-*.json`) |
| Free-window auto preference helper | `buildFreeWindowAutoPreference` (MH-8 input) |
| FleetRun supervisor: `grok` concrete kind | green — `resolveSupervisorWorkerKind("grok")` available |
| Fleet RPC capacity + dispatch | green — local `grok-local` account + headless executor |
| `codex_spawn worker_kind=grok` | green — never silent-substitutes codex |
| Store `runner_kind` CHECK | includes `grok_cli` (schema v3) |
| Mixed codex/claude/grok MH-5 pool | green under one claim registry |
| Typecheck | `packages/grok-harness` clean |

## Live RL-1 (cli_session, free grok-4.5)

maxFullSuccessConcurrency **≥ 48** (no 429s). Soft fleet derate: **24** (0.5×).

## Live RL-4 (cli_session, isolated cwd + always-approve)

```bash
bun packages/grok-harness/scripts/rl4-worktree-probe.ts --concurrency 1,2,4
```

maxFullSuccessConcurrency **4** at bands 1/2/4 (no rate limits). Use as floor for tool-using fleet soft caps; raise with higher bands when needed.

## How to select Grok harness (desktop)

```bash
export KHALA_CODE_DESKTOP_RUNTIME=grok_runtime
# or harnessSettingWrite mode: "grok_runtime"
# or model role registry: { role: "coder", harness: "grok" }
```

## How to start a Grok FleetRun

```bash
# Via fleet tools / RPC with workerKind: "grok" — capacity is local CLI readiness.
# codex_spawn with worker_kind=grok uses the headless executor (not codex).
```

## Not done

- UI harness pill chrome (MH-7 / EN)
- Agent computers (MH-9)
- Full auto MIXED capacity advertising for grok on paid/API plane
- Higher RL-4 bands (8+) soak under free window
