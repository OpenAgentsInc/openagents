# Cognee Teardown — 2026-07-21

Read-only architecture and product audit of the public `topoteretes/cognee`
source tree at an exact commit in the local reference clone
`~/work/projects/repos/cognee`. Nothing tracked was modified and nothing was
executed. Cognee is an open-source Python memory engine for agents: it ingests
data, extracts a knowledge graph with an LLM, persists the graph and its
embeddings side by side, and answers recall queries from the combined graph
and vector planes. This document supersedes the Blueprint framing of the
prior workspace audit
(`~/work/projects/2026-05-21-cognee-memory-blueprint-audit.md`). Blueprint is
archived. The current comparison targets are the extracted Effect-native AI
SDK (`OpenAgentsInc/ai`, consumed from npm) and the monorepo's
`packages/agent-experience-memory`.

## Summary

Cognee turns raw text, files, and structured data into a permanent knowledge
graph plus a fast session cache, then serves recall through a large retriever
stack that mixes graph traversal, vector search, and LLM completion. The
public memory API is four verbs — `remember`, `recall`, `improve`, `forget` —
with a session layer of typed Q&A, trace, feedback, and skill-run entries
under them. [source]

Since the 2026-05-21 workspace audit the project moved fast: 1,320 commits,
a version jump from 1.1.0 to 1.4.0, a default embedded graph database swap
to the Ladybug fork of Kuzu, Turso (libSQL) graph and vector backends, a
unified graph-plus-vector store engine with capability flags, typed COGX
export archives with importers for Mem0, Zep/Graphiti, and Letta, a
deterministic code-graph extraction task, out-of-process database workers,
and a commercial Cognee Cloud connection flow built into the SDK. [history]

The central OpenAgents decision: **do not run cognee inside OpenAgents, and
do not build a memory-provider seam around it yet. Port its best ideas into
the AI SDK: a typed graph-memory layer derived from `KhalaRuntimeEvent`
corpora, entity extraction as a typed Program with an Effect Schema
signature, deterministic datapoint identity, source-labeled recall results,
feedback-weighted ranking that never mutates truth, and a provenance-aware
delete planner.** The Python runtime rule, the dependency weight, the
LLM-in-the-loop background mutation model, and the dataset-ACL tenancy
posture all disqualify direct adoption, while the memory model itself is the
strongest one in the teardown catalog so far.

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Public repository | `https://github.com/topoteretes/cognee` | Public source and history |
| Local clone | `~/work/projects/repos/cognee` | The audited tree |
| Audited commit | `90b4acaac937dc1c0aeffaead8b707c896ebf3db` | Exact snapshot used here |
| Commit time | `2026-07-21T21:30:47+02:00` | Freshness of the audited tip |
| Commit subject | `COG-5929 chore(mcp): bump cognee-mcp to 0.5.5` | Latest audited change |
| Product version | `1.4.0` (`pyproject.toml`) | Post-1.0 maturity claim |
| License | Apache-2.0, with `NOTICE.md` and a `licenses/` directory | Permissive reuse boundary |
| Language | Python (`requires-python >=3.10,<3.15`), plus a Next.js 16 frontend | Runtime commitments |
| Source scale | 1,893 Python files repo-wide, about 243,000 Python lines under `cognee/` | Large implementation |
| MCP server | `cognee-mcp` 0.5.5, about 3,800 Python lines | Separate agent-facing package |
| Contributors | About 250 distinct author names in history (with name duplicates) | Broad, company-backed team |
| Activity | 539 commits in the 30 days before the audited tip | Very high velocity |
| Delta since prior audit | 1,320 commits since `0187fd8a4` (2026-05-20), 1,442 files changed | Fast-moving upstream |

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, manifests, or config at the commit.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — corroborated by a linked public source.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

This audit did not execute cognee, its test suite, its server, or its MCP
package. All behavior claims come from reading tracked source. Retrieval
quality, latency, and cost claims in upstream docs are not verified here.
[limitation]

## 2. What cognee is

Cognee models agent memory as two planes with a bridge between them. [source]

- **The permanent plane** is a knowledge graph plus embeddings. `add`
  ingests raw data into datasets, `cognify` runs an LLM task chain that
  classifies documents, chunks them, extracts entities and relations,
  summarizes, and persists nodes, edges, and vectors
  (`cognee/api/v1/add/add.py`, `cognee/api/v1/cognify/cognify.py`).
