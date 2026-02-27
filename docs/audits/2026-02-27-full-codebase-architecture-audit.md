# 2026-02-27 Full Codebase Architecture + Hygiene Audit

## Status Gate

- Open GitHub issues: none (`gh issue list --state open --limit 100` returned empty).
- Working tree before audit edits: clean (`git status --short` returned empty).

## Scope

- `apps/autopilot-desktop`
- `crates/nostr/core`
- `crates/nostr/client`
- `crates/spark`
- `crates/wgpui`, `crates/wgpui-core`, `crates/wgpui-render`
- Workspace lint/build guardrails and ownership boundaries

## Method

- Static architecture review against:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Code-shape metrics (`rg`, `wc -l`, symbol counts, hotspot scan).
- Build/lint posture checks:
  - `cargo check --workspace --tests`
  - `scripts/lint/ownership-boundary-check.sh`
  - `scripts/lint/clippy-regression-check.sh`
  - `cargo clippy -p autopilot-desktop --all-targets -- -W clippy::all -A clippy::unwrap_used -A clippy::expect_used -A clippy::panic`
  - `cargo fmt --all --check`
  - `cargo check --workspace --all-features` (blocked by local Xcode license gate)

## Health Snapshot

- Rust files in repo: 348
- Total Rust LOC (apps + crates): 133,556
- Largest MVP app files:
  - `apps/autopilot-desktop/src/app_state.rs` (3,404 lines)
  - `apps/autopilot-desktop/src/input.rs` (3,133 lines)
  - `apps/autopilot-desktop/src/pane_renderer.rs` (2,427 lines)
  - `apps/autopilot-desktop/src/pane_system.rs` (2,220 lines)
- Largest protocol files:
  - `crates/nostr/core/src/nip90.rs` (1,762 lines)
  - `crates/nostr/core/src/nip_sa/skill.rs` (971 lines)
  - `crates/nostr/core/src/nip_sa/state.rs` (959 lines)
- Largest UI toolkit files:
  - `crates/wgpui/src/platform/ios.rs` (2,047 lines)
  - `crates/wgpui/src/components/hud/frame.rs` (1,439 lines)
  - `crates/wgpui/src/animation/animator.rs` (1,344 lines)

## Findings (Ranked)

### 1) Critical: Lint contract is not currently enforceable in core protocol paths

Workspace policy denies panic/unwrap/expect (`Cargo.toml` workspace lints), but core library lane fails with production unwraps.

Evidence:
- Workspace lint policy: `Cargo.toml` (`unwrap_used = "deny"`, `expect_used = "deny"`, `panic = "deny"`).
- Core unwraps in non-test paths:
  - `crates/nostr/core/src/nip40.rs:121`
  - `crates/nostr/core/src/nip44.rs:388`
  - `crates/nostr/core/src/nip59.rs:117`
  - `crates/nostr/core/src/nip90.rs:1102`
  - `crates/nostr/core/src/nip_sa/budget.rs:86`
  - `crates/nostr/core/src/nip_sa/skill.rs:284`
  - `crates/nostr/core/src/nip_sa/state.rs:428`
  - `crates/nostr/core/src/nip_sa/trajectory.rs:412`
- Gate result: `scripts/lint/clippy-regression-check.sh` fails in lib lane due clippy `unwrap_used` errors.

Impact:
- Safety guarantees are inconsistent with documented lint posture.
- Regression gate cannot be used as a reliable release-quality signal.

### 2) Critical: Ownership drift between `nostr/core` and `spark`

`docs/OWNERSHIP.md` states `crates/nostr/core` "Must not own Spark wallet logic" (`docs/OWNERSHIP.md:75-85`). Current code still includes Spark wallet integration inside NIP-SA.

Evidence:
- Optional Spark dependency in protocol crate: `crates/nostr/core/Cargo.toml:42-66`.
- Spark wallet integration module exported in NIP-SA:
  - `crates/nostr/core/src/nip_sa/mod.rs:39-40`
  - `crates/nostr/core/src/nip_sa/mod.rs:55-68`
- Direct Spark types used in protocol module: `crates/nostr/core/src/nip_sa/wallet_integration.rs:15`.

