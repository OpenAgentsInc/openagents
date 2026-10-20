# AgentGraph after Unit

Recorded 2026-10-19, at `7150d98373`. Owner request: look at Unit and our
earlier audits of it, find what we said AgentGraph should be, and redo the
Foreman map at `/agent/foreman` as a GPU-drawn graph, repeatable for any
agent.

## Summary

- **AgentGraph** is now a pure Rust spec (`crates/agent-graph`,
  `openagents.agent-graph.v1`) that any Agent Lab definition runs through,
  and a GPUI view (`apps/openagents/web/agent-graph`) that draws any spec in
  a browser.
- `/agent/foreman` draws Foreman's graph with GPUI once the module loads.
  The server map and cards stay as the first paint, the fallback, and the
  text that screen readers and search read.
- From Unit we took the model (units, typed pins, graphs as units) and parts
  of its look. We took no code and none of its language, runtime, or
  standard library.

## Unit

[Unit](https://github.com/samuelmtimbo/unit) is "a General Purpose Visual
Programming Language and Environment" by Samuel Timbó, in TypeScript. Its
license file (`LICENCE`) is the MIT License, Copyright 2021 UNIT IO, Inc. We
read it at `7d55830db7` (2026-10-14).

- A **unit** is a multi-input, multi-output state machine with typed input
  and output **pins**. A **graph** wires units together with **merges**, and
  it **exposes** pins of its own, so a graph is itself a unit.
- A spec is JSON: `units`, `merges`, `inputs`, `outputs`, and `metadata`,
  which holds positions.
- The editor (`src/system/platform/component/app/Editor/Component.ts`) draws
  units as circles and components as rectangles, pins as circles of radius 5
  (`PIN_RADIUS`), links of 3 px that meet a node's surface, and live values
  as datum nodes beside the pins. A force simulation without d3 lays out the
  graph, and the wheel zooms by `exp(delta × 0.05)`.
- The palette is grayscale first, with accents by mode: green to add, red to
  remove, blue to change, and yellow for what's active.

## What our earlier audits planned

| Record | Plan | What happened |
| --- | --- | --- |
| `dc71c1bb74` (2026-06-30), Unit as a 2D visual layer | Adopt Unit's npm package for a fleet graph, a Verse overlay, and shareable diagrams | Rejected for a multi-megabyte bundle, required code generation, and a second reactive runtime |
| `2463494817`, `ba3299933b`, the Arbiter audit | Build our own: Unit's MIMO units, typed pins, merges as links, JSON with positions in metadata, a small force layout, and a link that lights only when a receipt backs it. Skip the language, the standard library, and the web OS | A TypeScript `arbiter-effect` package and a Gym graph pane (`8ab6bddb21` to `a6ff96321b`); retired 2026-07-08 and deleted in `e1fbd1c185` |
| `38347f6128` (2026-07-09), Blueprint Map | A live Blueprint map beside Sarah's chat, fed by `blueprint_delta` events | Epic #8626 and BM-1 to BM-5 (#8627 to #8631), all closed |
| openagents.com `docs/2026-08-20-scv-swarm-visualization-unit-audit.md` and `components/graph.ex` | Port Unit's look to HEEx: circles and rectangles, surface links, arrowheads, Unit's constants, the site's tokens; a staged layout instead of the force simulation | Shipped in that repository as `OpenAgentsWeb.UI.Graph`, credited to Unit with no code copied |

## What AgentGraph meant

- **2023, episodes 038 to 073.** AgentGraph was our node-graph library for
  agent workflows, after litegraph.js and Leva: a flow such as validate,
  embed, search the knowledge base, and answer, shown as cards anyone could
  open to "see exactly what's powering" an agent, edit, and share
  (`docs/transcripts/038.md`, lines 55 to 169). Episode 041 synced it with
  the database read-only; episode 056 deleted the React version; episode 073
  planned our own version without React.
- **The transcript index** groups episodes 033 to 047 under inspectability:
  "visual step-by-step agent execution" that makes intermediate work visible
  as part of what an open agent means (`docs/transcripts/README.md`).
- **2026, episode 250.** Delegated children and transcripts "must remain
  inspectable inline and in the agent graph" (`docs/transcripts/250.md`,
  line 78).