- **The session plane** is a fast cache of typed entries — Q&A turns, agent
  trace steps, feedback, and skill-run scores — validated as a Pydantic
  discriminated union in `cognee/memory/entries.py` and stored through a
  `SessionManager`.
- **The bridge** is `improve` (`cognee/api/v1/improve/improve.py`): it
  applies feedback weights to graph elements used by prior answers, persists
  session Q&A and trace feedback into the permanent graph, runs memify
  enrichment, and can build a global context index or a truth subspace.

The v1 API keeps the full pipeline vocabulary (`add`, `cognify`, `search`,
`memify`, `update`, `delete`, `prune`). The v2 memory API layers the four
memory verbs over it and adds `serve`, `disconnect`, `push`, `export`, and
visualization entry points (`cognee/__init__.py`). [source]

Recall serves 19 search modes from one enum
(`cognee/modules/search/types/SearchType.py`): chunk and summary retrieval,
RAG completion, triplet and graph completion with chain-of-thought and
context-extension variants, Cypher and natural-language graph queries,
temporal queries, lexical chunk search, coding-rule recall, agentic
completion, a `CODE` mode over the code graph, a `HYBRID_COMPLETION` mode,
and a `FEELING_LUCKY` auto-router. The v2 `recall` path adds source scopes
so a query can target session memory, trace memory, the graph, or all of
them, and it returns source-labeled result types (`ResponseQAEntry`,
`ResponseAgentTraceEntry`, `ResponseGraphEntry`,
`ResponseSessionContextEntry` in `cognee/api/v1/recall/recall.py`). [source]

## 3. The memory model

This is the strongest part of the codebase. Everything persisted derives from
one Pydantic base class, `DataPoint`
(`cognee/infrastructure/engine/models/DataPoint.py`). [source]

- **Deterministic identity.** A `DataPoint` id defaults to a random UUID,
  and the docstring states the consequence plainly: a random id never
  deduplicates across runs. A model that should merge idempotently declares
  `identity_fields` in its metadata, and the id becomes a UUID5 derived from
  those field values, namespaced by class name. `Entity` declares
  `identity_fields: ["name"]`, so the same entity name always produces the
  same node id (`cognee/modules/engine/models/Entity.py`).
- **Embeddability as metadata.** `index_fields` in the same metadata block
  declares which fields are embedded, so the vector plane is derived from
  the graph model rather than maintained beside it.
- **Provenance on every node.** `source_pipeline`, `source_task`,
  `source_node_set`, `source_user`, and `source_content_hash` record where a
  datapoint came from. `version`, `created_at`, and `updated_at` support
  updates.
- **Ontology grounding.** `ontology_uri` preserves a stable RDF IRI when a
  node matches an external ontology, so the graph can be exported as RDF and
  linked into other domains.
- **Ranking state, separated from truth.** `feedback_weight` and
  `importance_weight` live on the datapoint, and the newer truth-alignment
  fields on `Entity` (`truth_alignment`, `truth_subspace_signature`,
  `truth_epoch`) are explicitly excluded from both `index_fields` and
  `identity_fields` — ranking signals do not change identity or embedding.

The session entries are equally typed: `QAEntry` carries
`used_graph_element_ids` so later feedback can adjust the exact graph
elements an answer used, and `TraceEntry` records origin function, status,
parameters, return value, memory query, and error message
(`cognee/memory/entries.py`). [source]

## 4. Architecture walk — stores, servers, and scale-out

### 4.1 Store abstractions

Cognee separates a `GraphDBInterface` and a `VectorDBInterface`, each with a
config-selected adapter set. [source]

- Graph adapters in-tree: `ladybug` (default), `kuzu`, `neo4j_driver`,
  `neptune_driver`, `postgres`, and `turso`
  (`cognee/infrastructure/databases/graph/`). A root-level `kuzu/` package
  is now a compatibility shim that re-exports the `ladybug` module — the
  embedded default is the Ladybug fork of the Kuzu database, pinned as the
  pip dependency `ladybug` (`kuzu/database.py`, `pyproject.toml`).
- Vector adapters in-tree: `lancedb` (default), `pgvector`, and `turso`
  (`cognee/infrastructure/databases/vector/`).
