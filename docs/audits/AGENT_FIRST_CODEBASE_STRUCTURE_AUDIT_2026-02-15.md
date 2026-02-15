# Agent-First Codebase Structure Audit (2026-02-15)

This audit reviews the **repository structure** and **in-repo knowledge layout** from an agent-first perspective, using the article **"Harness engineering: leveraging Codex in an agent-first world" (Ryan Lopopolo)** as the baseline for what tends to compound well over time.

## Scope

In scope:
- Top-level repo layout (`apps/`, `packages/`, `docs/`, cross-cutting directories).
- Documentation structure as the system of record (progressive disclosure, maps vs manuals).
- Drift control and mechanical enforcement opportunities (linters, structural tests, doc gardening).

Out of scope:
- Deep correctness review of application logic.
- Refactoring implementations (this is a structure/legibility audit + recommendations).

## Executive Summary

What already works well:
- **`docs/README.md` exists and is a real index**, not a grab-bag.
- **Domain docs are already grouped** (`docs/autopilot/*`, `docs/lightning/*`) with runbooks/reference/testing sub-structure.
- **ADRs are established and indexed** (`docs/adr/INDEX.md`) and provide a clear authority hierarchy.
- Existing precedent for audits: `docs/audits/EFFECT_ARCHITECTURE_AUDIT_2026-02-11.md`.

Highest-impact structural gaps (agent-first):
1. **Knowledge drift is easy to introduce and hard to detect** (stale paths, “archived-but-linked” canonical specs, local-only docs that aren’t in git).
2. **`AGENTS.md` is carrying too much “manual” weight** instead of acting as a stable map to deeper sources of truth.
3. **Canonical contracts referenced by ADRs are not present in-repo**, which breaks “repo as system of record” for agent runs.

## Snapshot Metrics (Repo-Local)

- `AGENTS.md` size: **353 lines**
- Tracked docs under `docs/`: **113 files**
- ADR count: **25** (`docs/adr/ADR-*.md`)
- App surfaces: **6** (`apps/*`)
- Shared packages: **9** (`packages/*`)
- Missing-but-referenced canonical specs:
  - `crates/dsrs/docs/{ARTIFACTS.md,REPLAY.md}` are referenced **44** times across ADRs but are not present in this repo.

## Findings (Prioritized)

### P0: Repo knowledge is not consistently “in-repo”

Symptoms:
- There is a local-only `docs/local/` directory (gitignored) that contains potentially useful knowledge, but it is not part of the versioned knowledge base available to agents running from a clean checkout.
- Multiple ADRs anchor critical contracts (Verified Patch Bundle / replay / hashing) on canonical specs that are not present in this repo (and are only referenced as “archived”).

Why it matters (agent-first):
- Agents can only reliably use what is versioned and discoverable in the current checkout. “It’s in backroom” behaves like “it doesn’t exist” for most runs.

Recommendation summary:
- Pull contract-defining specs back into `docs/` in a stable location, and make docs indices point only at tracked content.

---

### P0: Drift-prone pointers (paths and maps)

Example:
- Mobile app path drift (`apps/expo` vs `apps/mobile`) is the kind of low-grade mismatch that compounds across onboarding, prompting, and agent navigation.

Why it matters:
- Agents pattern-match from existing references. If the map lies, the agent wastes time and will replicate the lie.

Recommendation summary:
- Add mechanical checks that validate doc references to key repo paths (at least `apps/*`, `packages/*`, `docs/*`).

---

### P1: `AGENTS.md` is closer to “manual” than “map”

Current state:
- `AGENTS.md` contains many operational details (testing, deploys, debugging, directory maps, policy) in a single file.

Why it matters:
- The article’s core lesson is progressive disclosure: **a short, stable entrypoint** that teaches agents where to look next outperforms a monolithic instruction file.

Recommendation summary:
- Keep `AGENTS.md` as an enforced **table of contents** + “non-negotiables”.
- Move deep operational content into `docs/` (or app-local `README.md`) and link to it.

---

### P1: “Crates” naming conflicts with the post-Rust story

Current state:
- Rust code was removed/archived, but `crates/` still exists and contains active protocol drafts (`crates/nostr/nips/*`).

Why it matters:
- Naming is part of the map. `crates/` implies Rust-era code. Agents will interpret it as deprecated/historical even when it’s active.

Recommendation summary:
- Move protocol/NIP drafts to a structure that communicates intent, e.g. `docs/protocol/nostr/nips/` or `protocol/nostr/nips/`.

---

### P2: Plans, status, and “execution artifacts” are not uniformly structured

Current state:
- Some domains already have “plans” and “status” subfolders (`docs/lightning/plans`, `docs/lightning/status`), while other work is captured in mixed runbooks/reference docs.

