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

## Durable graph memory boundary

OpenAgents issue #9164 adds one portable `GraphMemoryStore` contract and one
Desktop SQLite adapter. The portable service validates the exact SDK graph,
artifact inventory, ranking, archive, and delete-plan contracts. The Desktop
adapter stores one encrypted state for each owner and project.

Graph memory is off by default. The disabled adapter does not open SQLite. It
does not use Electron `safeStorage`, the file system, or cryptography. The
enabled adapter uses an OS-wrapped random data key and AES-256-GCM for each
state. SQLite contains only bounded scope references, revision data, and
ciphertext.

Each accepted graph binds these values:

- owner and project scope,
- source corpus and content digest,
- graph and manifest digest,
- graph policy,
- generation,
- consent, redaction, and policy evidence.

The service applies a source delete plan only when all current digests match.
It refuses stale or incomplete plans before mutation. A two-phase journal
makes restart recovery and repeated operations idempotent. Receipts keep
intended, applied, retained, unresolved, and failed facts separate. Full
forget accounts for the graph, vectors, summaries, rankings, and owner export
references. OpenAgents does not store an exported archive payload after it
returns the payload to the owner.

Issue #9164 does not add graph data to a prompt. Issue #9165 owns the separate
default-off extraction and recall path. Issue #9166 owns the quality and owner
lifecycle evaluation.

Docs index: https://github.com/OpenAgentsInc/ai/blob/main/docs/README.md

RLM consumption contract:
https://github.com/OpenAgentsInc/ai/blob/main/docs/rlm/OPENAGENTS-CONSUMPTION.md