- Both planes expose an empty `supported_databases` registry plus
  `use_graph_adapter` / `use_vector_adapter` hooks, so community adapters
  register from outside the package. [source]
- A **unified store engine** abstracts over backends that can serve both
  planes at once. `EngineCapability` flags declare `GRAPH`, `VECTOR`,
  `HYBRID_WRITE` (atomic graph-plus-vector writes), and `HYBRID_SEARCH`
  (combined queries in one backend), and a `provenance_delete_planner`
  plans deletion across the derived planes
  (`cognee/infrastructure/databases/unified/`). Hybrid backends exist for
  Postgres and Neptune Analytics
  (`cognee/infrastructure/databases/hybrid/`). [source]
- Relational metadata (users, datasets, sessions, permissions) lives in
  SQLAlchemy with Alembic migrations, SQLite by default and Postgres as the
  scale option. A checked-in plan proposes moving the session cache to
  Postgres as well (`SESSION_POSTGRES_CACHE_PLAN.md`). [source]

### 4.2 Serving surfaces

One engine, many front doors. [source]

- **Python SDK** — the `cognee` package itself, with the v1 pipeline API and
  the v2 memory API.
- **FastAPI server** — `fastapi`, `fastapi-users`, `uvicorn`, and `gunicorn`
  are mandatory dependencies, and `cognee/api/` carries route modules for
  datasets, search, sessions, permissions, settings, sync, and more.
- **MCP server** — `cognee-mcp` (version 0.5.5) exposes `cognify`,
  `save_interaction`, `search`, `get_document`, `get_chunk_neighbors`,
  `list_data`, `delete_dataset`, `delete`, `prune`, `remember`, `recall`,
  `forget`, `improve`, `cognify_status`, and graph-visualization UI tools
  over stdio, SSE, or streamable HTTP (`cognee-mcp/src/server.py`). It can
  also proxy to a remote instance through a cloud client
  (`cognee-mcp/src/cognee_client.py`).
- **CLI** — discovered commands include `add`, `search`, `cognify`,
  `delete`, `config`, `datasets`, and `agents`
  (`cognee/cli/_cognee.py`).
- **Frontend** — a Next.js 16 / React 19 workspace app
  (`cognee-frontend/package.json`), plus a local visualization server in the
  SDK (`start_ui`, `visualize_graph`).
- **Cognee Cloud** — `cognee.serve()` connects the local SDK either directly
  to a URL or to the commercial cloud through an Auth0 device-code flow and
  a management API (`cognee/api/v1/serve/serve.py`). `push` exports a local
  dataset as a COGX archive and uploads it to the cloud
  (`cognee/api/v1/push/push.py`). [source]

### 4.3 Portability and migration

New since the prior audit, and notable. `cognee.export` produces a typed
`GraphSnapshot` of real `DataPoint` objects, GraphML, or a versioned COGX
archive. `cognee.remember` accepts migration sources that import memory from
other systems: `Mem0Source`, `ZepSource`, `GraphitiSource`, `LettaSource`,
and `COGXArchiveSource` for cognee-to-cognee restore
(`cognee/migration/__init__.py`). Memory portability is a first-class
competitive feature, aimed directly at the neighboring memory engines.
[source] [inferred]

### 4.4 Scale-out and process isolation

- `cognee_db_workers/` runs Kuzu/Ladybug and LanceDB in separate worker
  processes behind small protocols (`kuzu_worker.py`, `lancedb_worker.py`),
  isolating embedded-database crashes and concurrency from the host process.
  [source]
- `distributed/` packages the pipeline for Modal with queued graph write
  batches (`distributed/app.py`, the optional `modal` extra). [source]
- A deterministic code-graph extraction task landed via the `enola` engine
  (commit `597bc1c25`), feeding the `CODE` search type — a notable move away
  from LLM-only extraction for source code. [history]

## 5. What changed since the 2026-05-21 audit

The prior audit reviewed commit `0187fd8a4` (version 1.1.0) and framed its
conclusions for Blueprint, which is now archived. The upstream deltas that
matter, from 1,320 commits: [history]

1. **Default graph store swap.** Kuzu became Ladybug (a maintained fork —
   the root `kuzu/` package is now a shim), with config migration logic that
   keeps existing local Kuzu paths working
   (`cognee/infrastructure/databases/graph/config.py`).
2. **Turso (libSQL) backends** for both graph and vector planes, giving one
   embedded file both roles.
