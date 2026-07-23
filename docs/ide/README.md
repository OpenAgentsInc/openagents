# OpenAgents IDE plans

This directory contains implementation plans for the classic coding-workbench
side of OpenAgents Desktop. The plans complement the product contracts in
`specs/desktop/` and `specs/openagents/`. They do not replace those contracts or
grant implementation authority by themselves.

Current destination plan:

- [Omega, the Zed-based OpenAgents IDE](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
  — the owner-selected primary Desktop, IDE, and company workroom destination.
  It defines the Effect authority-service boundary, native GPUI workrooms,
  existing-agent adapters, bounded Nostr interoperability, migration plan, and
  cutover gates. It cancels the separate Buzz installation.
- [OpenAgents IDE roadmap](./ROADMAP.md) — the superseded Electron
  dependency-ordered sequence and the factual record for delivered IDE
  behavior, contracts, and receipts.

Supporting evidence and architecture (not independent roadmaps):

- [OpenAgents IDE crash prevention implementation plan](./2026-07-20-openagents-ide-crash-prevention-implementation-plan.md) — the owner-directed cross-cutting safety plan that binds all repository-review OOM and main-process message SIGTRAP recommendations to eight implementation packets, 65 permanent-control IDs, 51 source regression rows, and one combined release gate.
- [IDE-13 portable capability foundation](./2026-07-20-ide-13-portable-capability-foundation.md) — the implemented schema, bounded attachment model, main-owned Effect coordinator, confirmed Desktop Sync projection, local evidence, and exact remaining real-placement acceptance gaps for issue `#9041`.
- [IDE-10 Effect run graph](./2026-07-19-ide-10-effect-run-graph.md) — the implemented schema-first terminal, declared-task, test, and Output graph. Safe named environment admission. Xterm projection. Process-group, retention, gap, redaction, artifact, semantic-success, receipt, packaged, and no-Rust evidence for issue `#9038`.
- [IDE-11 Effect DAP graph](./2026-07-20-ide-11-effect-dap-graph.md) — the implemented supervised debug graph, main-owned configuration admission, bounded DAP transport, debug projections, generation fences, renderer controls, retention, deletion, and exact macOS arm64 packaged evidence for issue `#9039`.
- [IDE-12 Effect source control](./2026-07-20-ide-12-effect-source-control.md) — the implemented exact-version Git, worktree, review, recovery, remote, provider, and delivery graph with exact macOS arm64 packaged evidence for issue `#9040`.
- [Managed agent sandboxes, Box compatibility, IDE, and Sarah](../sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md) — owner-accepted P1 dependency program for a canonical GCP `SandboxResource`, pinned Box SDK conformance, IDE-13/17 integration, Sarah broker, bounded mobile/web supervision, and live cleanup proof under epic `#9023`.
- [IDE-08 agent-native code graph](./2026-07-19-ide-08-agent-native-code-graph.md) — the exact project/worktree attachment, eleven-source context disclosure, hash/version-bound proposal graph, Pierre review, canonical apply/rebase/undo, bidirectional backlinks, host-only evidence, private retention boundary, packaged journey, and no-authority audit for issue `#9036`.
- [IDE-07 packaged daily-use basic IDE acceptance](./2026-07-19-ide-07-basic-ide-acceptance.md) — the exact-artifact/SHA contract, fifteen-journey matrix, frozen percentile/resource budgets, seven-launch chat-only zero-cost proof, custody/rollback oracle, platform limits, and narrow `OpenAgents basic IDE` claim boundary for issue `#9022`.
- [Khala editor theme owner work packet](./2026-07-19-khala-editor-theme-owner-work-packet.md) — the current owner supersession that makes the Khala projection the fixed default while retaining Tokyo Night as an owned fallback.
- [IDE-03 delivery: Monaco, built-in Vim, and Tokyo Night](./2026-07-19-ide-03-monaco-vim-tokyo-night.md) — the authority map, opaque model lifecycle, built-in Vim boundary, theme projection, percentile/resource evidence, and packaged Finder/LaunchServices journey for issue `#9018`.
- [OpenAgents Desktop basic IDE: VS Code outcomes with Monaco and Pierre](./2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md) — the detailed
  basic-editor component and adapter design. Its packet order is superseded by
  the canonical roadmap.
- [Zed-quality OpenAgents IDE: Effect and Rust architecture](./2026-07-18-zed-quality-ide-effect-rust-architecture.md) — retained evidence for the Effect authority boundary, process supervision, and measured native-workload gates. The Omega plan supersedes its shell and GPUI rejection.
- [What OpenAgents should take from Zed for the agent IDE](./2026-07-18-zed-agent-ide-adaptation-analysis.md) — retained project-graph, identity, local/remote, extension, and authority evidence. The Omega plan supersedes its Monaco, Pierre, Effect Native, and no-fork destination.
- [VS Code TypeScript reuse analysis for the OpenAgents IDE](./2026-07-18-vscode-typescript-reuse-analysis.md) — turns the current VS Code source into an exact package/adaptation ledger: Monaco plus focused URI/LSP/language-service/xterm/search/DAP candidates, with workbench, Explorer, extension-host, and internal `vs/*` boundaries kept outside the product dependency graph.
