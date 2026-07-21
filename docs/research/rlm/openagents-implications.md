# RLM Implications for OpenAgents

**STATUS: Point-in-time research record (2026-07-21).** Candidate analysis only.
Does not admit product work, spend, release, or public claims. Precedence:
`AGENTS.md`, `INVARIANTS.md`, ProductSpec, AssuranceSpec, Sol roadmap, live
issues.

## Prior OpenAgents position

The 2026-06-28 audit already recommended a **hybrid**:

- Python RLM as a **leaf executor** on sandbox-capable tiers.
- Effect/TypeScript as **online authority** (selection, gates, receipts).
- Do not reimplement the full RLM REPL stack inside Workers.

This folder’s paper + repo reading **supports that hybrid**. It does not reverse
it. The paper’s strongest results depend on exactly the properties that make a
Worker port unattractive: long-running loops, code execution, multi-hop subcalls,
and variable cost tails.

## Where RLM maps onto OpenAgents surfaces

| OpenAgents surface | RLM relevance | Suggested stance |
| --- | --- | --- |
| Pylon / local Codex coding | Long repos, dense multi-file questions, pair-style aggregation | Optional long-context backend behind existing assignment sandboxes |
| Khala coding delegation | Public-safe objective + pinned verify. Private workspace stays local | RLM may run **inside** the owner-local executor, never as a public authority |
| Full Auto / fleet runs | Multi-step orchestration over large issue/context packs | Candidate pattern for “orchestrator model writes programs over context object” |
| ProductSpec workroom / evidence | Long evidence packs | Possible offline analyst. Never a release oracle |
| Desktop / mobile chat | UX expectation of one completion | RLM can sit behind the same completion API shape if cost/latency UX is honest |
| Cloud Run monolith | Multi-tenant hostile input | Only isolated sandboxes. Never default local `exec` |
| Blueprint / promise registry | Governance | **No** RLM authority. RLM outputs are untrusted candidates |

## Concrete lessons to import (without adopting the whole paper)

### 1. Prompt-as-environment invariant

When a task’s input exceeds a reliable window, prefer:

- Store the corpus as an environment object (files, DB, object store, REPL var).
- Give the root model **metadata + tools**, not the full blob.

OpenAgents already does pieces of this (workspace checkouts, assignment
fixtures, tool use). RLM is a pure form of the same idea for **text corpora**.

### 2. Programmatic fan-out beats verbal sub-agents for dense work

For work that scales with input size (label every line, or score every pair), a
small number of hand-described sub-agents is the wrong abstraction. Prefer:

- Code that loops / batches over slices.
- Explicit concurrency caps and budgets.

This is closer to fleet/batch design than to “spawn three friends in chat.”

### 3. Depth 0 is already valuable

Paper and repo both show **REPL offload without recursion** helps long inputs.
OpenAgents can trial “context object + code tools” before enabling nested RLM
depth, which is where cost and error propagation explode (especially on weaker
coding models).

### 4. Train orchestrators, reuse leaf models

The training recipe focuses SFT/RL on **root REPL behavior**. That matches a
plausible OpenAgents path:

- Keep frontier APIs as leaves.
- Optionally post-train a smaller orchestrator on successful RLM trajectories
  from our domains (coding, evidence packs), only under admitted training
  authority and budget.

### 5. Measure dense long tasks separately from NIAH vanity

Do not claim “million-token support” from needle tests alone. If we ever claim
long-context product quality, use task classes like OOLONG (linear) and
pair-style (quadratic) analogs on **our** corpora, with receipts.

### 6. Guardrails are product requirements

From paper limits + repo knobs, any pilot should require:

- `max_depth`, `max_iterations`, `max_budget` / token caps, timeouts.
- Isolated execution (Docker / existing Pylon sandbox story), not host `exec`.
- Per-subcall logging compatible with ATIF / owner-only traces.
- Redaction: raw prompts and shell output stay private. Public counters stay
  exact-token based only where already specified.

## What not to do

- **Do not** rewrite `rlm` in Effect for Workers as an early goal.
- **Do not** treat RLM accuracy on BrowseComp/OOLONG as an OpenAgents product
  promise without a registered promise + verification gate.
- **Do not** let RLM subcalls bypass spend, network, or repository policy.
- **Do not** dump multi-million-token contexts into public traces or Forum posts.
- **Do not** assume one system prompt works across Codex, Claude, and Grok
  roots (paper negative result).

## Suggested pilot shape (if later admitted)

A minimal, reversible pilot consistent with current authority:

1. **Offline or owner-local only** — Pylon assignment or Desktop-local runner.
2. **Task class** — LongBench-style CodeQA or an internal “answer questions over
   this checkout / log pack” fixture.
3. **Depth 0 first**, then depth 1 with a cheaper sub-model.
4. **Hard USD and token caps** on every run.
5. **Compare** against: direct model, compaction summary, and current coding
   agent path.
6. **Evidence** — private trajectory log + public-safe summary metrics. No
   promise flip from a single lab run.

Promotion from pilot to product path needs a normal Sol issue / accepted plan,
not this research note.

## Relationship to other research threads

| Thread | Link |
| --- | --- |
| DSPy / RLM / GEPA hybrid decision | `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md` |
| Continual learning / memory | AgentCL notes under `docs/research/agentcl/` |
| Terminal-agent RL recipe | TMAX notes under `docs/research/tmax/` |
| Harness evolution | `docs/research/2026-07-04-harness-optimization-evolve-the-harness-audit.md` |
| MemoHarness memory | `docs/research/2026-07-18-memoharness-*.md` |

RLM is complementary to memory systems: memory decides **what to store and
retrieve**. RLM decides **how to process a huge active object** without stuffing
it into one window. They are not substitutes.

## Summary recommendation

Keep the 2026-06-28 hybrid. Treat `alexzhang13/rlm` as the reference
implementation of a long-context executor pattern that OpenAgents can host on
sandboxed capacity. Use this paper’s task-complexity framing when we evaluate
any long-context claim. Do not expand product scope from this folder alone.
