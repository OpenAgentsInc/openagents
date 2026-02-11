# Context Failures (Rot, Poisoning, Confusion)

- **Status:** Draft (operational spec)
- **Last updated:** 2026-02-09
- **Conflict rules:**
  - If terminology conflicts: `docs/GLOSSARY.md` wins.
  - If behavior conflicts with code: code wins.

This doc defines the context failure modes we care about and how they map to OpenAgents mitigations, telemetry, and routing decisions. It exists to prevent the default anti-pattern: "just add more context".

Related docs:

- RLM integration plan: `docs/autopilot/synergies/rlm-synergies.md`
- DSE spec: `docs/autopilot/dse/dse.md`
- Effect-only RLM/DSE design: `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`
- Foundations and routing triggers: `docs/AGENT_FOUNDATIONS.md`

---

## 1) Taxonomy

| Failure mode | What it is | What it looks like | Primary mitigation |
|--------------|------------|--------------------|--------------------|
| **Context rot** | Quality degradation past a model's soft limit as tokenized prompt grows | Agent gets worse over time; plausible but wrong answers; missed details | Keep token space small (BlobRefs/handles), trigger RLM strategy, use context ops |
| **Context poisoning** | Untrusted/incorrect content influences decisions (malicious or accidental) | Agent confidently repeats bad facts; "instructions" appear inside retrieved text | Provenance + trust labeling, isolate untrusted sources, verify via tools/tests |
| **Context confusion** | Too many unrelated facts/threads collide; model blends them | Wrong entities; mixing tasks; stale assumptions applied to new context | Hard scoping, explicit schemas/handles, bounded evidence injection |

---

## 2) Context Rot

Definition (canonical): see `docs/GLOSSARY.md` for **context rot**, **soft context limit**, **token space**, and **variable space**.

### 2.1 Why it is pernicious in agents

Context rot is not a clean failure (like "prompt too long"). The model continues to produce output as quality drops. In long-running agents this creates a slow-motion failure:

- Each additional turn adds logs, diffs, and tool output.
- Retrieval adds more snippets.
- "Helpful summaries" accumulate and drift.
- Eventually the prompt passes the soft limit and accuracy erodes.

### 2.2 Mitigations we should standardize

- **Two-bucket context posture**:
  - Store large context as blobs (BlobStore) and reference it by handles/BlobRefs.
  - Keep token space to bounded previews and derived summaries, not raw dumps.
- **Context ops as tool calls**:
  - For repo/log exploration: grep/read_lines/symbols/chunk/peek are explicit operations with receipts and budgets.
- **Routing and strategy switching**:
  - Track **context pressure** and trigger an RLM-style strategy before the prompt crosses soft limits.
  - Treat RLM as an inference-time strategy, not a prompt template.
- **Budgets and "stop on thrash"**:
  - RLM loops must have iteration/subcall/tool budgets.
  - Detect stuckness and stop early (or fall back) when iterations are not producing new evidence.
- **Distill after exploration**:
  - Use traces to distill repeating tactics into explicit signatures/modules/graphs so the default path is fast and reliable.
  - Keep RLM as a fallback for novel/high-context cases.

### 2.3 Telemetry we need to detect it

Minimum fields/events we should be able to derive from receipts/traces:

- Prompt token counts (when available) and/or rendered prompt size.
- A **context pressure** estimate and the inputs that drove it.
- Which blobs/handles were accessed and how much of each was surfaced into token space (previews).
- Loop-level counters: iterations, sub-LM calls, tool calls, elapsed time.
- "New evidence" signal per iteration (e.g., did we read any new spans/blobs vs rehashing).

---

## 3) Context Poisoning

Context poisoning is about *trust*, not length.

Examples:

- Retrieved text includes malicious instructions (prompt injection).
- Logs contain misleading errors or stack traces from unrelated runs.
- Stale docs contradict code, and the model follows docs anyway.

Mitigations:

- **Provenance-first evidence**:
  - Whenever possible, carry SpanRefs/paths/line ranges/commit hashes alongside extracted facts.
  - Avoid letting "raw retrieved text" act as an implicit authority.
- **Trust labeling**:
  - Tag inputs as trusted (repo code at commit), semi-trusted (generated artifacts), untrusted (user paste, web, logs).
  - Prefer "untrusted evidence requires verification" as a routing rule.
- **Isolation**:
  - Do not let untrusted text share the same channel as system instructions.
  - Keep untrusted content in variable space; only surface bounded excerpts with provenance to token space.
- **Verification**:
  - For objective claims, default to tool verification (tests/builds/grep) over LLM inference.

RLM note: RLM can *reduce* the amount of poisoned text that enters token space, but it does not remove the need for provenance/trust and verification.

---

## 4) Context Confusion

Context confusion is about *scoping* and *collisions*.

Examples:

- Mixing two threads/projects in the same prompt.
- Reusing variable names like `context` without clear scoping.
- Having multiple "summaries" with conflicting assumptions.

Mitigations:

- **Hard scope keys everywhere**: `{ orgId?, projectId?, userId, threadId, runId }`.
- **Explicit handles**: reference context by handle (`BlobRef`, `context_handle`) instead of copying raw text.
- **Schema-bound IO**: output must satisfy Effect Schema; avoid free-form "brain dumps" as intermediate state.
- **Evidence linking**: when the agent asserts a fact, attach provenance (path/span/ref) so confusion can be audited.

---

## 5) Routing Rules (MVP posture)

We should prefer deterministic boundaries and explicit strategy switching:

- If **context pressure** is low: use direct Predict with strict schema decode + bounded context.
- If **context pressure** is high (or "thrash without new evidence"): trigger an RLM strategy that:
  - externalizes context (VarSpace + BlobRefs)
  - uses context ops to fetch evidence surgically
  - enforces strict budgets
- If inputs are untrusted or ambiguous: escalate verification and provenance requirements before acting.
