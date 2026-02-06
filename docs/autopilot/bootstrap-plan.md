# Autopilot Bootstrap (DB-Backed, Effect Schema) Plan

Date: 2026-02-06

This doc proposes an Autopilot-native version of OpenClaw's "workspace bootstrap" flow, redesigned for a cloud-first, multi-surface Autopilot where bootstrap artifacts are treated as **durable state** rather than a directory of files.

Instead of seeding Markdown files like `SOUL.md` and `USER.md`, we model bootstrap state as **typed, versioned records** (Effect `Schema`) stored in a **database** (Durable Object SQLite and/or Convex). This unlocks runtime validation, queryability, receipts/replay stability, visibility controls, and a single-bundle export/import story. We can still render a "context file" view into the system prompt for the OpenClaw vibe, but the canonical representation is structured data with auditability.

## Source Reference (OpenClaw)

OpenClaw implements bootstrap via seeded workspace files and prompt injection:

- Seeding: `~/code/openclaw/src/agents/workspace.ts` (`ensureAgentWorkspace()`)
- Injection: `~/code/openclaw/src/agents/bootstrap-files.ts` + `pi-embedded-helpers/bootstrap.ts`
- Hook override point: `~/code/openclaw/src/agents/bootstrap-hooks.ts` + `src/hooks/internal-hooks.ts`
- Onboarding hatch UX: `~/code/openclaw/src/wizard/onboarding.finalize.ts` (sends "Wake up, my friend!" if `BOOTSTRAP.md` exists)

We keep the *product intent* (one-time "birth certificate" ritual + durable identity/user/soul docs) but change the storage and update mechanism.

## Goals

- Preserve the OpenClaw feel:
  - A one-time "birth certificate" bootstrap conversation
  - Persistent: Identity, User profile, Soul/persona, Heartbeat checklist, Tools notes, Memory
  - Ability to inject these into the system prompt every turn (with truncation)
  - Ability to run "bootstrap hooks" that can override injected content (experiments / safety)
- Cloud-first:
  - No filesystem dependency
  - Multi-surface friendly (web now, others later)
  - Typed contracts, runtime validation, and audit logs
- "Better than files":
  - Structured schemas (no full-doc diff parsing required)
  - Versioned updates, `updatedBy`, timestamps, and receipts
  - Visibility controls (e.g. "main-only" memory) enforced mechanically

## Why Effect Schema (Benefits)

Using Effect `Schema` for these bootstrap artifacts is the difference between "we store some text" and "we have a safe, evolvable configuration + memory system".

Benefits over DB-stored Markdown blobs:

- Runtime validation at every boundary (LLM/tool calls, UI edits, Convex, Durable Object storage) via `Schema.decode`.
- Single source of truth for TypeScript types + runtime constraints (no drift between types and validation).
- Safer partial updates (update `user.timeZone` without rewriting a whole doc).
- Queryability and indexing (e.g. `bootstrap.status`, `completedAt`, `memory.visibility`) instead of buried text.
- Versioning + migrations (upgrade old records deterministically; avoid “format drift”).
- Tool schemas “for free”: the same schemas can validate tool inputs/outputs and later generate JSON schema / forms.
- Better receipts/replay stability: canonical encodings are easier to hash than arbitrary Markdown formatting.
- Typed error modeling (`Schema.TaggedError`) for update conflicts, forbidden visibility, invalid inputs, etc.

## Non-Goals (For MVP)

- Multiple workspaces per user (Autopilot is 1:1 user <-> thread)
- Marketplace/MCP integration
- Arbitrary tool execution / code sandboxes (separate track)

## Concept Mapping: OpenClaw Files -> Autopilot Records

OpenClaw uses these canonical bootstrap files:

- `AGENTS.md`: global rules, "if BOOTSTRAP exists, run it"
- `BOOTSTRAP.md`: one-time ritual instructions
- `IDENTITY.md`: name, creature, vibe, emoji, avatar
- `USER.md`: user name, address style, timezone, notes
- `SOUL.md`: core truths, boundaries, vibe, continuity
- `TOOLS.md`: local tool/convention notes
- `HEARTBEAT.md`: optional checklist
- `memory/YYYY-MM-DD-*.md` + `MEMORY.md`: daily + long-term memory

Autopilot should store each as structured records, with a derived Markdown/string renderer for prompt context.

Proposed mapping:

