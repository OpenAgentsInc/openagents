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
  `experimental`; this change does not promote either release claim.
- Full Auto has explicit Grok and Cursor policies. Their experimental runtimes
  install no question extensions or filesystem/terminal authority, so a
  background turn cannot park on an owner interaction; unsupported reverse
  authority remains denied by the runtime.

## Real local peer proof

Host: Darwin arm64. Workspace: this repository. Output below contains no prompt,
response, path, credential, environment, or native payload.

| Provider | Pinned CLI | Driver result | Canonical observations |
| --- | --- | --- | --- |
| Grok CLI | `0.2.101` | success, 18.831 s | 103 events; start, reasoning, tool, text, finish; expected text observed |
| Cursor Agent CLI | `2026.6.24` | success, 8.816 s | 19 events; start, session info, reasoning, text, finish; expected text observed |

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

The remaining end-to-end L6 receipt is the bounded actual Desktop Full Auto
continuation using one of these registered lanes after integration. Release
support language remains owned by ACP-10 (#8897) and remains experimental.
