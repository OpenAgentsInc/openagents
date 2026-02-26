# 2026-02-26 Codebase Code Smell Audit

## Scope

- `apps/autopilot-desktop`
- `crates/spark`
- `crates/nostr/core`
- `crates/wgpui`
- Workspace root (`Cargo.toml`, lint/dependency posture)

## Method

- Static review of code structure and responsibility boundaries.
- Size/shape metrics (`rg`, `wc -l`) across all Rust files.
- Build/lint posture checks:
  - `cargo check --workspace` (passes)
  - `cargo clippy -p wgpui --all-targets -- -D warnings` (fails: 424 errors)
  - `cargo clippy -p autopilot-desktop --all-targets -- -D warnings` (blocked by `wgpui` lint debt)

## Topline Assessment

The codebase is functional for MVP iteration, but maintainability risk is concentrated in two places:

1. `wgpui` has become a very large, mixed-responsibility crate.
2. `autopilot-desktop` currently executes wallet/network calls synchronously inside UI event handlers.

This combination is the main source of future slowdown and fragility.

---

## Findings (Ranked)

### 1) Critical: `wgpui` is a monolith with responsibility bleed

**Evidence**

- Rust LOC by surface:
  - `crates/wgpui`: **100,917**
  - `apps/autopilot-desktop`: 2,205
  - `crates/spark`: 395
  - `crates/nostr/core`: 236
- Large single files:
  - [`crates/wgpui/src/platform/ios.rs`](/Users/christopherdavid/code/openagents/crates/wgpui/src/platform/ios.rs:1) (5,492 lines)
  - [`crates/wgpui/src/components/hud/frame.rs`](/Users/christopherdavid/code/openagents/crates/wgpui/src/components/hud/frame.rs:1) (1,439 lines)
  - [`crates/wgpui/src/animation/animator.rs`](/Users/christopherdavid/code/openagents/crates/wgpui/src/animation/animator.rs:1) (1,344 lines)
- iOS platform layer contains application-domain constructs (Codex/mission state), not just platform rendering glue:
  - [`crates/wgpui/src/platform/ios.rs:56`](/Users/christopherdavid/code/openagents/crates/wgpui/src/platform/ios.rs:56)
  - [`crates/wgpui/src/platform/ios.rs:140`](/Users/christopherdavid/code/openagents/crates/wgpui/src/platform/ios.rs:140)

**Why this is a smell**

- Ownership boundaries are unclear.
- Platform abstraction mixes with product semantics.
- Compile/test/lint cycles are much costlier than needed for MVP desktop iteration.

**Refactor suggestions**

- Split `wgpui` into:
  - `wgpui-core` (geometry/input/scene/theme/text primitives)
  - `wgpui-render` (renderer + GPU plumbing)
  - `wgpui-widgets` (components)
  - optional `wgpui-platform-ios`, `wgpui-platform-web`, `wgpui-testing`
- Move Codex/mission-specific iOS state and FFI bridge types to app-level crate(s), leaving only platform primitives in `wgpui`.
- Add explicit crate ownership boundaries (what is allowed in each crate).

---

### 2) Critical: Lint policy and implementation are out of sync

**Evidence**

- Workspace denies panic/unwrap/expect via clippy policy, but `wgpui` explicitly suppresses these:
  - [`crates/wgpui/src/lib.rs:24`](/Users/christopherdavid/code/openagents/crates/wgpui/src/lib.rs:24)
- Direct unwraps in production paths remain, e.g.:
  - [`crates/wgpui/src/theme/mod.rs:100`](/Users/christopherdavid/code/openagents/crates/wgpui/src/theme/mod.rs:100)
  - [`crates/wgpui/src/platform/web.rs:338`](/Users/christopherdavid/code/openagents/crates/wgpui/src/platform/web.rs:338)
  - [`crates/wgpui/src/markdown/parser.rs:457`](/Users/christopherdavid/code/openagents/crates/wgpui/src/markdown/parser.rs:457)
- Strict lint run result:
  - `cargo clippy -p wgpui --all-targets -- -D warnings` -> **424 errors**
  - top recurring lint categories include `float_cmp`, `unused_self`, `unreadable_literal`, `many_single_char_names`, `allow_attributes`, `unnested_or_patterns`.

**Why this is a smell**

- CI strictness cannot be raised meaningfully while this mismatch persists.
- “deny in policy / suppress in code” creates unclear quality contracts.

**Refactor suggestions**

- Pick one posture and enforce it consistently:
  - Option A: strict baseline with a tracked debt allowlist file.
  - Option B: scoped relaxations for legacy modules, but ban new debt in touched files.
- Remove crate-wide `#![expect(...)]` in favor of local, justified exceptions.
- Add a CI rule: touched files must pass clippy with no new warnings.

---

### 3) High: UI event loop blocks on wallet/network work

**Evidence**

- UI input handling directly triggers wallet actions:
  - [`apps/autopilot-desktop/src/input.rs:261`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input.rs:261)
- Wallet state performs synchronous `block_on` calls inside those actions:
  - [`apps/autopilot-desktop/src/spark_wallet.rs:78`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/spark_wallet.rs:78)
  - [`apps/autopilot-desktop/src/spark_wallet.rs:100`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/spark_wallet.rs:100)
  - [`apps/autopilot-desktop/src/spark_wallet.rs:146`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/spark_wallet.rs:146)
  - [`apps/autopilot-desktop/src/spark_wallet.rs:187`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/spark_wallet.rs:187)

**Why this is a smell**

- Any slow network operation can stall rendering/input responsiveness.
- Makes behavior under poor connectivity unpredictable.

**Refactor suggestions**

- Introduce an async action queue:
  - UI emits command enum (`Refresh`, `CreateInvoice`, `SendPayment`, etc.).
  - Background task executes network work.
  - UI state is updated via message channel.