| OpenClaw artifact | Autopilot record type | Notes |
| --- | --- | --- |
| `AGENTS.md` | `AgentRulesDoc` (global) | Usually code-defined; optionally DB-backed for hotfix/versioning. |
| `BOOTSTRAP.md` | `BootstrapRitualTemplate` (global) + `AutopilotBootstrapState` (per user) | Template defines the ritual; state tracks whether it is pending/complete. |
| `IDENTITY.md` | `IdentityDoc` | Structured fields, versioned. |
| `USER.md` | `UserDoc` | Structured fields, versioned. |
| `SOUL.md` | `SoulDoc` | Prefer structured arrays for truths/boundaries. |
| `TOOLS.md` | `ToolsDoc` | Tool surface and conventions (not "local binaries"). |
| `HEARTBEAT.md` | `HeartbeatDoc` | Checklist items + future schedule fields. |
| `memory/*` + `MEMORY.md` | `MemoryEntry[]` | Enforce `visibility` mechanically (main-only vs all). |

## Data Model (Effect Schema)

All domain types should be Effect `Schema` so we get:

- runtime validation on every read/write boundary
- branded IDs to prevent mixing types
- stable JSON encoding/decoding

Below are schema sketches (not final).

```ts
import { Schema } from "effect"

export const UserId = Schema.String.pipe(Schema.brand("UserId"))
export type UserId = typeof UserId.Type

export const ThreadId = Schema.String.pipe(Schema.brand("ThreadId"))
export type ThreadId = typeof ThreadId.Type

export const DocVersion = Schema.Int.pipe(Schema.positive(), Schema.brand("DocVersion"))
export type DocVersion = typeof DocVersion.Type

export const BootstrapStatus = Schema.Literal("pending", "in_progress", "complete")
export type BootstrapStatus = typeof BootstrapStatus.Type

export class IdentityDoc extends Schema.Class<IdentityDoc>("IdentityDoc")({
  version: DocVersion,
  name: Schema.String,
  creature: Schema.String,
  vibe: Schema.String,
  emoji: Schema.String,
  avatar: Schema.optional(Schema.String), // URL/data URI, later: typed union
  updatedAt: Schema.Date,
  updatedBy: Schema.Literal("user", "agent"),
}) {}

// Global rules (OpenClaw AGENTS.md analog).
export class AgentRulesDoc extends Schema.Class<AgentRulesDoc>("AgentRulesDoc")({
  version: DocVersion,
  // For now: keep as a string blob rendered into the system prompt.
  // Later: structured + compiled policies.
  body: Schema.String,
}) {}

// Global ritual definition (OpenClaw BOOTSTRAP.md analog).
export class BootstrapRitualTemplate extends Schema.Class<BootstrapRitualTemplate>(
  "BootstrapRitualTemplate"
)({
  version: DocVersion,
  // Rendered instructions (string) plus optional structured steps later.
  body: Schema.String,
}) {}

export class UserDoc extends Schema.Class<UserDoc>("UserDoc")({
  version: DocVersion,
  name: Schema.String,
  addressAs: Schema.String,
  pronouns: Schema.optional(Schema.String),
  timeZone: Schema.optional(Schema.String), // IANA TZ id
  notes: Schema.optional(Schema.String),
  context: Schema.optional(Schema.String),
  updatedAt: Schema.Date,
  updatedBy: Schema.Literal("user", "agent"),
}) {}

export class SoulDoc extends Schema.Class<SoulDoc>("SoulDoc")({
  version: DocVersion,
  coreTruths: Schema.Array(Schema.String),
  boundaries: Schema.Array(Schema.String),
  vibe: Schema.String,
  continuity: Schema.String,
  updatedAt: Schema.Date,
  updatedBy: Schema.Literal("user", "agent"),
}) {}

export class ToolsDoc extends Schema.Class<ToolsDoc>("ToolsDoc")({
  version: DocVersion,
  notes: Schema.String,
  updatedAt: Schema.Date,
  updatedBy: Schema.Literal("user", "agent"),
}) {}

export class HeartbeatDoc extends Schema.Class<HeartbeatDoc>("HeartbeatDoc")({
  version: DocVersion,
  checklist: Schema.Array(Schema.String),
  updatedAt: Schema.Date,
  updatedBy: Schema.Literal("user", "agent"),
}) {}

export class MemoryEntry extends Schema.Class<MemoryEntry>("MemoryEntry")({
  id: Schema.String, // later: brand
  createdAt: Schema.Date,
  kind: Schema.Literal("daily", "long_term"),
  title: Schema.String,
  body: Schema.String,
  visibility: Schema.Literal("main_only", "all"),
}) {}

export class AutopilotBootstrapState extends Schema.Class<AutopilotBootstrapState>(
  "AutopilotBootstrapState"
)({
  userId: UserId,
  threadId: ThreadId,
  status: BootstrapStatus,
  startedAt: Schema.optional(Schema.Date),
  completedAt: Schema.optional(Schema.Date),
  // "Birth certificate" configuration can evolve without changing prompt code.
  ritualVersion: Schema.Int,
}) {}
```

