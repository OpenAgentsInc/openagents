# MH-3 / MH-4 status receipt (Grok lane)

Date: 2026-07-08  
Issues: #8589 (MH-3), #8590 (MH-4)  
Branch work: `@openagentsinc/grok-harness` package + desktop re-exports

## Landed

### MH-3 (Axis A) — fixture-tier green

| Piece | Path |
| --- | --- |
| Package | `packages/grok-harness/` |
| Mock ACP fixture | `src/mock-acp-server.ts`, `src/in-process-acp-client.ts` |
| ACP client (real stdio) | `src/acp-client.ts` |
| Event projector | `src/event-projector.ts` → neutral turn events |
| Chat runtime | `src/chat-runtime.ts` (`startThread` / `startTurn` / `interruptTurn`) |
| Session store | `src/session-store.ts` |
| Desktop re-exports | `clients/khala-code-desktop/src/bun/grok-acp-chat-runtime.ts` |
| Tests | 5/5 unit pass (`bun run --cwd packages/grok-harness test`) |

Does **not** edit `agent-runtime-schema` (MH-0 owned). Provisional
literals: `grok_cli`, failure classes, `marginal_cost_class`.

### MH-4 (Axis B) — executor port + RL probes

| Piece | Path |
| --- | --- |
| Worker executor port | `src/worker-executor.ts` (pylon-core-shaped; not dumped in apps/pylon) |
| Readiness probe | `probeGrokReadiness()` |
| Headless claim runner | `createGrokHeadlessWorkerExecutor()` via `grok -p` |
| RL probe CLI | `scripts/rl-probe.ts` |
| Rate-limit receipts | `docs/grok/receipts/rl-probe-*.json` |

## Live RL-1 results (plane=`cli_session`, free Grok 4.5, host local)

Prompt: `Reply with only the single word: ok`  
Binary: `grok 0.2.91` logged into grok.com

| Concurrency | Success | Fail | Rate-limited | Wall clock |
| --- | ---: | ---: | ---: | ---: |
| 1 | 1 | 0 | 0 | ~6.4s |
| 2 | 2 | 0 | 0 | ~11.5s |
| 4 | 4 | 0 | 0 | ~12.7s |
| 8 | 8 | 0 | 0 | ~40.6s |
| 12 | 12 | 0 | 0 | ~37.3s |
| 16 | 16 | 0 | 0 | ~32.4s |
| 24 | 24 | 0 | 0 | ~48.4s |
| 32 | 32 | 0 | 0 | ~76.7s |
| **48** | **48** | **0** | **0** | ~168.8s |

**maxFullSuccessConcurrency measured ≥ 48** for tiny plain prompts on
this host/session. Hard ceiling **not yet found** (no 429s in this run).

### Interpretation for `auto` / fleet defaults

1. Free CLI plane is **far more parallel than the conservative 2–4 default**
   suggested before measurement.
2. Wall clock grows roughly with concurrency (queueing / shared machine
   contention) even when all workers succeed — treat **latency**, not just
   success count, as a fleet capacity signal.
3. These probes are **chat-only** (`-p` plain). Tool-using / worktree agents
   will be lower — run RL-4 before production fleet caps.
4. Encode default soft cap as data, e.g. `measured_full_success_floor: 48`
   with safety derate (recommend start fleet at **16–24** concurrent Grok
   workers until tool-loop RL-4 exists).
5. `marginal_cost_class: free` while plane stays cli_session and free window
   holds; flip when free ends.

## Not yet (next)

- Wire `runtimeMode: grok_runtime` into desktop selector / RPC (needs MH-0
  enums + UI pill)
- Real ACP live smoke (env-armed) beyond mock
- Worker on agent computers (MH-9 after CX-3)
- RL-4 worktree/tool loop concurrency
- pylon-core extraction coordination with PY-1 #8578

## Commands

```bash
bun run --cwd packages/grok-harness test
bun packages/grok-harness/scripts/rl-probe.ts --concurrency 1,2,4,8,16,24
```