- Add per-action timeout and cancellation behavior.

---

### 4) High: `autopilot-desktop` modules are doing multiple jobs each

**Evidence**

- [`apps/autopilot-desktop/src/input.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input.rs:19) handles:
  - window event routing
  - pane events
  - hotbar events
  - Nostr action dispatch
  - Spark action dispatch
  - keyboard mapping/parsing
- [`apps/autopilot-desktop/src/render.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/render.rs:219) handles:
  - GPU frame orchestration
  - pane layout iteration
  - Nostr pane rendering
  - Spark pane rendering
  - generic text formatting helpers
- [`apps/autopilot-desktop/src/pane_system.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/pane_system.rs:22) duplicates pane construction logic per pane kind.

**Why this is a smell**

- Hard to safely modify one behavior without side effects.
- Low cohesion increases regression risk.

**Refactor suggestions**

- Introduce pane-specific modules implementing a shared interface:
  - `PaneController` (state + actions)
  - `PaneRenderer` (paint + layout)
  - `PaneInput` (hit-testing + input handling)
- Move generic pane creation into `create_pane(kind, descriptor)`.
- Keep `render.rs` as orchestration only.

---

### 5) High: Security UX debt around sensitive key display

**Evidence**

- Nostr pane renders full secret material by default:
  - [`apps/autopilot-desktop/src/render.rs:329`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/render.rs:329)
  - [`apps/autopilot-desktop/src/render.rs:350`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/render.rs:350)

**Why this is a smell**

- Easy shoulder-surfing/screenshot exfiltration risk.
- Not safe default behavior for desktop usage.

**Refactor suggestions**

- Mask secret fields by default.
- Add explicit reveal action with timer + visual warning.
- Add copy buttons with one-time ephemeral reveal.

---

### 6) Medium-High: Spark network abstraction silently remaps unsupported networks

**Evidence**

- `Testnet` and `Signet` currently map to `Regtest`:
  - [`crates/spark/src/wallet.rs:22`](/Users/christopherdavid/code/openagents/crates/spark/src/wallet.rs:22)

**Why this is a smell**

- Caller intent can be silently violated.
- Can produce confusing or dangerous environment assumptions.

**Refactor suggestions**

- Replace silent remap with explicit error:
  - `UnsupportedNetwork(Network)` in `SparkError`.
- Keep mapping only where truly equivalent and documented.

---

### 7) Medium: Workspace dependency table drift after pruning

**Evidence**

- `workspace.dependencies` has entries not referenced by any member `Cargo.toml`:
  - `async-trait`, `base64`, `chrono`, `clap`, `once_cell`, `quickcheck`, `serde`, `serde_json`,
    `sha2`, `signal-hook`, `tempfile`, `tokio-tungstenite`, `toml`, `tracing`, `uuid`, `walkdir`.

**Why this is a smell**

- Increases cognitive load.
- Creates confusion about what is actually required for MVP.

**Refactor suggestions**

- Prune unused workspace dependencies now.
- Add a periodic dependency drift check.

---

### 8) Medium: Test coverage gap in MVP app/wallet layers

**Evidence**

- `apps/autopilot-desktop/src`: **0** unit tests.
- `crates/spark/src`: **0** unit tests.
- `crates/nostr/core/src`: 2 tests (good but narrow).

**Why this is a smell**

- Most volatile code paths (input/event/wallet integration) are unguarded.

**Refactor suggestions**

- Add focused tests first:
  - amount parsing and action routing in `input.rs`
  - Spark state transitions (`refresh`, `send`, `invoice`) with mocked wallet API
  - pane hit-testing invariants
- Add one integration smoke test: open app -> open Nostr pane -> open Spark pane -> action dispatch.

---

### 9) Medium: Library includes optional/editor complexity that MVP app does not need

**Evidence**

- Live editor includes full Vim-mode subsystem:
  - [`crates/wgpui/src/components/live_editor/mod.rs:4`](/Users/christopherdavid/code/openagents/crates/wgpui/src/components/live_editor/mod.rs:4)
  - [`crates/wgpui/src/components/live_editor/state.rs:20`](/Users/christopherdavid/code/openagents/crates/wgpui/src/components/live_editor/state.rs:20)

**Why this is a smell**

- Adds maintenance surface to core UI library even if unused by current product slice.

**Refactor suggestions**

- Feature-gate advanced editor modes (`live_editor_vim`), default off.
- If not required for MVP, move advanced editor lane behind optional crate/module.

---

## Refactor Roadmap (Suggested)

## Phase 0 (Immediate, 1-2 days)

- Add this audit to issue backlog with one issue per finding.
- Prune unused workspace dependency entries.
- Define lint policy for legacy vs touched code.

## Phase 1 (Stability, 3-5 days)

- Move Spark operations off UI thread.
- Add test harness for `autopilot-desktop` input/action logic.
- Add Spark crate tests with SDK abstraction boundary.

## Phase 2 (Architecture, 1-2 weeks)

- Split `autopilot-desktop` pane architecture into controller/renderer/input units.
- Start `wgpui` decomposition:
  - isolate platform-specific code
  - extract app/domain logic from `platform/ios.rs`
  - split optional/testing/storybook lanes

## Phase 3 (Quality hardening)

- Enforce clippy clean-on-touch policy.
- Burn down top recurring clippy categories in `wgpui`.
- Remove crate-level lint suppressions where no longer required.

---

## Concrete Next Steps

1. Open issues for Findings 1-9 with file/line references from this audit.
2. Implement Finding 3 first (UI-thread blocking wallet calls), since it directly impacts user-perceived responsiveness.
3. Begin Finding 2 in parallel (lint posture reset), because all future refactors benefit from cleaner signal.
