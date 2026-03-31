# Autopilot Coding Shell And Probe Direction

## Scope

This document explains the current Autopilot coding product shape in
`openagents` and the correct future boundary for Probe.

It answers four concrete questions:

- what Autopilot owns today
- how Codex is actually embedded today
- which parts must remain app-owned when the engine changes
- how Probe should replace or supplement Codex without becoming the product
  shell

Audit basis:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/codex/ROADMAP_CODEX.md`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/codex_exec.rs`
- `apps/autopilot-desktop/src/codex_remote.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `crates/codex-client/src/lib.rs`

## Bottom Line

Autopilot is already more than a thin Codex wrapper.

The desktop owns the user-facing coding shell:

- chat workspace selection
- thread rails and thread metadata
- workspace and project identity
- plan, diff, review, and compaction artifacts
- approval and tool-input queues
- remote companion projection
- OpenAgents-specific dynamic tools

Codex is currently the engine behind that shell.

Probe should take over that engine role over time, but it should not take over
Autopilot's product ownership. Autopilot remains the main rich surface for
coding inside OpenAgents. Probe becomes the owned runtime that can also run
outside the desktop product as a CLI or server-side agent.

## Current Autopilot Product Shape

## 1. The coding surface is part of the main product shell

`docs/MVP.md` already treats Autopilot as the main personal-agent surface
inside the broader desktop app. That shell is not isolated from the rest of the
product. It lives next to wallet, provider, data-market, CAD, and operator
surfaces because that cross-product context is part of the value of Autopilot.

That means the coding agent is not supposed to own:

- final wallet truth
- provider truth
- pane orchestration
- remote operator truth
- product-specific tool behavior

Those remain desktop responsibilities.

## 2. Chat, threads, workspaces, and projects are app-owned state

`AutopilotChatState` in `apps/autopilot-desktop/src/app_state.rs` is the center
of gravity for the coding shell.

It owns:

- active thread state and cached transcripts
- thread metadata and thread list filters
- selected workspace and browse mode
- project registry and workspace identity
- terminal session records
- saved plan, diff, review, and compaction artifacts
- pending approvals, tool calls, and tool user-input requests
- UI state for rails, menus, previews, and transcript behavior

That is not engine-owned UI glue. It is the app's durable product model for
coding work.

The important implication is simple: if the engine changes from Codex to Probe,
the shell does not disappear. The shell keeps owning thread/workspace/project
truth and projects whichever engine is active into the same product model.

## 3. The chat pane is already workspace-aware, not just transcript-aware

The chat system is not a single transcript widget. The app tracks multiple
workspace modes:

- Autopilot coding threads
- managed group chat
- managed system chat
- direct messages

Inside the Autopilot mode, thread rows and transcript status already project
workspace and project identity. The product therefore already thinks in terms
of:

- project-scoped coding work
- thread lifecycle
- operator-visible session controls
- durable artifacts attached to a coding session

That is the correct host model for Probe too.

## How Codex Is Embedded Today

## 1. `crates/codex-client` is the protocol seam

`crates/codex-client` is intentionally narrow. It is a JSON-RPC client over
stdin/stdout for the Codex app-server.

That means the current reusable seam is:

- protocol types
- request/response coverage
- notification normalization

The desktop does not embed Codex UI. It embeds a Codex runtime connection.

## 2. `codex_lane.rs` owns the live engine session integration

`apps/autopilot-desktop/src/codex_lane.rs` is the main desktop-side Codex lane.

It owns:

- connection and initialization
- readiness state
- model catalog publication
- thread bootstrap and resume
- request/notification routing
- approval handling
- tool-input handling
- thread list and loaded-thread refresh

The default config also shows the current product posture:

- connect on startup
- bootstrap a thread when needed
- identify as `openagents-autopilot-desktop`
- use app-owned dynamic tools on thread start

That is the current engine adapter layer. Probe eventually needs an equivalent
desktop-facing adapter, but it should emit into the same Autopilot shell model
instead of creating a parallel product surface.

## 3. OpenAgents-specific tools are injected from the app layer

`apps/autopilot-desktop/src/openagents_dynamic_tools.rs` defines the
product-specific dynamic tool set currently attached to Codex turns.

Those tools cover OpenAgents-owned surfaces such as:

- panes
- data market
- CAD
- treasury and wallet-adjacent actions
- provider control
- labor and claim workflows

This is one of the strongest proofs that Autopilot is already the product shell
and Codex is already the engine.

The engine is not the owner of those domains. The app decides which product
tools exist, when they are exposed, and how their results map back into product
truth.

Probe should preserve that separation.

## 4. Non-interactive execution is already app-owned

`apps/autopilot-desktop/src/codex_exec.rs` exposes `autopilot-codex-exec`.

That binary is important because it proves the current product direction is not
"desktop only" in a naive sense. The product already wants:

- one-shot local automation
- JSONL event streams
- ephemeral or persistent runs
- explicit cwd/model/sandbox/approval control

Probe should absorb this role long term, but the product requirement remains:
Autopilot needs an app-owned or app-supervised non-interactive execution lane.

## 5. Remote supervision is also app-owned

`apps/autopilot-desktop/src/codex_remote.rs` exposes the local-first remote
companion for the same desktop runtime.

The remote snapshot includes:

- thread summaries
- active-thread transcript
- approvals
- tool user-input requests
- session status
- saved artifacts
- wallet and provider truth
- workspace context
- terminal snapshot

That is not generic engine telemetry. It is the product's remote operator model.

So even if Probe gains its own server mode, Autopilot should keep owning the
remote projection that mixes coding state with wallet/provider/product truth on
the main machine.

## What Must Stay App-Owned In A Probe World

When Probe arrives, these contracts should remain Autopilot-owned:

- thread and project presentation
- workspace selection and browse-mode behavior
- plan/diff/review/compact artifact presentation
- wallet/provider/data-market/CAD tool exposure
- remote companion projection for the desktop product
- final pane orchestration and operator workflow

Probe can own the runtime internals that produce coding results, but it should
not become the owner of the surrounding product shell.

## What Probe Should Own

Probe should own the runtime concerns that currently sit behind Codex:

- session and turn execution
- transcript durability and recovery
- tool runtime and permission logic
- MCP and extension loading
- compaction and memory policy
- task and subagent orchestration
- standalone CLI
- standalone server / daemon

That gives OpenAgents an owned coding runtime without forcing the desktop app
to become the runtime home.

## Architecture Direction For Probe

## 1. Keep Autopilot as the rich host

Autopilot should remain the default user-facing product for:

- project-aware coding
- product-specific tools
- operator visibility
- remote supervision of the same desktop machine

That is already how the codebase thinks.

## 2. Give Probe its own deployable runtime shape

Probe should also stand on its own as:

- a CLI
- a local sidecar supervised by Autopilot
- a server-side runtime on dedicated machines

This is the main strategic gain from creating Probe as its own repo and runtime.

## 3. Borrow the right ideas from `opencode`

The strongest architectural lessons to carry forward are:

- per-workspace runtime instances rather than one giant global singleton
- a stable control plane between the runtime and its clients
- persistent sessions as first-class durable objects
- multiple clients over one engine boundary

Autopilot should be one of those clients, but it should be the richest
product-specific client.

## Recommended Transition Sequence

## Phase 1. Preserve the shell contracts

Before swapping engines, make the Autopilot-owned coding shell contracts
explicit:

- session lifecycle
- thread list and thread metadata
- workspace/project identity
- plan/diff/review/compact artifacts
- approvals and tool prompts
- remote snapshot fields

Those contracts should not depend on Codex-specific naming where avoidable.

## Phase 2. Add Probe as a second runtime lane

Do not rip out Codex immediately.

Add Probe as a second runtime path that can drive the same shell concepts:

- start or resume a session
- stream items and assistant output
- surface approvals and tool calls
- persist artifacts
- expose workspace/project identity

That lets the app compare both lanes honestly before a full cutover.

## Phase 3. Move standalone automation to Probe

Once Probe is credible, the owned standalone paths should shift there:

- CLI automation
- long-running server deployment
- non-desktop coding workers

Autopilot then becomes the primary product shell and supervisor rather than the
only place the runtime can exist.

## Final Recommendation

Treat the current Codex integration as proof that the right architecture is:

- Autopilot owns the product shell
- the engine sits behind a stable runtime seam
- product-specific tools stay app-owned
- remote product truth stays app-owned

Probe should inherit the engine role and expand it into CLI and server
deployments, but it should not dissolve Autopilot into a generic runtime
viewer.
