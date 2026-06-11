# Team And Shared Memory System Audit

Date: 2026-06-11

This is system #41 from the Bun/Effect terminal-agent systems list. It defines
how terminal-agent memory can be shared across users, teams, missions,
repositories, and agents without leaking private context or relying on
keyword-only routing.

## Target

Build a shared memory system that stores durable lessons, preferences, repo
facts, review patterns, budget caveats, and accepted-work evidence as typed
records with scope, provenance, redaction class, and retrieval policy.

Shared memory should help future work while staying auditable and removable.

## User-Visible Capability

Users and teams should be able to:

- Save a useful fact from a run.
- See why a memory item was applied.
- Scope memory to personal, team, repository, project, mission, or public
  context.
- Remove or correct a memory item.
- Prevent sensitive memories from being reused.
- Distinguish accepted facts from tentative notes.
- Search memory semantically and by typed fields.

Memory should never be invisible magic. Applied memory needs a ref in the run
context and should be visible in debug surfaces.

## Memory Record Model

Each memory record should include:

- Memory ref.
- Scope.
- Kind.
- Statement.
- Evidence refs.
- Confidence or review state.
- Visibility.
- Redaction class.
- Owner or team ref.
- Retrieval policy ref.
- Expiration or review date.
- Created, updated, and applied timestamps.

Record kinds should include accepted fix, denied path, build command, flaky
test, reviewer preference, repo style, run caveat, provider caveat, budget
caveat, product policy, and onboarding note.

## Bun/Effect Boundary

Use Effect services for:

- `SharedMemoryService`: create, update, delete, and read memory records.
- `MemoryRetrievalService`: typed and semantic retrieval for context assembly.
- `MemoryProjectionService`: public, team, and private views.
- `MemoryConsentService`: scope changes and promotion review.
- `MemoryAuditService`: application receipts and deletion receipts.

Use Schema for memory kinds, scopes, review states, visibility, and retrieval
queries. Use an embedding-backed or typed semantic selector for broad
retrieval. Do not add ad hoc keyword routing for memory application.

## Safety Rules

- Private memories do not cross into team or public context automatically.
- Raw prompts, raw logs, private repo content, secrets, provider payloads, and
  customer data are not stored as memory text.
- A memory applied to a run is referenced in the context snapshot.
- A memory that changes behavior must be explainable by kind, scope, and
  evidence ref.
- Deletion tombstones prevent stale rehydration from caches.
- Team memory promotion requires owner/team policy.
- Public memory projection requires redaction and freshness checks.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has repo memory, team rooms, mission records, and
semantic-only retrieval requirements in adjacent Autopilot surfaces. The
terminal-agent README does not yet include a team/shared memory audit.

Related open issue anchors:

- #4778 mission/work-order unification should keep shared memory on the same
  record spine as missions and work orders.
- #4770 team budgets and spend-to-evidence join needs team-scoped memory and
  caveats for future routing.
- #4769 repo connect and per-mission data-scope UX should show which memory
  scopes are enabled.

No terminal shared-memory claim should be green until retrieval, projection,
consent, deletion, and application receipts exist.

## Tests

Minimum coverage:

- Create memory at every supported scope.
- Retrieve by typed query and semantic query.
- Reject keyword-only routing for broad intent selection.
- Project public, team, and private views.
- Apply memory into context with provenance refs.
- Delete memory and prevent cache reappearance.
- Promote personal memory to team only through approval.
- Scan memory text for forbidden private material.

## Decision

Shared memory should be a typed, scoped knowledge layer with explicit
application receipts. It is not a hidden prompt file and not an excuse to
smuggle private run context into future work.