Notes:

- Keep versions per doc to support evolution and to let the agent say "I updated your Soul" with a diff summary.
- Use `visibility` to enforce "main-only" memory mechanically (OpenClaw uses session filtering).
- Prefer structured fields for things we will query/update independently (timezone, name, checklist items).

## Storage Strategy (DB-Backed)

We have two viable stores in the current stack:

1. Durable Object SQLite (recommended canonical for Autopilot)
  - Pros: co-located with the chat thread (1 DO per user); fast; no extra infra.
  - Cons: harder to build admin/UIs that query across users; limited analytics.

2. Convex (optional mirror / control plane)
  - Pros: easy UI querying/editing, indexing, dashboards.
  - Cons: adds another source of truth; cross-service auth; eventual consistency.

Recommendation:

- Canonical: store bootstrap docs + bootstrap state in the **Chat durable object** (same place as transcript).
- Optional: mirror selected fields to Convex for UI (identity, user timezone, bootstrap status) if/when needed.

## Single-File Export (Portable JSON Bundle)

We must support letting a user export their entire Autopilot bootstrap configuration in **one export**.

Requirements:

- Export is a **single JSON object** containing the whole schema set together (bootstrap state + docs + memory + optional audit).
- Export must be validated against the **JSON encoding** of our Effect Schemas (not ad hoc JSON).
  - Practically: define a top-level `Schema` for the export format using *encoded* schemas (e.g. dates as ISO strings).
- Export format is versioned and documented so other systems (and other OpenAgents installs) can import/export consistently.

Proposed top-level schema (v1 sketch):

```ts
import { Schema } from "effect"

export class AutopilotBootstrapExportV1 extends Schema.Class<AutopilotBootstrapExportV1>(
  "AutopilotBootstrapExportV1"
)({
  format: Schema.Literal("openagents.autopilot.bootstrap.export"),
  formatVersion: Schema.Literal(1),

  // Use encoded schemas for JSON stability (example: DateFromString instead of Date).
  exportedAt: Schema.DateFromString,

  // Optional metadata for debugging/support (not used for semantics).
  app: Schema.optional(Schema.Struct({
    name: Schema.Literal("autopilot"),
    version: Schema.optional(Schema.String),
  })),

  // The "payload"
  bootstrapState: AutopilotBootstrapState, // encoded form
  docs: Schema.Struct({
    rules: AgentRulesDoc,
    ritual: BootstrapRitualTemplate,
    identity: IdentityDoc,
    user: UserDoc,
    soul: SoulDoc,
    tools: ToolsDoc,
    heartbeat: HeartbeatDoc,
  }),
  memory: Schema.Array(MemoryEntry),

  // Optional append-only changes/receipts (Phase 2+).
  audit: Schema.optional(Schema.Array(Schema.Unknown)),
}) {}
```

Import semantics (MVP):

- Decode with `Schema.decodeUnknown(AutopilotBootstrapExportV1)` (runtime validation).
- If `formatVersion` is older: run migrations, then validate again.
- Write into the canonical store (DO SQLite) in a single transaction.
- Treat import as "replace bootstrap set" for the user/thread (simplest), with a future option for merge/partial import.

## Bootstrap Flow (Autopilot)

OpenClaw triggers the ritual when `BOOTSTRAP.md` exists. In Autopilot, we trigger when `AutopilotBootstrapState.status != "complete"`.

### 1) Ensure "workspace" records exist (write-if-missing)

OpenClaw: `ensureAgentWorkspace({ ensureBootstrapFiles: true })`.

Autopilot: `ensureAutopilotBootstrapState(userId)` + `ensureDefaultDocs(userId)`:

- If no bootstrap state exists: create `status = "pending"`, seed default docs:
  - IdentityDoc: empty-ish defaults (or placeholders)
  - UserDoc: empty-ish defaults
  - SoulDoc: default "Core Truths" etc (adapted from OpenClaw SOUL.md)
  - ToolsDoc: default notes about available tools/surfaces
  - HeartbeatDoc: empty checklist
- Never overwrite existing docs; only create missing ones.

### 2) Prompt injection: build "bootstrap context"

On each turn:

1. Load docs for this user/thread.
2. Filter by session kind:
  - Main session: include everything allowed.
  - Subagent session (future): include only global rules + tools surface (OpenClaw allowlist: `AGENTS`, `TOOLS`).
  - Enforce memory visibility: `main_only` memory excluded from non-main sessions.
