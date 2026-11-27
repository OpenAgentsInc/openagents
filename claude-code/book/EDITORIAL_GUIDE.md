# Editorial & Authoring Guide

This guide defines the voice, structure, and engineering rigor for every chapter in `book/`. Whether a chapter is drafted by a human author or an AI collaborator, it should read as if written by one clear-eyed systems engineer walking a peer through a real codebase at a whiteboard.

---

## 1. Core Voice & Pedagogical Principles

1. **Bottom-up, never encyclopedic.**
   Do not list every function in a file. Teach the *minimum set of load-bearing abstractions* that make the subsystem click, in dependency order.
2. **Problem → Mechanism → Invariant.**
   Never show a data structure before explaining the concrete bug, race condition, latency cliff, or security hazard that forced it to exist.
3. **Explain the scars.**
   Real production codebases are full of battle scars—`DANGEROUS_uncachedSystemPromptSection`, UNC path checks to stop NTLM leaks, `setAppStateForTasks`, `cachedClaudeMdContent` cycle breakers, `withheld` error suppression. Those comments and workarounds are the *most educational lines in the entire repo*. Highlight them.
4. **Build a mental model, not just prose.**
   Use small ASCII or Mermaid diagrams to show data flow, state transitions, and ownership boundaries. Summarize each chapter with a crisp **"Snap-In Point"** showing exactly how the next layer consumes what we just built.

---

## 2. Standard Chapter Anatomy

Every numbered chapter in `book/chapters/` should follow this repeatable rhythm:

```markdown
# Chapter NN — Title: Crisp Subtitle

> **Layer:** Layer X (Name)
> **Prerequisites:** Chapters ...
> **Primary Source Files:**
> - `path/to/fileA.ts` — one-line role
> - `path/to/fileB.ts` — one-line role

## Why This Layer Exists
Frame the exact engineering problem we are solving now that Layers 0..X-1 exist.

## 1. First Building Block
### The Problem
### The Real Implementation (short, annotated excerpt or exact symbol walkthrough)
### Hidden Invariants & Failure Modes

## 2. Second Building Block
...

## Putting the Layer Together
ASCII / Mermaid diagram showing how the pieces in this chapter compose.

## How This Snaps Into the Next Layer
Explicit handoff into Chapter NN+1 (naming the exact downstream caller/file).

## Reading Exercises & Code Checkpoints
2–4 targeted questions or `rg` commands the reader can run in this repo to verify their understanding.
```

---

## 3. Source Grounding & Honesty Rules

Because this is a reverse-engineering book written against a real source snapshot, **factual precision is non-negotiable**:

- **Verify before writing.** Never cite a file path, symbol name, line number, or behavior from memory. Read the file directly first.
- **Quote lightly and faithfully.** Include short, focused TypeScript excerpts (5–35 lines) that illuminate the design; paraphrase long boilerplate and point the reader to exact file paths and line ranges (e.g., `services/tools/toolOrchestration.ts:91-116`).
- **Flag build-time and runtime gates.** Whenever a code path is behind `feature('...')`, `process.env.USER_TYPE === 'ant'`, GrowthBook (`getFeatureValue_*`), or an env var, explicitly say so. Never present a gated experiment as universally active.
- **Flag snapshot absences transparently.** If an imported module is absent from this checkout (such as `types/message.ts`, `query/transitions.ts`, `services/contextCollapse/`, or `services/skillSearch/`), explicitly label what is **observed at call sites** vs. **absent from the snapshot**.
- **Do not invent build/test commands.** This checkout has no tracked `package.json`, `tsconfig.json`, or test harness. Exercises should use read-only inspection (`rg`, reading source files, tracing types) rather than claiming runnable CLI build commands.
- **Prefer repo-relative paths inside `book/`.** Use paths like `Tool.ts` or relative markdown links like `Tool.ts` from `book/chapters/` (or `Tool.ts` from `book/`) so links work cleanly on GitHub and locally.

---

## 4. Diagram & Formatting Conventions

- **Mermaid diagrams:** Stick to `flowchart TD`, `flowchart LR`, `sequenceDiagram`, and `stateDiagram-v2`. Always quote node labels containing parentheses, brackets, or colons (e.g., `A["query() loop"]`).
- **ASCII diagrams:** Keep lines under 80 columns so they render cleanly in terminals, editors, and narrow viewports.
- **Callout boxes:** Use GitHub-style alerts purposefully:
  - `> [!NOTE]` — subtle codebase detail or snapshot caveat.
  - `> [!IMPORTANT]` — load-bearing architectural invariant (e.g., prompt-cache boundary).
  - `> [!WARNING]` — subtle trap or failure mode the code explicitly guards against.
- **Consistent naming:**
  - Refer to the application as "Claude Code" (or "the agent harness"); the source snapshot is Claude Code 2.1.88. Don't call it `cc`, which is only the local folder name.
  - Use `monospace` for file paths, symbols, env vars, feature flags, and type names.

---

## 5. Chapter Naming & File Convention

- Directory: `book/chapters/`
- Filename: `NN-kebab-case-slug.md` (zero-padded two-digit prefix: `00-...`, `01-...`, `02-...`, ... `24-...`).
- When a new chapter is drafted or updated, update the **Manuscript Progress** table in [README.md](README.md) and add any new domain terms to [GLOSSARY.md](GLOSSARY.md).
