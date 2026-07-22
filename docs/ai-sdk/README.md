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
ciphertext. The adapter enables SQLite secure deletion. After each state
change, it truncates the write-ahead log before it reports success. A durable
scrub marker makes the adapter finish an interrupted cleanup before it returns
stored state.

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
references. The encrypted state keeps a bounded replay copy so that an export
retry returns the same bytes. An accepted source delete or full forget removes
that internal copy. Its receipt records the caller-held export as retained
because only the caller can delete those external bytes.

Issue #9164 did not add graph data to a prompt. Issue #9165 added the separate
default-off extraction and recall path. Issue #9165 is now complete.

The Desktop host has two independent controls. Both controls are off by
default. The extraction control uses the released DSE contract. The current
host uses a deterministic parser with zero model calls. The SDK adapter also
has a model path. That path requires a separate spend grant before a call.

The host uses only the confirmed user and assistant history for the current
thread. It removes system records. It applies the ATIF redaction boundary
before it makes immutable source corpora. It binds each corpus and graph to a
hashed owner and project scope. It does not put the current user message in
the extraction corpus.

Recall uses the released graph RLM operations with fixed limits. Each injected
fact has an original source citation. The host marks the block as untrusted
advice. A recalled fact cannot become profile truth or authority. An empty,
disabled, failed, background, delegated, or Full Auto path keeps the base
prompt unchanged. The disabled path does not open graph storage.

The owner-local evidence ledger records graph, manifest, classification,
query, operation, extraction receipt, and citation digests. It also records
limits, cap hits, truncation, exact or unavailable extraction usage, model-call
count, and spend evidence. An unranked text search has an explicit reason
instead of a ranking digest. The adapter stores extraction evidence before it
stores the graph. This order prevents a restart evidence gap. The ledger does
not record recalled text.

Issue #9166 completed the quality and owner lifecycle evaluation. The
[evaluation receipt](../sol/receipts/2026-07-22-graph-memory-evaluation.md)
binds a clean source commit, the exact SDK package integrities, the Desktop
build, the public-safe development and holdout splits, and the production
history and graph recall paths.

The comparison result is inconclusive. The history arm completed all seven
holdout rows. The graph arm had two complete rows, three partial rows, one
same-name setup failure, and one stale-graph failure. Neither arm emitted an
answer assertion. The false-merge and missed-entity metrics are unsupported
because two rows have no inspected graph state. This result does not enable
graph memory and does not authorize a quality or parity claim.

The same receipt records the real encrypted SQLite owner lifecycle. Inspect,
export, incomplete-delete refusal, forget, post-forget inspect, repeated
forget, and caller-held archive cleanup passed. The proof includes non-empty
graph, vector, summary, ranking, and archive planes before forget and zero
counts after forget.

Docs index: https://github.com/OpenAgentsInc/ai/blob/main/docs/README.md

RLM consumption contract:
https://github.com/OpenAgentsInc/ai/blob/main/docs/rlm/OPENAGENTS-CONSUMPTION.md
