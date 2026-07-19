# OpenAgents IDE plans

This directory contains implementation plans for the classic coding-workbench
side of OpenAgents Desktop. The plans complement the product contracts in
`specs/desktop/` and `specs/openagents/`. They do not replace those contracts or
grant implementation authority by themselves.

Canonical roadmap:

- [OpenAgents IDE roadmap](./ROADMAP.md) — the single dependency-ordered build
  sequence from the delivered IDE-00..07 daily-use foundation through the
  complete Pierre tree/diff plane, built-in toggleable Vim mode, the current
  Khala editor default with Tokyo Night fallback, Zed-quality agent
  integration, and full Cursor parity.

Supporting evidence and architecture (not independent roadmaps):

- [Managed agent sandboxes, Box compatibility, IDE, and Sarah](../sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md) — owner-accepted P1 dependency program for a canonical GCP `SandboxResource`, pinned Box SDK conformance, IDE-13/17 integration, Sarah broker, bounded mobile/web supervision, and live cleanup proof under epic `#9023`.
- [IDE-08 agent-native code graph](./2026-07-19-ide-08-agent-native-code-graph.md) — the exact project/worktree attachment, eleven-source context disclosure, hash/version-bound proposal graph, Pierre review, canonical apply/rebase/undo, bidirectional backlinks, host-only evidence, private retention boundary, packaged journey, and no-authority audit for issue `#9036`.
- [IDE-07 packaged daily-use basic IDE acceptance](./2026-07-19-ide-07-basic-ide-acceptance.md) — the exact-artifact/SHA contract, fifteen-journey matrix, frozen percentile/resource budgets, seven-launch chat-only zero-cost proof, custody/rollback oracle, platform limits, and narrow `OpenAgents basic IDE` claim boundary for issue `#9022`.
- [Khala editor theme owner work packet](./2026-07-19-khala-editor-theme-owner-work-packet.md) — the current owner supersession that makes the Khala projection the fixed default while retaining Tokyo Night as an owned fallback.
- [IDE-03 delivery: Monaco, built-in Vim, and Tokyo Night](./2026-07-19-ide-03-monaco-vim-tokyo-night.md) — the authority map, opaque model lifecycle, built-in Vim boundary, theme projection, percentile/resource evidence, and packaged Finder/LaunchServices journey for issue `#9018`.
- [OpenAgents Desktop basic IDE: VS Code outcomes with Monaco and Pierre](./2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md) — the detailed
  basic-editor component and adapter design. Its packet order is superseded by
  the canonical roadmap.
- [Zed-quality OpenAgents IDE: Effect and Rust architecture](./2026-07-18-zed-quality-ide-effect-rust-architecture.md) — the canonical runtime split and cross-surface completion contract: Effect/TypeScript owns the project, document, language, Git, agent, policy, persistence, and projection graph. Supervised authority-free Rust helpers are limited to PTY/containment and empirically justified native kernels.
- [What OpenAgents should take from Zed for the agent IDE](./2026-07-18-zed-agent-ide-adaptation-analysis.md) — makes Zed the main integrated agent-IDE architecture reference while retaining Monaco, Pierre, Effect Native, WorkContext, and OpenAgents' stricter authority and receipt boundaries.
- [VS Code TypeScript reuse analysis for the OpenAgents IDE](./2026-07-18-vscode-typescript-reuse-analysis.md) — turns the current VS Code source into an exact package/adaptation ledger: Monaco plus focused URI/LSP/language-service/xterm/search/DAP candidates, with workbench, Explorer, extension-host, and internal `vs/*` boundaries kept outside the product dependency graph.
