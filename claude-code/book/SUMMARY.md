# Dissecting an Agentic Coding Tool

- [Introduction](README.md)
- [Architecture Map](ARCHITECTURE_MAP.md)
- [Master Outline](OUTLINE.md)
- [Codebase Glossary](GLOSSARY.md)
- [Editorial & Authoring Guide](EDITORIAL_GUIDE.md)

## Prologue

- [Prologue — How to Read a 1,900-File Agent](prologue-how-to-read-a-1900-file-agent.md)

## Part I — Foundations: Atoms, State, and Contracts

- [1. The Atoms: Identity, Reactive Stores, Lazy Schemas, and Async Generators](ch001-atoms-types-stores-and-primitives.md) — written
- [2. The Tool Contract: Defining What an Agent Can Do](ch002-the-tool-contract.md) — written
- [3. The Command & Message Contracts: User Input Meets the Model](ch003-command-and-message-contracts.md) — written

## Part II — The Prompt, the Model, and the Engine

- [4. Prompt Engineering as Systems Engineering: Context & Cache Boundaries](ch004-prompt-engineering-and-cache-boundaries.md) — written
- [5. Talking to the Model: API Client, Streaming, Retries, and Accounting](ch005-talking-to-the-model.md) — written
- [6. Executing Tools: Validation, Hooks, Concurrency, and Streaming Execution](ch006-executing-tools.md) — written
- [7. The Heartbeat: `query.ts` and the Agentic Turn State Machine](ch007-the-heartbeat-query-loop.md) — written
- [8. The Session Envelope: `QueryEngine.ts` and Transcript Persistence](ch008-session-envelope-query-engine-and-transcripts.md) — written

## Part III — Surviving Long Horizon: Context Management & Memory

- [9. The Five-Layer Compaction Pipeline](ch009-five-layer-compaction-pipeline.md) — written
- [10. Memory Systems: `CLAUDE.md`, Auto-Memory, Session Memory, and Auto-Dream](ch010-memory-systems-claudemd-auto-memory-and-dream.md) — written

## Part IV — Safety, Policy, and Extensibility

- [11. The Permission Engine: Modes, Rules, Classifiers, and Persistence](ch011-the-permission-engine.md) — written
- [12. Shell Execution, Sandboxing, and Core Workspace Tools](ch012-shell-execution-sandboxing-and-workspace-tools.md) — written
- [13. The Hook System: Intercepting Every Lifecycle Event](ch013-the-hook-system.md) — written
- [14. Configuration, Settings Precedence, and Enterprise Policy](ch014-configuration-settings-and-enterprise-policy.md) — written

## Part V — Dynamic Capabilities: MCP, Skills, Plugins, and LSP

- [15. Model Context Protocol (MCP): Clients, Transports, Auth, and Tool Search](ch015-model-context-protocol-mcp.md) — written
- [16. Skills, Plugins, Output Styles, and LSP Intelligence](ch016-skills-plugins-output-styles-and-lsp.md) — written

## Part VI — Concurrency, Tasks, Subagents, and Swarms

- [17. The Background Task Runtime: Unified State, Disk-Backed Output, Notifications, and Proactive Scheduling](ch017-background-task-runtime.md) — written
- [18. Subagents, Context Forking, and Git Worktrees](ch018-subagents-context-forking-and-worktrees.md) — written
- [19. Agent Teams, Swarms, and Coordinator Mode: Multi-Agent IPC, Permission Bubbling, and Pure Orchestration](ch019-agent-teams-swarms-and-coordinator-mode.md) — written

## Part VII — The Terminal UI Engine

- [20. Inside `ink/`: A Custom React Renderer for the Terminal](ch020-inside-ink-terminal-react-renderer.md) — written
- [21. The Interactive REPL: Screens, Input, Keybindings, and Vim Mode](ch021-the-interactive-repl-screens-keybindings-and-vim.md) — written

## Part VIII — Entrypoints, Startup, SDK, and Remote Control

- [22. Cold Start Mastery: From `entrypoints/cli.tsx` Through `init.ts` to `main.tsx`](ch022-cold-start-mastery-cli-init-main.md) — written
- [23. Machine-Readable Modes: Headless CLI, Agent SDK, and Control Protocols](ch023-machine-readable-modes-headless-sdk-and-control-protocols.md) — written
- [24. Across the Wire: Remote Sessions, Always-On Bridge, and Direct Connect](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md) — written

## Part IX — Epilogue: The Snapshot Meets the Shipped Product

- [25. Bringing It to the Present: From 2.1.88 to 2.1.331](ch025-bringing-it-to-the-present.md) — written

## Appendices

- [Appendix A — Feature Flags & Environment Knobs](appendix-a-feature-flags-and-env-knobs.md)
- [Appendix B — Missing Snapshot Boundaries](appendix-b-missing-snapshot-boundaries.md)
- [Appendix C — Mini-Harness Blueprint](appendix-c-mini-harness-blueprint.md)
