# OpenAgents IDE plans

This directory contains implementation plans for the classic coding-workbench
side of OpenAgents Desktop. The plans complement the product contracts in
`specs/desktop/` and `specs/openagents/`; they do not replace those contracts or
grant implementation authority by themselves.

Current plan:

- [OpenAgents Desktop basic IDE: VS Code outcomes with Monaco and Pierre](./2026-07-18-openagents-desktop-basic-ide-vscode-pierre-plan.md)

Supporting architecture analysis:

- [What OpenAgents should take from Zed for the agent IDE](./2026-07-18-zed-agent-ide-adaptation-analysis.md) — makes Zed the main integrated agent-IDE architecture reference while retaining Monaco, Pierre, Effect Native, WorkContext, and OpenAgents' stricter authority and receipt boundaries.