3. **The unified store engine** with capability flags and hybrid
   write/search, plus the provenance delete planner.
4. **COGX export archives and competitor importers** (Mem0, Zep/Graphiti,
   Letta) — the memory-portability story did not exist at the prior audit.
5. **Cognee Cloud in the SDK.** Auth0 device flow, management API, `push`,
   and an MCP cloud client. The open-source SDK is now also the client of a
   commercial hosted product.
6. **Deterministic code-graph extraction** (enola-backed) and the `CODE`
   search type.
7. **Out-of-process database workers** and the Modal distributed lane.
8. **Truth subspace.** `improve(build_truth_subspace=True)` builds centroid
   structures for truth alignment, with alignment fields on `Entity` that
   stay out of identity and embedding. The mechanism is young and test
   coverage is broader than documentation. [source] [limitation]
9. **New search modes** — `HYBRID_COMPLETION`, `FEELING_LUCKY`, `CODE` —
   and observability spans across the memory API
   (`cognee/modules/observability/`).

The prior audit's core judgment — absorb the patterns, do not adopt the
service as an authoritative memory layer — survives the upstream churn and
survives the Blueprint retirement. The destination for the absorbed patterns
changes: it is now the AI SDK and `packages/agent-experience-memory`, not
Blueprint contracts. [inferred]

## 6. What is genuinely good

1. **Unified graph-plus-vector retrieval over one model.** The same
   `DataPoint` declares its embeddable fields and its graph shape, so the
   vector index is a projection of the graph model instead of a parallel
   store that drifts. The unified engine's capability flags
   (`HYBRID_WRITE`, `HYBRID_SEARCH`) make the one-backend-or-two decision
   explicit and testable. [source]
2. **Deterministic identity as an opt-in contract.** The `identity_fields`
   mechanism turns entity deduplication from a fuzzy post-process into an id
   derivation, and the docstring names the failure mode of the default
   random id honestly. This is exactly the shape of idea that ports cleanly
   to an Effect Schema world. [source]
3. **Typed session entries with used-element tracking.**
   `QAEntry.used_graph_element_ids` closes the loop between an answer and
   the graph elements that produced it, which makes feedback precise instead
   of global. [source]
4. **Ranking state separated from truth state.** Feedback weights and truth
   alignment fields are excluded from identity and embedding. The streaming
   feedback update nudges ranking weights without rewriting content.
   [source]
5. **Source-labeled recall.** The v2 recall response distinguishes session,
   trace, graph, and session-context results as separate typed entries
   rather than one flat string list. [source]
6. **Provenance-aware deletion.** A delete planner that maps a source
   deletion onto the derived graph, vector, and summary artifacts is the
   right shape for a real forget lifecycle. [source]
7. **Portability as a product feature.** Typed export snapshots, a
   versioned archive format, and importers for rival systems lower switching
   costs in both directions — a confident posture. [source] [inferred]
8. **Deterministic extraction where determinism is possible.** The
   enola-backed code graph shows the team routing around the LLM for a
   domain with an exact parser available. [history]

## 7. Honest weaknesses and frictions for OpenAgents

1. **Python runtime, wall to wall.** Cognee is a large Python system —
   about 243,000 lines — with a FastAPI server, Alembic migrations, and
   embedded native databases. The owner override of 2026-07-21 rejected
   Python in engine paths (the RLM Python leaf decision in
   `docs/rlm/2026-07-21-rlm-integration-audit-and-roadmap.md`), and the AI
   SDK's standing boundary is no Python and no arbitrary code execution in
   any engine path (`~/work/ai/docs/ROADMAP.md`). Adopting cognee in an
   engine path contradicts a fresh, explicit owner decision. [source]
2. **Dependency weight.** The mandatory dependency set is about 50 packages,
   including `openai`, `litellm`, `instructor`, `fastapi-users`, `lancedb`
   plus `pylance`, `ladybug`, `rdflib`, `networkx`, `fakeredis`, and
   `datamodel-code-generator` — before the optional extras for scraping,
   fastembed, Modal, and provider adapters (`pyproject.toml`). The lockfile
   discipline is real (CVE-driven pins with comments), but the surface is
   enormous. [source]
