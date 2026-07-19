# OpenAgents IDE plans

This directory contains implementation plans for the classic coding-workbench
side of OpenAgents Desktop. The plans complement the product contracts in
`specs/desktop/` and `specs/openagents/`; they do not replace those contracts or
grant implementation authority by themselves.

Current plan:

- [OpenAgents Desktop basic IDE: VS Code outcomes with Monaco and Pierre](./2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md)

Supporting architecture analysis:

- [Zed-quality OpenAgents IDE: Effect and Rust architecture](./2026-07-18-zed-quality-ide-effect-rust-architecture.md) — the canonical runtime split and cross-surface completion contract: Effect/TypeScript owns the project, document, language, Git, agent, policy, persistence, and projection graph; supervised authority-free Rust helpers are limited to PTY/containment and empirically justified native kernels.
- [What OpenAgents should take from Zed for the agent IDE](./2026-07-18-zed-agent-ide-adaptation-analysis.md) — makes Zed the main integrated agent-IDE architecture reference while retaining Monaco, Pierre, Effect Native, WorkContext, and OpenAgents' stricter authority and receipt boundaries.
- [VS Code TypeScript reuse analysis for the OpenAgents IDE](./2026-07-18-vscode-typescript-reuse-analysis.md) — turns the current VS Code source into an exact package/adaptation ledger: Monaco plus focused URI/LSP/language-service/xterm/search/DAP candidates, with workbench, Explorer, extension-host, and internal `vs/*` boundaries kept outside the product dependency graph.
