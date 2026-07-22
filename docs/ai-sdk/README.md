# The OpenAgents AI SDK

**Home moved.** The Effect-native OpenAgents AI SDK is developed and published
from [OpenAgentsInc/ai](https://github.com/OpenAgentsInc/ai) under Apache-2.0.
Install from npm under dist-tag `rc` (pre-stable never takes `latest`):

```sh
npm install @openagentsinc/ai@0.2.1-rc.2
# pin the full train — never `latest`, a floating `rc` tag, or a range
```

## Pinned train (OpenAgents monorepo)

Current consumer pin for OpenAgents Desktop (train `0.2.1-rc.2`, #9161/#9163):

| Package                                 | Version      | Role                                           |
| --------------------------------------- | ------------ | ---------------------------------------------- |
| `@openagentsinc/ai`                     | `0.2.1-rc.2` | umbrella and explicit graph subpaths           |
| `@openagentsinc/rlm`                    | `0.2.1-rc.2` | first-class RLM engine (Tier D + Tier S)       |
| `@openagentsinc/history-corpus`         | `0.2.1-rc.2` | history adapter and `history_recall` host tool |
| `@openagentsinc/agent-harness-contract` | `0.2.1-rc.2` | L2-L5 harness and host-tool wire               |
| `@openagentsinc/agent-runtime-schema`   | `0.2.1-rc.2` | L1 vocabulary                                  |
| `@openagentsinc/dse`                    | `0.2.1-rc.2` | typed model programs (single authority, #9163) |
| `@openagentsinc/graph-corpus`           | `0.2.1-rc.2` | derived graph corpora (OA-GMEM consumption)    |
| `@openagentsinc/conformance-kit`        | `0.2.1-rc.2` | development-only graph-memory law runners      |

The SDK release receipt records the registry integrity and external install
proof:
[AI SDK graph-memory release receipt](https://github.com/OpenAgentsInc/ai/blob/main/docs/releases/2026-07-22-ai-sdk-graph-memory-0.2.1-rc.2.md).

OpenAgents owns application composition, owner authorization, durable storage,
consent, prompt admission, evaluation, and release decisions. The AI SDK owns
the portable DSE, graph-corpus, ranking, archive, RLM, and conformance
contracts. An SDK package does not grant application authority.

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