3. **LLM-in-the-loop cost and mutation.** `cognify` and `improve` spend
   provider tokens to build and enrich the graph, and `improve` mutates the
   permanent plane from session data, optionally in the background
   (`run_in_background=True`). OpenAgents treats exact token accounting and
   evidence-gated promotion as invariants — a background job that silently
   promotes session content into durable memory is the exact pattern the
   prior audit rejected, and it is still present. [source]
4. **Tenancy is dataset ACL plus FastAPI users.** Permissions gate datasets
   per user/tenant (`cognee/modules/users/permissions/`), which is a
   reasonable SaaS posture but weaker than the OpenAgents model, where
   owner scope, consent state, ATIF redaction, and visibility classes gate
   every projection (`packages/agent-experience-memory/README.md`). Nothing
   in cognee redacts recalled content before it enters a prompt. [source]
5. **Cloud coupling.** The SDK now embeds a commercial cloud login and
   upload path. Self-hosting stays possible, but the center of gravity of
   the project is moving toward a hosted memory service that OpenAgents
   would neither control nor meter. Google Cloud is the sole production
   infrastructure authority here, and a third-party memory cloud is not an
   admissible dependency. [source] [inferred]
6. **License is clean, activity is high, but churn is real.** Apache-2.0
   with a NOTICE file is fully compatible. The 539-commits-per-month pace
   cuts both ways: fixes arrive fast, and interfaces move fast — the
   graph-store default changed underneath users within two months.
   [history]

## 8. The recommendation

Three paths were considered against the current landscape: the AI SDK's
`history-corpus` (cursor-addressed corpus recall), `rlm` (Effect-native
recursive recall — recall, not compaction), the `KhalaRuntimeEvent` durable
log, the Phase 2 Programs and Phase 3 multi-corpus roadmap
(`~/work/ai/docs/ROADMAP.md`), and the owner-profile layer in
`packages/agent-experience-memory`.

### Path (a) — run cognee as-is inside OpenAgents

The only defensible placement would be an owner-local sidecar: cognee's MCP
server on the owner machine, feeding recalled context into desktop or Pylon
sessions as advisory input. Even there it fails the current contracts: a
Python engine in the recall path contradicts the 2026-07-21 owner override,
the recall output enters prompts without any ATIF redaction pass, the
`improve` loop spends provider tokens outside exact accounting, and the
50-package Python environment becomes an operational surface the repo has
deliberately refused elsewhere. A production placement on Google Cloud would
add a stateful FastAPI service with embedded databases to operate — for a
capability the SDK roadmap already owns. **Rejected.** Reading the clone as
reference material stays fine. [inferred]

### Path (b) — port the ideas into the AI SDK

The cognee ideas map one-to-one onto work the SDK has already scheduled:

- **A graph-memory layer over `KhalaRuntimeEvent` corpora.** Phase 3 already
  plans multi-corpus recall with corpus composition laws and per-corpus
  redaction. A derived entity/relation index over the durable log — never a
  second truth store — is the cognee "permanent plane" rebuilt on the SDK's
  one-event-union rule. Deterministic identity via schema-declared identity
  fields (the `identity_fields` idea) gives entity nodes stable ids across
  re-derivation, which is what makes a derived index rebuildable and
  idempotent.
- **Entity extraction as a typed Program.** Phase 2 Programs declare
  signatures as Effect Schemas with decode as the only validity authority.
  Cognee's extraction prompt returning a `KnowledgeGraph` Pydantic model
  through `instructor` is exactly a Program with a graph-shaped output
  schema. The cognify chain — classify, chunk, extract, persist, index —
  becomes a composition of bounded typed operations with honest partial
  outcomes, not a background pipeline.
- **Source-labeled recall results.** The `history_recall` vocabulary should
  keep cognee's lesson: every recalled item names its plane (event log,
  thread snapshot, derived graph, profile memory) as a typed field, so a
  renderer and a redaction gate can treat planes differently.
- **Feedback-weighted ranking, never truth mutation.** The streaming weight
  update and used-element tracking fold into
  `packages/agent-experience-memory`'s ranking module as ranking features.
  Promotion into durable memory stays evidence-gated and consent-gated —
  the existing package already holds that line.
- **A provenance delete planner.** The owner `forget` lifecycle should plan
  deletion across every derived artifact (graph nodes, vectors, summaries)
  reachable from a source, the way cognee's unified engine does.
- **A typed export archive.** A COGX-shaped versioned snapshot format for
  corpora and profile memory serves owner export and future portability
  without adopting cognee's format.

