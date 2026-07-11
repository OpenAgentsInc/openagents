# OpenAgents Sub-Agent Rendering — Design Frame and First Desktop Slice

Date: 2026-07-10
Author: Fable (agent).
Status: **ACTIVE DESIGN; FIRST DESKTOP SLICE IMPLEMENTED.** The original frame
below remains the cross-surface design agenda. Section 6 records the first
implemented interaction and the boundary it establishes.

Input / prerequisite reading:
`docs/teardowns/2026-07-10-codex-subagents-rendering-analysis.md` (the Codex
sub-agent data model, protocol, and TUI-vs-desktop rendering gap).

---

## 1. The lesson we are designing against

Codex proves that a genuinely rich sub-agent protocol can still render badly when
one client surface can't hold the shape of the data. Concretely (from the
analysis):

- Codex's terminal TUI and its desktop app consume the **same** app-server
  protocol (same `ThreadItem` / `ServerNotification` stream). Neither is
  privileged on data.
- The **terminal under-renders**: one active thread's transcript at a time,
  sub-agents flattened into one-line edge rows + a hard-capped 6-item "Sub-agents
  running" peek + switch-to-view navigation. The parent/child graph that its own
  `agent-graph-store` maintains is never drawn.
- The **desktop app over-renders** the identical protocol because a windowed GUI
  is dimensionally unconstrained.

Result: a real capability gap *by rendering surface* — "you kind of have to use a
desktop app for some things." That split is the anti-pattern.

## 2. Our premise: no capability gap by construction

OpenAgents' architecture is supposed to make that split impossible:

- **Typed catalog + one honest projection.** A sub-agent run should be one typed,
  serializable projection (lifecycle, parent/child edges, per-child state,
  streaming transcript refs). Every surface renders *that same projection*.
- **One renderer per surface, same catalog.** Under the Effect Native direction,
  desktop / mobile / web are swappable renderers over one typed component set —
  so the *same* sub-agent projection should render honestly everywhere. No
  surface should be forced to flatten the tree just because it's a terminal; our
  surfaces are GUIs by default.
- **Khala Sync as the transport of truth.** Sub-agent topology and state are
  sync-projected scopes, not per-client reconstructions. A late-joining or
  reconnecting client rehydrates the full tree from the projection, rather than
  replaying a lossy event peek.

The design bet: **the same projection must render with equal fidelity on every
surface.** If any surface has to truncate or drop structure to fit, that's a
projection or renderer bug, not an acceptable tier.

## 3. Key questions to answer (the actual work)

1. **What is the sub-agent projection?** The minimal typed shape: agent identity
   (stable path/id), parent edge + depth, lifecycle status (running / waiting /
   done / errored / interrupted), effective model + reasoning config, a handle to
   the child's own transcript scope, and inter-agent messages. (Codex's
   `CollabAgentToolCall` + `SubAgentActivity` + `agents_states` map + the
   `agent-graph-store` tree is a concrete reference for the fields.)
2. **How do we render concurrency without a "switch to view" downgrade?** Live
   multi-panel? A spawn tree with expandable per-child transcripts? What is the
   mobile form of a tree of concurrently-streaming agents?
3. **In-progress fidelity.** Codex renders nothing for an in-flight spawn until it
   resolves. Our default must show a live child the moment it exists. What's the
   streaming contract from Khala Sync that makes that cheap?
4. **Where does the topology live?** A Khala-synced graph scope (depth + BFS
   descendants) vs. per-client reconstruction. Persisted, resumable, shared across
   an operator's devices.
5. **What is the honest degradation, if any?** If a surface genuinely cannot show
   N concurrent transcripts (small screen), the degradation must be *explicit and
   navigable* (collapse with a live status badge), never silent truncation to a
   fixed 6-item cap.
6. **Model/reasoning/personality per child.** Like Codex post-5.6, each sub-agent
   resolves its own model + reasoning-summary + personality. The projection should
   record each child's *effective* config so every renderer can label it honestly.

## 4. Explicit non-goals of this doc

- Not choosing a widget library, layout, or component names.
- Not defining the wire schema or Khala Sync scope shape.
- Not a commitment to mirror Codex's tool set (`spawn / send / wait / resume /
  close`) — that's one reference point, not our contract.

## 5. Original next step

The first decision from §3 is now recorded below. The remaining questions still
need a full live/Sync projection design; the historical Desktop slice is an
interaction contract, not a substitute for that authority model.

## 6. First implemented interaction: inline child activity

The Desktop historical Codex trace now separates two complementary views of the
same child graph:

- The right inspector remains the complete topology and the place
  to scan or navigate every discovered descendant.
- The parent timeline renders a confirmed child-start edge as an elevated
  subagent card at the causal point where the child began. The card shows the
  child's lifecycle state and one bounded latest-activity preview, and opens the
  child's full thread directly.

This is deliberately a **link projection**, not transcript flattening. The
parent item carries an exact child thread ref from the persisted Codex event;
the preview is read from that child's own bounded history tail, redacted through
the same history projector, and capped to 360 characters. Later interaction
events do not create duplicate launch cards. If the referenced history is
absent, the source item states that absence rather than fabricating a child.

The typed `TimelineEvent` contract now supports an event-local intent and an
`agent` semantic variant in both DOM and React Native renderers. That makes the
interaction portable: desktop gets the spacious inline card now, while mobile
can lower the same exact edge, state, preview, and navigation intent into a
compact disclosure without inventing a second subagent model.

Implementation and enforcement live in:

- `apps/openagents-desktop/src/codex-history.ts` for exact edge enrichment and
  the bounded child preview;
- `apps/openagents-desktop/src/renderer/history-workspace.ts` for the inline
  projection and typed child navigation;
- `apps/openagents-desktop/tests/codex-subagent-history.test.ts` and
  `src/renderer/history-workspace.test.ts` for data and rendering oracles;
- Effect Native demand `D-DESK-07` for the cross-renderer primitive.

The next slice should replace historical tail sampling with the same shape fed
by a live Runtime Gateway/Sync child projection. The UI contract should remain
unchanged: exact identity, explicit lifecycle, bounded latest activity, and
direct access to the independent child transcript.
