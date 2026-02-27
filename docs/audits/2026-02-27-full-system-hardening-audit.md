# 2026-02-27 Full System Hardening Audit (Round 2)

## Scope

- `apps/autopilot-desktop`
- `crates/nostr/core`
- `crates/nostr/client`
- `crates/spark`
- `crates/wgpui`, `crates/wgpui-core`, `crates/wgpui-render`
- Workspace guardrails and lint posture

## Method

- Reviewed against product/ownership authority:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Executed build/test/lint/hygiene checks:
  - `cargo fmt --all --check` (pass)
  - `scripts/lint/ownership-boundary-check.sh` (pass)
  - `cargo check --workspace --tests` (pass)
  - `scripts/lint/clippy-regression-check.sh` (fails in lib lane)
  - `cargo test --workspace --tests` (pass)
  - strict production clippy lanes:
    - `cargo clippy -p nostr --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (fail)
    - `cargo clippy -p autopilot-desktop --bin autopilot-desktop --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (fail)
    - `cargo clippy -p nostr-client --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (pass)
    - `cargo clippy -p wgpui --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (pass)
    - `cargo clippy -p wgpui-core --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (pass)
    - `cargo clippy -p wgpui-render --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (pass)
    - `cargo clippy -p openagents-spark --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` (pass)

## System Snapshot

- Total Rust LOC (`apps/` + `crates/`): `148,368`
- LOC by active surface:
  - `apps/autopilot-desktop`: `17,721`
  - `crates/nostr/core`: `32,957`
  - `crates/nostr/client`: `781`
  - `crates/spark`: `461`
  - `crates/wgpui`: `93,380`
  - `crates/wgpui-core`: `1,666`
  - `crates/wgpui-render`: `1,206`
- Largest active files:
  - `apps/autopilot-desktop/src/app_state.rs` (`3,944`)
  - `apps/autopilot-desktop/src/input.rs` (`3,027`)
  - `apps/autopilot-desktop/src/pane_renderer.rs` (`2,451`)
  - `apps/autopilot-desktop/src/pane_system.rs` (`2,340`)
  - `crates/nostr/core/src/nip77.rs` (`2,032`)
  - `crates/nostr/core/src/nip90.rs` (`1,759`)

## Findings (Ranked)

### 1) Critical: Production panic/unwrap still exists in MVP runtime paths

Evidence:

- Desktop runtime panic path:
  - `apps/autopilot-desktop/src/pane_registry.rs:54`
- Nostr production unwraps (strict `--lib` clippy failures):
  - `crates/nostr/core/src/nip42.rs:219`
  - `crates/nostr/core/src/nip42.rs:305`
  - `crates/nostr/core/src/nip69.rs:500`
  - `crates/nostr/core/src/nip89.rs:263`

Why this matters:

- This directly violates current workspace policy (`panic/unwrap/expect` denied).
- Failures are in protocol/runtime paths, not only in tests.

### 2) High: Nostr timestamp acquisition is duplicated and inconsistently hardened

Evidence:

- Multiple direct `SystemTime::now().duration_since(UNIX_EPOCH)` callsites across NIPs:
  - `nip40`, `nip42`, `nip59`, `nip66`, `nip69`, `nip90`, `nip_sa/state`, `nip_sa/budget`
- Some callsites are guarded, others still unwrap.

Why this matters:

- Same logic is repeated with mixed safety semantics.
- Time handling bugs become distributed and harder to fix consistently.

### 3) High: Nostr tag parsing remains manual and index-fragile

Evidence:

- Repeated ad hoc tag lookups like `find(... t[0] == ...)` and positional indexing.
- Concrete hazard already present:
  - `crates/nostr/core/src/nip89.rs:260-267` does a second `find(...).unwrap()` after optional parsing.

Why this matters:

- Manual `Vec<Vec<String>>` indexing is brittle.
- Error behavior varies by module; easier to reintroduce panic paths.

### 4) High: Nostr core modules are very large and mix protocol logic with extensive inline tests

Evidence:

- File sizes:
  - `crates/nostr/core/src/nip77.rs` (`2,032` lines)
  - `crates/nostr/core/src/nip90.rs` (`1,759` lines)
  - `crates/nostr/core/src/nip69.rs` (`951` lines)
  - `crates/nostr/core/src/nip28.rs` (`924` lines)
- `cargo test -p nostr --tests` is strong (pass), but test logic is heavily co-located with implementation in several modules.

Why this matters:

- Increases review/debug friction for protocol changes.
- Raises risk of accidental coupling between parser/business logic/test fixtures.

### 5) Medium-High: Autopilot desktop state/input/render remain concentrated in mega-modules

Evidence:

- `apps/autopilot-desktop/src/app_state.rs` (`3,944` lines)
- `apps/autopilot-desktop/src/input.rs` (`3,027` lines)
- `apps/autopilot-desktop/src/pane_renderer.rs` (`2,451` lines)
- `apps/autopilot-desktop/src/pane_system.rs` (`2,340` lines)

Why this matters:

- Cross-cutting changes are expensive and regression-prone.
- Harder to enforce narrow ownership per pane/domain.

### 6) Medium: Dead-code suppressions indicate unfinished or dormant pathways

Evidence:

- Notable clusters:
  - `crates/nostr/core/src/nip77.rs` (`#[allow(dead_code)]` across protocol internals)
  - `crates/nostr/core/src/nip69.rs` (multiple helper builders marked dead)
  - `apps/autopilot-desktop/src/app_state.rs` (multiple dead-code allowances)

Why this matters:

- API surface appears larger than what runtime actually exercises.
- Makes it harder to identify true supported behavior.

### 7) Medium: Lint debt signal remains noisy at workspace level

Evidence:

- `scripts/lint/clippy-baseline.toml`:
  - `LIB_WARNINGS=244`
  - `TEST_WARNINGS=445`
  - `EXAMPLE_WARNINGS=288`
- `scripts/lint/clippy-debt-allowlist.toml` still includes a broad `wgpui` and app footprint.

Why this matters:

- Harder to detect meaningful new regressions.
- Incentivizes ignoring warnings in touched areas.

### 8) Medium: Nostr client parser has minimal direct test depth

Evidence:

- `cargo test -p nostr-client --tests`: `1` test (`relay::tests::test_parse_event_message`).
- Core parser file is non-trivial (`crates/nostr/client/src/relay.rs`, `398` lines).

Why this matters:

- Relay message parsing is edge-heavy and externally facing.
- Low dedicated test count increases protocol-compatibility regression risk.

## Positive Signals

- Ownership boundary script passes.
- Full workspace tests pass (`cargo test --workspace --tests`).
- Nostr test matrix is broad and green (`cargo test -p nostr --tests`).
- WGPUI core/runtime crates pass strict production panic/unwrap checks.

## Recommendations

## P0 (Immediate hardening)

1. Remove all production panic/unwrap violations now.
- Fix targets:
  - `apps/autopilot-desktop/src/pane_registry.rs:54`
  - `crates/nostr/core/src/nip42.rs`
  - `crates/nostr/core/src/nip69.rs`
  - `crates/nostr/core/src/nip89.rs`
- Acceptance: strict clippy commands above all pass for `nostr` and `autopilot-desktop`.

2. Add a shared Nostr time helper and migrate all NIP callsites.
- Introduce e.g. `nip01::unix_now_secs() -> Result<u64, Nip01Error>` (or dedicated utility module).
- Remove direct `SystemTime...duration_since...` duplication.
- Acceptance: no direct `duration_since(UNIX_EPOCH).unwrap()` in `crates/nostr/core/src`.

3. Introduce tag parsing helpers and refactor high-risk modules first.
- Start in `nip89`, `nip17`, `nip60`, `nip61`, `nip69`.
- Helper API should provide typed, non-panicking access (string/number/optional fields).
- Acceptance: no manual re-find + unwrap pattern in parser paths.

## P1 (Architecture tightening)

1. Split oversized NIP modules by concern.
- Suggested first split:
  - `nip77`: codec, model, reconciliation engine, tests
  - `nip90`: request/result model, tag serialization, builders, tests
  - `nip69`: builder + parsing + tests
- Acceptance: each extracted unit compiles independently and test paths unchanged.

2. Split `autopilot-desktop` mega-modules by pane/domain boundaries.
- Start with `app_state` and `input` reducer extraction per pane lane.
- Keep `main.rs`/orchestration thin; push domain logic into dedicated modules.
- Acceptance: no behavior changes, reduced single-file concentration, unchanged tests.

3. Reduce dead-code allowances with an explicit decision per symbol.
- Either wire into runtime or remove.
- Track removals by module.
- Acceptance: measurable reduction in `#[allow(dead_code)]` counts in active app + Nostr modules.

## P2 (Quality signal improvements)

1. Strengthen `nostr-client` parser coverage.
- Add table-driven tests for `EVENT`, `OK`, `EOSE`, `NOTICE`, `AUTH`, malformed arrays, malformed field types.
- Add roundtrip tests for all supported outbound messages.
- Acceptance: parser test count and branch coverage materially increase.

2. Tighten lint debt governance.
- Add per-file warning budgets for high-churn files.
- Require debt-allowlist entries to include expiry issue/date.
- Acceptance: baseline warning totals trend downward release-over-release.

3. Keep strict production hardening lane mandatory.
- Add/keep CI lane running:
  - `nostr --lib` strict panic/unwrap checks
  - `autopilot-desktop --bin` strict panic/unwrap checks
- Acceptance: these lanes remain green on main.

## Suggested Implementation Sequence

1. P0.1 panic/unwrap removals (`nostr` + `autopilot-desktop`).
2. P0.2/P0.3 shared time + tag parser utilities in `nostr/core`.
3. P1.1 NIP module decomposition (`nip77` then `nip90`).
4. P1.2/P1.3 autopilot module split + dead-code cleanup.
5. P2 parser coverage and lint-governance hardening.
