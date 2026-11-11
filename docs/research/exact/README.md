# Exact: research and what Coax should take from it

Expo released Exact on 2026-11-01. It is a cross-platform app framework
written in Rust and designed for AI agents. This folder covers what Exact
is, how it compares with Coax and Coaxial, and what we should adapt.

The reference clone is at `projects/repos/exact` in the workspace root
(HEAD `94b8671`, 2026-11-09). The templates repo is beside it at
`projects/repos/exact-templates`. Both are read-only references.

| File | What it covers |
| --- | --- |
| [01-what-exact-is.md](01-what-exact-is.md) | Every point in the announcement thread, checked against the source |
| [02-coax-comparison.md](02-coax-comparison.md) | Each Exact capability set against what Coax and Coaxial have today |
| [03-what-to-adapt.md](03-what-to-adapt.md) | What to build in Coax, in order, and what to leave out |
| [04-apps-audit-2026-11-10.md](04-apps-audit-2026-11-10.md) | The apps work of 2026-11-10 (component system, Linux host, agent stack) checked against this research |

## Summary

Exact has three main parts:

- **Contract**, a small typed language for views, state and actions.
- **The plan**, the compiled form of a Contract app: tables plus bytecode.
- **Hosts** that run the plan: the browser DOM, AppKit, UIKit, a GPU
  painter called Ink, Android, Windows and a terminal.

The parts that matter most for us are not the UI language. They are the
agent loop around it:

- One driver with ten operations that works the same on every host.
- A clock that the agent controls.
- Tests written as scripts of driver operations.
- Compiler errors that teach the fix.
- A refusal to drive a stale build.
- Typed `tool` declarations that an outside agent can call through WebMCP.

Coax already has much of the hard part. It has a constrained, checked
language, diagnostics with stable codes, typed clocks, replay that matches
bit for bit across hosts, effects inventories, mutation testing, emitted
shaders, and hot reload keyed on the state layout. What Coax lacks is the
layer that lets an agent operate a running app:

- one driver, with targets named by stable ids;
- a clock the driver controls in every host;
- test scripts that replay on every host;
- a retained UI tree, so `tree` and `layout` have something to answer;
- a first frame baked at compile time.

[03-what-to-adapt.md](03-what-to-adapt.md) orders this into nine phases of
work. Its review section adapts the plan to worlds: the driver targets world
entities, agent tools are deeds, the agent clock steps the room too, and drive
scripts are replay streams. The first three phases are the agent driver, the
agent clock and drive scripts. They reuse what Coaxial already has (`?probe`, `shot`,
`coaxial.*` probes, the layout trace, the deed pipeline, the offline room and
`replay-hosts`) and pay off in every realm and app at
once.

## What was verified

- The repository facts were read from source, with file paths. Examples
  include the pass structure, plan format, diagnostic ids, driver
  operations, clock implementation, OTA format and the `tool` grammar.
- The benchmark claims in the thread were not reproduced here. These are
  the Pokédex web benchmark, 120 fps against SwiftUI, and startup against
  the Shop app. The repository does contain the harnesses:
  `apps/realworld`, `bench/board/` and `host/web-js/build.mjs` size tables.
