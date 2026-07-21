# The OpenAgents AI SDK

**Home moved.** The Effect-native OpenAgents AI SDK is developed and published
from [OpenAgentsInc/ai](https://github.com/OpenAgentsInc/ai) under Apache-2.0.
Install from npm under dist-tag `rc` (pre-stable never takes `latest`):

```sh
npm install @openagentsinc/ai@0.2.0-rc.1
# pin the full train — never `latest`, a floating `rc` tag, or a range
```

## Pinned train (OpenAgents monorepo)

Current consumer pin for OpenAgents Desktop RLM (OPENRLM-SDK #9154):

| Package | Version | Role |
| --- | --- | --- |
| `@openagentsinc/ai` | `0.2.0-rc.1` | umbrella (`./rlm` re-exports) |
| `@openagentsinc/rlm` | `0.2.0-rc.1` | first-class RLM engine (Tier D + Tier S) |
| `@openagentsinc/history-corpus` | `0.2.0-rc.1` | history adapter + `history_recall` host tool |
| `@openagentsinc/agent-harness-contract` | `0.2.0-rc.1` | L2–L5 harness / host-tool wire |
| `@openagentsinc/agent-runtime-schema` | `0.2.0-rc.1` | L1 vocabulary |
| `@openagentsinc/ai-sdk-sandbox-local` | `0.1.3-rc.1` | L3 interop (unchanged pin until next train) |

## OpenAgents-owned adapters (desktop)

These adapters stay in the monorepo. They are not copied engines.

- `apps/openagents-desktop/src/desktop-history-corpus-source.ts` —
  `DesktopHistoryCorpusSource` / `RlmCorpusSource` Layer over the durable
  event log + thread snapshots. Owns authorization and visibility policy.
- `apps/openagents-desktop/src/history-recall-host.ts` — `history_recall`
  host dispatch (HistoryRecall Tier D vocabulary) plus Rlm deterministic
  Grep path and `makeDesktopRlmToolHandler`. No artifact sink. Strategy pin
  is `openagents.desktop.rlm.history.v1`.
- `apps/openagents-desktop/src/renderer/history-recall-card.ts` — cited-span
  renderer.

Engine or public-contract fixes go to `OpenAgentsInc/ai` and return through an
exact version pin bump.

Docs index: https://github.com/OpenAgentsInc/ai/blob/main/docs/README.md

RLM consumption contract:
https://github.com/OpenAgentsInc/ai/blob/main/docs/rlm/OPENAGENTS-CONSUMPTION.md
