# OpenAgents Codebase Audit — 2025-11-07 20:55

This audit reviews the Swift iOS/macOS codebase for code smells, overlong files, duplication, and stale code. It also captures recent GitHub issues and PRs. See linked subdocuments for details.

## Summary

- Swift files: 166
- Swift LOC (total): 31,346
- App/source LOC: 17,806 • Test LOC: 9,168
- Top hotspots (very long files):
  - DesktopWebSocketServer.swift (1,620 lines)
  - ExploreOrchestrator.swift (1,150 lines)
  - BridgeManager.swift (576 lines)
- Smell counts:
  - Forced cast `as!`: 6
  - Forced try `try!`: 2
  - Forced unwrap after call `)!`: 89
  - Identifier forced unwrap `x!`: 66
  - `fatalError(...)`: 4
  - TODO/FIXME/HACK markers: 6
- Logging: 142 `print(...)` calls in app/source code; central logging recommended
- Linting/formatting: No `.swiftlint.yml` or `.swiftformat` detected in repo
- Stale/deprecated code: One explicit deprecation comment in DesktopWebSocketServer.swift:313; legacy JS/TS/Rust removed; `packages/tricoder/` retained as deprecated by policy

## Contents

- Metrics and hotspots: metrics.md
- Code smells: smells.md
- Long files: long-files.md
- Duplication: duplication.md
- TODOs and markers: todos.md
- Stale/dead code: stale.md
- GitHub issues and PRs: issues-prs.md
- Recommendations: recommendations.md

## High-priority actions (TL;DR)

1) Split DesktopWebSocketServer.swift into focused components (transport, routing, Tinyvex/history API, session hub). Add unit tests per component.
2) Introduce SwiftLint + SwiftFormat; ban `print`, discourage force unwraps/casts, require `os.Logger`.
3) Replace `print` with `os.Logger` via shared Logging utility; add DEBUG gating.
4) Reduce forced unwraps (`)!`/`x!`) with `guard` + errors and optional chaining.
5) Remove or hard-deprecate the raw JSONL hydrate path in the desktop server.
6) Break down ExploreOrchestrator.swift into sub-orchestrators/state-machine reducers.
7) Ensure CI builds + tests on PRs; enforce lint/format checks.