Why it matters:
- The article emphasizes plans as first-class, versioned artifacts. This is what allows long-running agent work to stay coherent without out-of-band context.

Recommendation summary:
- Standardize a repo-wide plan layout: `docs/plans/{active,completed}/` plus `docs/plans/tech-debt.md` (or similar), and link from domain docs.

---

### P2: Tooling cohesion is easy to lose without repo-level guardrails

Observed structure:
- Multiple independent node projects (one per app/package), with mixed lockfile/tooling patterns.
- Root `node_modules/` exists despite no root `package.json` (potentially confusing for agents and for deterministic tooling).

Why it matters:
- Agent throughput amplifies inconsistencies. A single “blessed” way to run checks and a single “blessed” dependency topology reduces drift.

Recommendation summary:
- Consider a root workspace (pnpm/bun/npm workspaces) or, at minimum, a single `scripts/verify` that runs the right checks across surfaces and explains prerequisites.

---

### P3: PDF-only knowledge reduces agent legibility

Current state:
- `docs/rlm/` contains at least one large PDF.

Why it matters:
- Agents can work with PDFs, but Markdown summaries and extracted key sections are far easier to search, link, lint, and keep fresh.

Recommendation summary:
- Add a Markdown “notes” file alongside each PDF with a synopsis and why it matters to OpenAgents.

## Recommendations (Concrete, Ordered)

### P0: Make the knowledge base self-consistent and checkable

1. Create in-repo canonical specs for execution artifacts referenced by ADRs:
   - `docs/execution/ARTIFACTS.md` (Verified Patch Bundle schemas/fields)
   - `docs/execution/REPLAY.md` (REPLAY.jsonl schema)
   - Update ADR links to point to the in-repo versions.
2. Replace any `docs/*` index pointers to gitignored content with tracked equivalents.
3. Add a cheap doc-check script (even if only run locally at first) to validate:
   - referenced repo paths exist
   - markdown links resolve
   - domain doc folders contain an index (`README.md`) listing their key entrypoints

### P1: Turn `AGENTS.md` into a TOC (progressive disclosure)

1. Cut `AGENTS.md` down to:
   - authority/conflict rules
   - repo map (“where do I change things?”)
   - verification commands (by surface)
   - non-negotiable invariants (no stubs, schema boundaries, receipts)
   - links to deeper docs
2. Move operational guides into `docs/` (or app-local docs) and link them:
   - debugging/telemetry correlation already lives in `docs/autopilot/testing/*` (good pattern to repeat)
3. Add a lightweight “agent onboarding map” doc:
   - `docs/AGENT_MAP.md` (or similar) listing the 10-15 highest-signal documents with one-line “when to use this”.

### P1: Align naming with intent (reduce misrouting)

1. Move `crates/nostr/nips/*` into `docs/protocol/nostr/nips/*` (or `protocol/...`) and leave a short redirect doc behind.
2. Add missing `README.md` indexes for:
   - `docs/codex/`
   - `docs/audits/`
   - `docs/research/`
   - `docs/rlm/`

### P2: Standardize “plans” as artifacts

1. Introduce a repo-level plan hub:
   - `docs/plans/README.md` (index + conventions)
   - `docs/plans/active/` and `docs/plans/completed/`
2. Provide a plan template that is agent-friendly:
   - fixed sections: Goal, Constraints, Non-goals, Milestones, Verification, Rollback, Decision log
3. Update domain docs to link to relevant plans, and move long “plan” docs out of reference/runbooks.

### P2: Add mechanical architecture enforcement where it pays off

1. Replicate the idea behind `apps/mobile/.dependency-cruiser.js` across `apps/web` and core packages:
   - define allowed dependency directions
   - forbid cross-layer imports (enforced)
2. Add structural tests with error messages written for remediation (agent context injection).

## Suggested Target Doc Layout (Non-Disruptive)

Keep existing domain docs, but add a few cross-cutting “system of record” anchors:

```text
docs/
  README.md
  PROJECT_OVERVIEW.md
  ROADMAP.md
  GLOSSARY.md
  adr/
  audits/
  execution/         # new: canonical artifact/replay specs referenced by ADRs
  plans/             # new: repo-level plan hub
  autopilot/
  lightning/
  protocol/          # new or migrate from crates/
```

## Next Steps

If you want follow-on PRs after this audit, the highest leverage sequence is:

1. Bring canonical execution specs in-repo (`docs/execution/*`) and update ADR links.
2. Add doc-lint checks (path existence + markdown link resolution) and run them in CI.
3. Refactor `AGENTS.md` into a shorter TOC and move the rest into structured docs.
