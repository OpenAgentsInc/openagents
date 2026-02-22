# iOS Codex WGPUI Parity Gates

Status: active (`OA-IOS-WGPUI-CODEX-009`)

## Purpose

Enforce iOS Codex as WGPUI-first while validating app-server protocol behavior across Rust core and iOS host integration.

## CI guardrails

Run:

```bash
apps/autopilot-ios/scripts/verify-codex-wgpui-guardrails.sh
```

This fails on:

1. New Codex/Khala Swift files in `apps/autopilot-ios/Autopilot/Autopilot/`.
2. Net-new SwiftUI product-surface additions in Codex-related files (`TextField`, `List`, `VStack`, etc.).
3. Net-new Swift-owned Codex state/business logic declarations and methods.

Override for emergency local debugging only:

```bash
OA_IOS_ALLOW_SWIFT_CODEX_LOGIC_DIFF=1 apps/autopilot-ios/scripts/verify-codex-wgpui-guardrails.sh
```

## Parity harness

Run:

```bash
apps/autopilot-ios/scripts/run-codex-parity-harness.sh
```

Coverage matrix:

1. Auth flow race + stale-response handling (`openagents-client-core` Rust tests).
2. Worker selection semantics (desktop/shared preference, running fallback) in Rust + iOS bridge test.
3. Handshake envelope extraction (`desktop/handshake_ack`) in Rust tests.
4. Khala stream subscribe/resume and stale-cursor mapping in Rust tests.
5. Thread start / turn interrupt control reconciliation in Rust tests.
6. Reconnect backoff + disconnect classification in Rust tests.
7. iOS app-server API envelope checks (auth/session/workers/events/sync token + request/stop) via `AutopilotTests`.

Skip xcode tests only when explicitly requested:

```bash
OA_IOS_SKIP_XCODE_TESTS=1 apps/autopilot-ios/scripts/run-codex-parity-harness.sh
```

## local-ci integration

The lane is wired into `scripts/local-ci.sh`:

```bash
./scripts/local-ci.sh ios-codex-wgpui
```

`changed` mode also runs this lane when iOS Codex/WGPUI files are touched.

## Real-device evidence

Use `apps/autopilot-ios/docs/real-device-codex-handshake-runbook.md` and attach evidence links in the issue/PR:

1. capture IDs (`worker_id`, `handshake_id`, `request_id`)
2. capture protocol event order
3. capture pass/fail for reconnect + replay behavior