All of this is re-derivation into Effect Schema and Effect services — no
vendoring, no Python, engine work filed on `OpenAgentsInc/ai`, consumption
work in the monorepo, per the standing division of labor. **Selected.**

### Path (c) — a memory-provider seam in the SDK

A `@openagentsinc/ai` memory-provider seam — typed `remember`, `recall`,
`forget` verbs behind an Effect service so cognee or an owned engine could
back it — is attractive later and premature now. There is no owned
graph-memory engine yet, so the seam would be designed against exactly one
real implementation (a Python process the repo cannot run in engine paths),
which is how bad abstractions calcify. The honest sequence is: build the
Effect-native graph-memory layer first (path b), let `history_recall` and
the owner-profile layer stabilize the verbs, and only then decide whether a
provider seam earns its keep. **Deferred, with a named revisit condition:
reconsider when a second real backing engine exists or when a customer
integration demands an external memory provider.** [inferred]

### The decision

**Path (b).** Cognee is the best-engineered memory model in the catalog, and
every one of its durable ideas fits work the AI SDK has already scheduled
(Programs, multi-corpus recall) or the monorepo already owns
(agent-experience-memory ranking and forget). The runtime, cost, tenancy,
and cloud frictions make the running system itself inadmissible. Port the
ideas, cite this teardown in the candidate packets, and keep the clone as a
reference for the details — the delete planner, the identity derivation,
and the capability flags reward a close read.

## 9. Adapt / reject lists

Fast Follow candidate lessons in the `docs/teardowns/` evidence lane.
Nothing here is dispatch authority. Implementation requires the normal
admission path.

**Adapt (re-derive, never vendor):**

- Schema-declared deterministic identity for derived entities
  (`identity_fields` → stable UUID5 per class). [source]
- Embeddable-field declaration on the datapoint schema, so the vector plane
  is a projection of the graph model. [source]
- The graph-memory layer as a derived, rebuildable index over the durable
  event log — mapped onto SDK Phase 3 multi-corpus recall. [inferred]
- Entity/relation extraction as a typed Program with a graph-shaped output
  schema — mapped onto SDK Phase 2 Programs. [inferred]
- Source-labeled recall result types per memory plane. [source]
- Used-element tracking on answers plus streaming feedback-weight updates,
  confined to ranking. [source]
- Provenance-aware deletion planning across derived artifacts. [source]
- Versioned typed export archives for owner export and portability.
  [source]
- Capability flags for store backends (`HYBRID_WRITE`, `HYBRID_SEARCH`) as
  a pattern for corpus-source capabilities. [source]
- Deterministic extraction where an exact parser exists (the code-graph
  lesson). [history]

**Reject:**

- Cognee as a runtime dependency, sidecar, or engine-path component — the
  no-Python engine rule is a fresh owner decision. [source]
- Background `improve` that promotes session content into durable memory
  without an evidence gate. [source]
- Dataset ACLs as the tenancy model — owner scope, consent, redaction, and
  visibility classes stay the OpenAgents boundary. [source]
- Any dependency on Cognee Cloud — Google Cloud is the sole production
  infrastructure authority. [source]
- Broad natural-language or Cypher query surfaces exposed to agents without
  typed scoping. [source]
- A memory-provider seam designed now around a single external Python
  backing. [inferred]

## 10. Watch items

- **Ladybug fork health** — the embedded default now rides a fork of Kuzu.
  Fork governance and release cadence decide whether the default store stays
  dependable. [history]
- **Truth subspace** — the centroid-based truth-alignment mechanism is
  young. If it matures into a verifiable claim-alignment method, it becomes
  interesting to the assurance program. [source] [limitation]
- **COGX as an interchange format** — if rival memory systems adopt it, a
  typed importer becomes worth a bounded candidate packet. [inferred]
- **Cloud/OSS divergence** — watch whether recall quality features start
  landing cloud-first, which would change the reference value of the OSS
  tree. [inferred]
- **MCP memory surface convergence** — the minimal `remember`/`recall`/
  `forget` MCP tool set is becoming a de facto agent-memory interface shape
  worth tracking for the harness MCP pilot. [source] [inferred]
- **Deterministic extraction breadth** — more enola-style non-LLM extractors
  would strengthen the case for typed extraction Programs with deterministic
  tiers. [history]