Impact:
- Boundary violation risk and tighter coupling between protocol and wallet runtime concerns.
- Harder to keep NIP model crate portable and dependency-light.

### 3) High: `autopilot-desktop` runtime/app model is concentrated in several "god modules"

Evidence:
- `RenderState` aggregates windowing, renderer, input state, wallet state, SA/SKL/AC lanes, pane states, command palette, and orchestration fields in one struct (`apps/autopilot-desktop/src/app_state.rs:2711-2773`).
- `app_state.rs` defines dozens of pane/domain state types and impls in one file (3,404 lines).
- `input.rs` centralizes event-loop handling, pane action reducers, command construction, runtime lane reconciliation, and validation (3,133 lines, 86 functions, 93 matches).
- `pane_renderer.rs` and `pane_system.rs` remain large and heavily branch-based.

Impact:
- High change risk and slower iteration for MVP-critical flow adjustments.
- Difficult ownership and test isolation for individual pane domains.

### 4) High: Pane metadata/routing duplication creates drift risk

Evidence:
- `PaneKind` handling duplicated across at least four files (`pane_registry`, `pane_system`, `pane_renderer`, `input`).
- Manual index mapping in registry (`apps/autopilot-desktop/src/pane_registry.rs:48-77`) coupled to static array order (`apps/autopilot-desktop/src/pane_registry.rs:107+`).
- Large action fan-out in `input.rs` (`run_*_action` pattern) and separate hit-action graph in `pane_system.rs`.

Impact:
- Any new pane or action requires cross-file synchronized edits.
- Easy to introduce parity bugs (command exists but not rendered/hit-tested/routed consistently).

### 5) High: Dead/legacy state pathways remain in hot runtime code

Evidence (warnings from check/clippy):
- Unused methods: `ProviderRuntimeState::toggle_online`, `tick`, `start_online`, `go_offline`, `move_degraded` (`apps/autopilot-desktop/src/app_state.rs:2544-2634`).
- Unconstructed variants:
  - `RuntimeCommandErrorClass::Internal` (`apps/autopilot-desktop/src/runtime_lanes.rs:56`)
  - `SaRunnerMode::Degraded` (`apps/autopilot-desktop/src/runtime_lanes.rs:190`)
  - `SkillTrustTier::Revoked` (`apps/autopilot-desktop/src/runtime_lanes.rs:253`)
  - `SaLifecycleCommand::PublishTickResult` (`apps/autopilot-desktop/src/runtime_lanes.rs:342`)

Impact:
- Confusing runtime contract surface; harder to reason about real state transitions.
- Signals incomplete migration from older simulation pathways.

### 6) Medium-High: Runtime lane transport types are heavier than needed

Evidence:
- Clippy flags `large_enum_variant` for `SaLaneUpdate` with large snapshot payload (`apps/autopilot-desktop/src/runtime_lanes.rs:401-404`).
- Worker channels move snapshot-rich enums continuously.

Impact:
- Unnecessary copies/alloc pressure in hot update loop.
- Adds friction if update frequency increases.

### 7) Medium: Workspace hygiene debt is still broad in `wgpui`

Evidence:
- `scripts/lint/clippy-baseline.toml` still carries high warning baseline (`LIB_WARNINGS=244`, `TEST_WARNINGS=445`, `EXAMPLE_WARNINGS=288`).
- `scripts/lint/clippy-debt-allowlist.toml` includes a large cross-section of `wgpui` files.
- Clippy all-target scan surfaces recurring style/control-flow debt in active UI modules.

Impact:
- Hard to distinguish meaningful new regressions from known debt noise.
- Tooling signal quality is reduced for day-to-day development.

### 8) Medium: Formatting gate is not clean across workspace

Evidence:
- `cargo fmt --all --check` reports diffs in multiple files (including `crates/nostr/core/src/nip06.rs`, `crates/wgpui/src/lib.rs`, `crates/wgpui/src/markdown/renderer.rs`, `crates/wgpui-core/src/scene.rs`, `crates/wgpui-render/src/svg.rs`).

Impact:
- Inconsistent formatting churn in unrelated files during routine work.
- Increased noise in review/commit boundaries.