3. Apply hook overrides (see below).
4. Render to prompt context blocks (string/Markdown) and truncate each block with a `maxChars` limit (OpenClaw: 20k with head/tail + marker).
5. Build the final system prompt:
  - Base system prompt (global Autopilot rules)
  - + injected bootstrap docs ("context files")
  - + if `status != complete`: include "bootstrap ritual" instructions (the BOOTSTRAP equivalent)

### 3) Completing bootstrap (replaces "delete BOOTSTRAP.md")

In OpenClaw, the agent deletes `BOOTSTRAP.md`.

In Autopilot, the agent calls an internal tool like:

- `bootstrap.complete({})`

which:

- sets `AutopilotBootstrapState.status = "complete"`
- stamps `completedAt`
- optionally snapshots the finalized docs (or emits a "birth certificate" receipt)

## Update Surface: Tools + UI

OpenClaw updates files by editing Markdown. In Autopilot, we need a safe update mechanism.

### Agent-driven updates (tools)

Add internal tools that update specific records:

- `identity.update({ name?, creature?, vibe?, emoji?, avatar? })`
- `user.update({ name?, addressAs?, pronouns?, timeZone?, notes?, context? })`
- `soul.update({ coreTruths?, boundaries?, vibe?, continuity? })`
- `heartbeat.setChecklist({ checklist })`
- `memory.append({ kind, title, body, visibility })`

Each tool:

- validates input with Effect Schema
- writes a new version (or appends an update event) in DB
- emits a deterministic receipt: `{ tool, params_hash, output_hash, latency_ms, side_effects }`

### User-driven updates (UI)

Add a lightweight "Profile" panel later:

- shows identity + user profile + soul summary
- edits write through the same schema + receipt path

## Hook System (Bootstrap Overrides)

OpenClaw has internal hooks that can mutate the injected bootstrap files in-memory (e.g. `soul-evil` swaps soul content).

Autopilot should implement a similar seam:

- Event: `agent:bootstrap` (context: docs + session kind + user/thread id)
- Hook registry:
  - code-defined hooks (for safety-critical behavior)
  - optional DB-configured hook enablement (for experiments)
- Hooks can:
  - modify the injected rendered context (in-memory only)
  - or request additional context blocks

"Better than OpenClaw":

- Make hooks produce receipts and add them to the replay log (so we can explain why the prompt differed).
- Enforce ordering + timeouts (hooks must be deterministic and bounded).

## Truncation / Prompt Budgeting

We should adopt the OpenClaw truncation strategy (head + marker + tail) per doc.

Configuration knobs:

- `bootstrapMaxChars` (global default)
- optional per-doc max (Soul/Memory tends to grow)

Additionally:

- If a doc exceeds max repeatedly, prefer summarizing into a structured "compressed" form stored alongside the raw.

## Implementation Plan (Phased)

### Phase 1: Minimal DB-backed bootstrap state

- Add schemas + storage in DO
- Add `status: pending|complete`
- Inject a short ritual instruction when pending
- Add tools to update identity/user/soul and to `bootstrap.complete()`
- Render docs into system prompt (truncated)

### Phase 2: Versioning + audit

- Version increments per update
- Store an append-only change log (who changed what, when)
- Agent must announce when it changes Soul (per OpenClaw SOUL.md guidance)

### Phase 3: Hook seam

- Implement `agent:bootstrap` hook registry
- Add one trivial hook for validation/testing (e.g. inject "hook ok" marker)
- Add experimental hooks only after receipts/replay are solid

### Phase 4: Memory system

- Implement MemoryEntry storage + visibility enforcement
- Add `/new`-equivalent UX or button to snapshot memory
- Add long-term memory summary record (main-only)

## Testing / Verification

- Unit tests:
  - Schema decode/encode roundtrips for each doc type
  - Truncation logic (marker present, head/tail kept)
  - Visibility filtering (main vs subagent)
- Integration tests (worker):
  - New thread starts with `bootstrap.status = pending`
  - After calling update tools + `bootstrap.complete`, status persists and ritual instructions stop injecting
  - `get-messages` includes tool-call/tool-result parts for updates

## Open Questions

- Canonical store: DO-only vs DO+Convex mirror?
- How do we want the user to "edit Soul": form UI (structured) vs rich text editor (markdown-like)?
- How do we represent global rules (`AGENTS.md`)?
  - Keep in code (simpler), or store versioned in DB for hotfixes?
- Do we want a formal "bootstrap wizard" UI before chat, or keep it purely conversational?
