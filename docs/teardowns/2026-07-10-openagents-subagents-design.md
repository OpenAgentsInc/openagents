# OpenAgents Sub-Agent Rendering — Early Design Frame

Date: 2026-07-10
Author: Fable (agent).
Status: **EARLY FRAME, NOT A SPEC.** This is a starting point for framing "what
OpenAgents' version of sub-agent rendering should be," written as an immediate
follow-on to the Codex analysis. It raises the questions; it does not answer them.

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

## 5. Next step

Turn §3's questions into a real design doc once we've decided the sub-agent
projection's typed shape and picked the first surface to render it. Use the Codex
analysis as the "what good/rich looks like, and how it goes wrong" reference.
