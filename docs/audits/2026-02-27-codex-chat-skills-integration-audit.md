# 2026-02-27 Codex Chat + Skills Integration Audit

## Scope

This audit covers:

- Current Codex-related state in this repo (`/Users/christopherdavid/code/openagents`).
- Archived Codex-related code in backroom (`/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`).
- Current upstream Codex app-server v2 surface in (`/Users/christopherdavid/code/codex`).
- A concrete recommendation for how to wire Codex into the current basic chat pane and skill flow for testing/registration.

## Executive Summary

- The current MVP repo has **no live Codex app-server integration**. Chat is a local simulated state machine.
- We do have a local `skills/` registry and SKL/Nostr derivation wiring, but the current UI path does not query Codex skills or send Codex `skill` input items.
- Backroom contains enough reusable code to bootstrap quickly (`crates/codex-client` and some desktop wiring patterns), but it is behind current Codex protocol v2 on skills-related fields.
- To test skills via Codex in this repo, we should restore a **minimal local Codex lane** (worker + typed commands) and avoid restoring the old runtime sync stack.
- To fully support Codex-backed skill registration/testing, we must sync a small set of protocol updates from `~/code/codex`:
  - `skills/list` params/response updates
  - `skills/config/write`
  - `UserInput::Skill` (and ideally `UserInput::Mention`)

## Current OpenAgents State (Main)

### 1) Chat pane is local simulation, not Codex-backed