- The 2023 React code (`agentgraph/`, `resources/js/agentgraph/`) was removed
  in `51c057a557`. The TypeScript `openagents.live_agent_graph.v1` schema of
  CUT-11 (#8691) is gone from `main`. Nothing on `main` was named AgentGraph
  before this change.

The common thread is a graph you can open on any agent to see what it is
made of and how data moves through it.

## What we built

| Part | Path | What it does |
| --- | --- | --- |
| Spec | `crates/agent-graph/src/lib.rs` | Units with typed pins, a `parent` for nesting, tags, and details; links from an output pin to an input pin of the same data kind; `feedback` for data that comes back around; `validate` |
| Layout | `crates/agent-graph/src/layout.rs` | Deterministic: top-level units in columns by the longest path of links, feedback and cycles drawn as loops, children stacked inside their graph unit |
| Derivation | `crates/agent-graph/src/lab.rs` | `lab::of(experiment, id, notes)` for any Agent Lab experiment: runtime and model, instructions with each skill inside, check-ins, the places it reads, tools by group through its authority, memory, the stall ladder with its rungs, and the suite |
| Foreman | `crates/agent-graph/src/foreman.rs` | `foreman::graph()`: the lessons each part answers, the replay, and what we measure |
| View | `apps/openagents/web/agent-graph` | GPUI over `openagents-gpui-web`: pan by dragging, zoom with the wheel or the buttons, drag a unit, select a unit to read it in the side panel |
| Definition | `crates/agent-graph/src/definition.rs` | The whole definition as one document, schema `openagents.agent-definition-export`, `version` 1: the Agent Lab experiment as stored (instructions, every skill's full text, runtime, model, access, autonomy, suite), plus the tools, commands, and places its text names, its grants, its stall ladder, and notes such as lessons and measures |
| Page | `crates/openagents-web/src/pages/foreman.rs` | `/agent/foreman` with the graph's JSON in the page; `/agent/{id}`, `/agent/{id}/graph.json`, and `/agent/{id}/definition.json` for Agent Lab templates |
| Toolbar | `apps/openagents/web/agent-graph/start.js` | Full screen (the Fullscreen API, or filling the window where it is missing, as on iPhone; Esc or the button leaves), Copy JSON, and Download JSON (`foreman.agent.json`). Download JSON is a plain link, so it works without script or a GPU |

From Unit we ported:

- The model: units, typed input and output pins, links between pins, and
  units nested inside graph units.
- Pins on the unit's edge with Unit's radius of 5, rounded units, and data
  shown on links. Unit shows a datum beside a pin; we move small pieces of
  data along the link instead, which reads better at a distance.
- A dark, mostly gray palette with one accent per part, taken from the
  site's tokens instead of Unit's mode colors.

We left out:

- Unit's language, runtime, standard library, and web OS. Our graph runs
  nothing; it describes a definition.
- The force simulation. A fixed layout opens the same way every time, as
  the openagents.com audit also chose.
- Merges as their own shapes, exposed pins, editing, and the minimap.

### Measured

- Module: 5,412,461 bytes raw, 2,141,944 gzip, 1,508,713 brotli (`wasm-opt
  -Os`), against 2,635,798 brotli for `/demo/gpui`. It bundles Geist
  (regular, medium, semibold) and Geist Mono only.
- Headless Chrome with WebGPU at 1440 × 900 and 390 × 844: the graph draws;
  drag pans, the wheel zooms around the pointer, a click or tap selects a
  unit and fills the panel, and dragging a unit moves it with its children.
  No console errors.

## Drawing and editing in the desktop app

The desktop's Agent Lab has a Graph tab (#12231) that draws the draft the
form edits, with the same GPUI view as the web
(`apps/openagents/crates/agent-graph-view`). Select a part, then use the
panel's changes: turn check-ins on or off and change how often, ask for
standing authority or stop, change the stall window, add or remove a tool,
a place it reads, or a skill.

Each change is an `agent_graph::edit::Edit`. `edit::plan` turns it into the
form's own steps, a field change or an action by its key, and the desktop
sends them through the same `Lab` the form uses, so the change passes the
same checks and the form shows it at once. Because tools and places are read
from the agent's text, adding a tool names it on a "Tools I use:" line at
the end of the instructions, and removing one takes the code formatting off
every mention, so the agent no longer names it. The Agent Lab form gained
the controls these need: check-ins, minutes between them, standing
authority, and the stall window.

## Add a graph for a new agent

1. Publish the agent as an Agent Lab template: add it to
   `agent_lab::templates::offered()`.
2. It now has a graph: `agent_graph::lab::of(&experiment, id,
   &Notes::default())`, a page at `/agent/{id}` (its published ID or its
   title in lower case with dashes), and `/agent/{id}/graph.json`.
3. Name its tools in code formatting in its instructions or skills, such as
   `` `read_chat` ``. The graph lists a tool, command, or place only while
   the agent's text names it. A new app tool goes in
   `agent_graph::catalog::TOOLS`.
4. For what a definition can't say about itself, such as lessons or
   measures, pass `Notes` with tags, extra units, and links, as
   `agent_graph::foreman::notes()` does.
5. Run `cargo test -p agent-graph`. Add a test like
   `foremans_graph_is_valid_and_names_every_tool_its_definition_names`.

The view needs no change for a new agent.

## Follow-ups

- [#12229](https://github.com/OpenAgentsInc/openagents/issues/12229) Light
  links from real agent events.
- [#12230](https://github.com/OpenAgentsInc/openagents/issues/12230) Keyboard
  and screen reader access to the canvas.
- [#12231](https://github.com/OpenAgentsInc/openagents/issues/12231) Draw and
  edit agents in the desktop app.
- [#12232](https://github.com/OpenAgentsInc/openagents/issues/12232) Pinch
  zoom and page scroll on phones.
- [#12233](https://github.com/OpenAgentsInc/openagents/issues/12233) Share
  font files with `/demo/gpui`.
