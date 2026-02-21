# Apps Proto-First Audit (2026-02-20)

## Scope

This audit measures how proto-first each app under `apps/` is today.

Included apps:

- `apps/runtime`
- `apps/openagents.com`
- `apps/mobile`
- `apps/desktop`
- `apps/autopilot-ios`
- `apps/autopilot-desktop`
- `apps/inbox-autopilot`
- `apps/lightning-ops`
- `apps/lightning-wallet-executor`
- `apps/onyx`

Out-of-scope-but-relevant check requested during follow-up:

- desktop-capable binaries still present under `crates/` were also reviewed for ownership clarity.

## Proto-First Definition Used Here

An app is proto-first when shared cross-surface contracts are:

1. Defined first in `proto/`.
2. Consumed via generated types or strict proto-derived adapters.
3. Enforced at runtime/tests against proto enums/fields (not only docs).
4. Not re-authored as independent schema authority in app-local TS/Swift/Rust/PHP models.

This follows:

- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0028-layer0-proto-canonical-schema.md`
- `proto/README.md`

## Maturity Scale

- `P0` None: no meaningful proto contract usage in app code.
- `P1` Proto-aware: proto policy exists, app still hand-authors contract shapes.
- `P2` Proto-compatible adapters: app can decode/map proto-compatible payloads, but local schemas still authoritative.
- `P3` Proto-enforced boundaries: runtime checks/tests enforce convergence to proto artifacts.
- `P4` Proto-native: generated proto types are the direct source for app contracts and wire payloads.

## Repo-Wide Findings

1. Proto doctrine is established and accepted.
   - `docs/plans/archived/adr-legacy-2026-02-21/ADR-0028-layer0-proto-canonical-schema.md`
   - `proto/README.md`
2. Proto generation is verified, but generated artifacts are not committed and currently not consumed by apps as a primary contract source.
   - `scripts/verify-proto-generate.sh`
   - `buf.gen.yaml`
3. Only runtime currently runs strong proto convergence checks in code.
   - `apps/runtime/lib/openagents_runtime/contracts/layer0_proto_contract.ex`
   - `apps/runtime/lib/mix/tasks/runtime.contract.check.ex`
4. Khala wire docs define proto enums/messages, but runtime and SDK still largely use string topics/error codes in implementation.
   - `docs/protocol/OA_SYNC_WS_MAPPING.md`
   - `apps/runtime/lib/openagents_runtime_web/sync_channel.ex`
   - `packages/khala-sync/src/types.ts`
5. `apps/autopilot-desktop` is confirmed WGPUI-based in active code and builds as such.
   - `apps/autopilot-desktop/Cargo.toml`
   - `apps/autopilot-desktop/src/main.rs`
   - verified with `cargo check -p autopilot-desktop` on 2026-02-20

## Scorecard (All Apps)

| App | Proto-First Level | Current state |
|---|---|---|
| `apps/runtime` | `P3` | Strong proto enforcement at boundaries; sync wire implementation still string-based in key places |
| `apps/lightning-ops` | `P2` | Proto-compatible adapters and tests exist, but Effect schemas in `contracts.ts` remain local authority |
| `apps/openagents.com` | `P1` | Proto-aware architecture, but API/UI contracts are still hand-authored DTOs and manual JSON shapes |
| `apps/mobile` | `P1` | Uses Khala and runtime APIs, but worker/sync models are hand-authored TS types |
| `apps/desktop` | `P1` | Khala lane is feature-gated/configured, but no proto-generated contract usage |
| `apps/autopilot-ios` | `P1` | Docs state proto authority, but Swift runtime models/parsers are hand-authored |
| `apps/autopilot-desktop` | `P0` | Runtime sync path is manual JSON + SSE parsing, no proto contract binding |
| `apps/inbox-autopilot` | `P0` | Local daemon/app architecture with no OpenAgents proto contract integration |
| `apps/lightning-wallet-executor` | `P0` | Uses Effect contracts; no OpenAgents proto contract consumption in app code |
| `apps/onyx` | `P0` | Local-first notes app with no OpenAgents proto schema consumption |

## Per-App Analysis

## 1) `apps/runtime` (`P3`)

Evidence of proto-first enforcement:

- Runtime checks convergence with proto artifacts:
  - `apps/runtime/lib/openagents_runtime/contracts/layer0_proto_contract.ex`
  - `apps/runtime/lib/mix/tasks/runtime.contract.check.ex`
- Runtime adapters are proto-derived and validate payload compatibility:
  - `apps/runtime/lib/openagents_runtime/contracts/layer0_type_adapters.ex`
- Tests enforce reason/proto/json convergence:
  - `apps/runtime/test/openagents_runtime/contracts/policy_reason_contract_test.exs`
  - `apps/runtime/test/openagents_runtime/contracts/layer0_proto_contract_test.exs`

Remaining gap to `P4`:

- Sync channel and tests still operate on string topics and string error codes, not generated `openagents.sync.v1` enums:
  - `apps/runtime/lib/openagents_runtime_web/sync_channel.ex`
  - `apps/runtime/test/openagents_runtime_web/channels/sync_channel_test.exs`

## 2) `apps/lightning-ops` (`P2`)

Evidence:

- Local contract authority remains handwritten Effect schemas:
  - `apps/lightning-ops/src/contracts.ts`
- Proto-compat adapter layer is present and tested:
  - `apps/lightning-ops/src/controlPlane/protoAdapters.ts`
  - `apps/lightning-ops/test/control-plane-proto-adapters.test.ts`
- Transport is API-based and not proto-generated:
  - `apps/lightning-ops/src/controlPlane/apiTransport.ts`

Status:

- Proto compatibility exists at boundaries, but the app is not yet proto-authoritative.

## 3) `apps/openagents.com` (`P1`)

Evidence:

- Admin surface hand-authors sync/runtime models and merges unknown payloads:
  - `apps/openagents.com/resources/js/pages/admin/index.tsx`
- Control-plane service defines/mutates payload shapes manually in PHP:
  - `apps/openagents.com/app/Services/L402/L402OpsControlPlaneService.php`
- Tests assert JSON contract shapes, not generated proto types:
  - `apps/openagents.com/tests/Feature/Api/Internal/LightningOpsControlPlaneApiTest.php`

Status:

- Proto-aware at architecture level, not proto-first in implementation.

## 4) `apps/mobile` (`P1`)

Evidence:

- Runtime worker/sync models are handwritten TS types:
  - `apps/mobile/app/services/runtimeCodexApi.ts`
- Khala updates are parsed/merged with app-local helpers:
  - `apps/mobile/app/screens/CodexWorkersScreen.tsx`
- Uses shared Khala SDK, but SDK itself is string-topic/manual-frame based:
  - `packages/khala-sync/src/types.ts`

Status:

- Proto compatibility is implicit; no generated proto contract ownership in app code.

## 5) `apps/desktop` (`P1`)

Evidence:

- Khala is configured as a feature-gated provider by env/config:
  - `apps/desktop/src/effect/config.ts`
  - `apps/desktop/src/effect/connectivity.ts`
- No direct generated proto contract usage in app runtime code.

Status:

- Proto-aware lane wiring, not proto-first implementation.

## 6) `apps/autopilot-ios` (`P1`)

Evidence:

- Runtime sync/worker payload models are handwritten Swift structs:
  - `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexModels.swift`
- Stream parsing is manual SSE parsing in Swift client code:
  - `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexClient.swift`
- Docs assert proto authority, but generated model usage is not in app code:
  - `apps/autopilot-ios/docs/codex-connection-roadmap.md`

Status:

- Proto doctrine acknowledged, implementation still DTO-first.

## 7) `apps/autopilot-desktop` (`P0`)

Evidence:

- Runtime sync integration is manual JSON + SSE parse logic:
  - `apps/autopilot-desktop/src/main.rs`

Status:

- No meaningful proto contract binding in current app code.

## 8) `apps/inbox-autopilot` (`P0`)

Evidence:

- Local daemon/app stack uses local typed routes/events and SSE; no OpenAgents proto contract integration:
  - `apps/inbox-autopilot/daemon/src/routes.rs`
  - `apps/inbox-autopilot/README.md`

Status:

- Out of the current OpenAgents proto-first adoption lane.

## 9) `apps/lightning-wallet-executor` (`P0`)

Evidence:

- Contract authority is Effect schemas from shared package, not OpenAgents proto contracts:
  - `apps/lightning-wallet-executor/src/contracts.ts`

Status:

- Not proto-first for OpenAgents proto surface contracts.

## 10) `apps/onyx` (`P0`)

Evidence:

- Desktop note editor with local-first vault/data model and no runtime proto surface integration:
  - `apps/onyx/src/app.rs`
  - `apps/onyx/src/vault.rs`
  - `apps/onyx/docs/ARCHITECTURE.md`

Status:

- No OpenAgents proto contract usage in current app code.

## Desktop Binaries Still in `crates/` (Ownership Check)

These are not part of `apps/`, but they are desktop-capable binaries today:

- `crates/autopilot` (`[[bin]] autopilot`) with WGPUI + Winit deps:
  - `crates/autopilot/Cargo.toml`
  - `crates/autopilot/src/main.rs`

Interpretation:

- The canonical runtime/Codex desktop app is currently `apps/autopilot-desktop`.
- `apps/onyx` was moved from `crates/onyx` to `apps/onyx` on 2026-02-20.
- `crates/manatap` was removed on 2026-02-20 after confirming it was a standalone prototype/demo.
- `crates/autopilot` in particular can create ownership ambiguity with `apps/autopilot-desktop` and should be explicitly resolved by doctrine (keep-as-is, split responsibilities, or migrate).

## Clarification: “Retire Khala” Wording

The wording “retire Khala” (in older migration text) is incorrect for the current architecture.

Correct meaning:

- The target is to retire the legacy reactive/sync lane and legacy non-proto schema authority.
- Khala is the runtime-owned sync engine and remains the intended steady-state sync plane.

So “retire Khala” should be read as “retire the old lane that Khala replaced” (the prior vendor-managed sync dependency path).

## Khala Proto-Centric Status (Current)

1. Doctrine and schema governance: mostly complete.
   - Proto canonical source and ADRs are in place.
2. Runtime core enforcement: strong.
   - Layer-0 adapters and contract checks are active.
3. Khala sync wire implementation: partial.
   - Proto messages/enums are defined, but channel+SDK still mostly use string topics/codes.
4. Client app adoption: early.
   - Most clients still hand-author DTOs and merge logic.
5. Generated proto consumption across apps: limited.
   - Generation is verified, but generated contract packages are not the default in app code.

## Overall Conclusion

The codebase is proto-governed at the architecture level but not yet proto-first across all app implementations.

Current state is best described as:

- Runtime: proto-enforced (`P3`).
- Lightning ops: proto-compatible adapters (`P2`).
- Most client/control-plane apps: proto-aware but still DTO-first (`P1`/`P0`).
