# ACP Desktop ProviderLane receipt — 2026-07-16

Issue context: ACP-8 (#8895) landed the provider control UX at `df03cf2ef7`.
This follow-up closes the execution gap recorded by L6 (#8901): its Grok and
Cursor registry rows were still non-runnable placeholders.

## Implemented boundary

- The main-owned ACP host now supplies the existing shared `AcpProviderLaneDriver`.
- Both pinned peers are concrete `ProviderLane` values and use the shared lane
  dispatcher, local-turn journal, canonical event mapper, interrupt path, and
  renderer stream channels.
- Registry admission is fail-closed on the real executable probe, pinned profile
  compatibility, and authentication state. Both providers remain visibly
  `experimental`. This change does not promote either release claim.
- Full Auto has explicit Grok and Cursor policies. Their experimental runtimes
  install no question extensions or filesystem/terminal authority, so a
  background turn cannot park on an owner interaction. Unsupported reverse
  authority remains denied by the runtime.

## Real local peer proof

Host: Darwin arm64. Workspace: this repository. Output below contains no prompt,
response, path, credential, environment, or native payload.

| Provider | Pinned CLI | Driver result | Canonical observations |
| --- | --- | --- | --- |
| Grok CLI | `0.2.101` | success, 18.831 s | 103 events. Start, reasoning, tool, text, finish. Expected text observed |
| Cursor Agent CLI | `2026.6.24` | success, 8.816 s | 19 events. Start, session info, reasoning, text, finish. Expected text observed |

Both runs used `createAcpProviderHost(...).driver(provider).runTurn(...)`, the
same driver instance registered by Desktop main. Both returned a provider
session reference and shut down cleanly.

## Verification

```text
pnpm --filter @openagentsinc/openagents-desktop typecheck
PASS

pnpm vp test --run --max-concurrency 1 --root ../.. \
  apps/openagents-desktop/src/acp-provider-host.test.ts \
  apps/openagents-desktop/src/provider-lane-acp.test.ts \
  apps/openagents-desktop/src/full-auto-lane.test.ts \
  apps/openagents-desktop/src/provider-lane-registry.test.ts \
  apps/openagents-desktop/src/full-auto-reconcile.test.ts
PASS — 4 files, 14 tests

pnpm --filter @openagentsinc/openagents-desktop build
PASS — production Electron assets built
```

## Actual Desktop Full Auto receipt

The integrated production build was launched with the repository's isolated
app-proof gate, a fresh temporary user-data directory, the opt-in loopback
control server, and a disposable Git workspace. No default Desktop profile or
credential store was inspected.

- `/v1/lanes` reported `acp:grok-cli` configured, admitted, Full Auto capable,
  and experimental.
- `start --lane acp:grok-cli` created thread
  `35347e8f-1dfa-4e57-8d9d-546cdad8844f` and dispatched turn
  `turn.full-auto.22b3a6d7-8e81-48c1-b0d4-6886b4922eaf` through Desktop main.
- The durable journal reached `phase: completed`, `disposition: completed` in
  13.446 seconds. The real peer made and committed one harmless README change
  in the disposable repository (`1a41dae`).
- The control API then durably disabled the thread. Final state was
  `enabled: false`, `continuationCount: 0`, `live.state: turn_completed`. No
  second turn fired. Desktop shut down cleanly.

This closes the L6 actual-ACP-loop evidence gap. Release support language
remains owned by ACP-10 (#8897) and remains experimental.
