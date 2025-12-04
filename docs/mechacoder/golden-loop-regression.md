# Golden Loop regression fixture & matrix

This doc keeps the Golden Loop v2 coverage mapped to reusable fixtures and commands across the three entrypoints:

- **CLI (do-one-task)** – single task via orchestrator
- **Overnight loop** – multi-task orchestrator run
- **Desktop UI harness** – Electrobun/Playwright HUD tests

## Reusable stub repo

`src/agent/orchestrator/golden-loop-fixture.ts` exports `createGoldenLoopFixture(options)` which builds a temporary git repo with `.openagents/project.json` and a ready task.

Quick use:

```ts
import { createGoldenLoopFixture } from "../src/agent/orchestrator/golden-loop-fixture.js";

const { dir } = createGoldenLoopFixture({
  name: "matrix",
  testCommands: ["echo tests"],
  allowPush: false,
});
// Now run CLI/overnight against `dir`
```

The fixture defaults mirror Golden Loop v2: main branch, task ready state, safe test command (`echo tests`).

## Regression matrix (local or CI)

| Entry point | What to run | Coverage source | Notes |
| --- | --- | --- | --- |
| CLI do-one-task (orchestrator path) | `bun test src/agent/orchestrator/orchestrator.e2e.test.ts` | Claude Code success/fallback + typecheck failure recovery using `createGoldenLoopFixture` | Ensures single-task flow closes tasks, writes progress, and commits |
| Overnight loop | `bun test src/agent/overnight.test.ts` and `bun test src/agent/orchestrator/golden-loop-smoke.e2e.test.ts` | Log creation, start/stop lifecycle, HUD smoke | Uses stub repos; keeps overnight invariants (no dirty main, logs in `docs/logs/`) |
| Desktop HUD harness | `bun test e2e/tests/integration/golden-loop.spec.ts` and `bun test e2e/tests/realtime/realtime-updates.spec.ts` | Playwright harness renders Golden Loop sequences and HUD events | Uses injected HUD fixtures (`e2e/fixtures/hud-messages.ts`) |

Recommended minimal CI gate:

```bash
bun test src/agent/orchestrator/orchestrator.e2e.test.ts \
  src/agent/orchestrator/golden-loop-smoke.e2e.test.ts \
  src/agent/overnight.test.ts \
  e2e/tests/integration/golden-loop.spec.ts \
  e2e/tests/realtime/realtime-updates.spec.ts
```

## Acceptance criteria mapping

Golden Loop v2 acceptance rules (see `docs/mechacoder/GOLDEN-LOOP-v2.md#3-acceptance-criteria`) stay covered by:

- **Task lifecycle, verification, commits**: orchestrator e2e (`orchestrator.e2e.test.ts`, `orchestrator.verification.test.ts`)
- **HUD/UI contract**: Playwright HUD tests (`golden-loop.spec.ts`, `realtime-updates.spec.ts`) + HUD emit unit tests
- **Logs & recovery**: overnight tests (`overnight.test.ts`, log creation cases) and failure-path smoke in `golden-loop-smoke.e2e.test.ts`

When adding new Golden Loop features, extend the matrix with the nearest entrypoint and reuse `createGoldenLoopFixture` to keep fixtures consistent.