### 9) Medium: Full-feature lane is not currently verifiable in this environment

Evidence:
- `cargo check --workspace --all-features` blocked by local Xcode license acceptance (`xcodebuild -license`) during Breez/Spark feature build scripts.

Impact:
- `--all-features` regressions can slip if only default lanes are checked locally.

## Cleanup Recommendations

## P0 (Immediate, unblock quality gates)

1. Make `clippy --workspace --lib` pass with current deny policy.
- Replace all `SystemTime::now().duration_since(...).unwrap()` in core protocol code with fallible helpers returning typed errors.
- Replace `nonce.try_into().unwrap()` and similar infallible-by-construction unwraps with explicit validation branches.
- Replace `Option::unwrap()` in NIP-SA skill validation with guarded error paths.

2. Align ownership boundaries for wallet integration.
- Remove Spark wallet integration from `nostr/core` (`nip_sa/wallet_integration.rs`) into an adapter layer owned by app or `spark` crate.
- Keep NIP-SA state types protocol-only; wallet fetch/update happens outside protocol crate.

3. Remove or wire dead runtime variants/methods.
- Either implement real `Degraded`/`Revoked` transitions and `PublishTickResult` dispatch path, or delete dormant states and simplify.

## P1 (Near-term architecture hardening)

1. Split `autopilot-desktop` into domain modules with explicit boundaries.
- Suggested first extraction set:
  - `state/provider_runtime.rs`
  - `state/panes/{earnings,relay,sync,network,...}.rs`
  - `input/reducers/{sa,skl,ac,wallet}.rs`
  - `render/panes/{agent,skill,credit,wallet}.rs`

2. Replace manual pane index map with generated/static keyed map.
- Avoid `PaneKind -> array index` match table in `pane_registry`.
- Make pane spec the single source for title, command ID, defaults, and startup/hotbar metadata.

3. Introduce typed request structs where argument lists are too long.
- Start with `NetworkRequestsState::queue_request_submission` (`apps/autopilot-desktop/src/app_state.rs:730-739`).

## P2 (Performance + maintainability)

1. Reduce update enum payload size.
- Box large snapshot variants in lane update enums or move to shared snapshot store + small delta events.

2. Standardize update-drain loops and minor clippy debt in hot paths.
- Convert manual `loop { match try_recv() ... }` to `while let` forms (`runtime_lanes`, `spark_wallet`).
- Apply targeted clippy fixes (`manual_clamp`, `needless_range_loop`, `assigning_clones`) in app hot paths first.

3. Normalize formatting baseline.
- Run `cargo fmt --all`, review unrelated churn once, and keep fmt-clean gate in routine flow.

## P3 (Process)

1. Keep strict posture but make it executable.
- `scripts/lint/clippy-regression-check.sh` should be green in default dev lane.
- Track all temporary lint exemptions with bounded owner/date/reason and expiry issue.

2. Add a periodic architecture sweep cadence.
- Monthly audit on: boundary drift, largest-file trend, dead code warnings, and lint-gate status.

## Proposed Cleanup Order

1. `nostr/core` panic-safety + ownership extraction (highest risk, fastest gate impact).
2. `autopilot-desktop` dead-path removal + pane registry de-dup.
3. `autopilot-desktop` modular extraction of state/input/render layers.
4. `wgpui` debt reduction in touched active modules, keeping examples/storybook debt isolated.
5. Workspace-wide fmt normalization once P0/P1 churn settles.

## Validation Artifacts (this audit run)

- `cargo check --workspace --tests`: pass (with dead-code warnings in runtime lane module).
- `scripts/lint/ownership-boundary-check.sh`: pass.
- `scripts/lint/clippy-regression-check.sh`: fail (lib lane blocked by `nostr/core` unwrap violations).
- `cargo clippy -p autopilot-desktop --all-targets -- -W clippy::all -A clippy::unwrap_used -A clippy::expect_used -A clippy::panic`: pass with warnings (actionable cleanup list).
- `cargo fmt --all --check`: fail (format drift in multiple files).
- `cargo check --workspace --all-features`: blocked by local Xcode license gate.
