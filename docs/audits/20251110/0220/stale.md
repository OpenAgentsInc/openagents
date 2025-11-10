# Stale / Deprecated / Placeholder Items

- Simplified views
  - `ios/OpenAgents/SimplifiedMacOSView.swift` — superseded by `Views/macOS/ChatMacOSView.swift` as per ADR‑0007. Keep only if still used behind feature flag; otherwise deprecate with clear comment and remove from targets.
  - `ios/OpenAgents/SimplifiedIOSView.swift` — used behind `Features.simplifiedIOSUI`. If no longer needed for demos, remove or constrain via build config.

- Orchestration save hooks
  - `DesktopWebSocketServer+Orchestration.saveCompletedConfig` — TODO to trigger scheduler reload. Implement once scheduler.bind/coordinator.run_once are in place.

- FM probe/detector
  - `ios/OpenAgents/FMProbe.swift` — ensure this remains useful in-app; otherwise, move to Developer tools or tests.

- Legacy tests vs current ACP types
  - Some tests reference `ACP.Client.ToolUse`/`TextBlock` not present in current client modeling. Either add light shims/aliases or retire the specific tests in favor of update‑level fixtures already present.

