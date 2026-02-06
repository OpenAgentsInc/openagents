# Microcode and OpenAgents Autopilot / Effect / DSE: Synergies and Learnings

This doc analyzes **Microcode** (`~/code/microcode`, Modaic) and how its patterns map onto our **Autopilot**, **Effect**, and **DSE** direction.

**Microcode in one sentence:** a minimal terminal agent that runs a **precompiled “program artifact”** (Modaic `AutoProgram.from_precompiled`) with runtime-config knobs (models, iteration/token/output budgets, verbose trajectories, trace toggle), plus pragmatic UX for **large pasted content** and **dynamic MCP tool mounting**.

---

## 1) Microcode recap (relevant bits)

### 1.1 “Precompiled program” artifact + runtime config

Microcode does *not* ship a big pile of prompt strings. It loads a **named, versioned precompiled artifact**:

- `AutoProgram.from_precompiled(MODAIC_REPO_PATH, rev=MODAIC_ENV, config=...)`
- `MODAIC_REPO_PATH = "farouk1/nanocode"` (external registry)
- `rev` is `"prod"` vs `"dev"` style environment pinning

Runtime config includes:
- **model selection**: `lm` (primary) and `sub_lm` (auxiliary)
- **budgets**: `max_iters`, `max_tokens`, `max_output_chars`
- **debug/obs**: `verbose`, `track_trace` (and W&B env wiring)

This is structurally similar to what we want from DSE: **compile produces an immutable policy bundle**, runtime **pins** and executes it with explicit knobs and receipts.

### 1.2 Config precedence + persistence (DX posture)

Microcode resolves settings with a clean precedence chain:

- CLI flags override env and cache
- env overrides cache
- cache persists choices between runs (JSON files under `~/.cache/microcode`)

It uses atomic writes and attempts `chmod 0o600` for key files (good “local secret hygiene”).

### 1.3 Paste/large-input handling (side channel idea)

Microcode detects large pasted text (threshold via `MICROCODE_PASTE_THRESHOLD`) and replaces it with a placeholder like:

- `[pasted 12345+ chars]`

It stores the full payload in a separate structure and can later “consume” it when the placeholder appears in the user input.

The key transferable concept for Autopilot/DSE is not the exact UI implementation; it’s the **separation of large blobs from the main conversational payload**, while keeping stable references (placeholder ids / hashes) that can be used for:

- prompting without context-window blowups
- receipts/replay (store blob once, reference many times)
- retrieval / incremental loading

### 1.4 Dynamic MCP tool mounting (capability injection)

Microcode supports `/mcp ...` to load an MCP server via `mcp2py.load(...)`, then registers tools into the precompiled agent:

- tools are namespaced: `<serverName>_<toolName>`
- existing tools for a server can be removed and re-registered
- supports auth/headers/auto-auth flags

This is a concrete “capability mounting” pattern: tools are not static; they can be introduced at runtime, and the agent gets a **namespaced capability surface**.

### 1.5 “Trajectories” / internal monologue as a debug surface

Microcode encourages running with verbose mode to see the agent’s “trajectories” (debugging only). It also supports a `track_trace` switch (backed by a tracing dependency in its stack).

The transferable idea is: provide a *first-class* debug surface that can be enabled without changing the program, and that is compatible with replay/audit.

---

## 2) Alignment with our constraints and goals

| Our goal/constraint | Microcode pattern |
|---|---|
| **DSE: compile → artifacts → runtime pin/rollback** | Microcode treats the agent “program” as a *precompiled artifact* resolved by `(repo, rev)` and configured at runtime. |
| **Effect-first wiring** | Microcode’s config precedence/persistence maps cleanly to Effect `ConfigProvider` + service layers. |
| **Budgets everywhere** | Microcode has explicit `max_iters`, `max_tokens`, `max_output_chars`. This matches our desire for bounded runs (Horizons budgets + DSE constraints). |
| **Replayability** | Placeholder-based large-blob handling hints at storing blobs once and referencing them by id/hash in receipts and replays. |
| **Tools as capabilities** | MCP mounting shows a pragmatic way to grow tool surfaces dynamically while keeping namespacing. |
| **Obs + debug** | Verbose trajectories + trace toggles map to “debug mode” execution with richer receipts and traces. |

---

## 3) Synergies we should adopt (Effect-first)

### 3.1 Treat “compiled programs” as *first-class artifacts* (not just “params”)

