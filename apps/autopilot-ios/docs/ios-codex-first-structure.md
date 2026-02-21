# Autopilot iOS App: Codex-First Structure

Status: Proposed  
Date: 2026-02-19  
Scope: structure and delivery shape for a native iOS Autopilot app focused on Codex first.

## Goal

Ship a native iOS Autopilot app that can immediately:

1. authenticate the user,
2. show/runtime-admin Codex workers,
3. stream worker events live,
4. keep architecture aligned with runtime/Laravel authority boundaries.

Codex-first means iOS starts with worker visibility/admin and thread activity flow, then expands to broader Autopilot features.

## Non-Negotiable Architecture Boundaries

1. Runtime + Postgres remain source of truth for worker lifecycle/events/policy.
2. iOS calls Laravel public APIs only (never runtime internal `/internal/v1/*` directly).
3. Khala remains projection/read-model only; no authority moved to client.
4. Worker ownership and policy checks stay server-side.
5. Proto definitions in `proto/` remain contract authority; iOS models should track proto-compatible envelopes.

References:

- `docs/adr/ADR-0029-khala-sync-layer-and-codex-agent-mode.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `proto/README.md`

## Recommended Repository Shape

Add a native app surface:

`apps/autopilot-ios/`

Suggested layout:

```text
apps/autopilot-ios/
  OpenAgentsApp.xcodeproj
  OpenAgentsApp/
    App/
      OpenAgentsApp.swift
      AppRouter.swift
    Features/
      Auth/
      CodexWorkers/
        WorkerList/
        WorkerDetail/
        WorkerStream/
        WorkerActions/
      Settings/
    Core/
      API/
        HTTPClient.swift
        RequestBuilder.swift
      Auth/
        SessionStore.swift
        TokenProvider.swift
      RuntimeCodex/
        RuntimeCodexClient.swift
        KhalaSyncClient.swift
        RuntimeCodexModels.swift
      Khala/
        KhalaTokenClient.swift
      Observability/
        RequestCorrelation.swift
        Logger.swift
      Contracts/
        Generated/   # proto-generated Swift models (or mapped wrappers)
      Persistence/
        LocalCache.swift
      Config/
        Environment.swift
    Resources/
    Tests/
      Unit/
      Integration/
      Snapshot/
    UITests/
```

## Codex-First Feature Scope (Phase 1)

Minimum iOS features:

1. Worker list (`GET /api/runtime/codex/workers`)
2. Worker snapshot (`GET /api/runtime/codex/workers/{workerId}`)
3. Worker live stream via Khala websocket (`POST /api/sync/token` + `/sync/socket/websocket`)
4. Worker request action (`POST /api/runtime/codex/workers/{workerId}/requests`)
5. Worker stop action (`POST /api/runtime/codex/workers/{workerId}/stop`)

These should mirror existing web/mobile runtime proxy behavior.

References:

- `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
- `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`
- legacy Expo mobile parity reference (removed root; use git history)

## API and Data Layer Design

Use a single `RuntimeCodexClient` in iOS that owns:

1. typed request/response envelopes,
2. error classification (`auth`, `forbidden`, `conflict`, `invalid`, `network`, `unknown`),
3. Khala frame parsing/reconnect with exponential backoff,
4. watermark advancement and idempotent event merge,
5. header propagation for `x-request-id` and tracing metadata.

Guideline: match the legacy mobile runtime semantics (now represented in iOS client tests/docs), then optimize for Swift ergonomics.

## UI Composition (SwiftUI)

Recommended screen stack:

1. `AuthGateView`
2. `CodexWorkersListView`
3. `CodexWorkerDetailView`
4. `CodexWorkerStreamView`
5. `CodexWorkerActionsView`

State model:

1. screen-local `@State` for short-lived input state,
2. feature stores (`@Observable`/actors) for worker list/snapshot/stream lifecycle,
3. clear split between view models and transport client.

## Auth and Security

1. Reuse OpenAgents session/token flow already used by mobile/web.
2. Store credentials in Keychain-backed storage.
3. Send auth token only to Laravel API base.
4. Enforce server-side ownership checks as authoritative.

## Khala Lane (Optional After Codex Core)

After Codex worker admin path is stable:

1. integrate `POST /api/khala/token` for short-lived token minting,
2. subscribe to projection read models as UX enhancement only,
3. keep control actions routed through Laravel runtime APIs.

## Delivery Phases

### Phase A: Foundation

1. App skeleton, environment config, auth/session persistence.
2. HTTP client, error envelope mapping, request correlation headers.

### Phase B: Codex Read Path

1. Worker list/snapshot screens.
2. Khala websocket reader with watermark resume and reconnect logic.
3. Stream event timeline UI.

### Phase C: Codex Admin Path

1. Request + stop controls with policy-safe error handling.
2. Ownership/forbidden UX states.

### Phase D: Hardening

1. Offline and retry behavior.
2. Structured logging and trace correlation surfaces.
3. End-to-end fixtures and reliability tests.

### Phase E: Khala Enhancement

1. Token mint + subscription read models.
2. Projection badges and lag visibility.

## Testing Strategy

1. Unit tests for `RuntimeCodexClient` request/response mapping.
2. Khala parser/reconnect tests with fixture update batches.
3. Integration tests against staging Laravel runtime proxy.
4. UI tests for worker list/detail/admin flows.

Recommended fixture parity source:

- legacy Expo mobile fixture parity reference (removed root; use git history)
- `docs/autopilot/testing/STREAM_TESTING.md`

## Practical Build Order (First 2 Weeks)

1. Scaffold `apps/autopilot-ios/` app shell + auth.
2. Implement list/snapshot.
3. Implement stream and timeline UI.
4. Implement request/stop actions.
5. Run staging E2E against Laravel runtime proxy APIs.

## Definition of Ready (Codex-First iOS)

1. User signs in and sees principal-scoped worker list.
2. Worker detail updates through live stream reconnects.
3. Request/stop actions return policy-accurate server responses.
4. Correlation headers are propagated for incident tracing.
5. No runtime authority moved client-side.
