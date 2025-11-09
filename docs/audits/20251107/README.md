# OpenAgents Codebase Audit — 2025-11-07

This audit reviews the Swift iOS/macOS codebase under `ios/` with a focus on maintainability, correctness, duplication, and risk. It consolidates scanning results and targeted file reviews and concludes with prioritized recommendations.

See also:
- Detailed findings: findings.md
- Metrics and hotspots: metrics.md

## Executive Summary

Overall, the project structure is clean and modern (Swift-only, SwiftUI, JSON-RPC ACP bridge). There is strong test coverage in `OpenAgentsCoreTests` and useful UI/integration tests. However, several very large, monolithic files and duplicated rendering/utility logic increase change risk. Logging is verbose with many `print` calls scattered across hot paths. A few developer-specific defaults and minor layering violations exist. Bridge security is currently LAN-only without authentication.

Top risks (with recommended priority):

1) Oversized Monoliths (P0)
   - `ios/OpenAgents/AcpThreadView.swift` (~1,759 lines)
   - `ios/OpenAgentsCore/.../DesktopWebSocketServer.swift` (~1,464 lines)
   - `ios/OpenAgentsCore/.../ExploreOrchestrator.swift` (~1,150 lines)
   Impact: Slows reviews, increases defect risk, discourages reuse. Refactor into subviews/modules.

2) Duplication in Rendering/Utilities (P0)
   - `prettyShellCommand` and tool-call UI exist in multiple places (`AcpThreadView.swift` and `ACP/Renderers/ToolCallView.swift`).
   - JSON helpers duplicated inside a view file.
   Impact: Divergent behavior, harder testability. Extract shared helpers into `OpenAgentsCore` and reuse renderers.

3) Logging Noise and Inconsistency (P0)
   - 160+ `print` usages in app+core sources; mixed with `os.Logger` in some places.
   Impact: Noisy logs, potential performance hit; inconsistent privacy annotations. Introduce a centralized logging facade with DEBUG gating.

4) Bridge Defaults and Security (P0→P1)
   - Hard-coded default host `192.168.1.11` and no authentication for JSON-RPC handshake.
   Impact: Fragile defaults; unauthenticated LAN exposure. Use last-known/simulator defaults; plan token/QR pairing or Tailscale-only guidance.

5) Layering/Ownership (P1)
   - Heavy business logic and JSON conversion living inside a SwiftUI view file (`AcpThreadView.swift`).
   Impact: Hard to test; UI state tightly coupled with data transforms. Move transforms to `OpenAgentsCore` or a view model.

6) CI/Linting Gaps (P1)
   - No SwiftLint/SwiftFormat config checked in; no CI to enforce builds/tests.
   Impact: Style drift, accidental breakage. Add GitHub Actions (build + tests) and linting.

7) Docs Drift (P1)
   - `docs/ios-bridge/README.md` still references a token-based Hello/Ack flow; code/ADR use JSON-RPC `initialize`.
   Impact: Confusion for contributors. Update docs to match ADR-0004 (JSON-RPC handshake).

8) Minor Hygiene (P2)
   - Duplicate imports, TODO stubs (e.g., voice input), large `ios/build/` present locally (ensure ignored).

## Quick Wins (1–2 days)

- Extract `prettyShellCommand` and command-array parsing to a single helper in `OpenAgentsCore` and reuse in both `AcpThreadView` and `ToolCallView` (tests already exist for renderers).
- Replace scattered `print` with a small `OpenAgentsLog` wrapper using `os.Logger` under the hood; gate verbose logs behind DEBUG flag.
- Update `BridgeConfig.defaultHost` to a neutral default; use persisted last-known endpoint or simulator loopback. Avoid developer-specific IPs.
- Split `AcpThreadView.swift` into:
  - `AcpThreadViewModel` (pure timeline computation + state)
  - `AcpThreadView` (UI only)
  - `ToolCallCell` reuse `ToolCallView`
- Update `docs/ios-bridge/README.md` to mirror ADR-0004 (JSON-RPC `initialize`, no Hello/Ack).
- Add `.github/workflows/ci.yml` to build iOS/macOS targets and run tests; add SwiftLint/SwiftFormat.

## Medium-Term (1–3 weeks)

- Modularize `DesktopWebSocketServer` into smaller components (handshake + JSON-RPC router + agent process/tailer + threads listing) with unit tests per component.
- Migrate manual DispatchQueue patterns to async/await where feasible; consider private actor(s) for server state (clients/process tracking) for safety.
- Move JSON-value conversion helpers out of view files into `OpenAgentsCore`.
- Add basic auth/pairing to the bridge (at minimum: a pre-shared token; ideally: QR provisioning) or document Tailscale-only usage.

## Longer-Term (1–2 months)

- Formalize a small logging schema (categories, levels, privacy) and wire logs into Console categories. Add sampling for high-frequency logs.
- Strengthen orchestration and bridge error handling with typed error responses and retry policies, with tests for failure modes.
- Consider a slim data layer for thread/timeline caching that cleanly separates transforms from SwiftUI.

## What’s Working Well

- Clear Swift-only architecture with shared `OpenAgentsCore`.
- Good test surface area (core + integration + UI).
- ADRs and docs provide helpful context and intent.

## Appendix: Key Hotspots (by size)

- ios/OpenAgents/AcpThreadView.swift (~1,759)
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift (~1,464)
- ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift (~1,150)
- See metrics.md for the full top list.

---

## Addendum — 2025-11-09 (Status Update)

The following audit actions have been completed and merged to main:

- Programmatic orchestration control: added JSON‑RPC methods `orchestrate/scheduler.run_now` and enhanced `orchestrate/scheduler.status` (with `next_wake_time`), plus an alias `orchestrate/scheduler.advance` for tests. Desktop server now caches the active orchestration config on `config.activate` and computes status using `SchedulePreview`.
- macOS local adapter: `LocalJsonRpcClient` implements orchestration config set/activate and scheduler status/run_now locally (no socket) for tests and operator flows.
- Test harness: new `OrchestrationSchedulerTests` validates config set → activate → status → run_now and observes ACP `session/update` via the server’s Combine publisher.
- Environment‑dependent tests: Claude CLI execution test now soft‑skips when the CLI exists but cannot execute (e.g., missing Node runtime), preventing unrelated failures.
- Tinyvex titles: implemented clear title end‑to‑end (DB + RPC + sidebar UI); documented in chat‑desktop Issue #29.

Remaining medium/long‑term items from this audit were either addressed separately or slated for follow‑ups (e.g., modularizing large files, CI/lint). See 2025‑11‑09 audit for next steps.