Microcode’s strongest signal is the product posture: **load a precompiled program artifact, then run it**.

For us, DSE already defines compiled artifacts per `Signature`. Microcode suggests we should also support “program bundles” at higher levels:

- **Signature artifact**: `signatureId + compiled_id + params + eval`
- **Program artifact (optional later)**: a graph/pipeline artifact that pins a whole multi-step flow (nodes + promoted signature artifacts + budgets + tool policy)

This aligns with:
- our DSE artifact registry and promotion
- Horizons’ “graph runs are explicit and auditable”

### 3.2 Config precedence + persistence as a standard runtime service

Microcode’s env/cache/flag precedence is exactly the kind of deterministic behavior we want in Worker runtimes too, just with different storage:

- env/bindings (Cloudflare) + request overrides
- durable storage (DO SQLite) for user preferences / pinned artifacts

Recommended adaptation:
- define an Effect service for **runtime policy configuration resolution** (explicit precedence, typed, auditable)
- record resolved config (or its hash) in receipts

### 3.3 Large blob “side channel” for prompts + receipts

We should adapt the placeholder idea into DSE Prompt IR and receipts:

- add a `ContextEntry` type that can be either:
  - inline small text, or
  - a **blob reference**: `{ blobId, sha256, size, mime, storageBackend }`
- store blob content once (DO SQLite / R2), reference by hash/id
- in eval/replay, we can rehydrate blobs when needed (with size limits)

This directly supports our “everything is logged and replayable” posture without forcing enormous prompt strings into every run.

### 3.4 Dual-model policy (“primary” + “sub”) as a first-class knob

Microcode bakes in the idea that a single “agent” uses:
- a primary model for main reasoning
- a smaller sub model for auxiliary work

In DSE terms, this is a policy bundle feature:
- allow artifacts to pin **multiple model roles** (main, judge, repair, router, summarizer)
- keep the selection explicit and hashable

### 3.5 Dynamic MCP mounting as a ToolProvider pattern

Microcode’s approach suggests a clean interface for us:

- `McpToolProvider` Effect service that can:
  - mount an MCP server as a namespaced tool set
  - provide schemas/contracts for those tools
  - enforce tool policy allowlists per signature/artifact
  - emit receipts for tool calls with stable ids

We should keep this **orthogonal** to DSE: DSE consumes “tool contracts”; ToolProviders supply implementations.

### 3.6 Debug surfaces: “verbose trajectories” as structured traces

We should avoid shipping raw “internal monologue,” but we *do* want debuggability:

- a `DseDebug` flag that enables:
  - prompt IR snapshots (normalized + rendered)
  - transform application logs (which knobs changed what blocks)
  - expanded receipts/traces

Key is to store the debug evidence as structured trace events that can be redacted and replayed.

---

## 4) What we should *not* copy

- **Blindly importing Modaic**: Microcode’s engine is external and Python/RLM-specific. Our DSE compile/runtime is TypeScript + Effect-native by constraint.
- **Unbounded verbose dumps**: “trajectories” are useful, but we must keep strict bounds and redaction policies (privacy modes, truncation) aligned with our receipts/replay requirements.

---

## 5) Concrete updates to our DSE/Autopilot plan

This is what we should fold into `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`:

- Add a named pattern: **Artifact pinning posture** (Microcode-like): runtime always resolves/pins an artifact id (`compiled_id`) and logs it.
- Expand budgets to include **max output chars** (in addition to time/tool/LLM calls).
- Add a **BlobStore + BlobRef** concept to Prompt IR + receipts/replay.
- Make “multi-model roles” explicit in DSE params/artifacts (main/sub/repair/judge).
- Add a `ToolProvider` section explicitly calling out **MCP mounting** as a provider pattern (namespacing, schemas, policies, receipts).
- Add a `DseDebug`/trace toggle that produces structured trace events (instead of unstructured monologue).

---

## 6) References (Microcode)

- `~/code/microcode/README.md` (features, “trajectories,” MCP, caching)
- `~/code/microcode/microcode/main.py` (precompiled program load, config, CLI loop)
- `~/code/microcode/microcode/utils/paste.py` (paste placeholder pattern)
- `~/code/microcode/microcode/utils/mcp.py` (MCP tool mounting + namespacing)
- `~/code/microcode/microcode/utils/cache.py` (atomic persistence + chmod 600)

