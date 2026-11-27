# Reverse Engineering Claude Code

> **A bottom-up, building-blocks reverse-engineering guide to the Claude Code codebase (2.1.88 through 2.1.331).**

This is the ninth book in the [OpenAgents book library](../../docs/books/README.md) and a full-book companion to [Coder's Claude Code field-guide chapter](../../coder/book/ch025-claude-code.md). It teaches the entire Claude Code architecture (~1,884 TypeScript/TSX files across 1,910 tracked repository files in the 2.1.88 snapshot, plus an epilogue mapping eight months of drift through 2.1.331) from the ground up. Instead of dumping a reference manual or diving headfirst into an 800 KB `main.tsx`, the book starts with the smallest zero-dependency atoms—branded IDs, a 34-line reactive store, an 8-line lazy schema wrapper, an 88-line bounded async-generator multiplexer—and builds upward layer by layer until every subsystem snaps into place.

By the final chapter, you will not just know *where* features live in Claude Code; you will understand *why* every abstraction was forced into existence by real production constraints: prompt-cache economics, cold-start latency, circular imports, concurrent tool execution, permission boundaries, context-window exhaustion, terminal rendering at 60 FPS, and multi-agent coordination.

---

## Who This Book Is For

- **Engineers building AI agents and coding assistants** who want to see how a production-grade agent harness solves tool scheduling, permissions, compaction, hooks, MCP, subagents, and streaming UI.
- **Systems-minded TypeScript developers** curious how a massive CLI/TUI application manages module graphs, dead-code elimination, custom React reconciliation in the terminal, and zero-dependency state stores.
- **Codebase archaeologists** who want a disciplined, source-verified map of every layer in the Claude Code architecture.

---

## Pedagogical Method: Strict Bottom-Up Assembly

Every chapter follows the same rule: **no magic boxes**.

1. **No forward dependencies.** A chapter only builds on primitives explained in earlier chapters (or explicitly flags a forward reference as a seam to be filled in later).
2. **Every claim cites real code.** Every interface, invariant, and edge case points to exact repo-relative file paths and symbols verified directly against the pinned 2.1.88 snapshot (and tagged with evidence labels in Chapter 25 for 2.1.89–2.1.331).
3. **Explain the *why* before the *what*.** Before reading an implementation, each section frames the production failure mode that forced the design—cache busting, NTLM credential leaks on UNC paths, import cycles, stale usage counts after compaction, or subagents mutating the parent's React tree.

---

## Repository Snapshot Caveats

This book is an external-system reverse-engineering manuscript (`status: "written"` in [`chapters.json`](chapters.json)). The proprietary Claude Code TypeScript source tree is **not** vendored in the OpenAgents repository; source citations such as `Tool.ts`, `query.ts`, and `state/store.ts` refer to the pinned 2.1.88 snapshot. Readers should know five concrete facts up front (covered in full in [Chapter 00](prologue-how-to-read-a-1900-file-agent.md)):

1. **Version: Claude Code 2.1.88 (with Chapter 25 carrying forward to 2.1.331).** The snapshot carries no version string (the build injects `MACRO.VERSION`), so the version comes from matching the code against the [official changelog](https://code.claude.com/docs/en/changelog). Everything listed through 2.1.87 is present (for example the `X-Claude-Code-Session-Id` header and the `.jj`/`.sl` exclusions from 2.1.86, and the `if` field on hooks from 2.1.85). The 2.1.89 entry, which also covers the unlisted 2.1.88, is largely present too: the `PermissionDenied` hook, the `"defer"` hook decision, `CLAUDE_CODE_NO_FLICKER`, `showThinkingSummaries`, `TaskCreated` and `/buddy`. Nothing from 2.1.90 onward is in the snapshot; 2.1.88 is the build published on 2026-03-31.
2. **Source-only snapshot:** There is no tracked `package.json`, `tsconfig.json`, lockfile, or test runner config in the snapshot. Treat code listings as architectural truth rather than a runnable build recipe.
3. **A few imported files are absent:** Some modules imported by tracked source—most notably `types/message.ts`, `query/transitions.ts`, `services/contextCollapse/`, and `services/skillSearch/`—are not present in the snapshot. Where a chapter touches those boundaries, it documents what can be verified from call sites and marks absent definitions explicitly (cataloged in [Appendix B](appendix-b-missing-snapshot-boundaries.md)).
4. **Build-time feature gating (`bun:bundle`):** Many modules sit behind `feature('...')` guards and `process.env.USER_TYPE === 'ant'` checks for Bun dead-code elimination (DCE). Presence in the snapshot does not mean a feature ships in every build (cataloged in [Appendix A](appendix-a-feature-flags-and-env-knobs.md)).
5. **React Compiler output in `.tsx` files:** Tracked `.tsx` files include React Compiler memoization transforms (`import { c as _c } from "react/compiler-runtime"`) and inline base64 source maps containing the original pre-compiled TSX.

---

## Directory Layout

```text
claude-code/book/
├── README.md                                                     # You are here
├── SUMMARY.md                                                    # Book navigation & SSR table of contents
├── chapters.json                                                 # Machine-checked chapter registry (25 written chapters)
├── examples.json                                                 # Empty Coax example inventory (written survey book)
├── OUTLINE.md                                                    # Complete 9-part, 25-chapter curriculum
├── ARCHITECTURE_MAP.md                                           # Visual layer cake, dependency DAG, cross-cutting themes
├── EDITORIAL_GUIDE.md                                            # Authoring craft, chapter template, verification rules
├── GLOSSARY.md                                                   # Living dictionary of codebase types, terms, and gates
├── prologue-how-to-read-a-1900-file-agent.md                     # Prologue (Ch 00): Codebase anatomy & 5 recurring idioms
├── ch001-atoms-types-stores-and-primitives.md                    # Ch 1: Layer 0 leaf primitives
├── ch002-the-tool-contract.md                                    # Ch 2: Tool.ts, buildTool, GlobTool, tools.ts
├── ch003-command-and-message-contracts.md                        # Ch 3: Command contract, processUserInput, messages.ts
├── ch004-prompt-engineering-and-cache-boundaries.md              # Ch 4: context.ts, system prompt & cache boundaries
├── ch005-talking-to-the-model.md                                 # Ch 5: API client, streaming, retries, usage & cost
├── ch006-executing-tools.md                                      # Ch 6: Tool execution, hooks, batching & spill storage
├── ch007-the-heartbeat-query-loop.md                             # Ch 7: query.ts AsyncGenerator turn state machine
├── ch008-session-envelope-query-engine-and-transcripts.md        # Ch 8: QueryEngine.ts, JSONL transcripts & /rewind
├── ch009-five-layer-compaction-pipeline.md                       # Ch 9: 5-stage compaction pipeline & circuit breaker
├── ch010-memory-systems-claudemd-auto-memory-and-dream.md        # Ch 10: CLAUDE.md, memdir, SessionMemory & autoDream
├── ch011-the-permission-engine.md                                # Ch 11: Permission modes, rules, classifier & modals
├── ch012-shell-execution-sandboxing-and-workspace-tools.md       # Ch 12: Bash/PowerShell, sandbox & workspace tools
├── ch013-the-hook-system.md                                      # Ch 13: Hook events, 4 backends, SSRF guard & snapshots
├── ch014-configuration-settings-and-enterprise-policy.md         # Ch 14: Settings cascade, live watch, MDM & migrations
├── ch015-model-context-protocol-mcp.md                           # Ch 15: MCP transports, OAuth/XAA, elicitation & ToolSearch
├── ch016-skills-plugins-output-styles-and-lsp.md                 # Ch 16: Skills, 3-layer plugins, output styles & LSP
├── ch017-background-task-runtime.md                              # Ch 17: 7 task kinds, disk outputs, cron & sleep wakeups
├── ch018-subagents-context-forking-and-worktrees.md              # Ch 18: Subagent isolation, cache-sharing forks & worktrees
├── ch019-agent-teams-swarms-and-coordinator-mode.md              # Ch 19: Teammate mailboxes, permission bridge & coordinator
├── ch020-inside-ink-terminal-react-renderer.md                   # Ch 20: Custom React renderer, pure-TS Yoga & Screen diffing
├── ch021-the-interactive-repl-screens-keybindings-and-vim.md     # Ch 21: REPL.tsx, QueryGuard, keybinding chords & Vim mode
├── ch022-cold-start-mastery-cli-init-main.md                     # Ch 22: cli.tsx fast-paths, init.ts, main.tsx & shutdown
├── ch023-machine-readable-modes-headless-sdk-and-control-protocols.md # Ch 23: print.ts, StructuredIO NDJSON & MCP server mode
├── ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md # Ch 24: REPL bridge, CCR transports & upstreamproxy
├── ch025-bringing-it-to-the-present.md                           # Ch 25: Epilogue — 2.1.88 to 2.1.331 changelog & binary diff
├── appendix-a-feature-flags-and-env-knobs.md                     # App A: Complete 90-flag feature() & env-knob inventory
├── appendix-b-missing-snapshot-boundaries.md                     # App B: 187-target boundary audit & call-site reconstructions
└── appendix-c-mini-harness-blueprint.md                          # App C: ~500-line educational toy clone & production crosswalk
```

---

## Reference Guides, Prologue, and Appendices

| Guide | File | Role |
| --- | --- | --- |
| Master Curriculum & Chapter Blueprints | [OUTLINE.md](OUTLINE.md) | 9-part, 25-chapter blueprint |
| Layered Architecture & Dependency Map | [ARCHITECTURE_MAP.md](ARCHITECTURE_MAP.md) | Visual 9-layer cake & dependency DAG |
| Authoring & Verification Guide | [EDITORIAL_GUIDE.md](EDITORIAL_GUIDE.md) | Craft, chapter template & verification rules |
| Codebase Glossary | [GLOSSARY.md](GLOSSARY.md) | Types, terms, and build gates (v6) |
| Prologue (Chapter 00) | [How to Read a 1,900-File Agent](prologue-how-to-read-a-1900-file-agent.md) | Codebase anatomy & 5 recurring idioms |
| Appendix A | [Feature Flags & Environment Knobs](appendix-a-feature-flags-and-env-knobs.md) | 90 `feature()` gates & env-knob inventory |
| Appendix B | [Missing Snapshot Boundaries](appendix-b-missing-snapshot-boundaries.md) | 187-target audit & call-site reconstructions |
| Appendix C | [Mini-Harness Blueprint](appendix-c-mini-harness-blueprint.md) | ~500-line toy clone & production crosswalk |

---

## Chapters

All 25 numbered chapters are complete (`written` status in [`chapters.json`](chapters.json)):

| Chapter | Status |
| --- | --- |
| [1. The Atoms: Identity, Reactive Stores, Lazy Schemas, and Async Generators](ch001-atoms-types-stores-and-primitives.md) | Written: anthropic/claude-code@2.1.88 |
| [2. The Tool Contract: Defining What an Agent Can Do](ch002-the-tool-contract.md) | Written: anthropic/claude-code@2.1.88 |
| [3. The Command & Message Contracts: User Input Meets the Model](ch003-command-and-message-contracts.md) | Written: anthropic/claude-code@2.1.88 |
| [4. Prompt Engineering as Systems Engineering: Context & Cache Boundaries](ch004-prompt-engineering-and-cache-boundaries.md) | Written: anthropic/claude-code@2.1.88 |
| [5. Talking to the Model: API Client, Streaming, Retries, and Accounting](ch005-talking-to-the-model.md) | Written: anthropic/claude-code@2.1.88 |
| [6. Executing Tools: Validation, Hooks, Concurrency, and Streaming Execution](ch006-executing-tools.md) | Written: anthropic/claude-code@2.1.88 |
| [7. The Heartbeat: `query.ts` and the Agentic Turn State Machine](ch007-the-heartbeat-query-loop.md) | Written: anthropic/claude-code@2.1.88 |
| [8. The Session Envelope: `QueryEngine.ts` and Transcript Persistence](ch008-session-envelope-query-engine-and-transcripts.md) | Written: anthropic/claude-code@2.1.88 |
| [9. The Five-Layer Compaction Pipeline](ch009-five-layer-compaction-pipeline.md) | Written: anthropic/claude-code@2.1.88 |
| [10. Memory Systems: `CLAUDE.md`, Auto-Memory, Session Memory, and Auto-Dream](ch010-memory-systems-claudemd-auto-memory-and-dream.md) | Written: anthropic/claude-code@2.1.88 |
| [11. The Permission Engine: Modes, Rules, Classifiers, and Persistence](ch011-the-permission-engine.md) | Written: anthropic/claude-code@2.1.88 |
| [12. Shell Execution, Sandboxing, and Core Workspace Tools](ch012-shell-execution-sandboxing-and-workspace-tools.md) | Written: anthropic/claude-code@2.1.88 |
| [13. The Hook System: Intercepting Every Lifecycle Event](ch013-the-hook-system.md) | Written: anthropic/claude-code@2.1.88 |
| [14. Configuration, Settings Precedence, and Enterprise Policy](ch014-configuration-settings-and-enterprise-policy.md) | Written: anthropic/claude-code@2.1.88 |
| [15. Model Context Protocol (MCP): Clients, Transports, Auth, and Tool Search](ch015-model-context-protocol-mcp.md) | Written: anthropic/claude-code@2.1.88 |
| [16. Skills, Plugins, Output Styles, and LSP Intelligence](ch016-skills-plugins-output-styles-and-lsp.md) | Written: anthropic/claude-code@2.1.88 |
| [17. The Background Task Runtime: Unified State, Disk-Backed Output, Notifications, and Proactive Scheduling](ch017-background-task-runtime.md) | Written: anthropic/claude-code@2.1.88 |
| [18. Subagents, Context Forking, and Git Worktrees](ch018-subagents-context-forking-and-worktrees.md) | Written: anthropic/claude-code@2.1.88 |
| [19. Agent Teams, Swarms, and Coordinator Mode: Multi-Agent IPC, Permission Bubbling, and Pure Orchestration](ch019-agent-teams-swarms-and-coordinator-mode.md) | Written: anthropic/claude-code@2.1.88 |
| [20. Inside `ink/`: A Custom React Renderer for the Terminal](ch020-inside-ink-terminal-react-renderer.md) | Written: anthropic/claude-code@2.1.88 |
| [21. The Interactive REPL: Screens, Input, Keybindings, and Vim Mode](ch021-the-interactive-repl-screens-keybindings-and-vim.md) | Written: anthropic/claude-code@2.1.88 |
| [22. Cold Start Mastery: From `entrypoints/cli.tsx` Through `init.ts` to `main.tsx`](ch022-cold-start-mastery-cli-init-main.md) | Written: anthropic/claude-code@2.1.88 |
| [23. Machine-Readable Modes: Headless CLI, Agent SDK, and Control Protocols](ch023-machine-readable-modes-headless-sdk-and-control-protocols.md) | Written: anthropic/claude-code@2.1.88 |
| [24. Across the Wire: Remote Sessions, Always-On Bridge, and Direct Connect](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md) | Written: anthropic/claude-code@2.1.88 |
| [25. Bringing It to the Present: From 2.1.88 to 2.1.331](ch025-bringing-it-to-the-present.md) | Written: anthropic/claude-code@2.1.88..2.1.331 |

---

## Cross-Reference to OpenAgents Coax Counterparts

While this book reverse-engineers Claude Code's TypeScript architecture in full, [Coder: Coding Agents From Scratch](../../coder/book/README.md) builds OpenAgents' own pure-Coax harness (`openagents/core/`, `coder/tui/`, `apps/openagents-desktop/`) and summarizes what it borrows and leaves in [Chapter 25: Claude Code](../../coder/book/ch025-claude-code.md) (plus driving the `claude` CLI directly in [Chapter 10: Drive Claude Code](../../coder/book/ch010-drive-claude-code.md)). Use the table below to jump from a Claude Code subsystem to its OpenAgents Coax counterpart:

| Subsystem | Reverse Engineering Claude Code | OpenAgents Coax Counterpart (`coder/book/`) |
| --- | --- | --- |
| Agentic turn & query loop | [Ch 7: The Heartbeat (`query.ts`)](ch007-the-heartbeat-query-loop.md) | [Ch 1: The simplest loop](../../coder/book/ch001-the-simplest-loop.md) |
| Tool contract, validation & batching | [Ch 2: The Tool Contract](ch002-the-tool-contract.md), [Ch 6: Executing Tools](ch006-executing-tools.md) | [Ch 4: Tools as typed capabilities](../../coder/book/ch004-tools-as-typed-capabilities.md) |
| Permission engine & OS sandboxing | [Ch 11: The Permission Engine](ch011-the-permission-engine.md), [Ch 12: Shell Execution & Sandboxing](ch012-shell-execution-sandboxing-and-workspace-tools.md) | [Ch 5: A sandbox and approvals](../../coder/book/ch005-a-sandbox-and-approvals.md) |
| History condensation & compaction | [Ch 9: The Five-Layer Compaction Pipeline](ch009-five-layer-compaction-pipeline.md) | [Ch 7: Condense the history](../../coder/book/ch007-condense-the-history.md) |
| Subagents, context forking & worktrees | [Ch 18: Subagents, Context Forking, and Git Worktrees](ch018-subagents-context-forking-and-worktrees.md) | [Ch 14: Forks, subagents and jobs](../../coder/book/ch014-forks-subagents-and-jobs.md), [Ch 41: Worker protocol and sub-agents](../../coder/book/ch041-worker-protocol-and-sub-agents.md) |
| Terminal rendering & interactive UI | [Ch 20: Inside `ink/`](ch020-inside-ink-terminal-react-renderer.md), [Ch 21: The Interactive REPL](ch021-the-interactive-repl-screens-keybindings-and-vim.md) | [Ch 36: A terminal UI](../../coder/book/ch036-a-terminal-ui.md), [Ch 40: The desktop app](../../coder/book/ch040-the-desktop-app.md) |
| Headless SDK & worker control protocol | [Ch 23: Machine-Readable Modes](ch023-machine-readable-modes-headless-sdk-and-control-protocols.md), [Ch 25: 2.1.88 to 2.1.331](ch025-bringing-it-to-the-present.md) | [Ch 10: Drive Claude Code](../../coder/book/ch010-drive-claude-code.md), [Ch 41: Worker protocol and sub-agents](../../coder/book/ch041-worker-protocol-and-sub-agents.md) |

---

## Where to Start Reading

1. Start with [**ARCHITECTURE_MAP.md**](ARCHITECTURE_MAP.md) to get the visual 9-layer map in your head.
2. Read [**Chapter 00: How to Read a 1,900-File Agent**](prologue-how-to-read-a-1900-file-agent.md) to learn the five recurring idioms that appear in almost every file.
3. Follow Chapters 1–24 in order—building from leaf atoms (`types/ids.ts`, `state/store.ts`) all the way up to `entrypoints/cli.tsx`, `main.tsx`, SDK control protocols, and the remote bridge—then finish with [Chapter 25](ch025-bringing-it-to-the-present.md) to bridge from 2.1.88 to 2.1.331.
4. Keep [**GLOSSARY.md**](GLOSSARY.md) and Appendices [**A**](appendix-a-feature-flags-and-env-knobs.md), [**B**](appendix-b-missing-snapshot-boundaries.md), and [**C**](appendix-c-mini-harness-blueprint.md) open as reference guides while exploring the architecture or building your own agent harness (see also [Coder: Coding Agents From Scratch](../../coder/book/README.md)).