Key files:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/panes/chat.rs`

What exists now:

- `AutopilotChatState::submit_prompt()` appends a user message plus a synthetic queued Autopilot message.
- `AutopilotChatState::tick()` transitions `queued -> running -> done` on timers.
- No thread start/resume, no turn start, no streaming delta processing, no Codex notification handling.

### 2) Skill pane is wired to local simulated runtime lanes

Key files:

- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `apps/autopilot-desktop/src/input/reducers/skl.rs`
- `apps/autopilot-desktop/src/panes/skill.rs`
- `apps/autopilot-desktop/src/skills_registry.rs`

What exists now:

- SKL lane validates and derives from local `skills/` files.
- `SubmitSkillSearch` currently searches local filesystem skill slugs, not Codex app-server.
- Pane actions (`Discover`, `Inspect Manifest`, `Install Skill`) drive internal lane commands, not Codex APIs.

### 3) This repo currently has no Codex app-server client code

- No `AppServerClient` usage.
- No local crate equivalent to old `crates/codex-client`.
- No worker lane for Codex request/notification streaming.

## Backroom Codex Inventory

### Reusable core crates

- `crates/codex-client`
  - JSON-RPC client over stdio for Codex app-server.
  - Includes methods for `thread/*`, `turn/*`, `skills/list`, account, model, mcp status/login.
- `crates/openagents-codex-control`
  - Worker request/receipt parsing and targeting logic for runtime shared-worker control flows.

### Archived desktop integration surfaces

- `apps/autopilot-desktop/src/main.rs`
  - Full legacy integration (AppServer spawn, session/thread mapping, turn dispatch, notification loop).
- `apps/autopilot-desktop/src/codex_control.rs`
  - Auto-response helpers for server-initiated requests.
- `apps/autopilot-desktop/src/runtime_codex_proto.rs`
  - Runtime sync protocol parsing for shared worker event streams.
- `apps/autopilot-desktop/src/runtime_auth.rs`
- `apps/autopilot-desktop/src/sync_apply_engine.rs`
- `apps/autopilot-desktop/src/sync_checkpoint_store.rs`
- `apps/autopilot-desktop/src/sync_lifecycle.rs`

### Other codex-related archived artifacts

- `crates/openagents-client-core/src/codex_control.rs`
- `crates/openagents-client-core/src/codex_worker.rs`
- `proto/openagents/codex/v1/*`
- `proto/openagents/protocol/v1/codex_*`
- `generated/rust/openagents/codex/v1/openagents.codex.v1.rs`

## Backroom vs Current Codex Protocol (Drift)

Reference surface reviewed from `~/code/codex`:

- `codex-rs/app-server-protocol/src/protocol/common.rs`
- `codex-rs/app-server-protocol/src/protocol/v2.rs`
- `codex-rs/app-server/README.md`
- `codex-rs/app-server/tests/suite/v2/skills_list.rs`

### Skills-specific drift that matters for this repo

1. Backroom `codex-client` only models a partial `skills/list` shape.

- Missing `perCwdExtraUserRoots` in `SkillsListParams`.
- Missing `enabled`, `interface`, and `dependencies` in `SkillMetadata`.

2. Backroom `codex-client::UserInput` only supports `Text`.

- Missing `UserInput::Skill { name, path }`, which is the explicit skill attachment path for turns.
- Missing `UserInput::Mention` support.

3. Backroom `codex-client` does not expose `skills/config/write`.

- Codex v2 supports user-level enable/disable by SKILL.md path.

## How We Should Plug Into the Current Basic Chat Pane

### Design goal

Keep the existing MVP desktop architecture (single app crate + lane workers) and add one focused Codex lane that powers chat and skills testing.

### Recommended integration shape

1. Add a dedicated Codex worker lane in `apps/autopilot-desktop`.

- Pattern: mirror Spark worker style (thread + internal tokio runtime + command/update channels).
- Lane owns app-server process lifecycle and notification stream.

2. Keep chat UI mostly unchanged, replace synthetic execution with lane commands.

- On app startup: initialize app-server and open/bootstrap thread.
- On send: dispatch `turn/start`.
- Map notifications to message state:
  - `turn/started` -> running
  - `item/agentMessage/delta` -> append assistant text
  - `turn/completed` -> done
  - `turn/error` -> error

3. Wire skill pane to Codex `skills/list`.

- `Discover` should call `skills/list` for current repo cwd.
- Pass `perCwdExtraUserRoots` pointing to `<repo>/skills` so Codex scans this project-owned registry without changing global user skills.
- Show Codex-returned `enabled/scope/path` in pane state.

4. Support explicit skill usage in chat turns.

- Persist selected skill from skill pane into chat context.
- When sending a turn, include both:
  - `UserInput::Text`
  - `UserInput::Skill { name, path }`

5. Optional but recommended: support enable/disable via `skills/config/write`.

- Use selected skill path.
- Refresh `skills/list` after write to confirm effective state.

## What We Should Restore From Backroom (Minimal)

Restore now:

1. `crates/codex-client` as a starting point.
2. Codex lane architecture pattern from backroom desktop main loop (not the full file).
3. Small utility logic from `codex_control.rs` only if we decide to auto-handle server requests.

Do not restore now (too heavy for MVP chat/skills scope):

1. Runtime sync stack (`runtime_auth.rs`, `runtime_codex_proto.rs`, `sync_*`).
2. Shared worker control stack (`openagents-codex-control`, codex proto relay flows).
3. Legacy broad desktop orchestration from backroom `main.rs`.

## What We Still Need From Raw Codex (`~/code/codex`)

We do **not** need to pull app-server internals; we need to align our client types/methods with upstream protocol v2.

Required pulls/sync:

1. Protocol type parity for:

- `UserInput::Skill` and `UserInput::Mention`
- `SkillsListParams` including `perCwdExtraUserRoots`
- `SkillMetadata` including `enabled`, `interface`, `dependencies`
- `SkillsConfigWriteParams` and `SkillsConfigWriteResponse`

2. Method surface parity for:

- `skills/list`
- `skills/config/write`
- (optional next step) `skills/remote/list`, `skills/remote/export`

3. Behavior parity tests for skills lane:

- Relative extra roots rejected.
- Unknown-cwd extra roots ignored.
- Cache reuse vs `forceReload` behavior.

## Proposed Implementation Sequence

1. Introduce `crates/codex-client` (restored + updated for current v2 skills/user-input types).
2. Add `codex_lane.rs` to `apps/autopilot-desktop` with command/update channels and app-server lifecycle.
3. Replace chat submit simulation path with lane-backed turn dispatch and notification-driven transcript updates.
4. Rewire skill pane actions to Codex `skills/list` + local `skills/` root injection via `perCwdExtraUserRoots`.
5. Add explicit skill attachment in chat turn input (`UserInput::Skill`).
6. Add `skills/config/write` action support (optional in same phase if fast).
7. Add focused tests for:
  - lane startup failure behavior
  - message state transitions from notifications
  - skills discovery integration against a fixture skill root

## Acceptance Criteria

1. Chat pane sends real `turn/start` requests and renders streaming assistant output from Codex notifications.
2. Skill pane discovery reflects Codex `skills/list` output for this repo’s `skills/` folder.
3. Selecting a skill and sending a message includes explicit `UserInput::Skill` in the turn request.
4. No runtime sync/remote worker stack is required for local desktop chat+skills testing.

## Risks and Mitigations

- Risk: protocol drift between our local `codex-client` types and upstream Codex changes.
  - Mitigation: add a small parity checklist test against method/type contracts from `app-server-protocol` v2.
- Risk: chat pane complexity regressions from adding async lane behavior.
  - Mitigation: keep lane boundary strict (commands in, updates out), avoid adding Codex logic directly in render/input files.
- Risk: confusion between Nostr SKL registry flows and Codex local skills flows.
  - Mitigation: keep explicit source labels in pane state (`source: codex` vs `source: nostr/runtime`).
