Codex Gap Analysis

Scope
- Sources reviewed: `/home/christopherdavid/code/codex/README.md`, `/home/christopherdavid/code/codex/codex-rs/README.md`, `/home/christopherdavid/code/codex/docs/*.md`, `/home/christopherdavid/code/codex/codex-cli/README.md`, and OpenAgents Codex integration code/docs (notably `crates/codex-agent-sdk/README.md`, `crates/autopilot/src/app/agents/codex_backend.rs`, `crates/autopilot/src/app_entry/coder_actions.rs`, `crates/compute/README.md`, `crates/oanix/docs/README.md`, `crates/rlm/src/bin/rlm_mcp_server.rs`).
- Goal: map Codex CLI capabilities to OpenAgents implementation coverage and prioritize integration gaps.

Codex feature inventory (from codex repo docs)
- Core CLI flows: interactive TUI, non-interactive `codex exec`, streaming output.
- Auth: ChatGPT OAuth login and API key auth.
- Config: `~/.codex/config.toml` with advanced settings, MCP config, notifications.
- Approvals + sandboxing: approval modes (suggest/auto-edit/full-auto), sandbox policies (`read-only`, `workspace-write`, `danger-full-access`), platform sandbox behavior.
- Slash commands and skills: command surface for interactive control; skills system; custom prompts.
- AGENTS.md support: hierarchical guidance toggle via feature flag.
- MCP support: client for connecting to external MCP servers; experimental MCP server (`codex mcp-server`).
- Notifications: post-turn notification hook + OS-specific notification behavior.
- Multi-provider support (legacy TS CLI): configurable provider backends and base URLs.

OpenAgents current coverage (high level)
- Codex execution via SDK: `codex-agent-sdk` is used by Autopilot and Adjutant for headless runs and streaming events.
- Sandbox + approvals: OpenAgents maps CoderMode to Codex SDK sandbox + approval policy (read-only/workspace-write/danger-full-access, approval modes).
- UI: Autopilot GPU UI with streaming, tool cards, session state. (Command palette currently disabled per request.)
- MCP: OpenAgents has its own MCP servers (e.g., `rlm-mcp-server`), plus internal MCP management in Autopilot UI, but does not expose OpenAgents as a Codex-compatible MCP server.
- Compute backend: `CodexCodeBackend` exists in compute for sandboxed Codex CLI execution.
- Observability: rlog recorder, session checkpoints, and system status panels.

Gap analysis (Codex vs OpenAgents)

Feature: Codex CLI interactive UX (TUI, slash commands)
- Codex: interactive TUI + slash commands.
- OpenAgents: Autopilot UI supports commands, but does not implement Codex CLI UX parity or Codex slash command surface.
- Gap: no direct Codex CLI-compatible interaction layer in OpenAgents.

Feature: `codex exec` non-interactive mode
- Codex: dedicated non-interactive execution mode.
- OpenAgents: no OpenAgents CLI wrapper for Codex exec; headless runs are in Autopilot/Adjutant internals.
- Gap: missing direct CLI parity for automation / CI usage.

Feature: Config parity with `~/.codex/config.toml`
- Codex: rich config, MCP config, notifications, exec policy.
- OpenAgents: `.openagents` config and Autopilot settings; no explicit import/bridge from Codex config.
- Gap: no shared config bridge, users must duplicate settings.

Feature: Authentication UX
- Codex: ChatGPT OAuth and API key flows in CLI.
- OpenAgents: relies on Codex CLI being installed/authenticated via SDK; no OpenAgents UX for Codex OAuth state.
- Gap: no auth status UI or guidance for Codex auth errors beyond basic detection.

Feature: Approval modes + sandbox policy
- Codex: explicit approval modes and sandbox modes with dedicated CLI flags.
- OpenAgents: CoderMode maps to sandbox + approval via SDK; no full parity with Codex CLI modes (suggest/auto-edit/full-auto semantics).
- Gap: missing explicit mode selection parity and per-command execution policy surface.

Feature: Skills + custom prompts
- Codex: skills + custom prompt docs and runtime.
- OpenAgents: OpenAgents has a distinct skill system (SKILL.md) and internal prompts, but no integration with Codex skills or prompt packs.
- Gap: no compatibility layer or sharing of Codex skill definitions.

Feature: AGENTS.md hierarchical mode
- Codex: explicit hierarchical AGENTS feature flag.
- OpenAgents: AGENTS.md is used as context injection, but no hierarchical precedence flag or Codex-style behavior.
- Gap: lacks explicit hierarchical AGENTS support and consistent precedence semantics.

Feature: MCP client + MCP server behavior
- Codex: MCP client and experimental MCP server.
- OpenAgents: has MCP servers (RLM) and MCP management UI, but no Codex-compatible MCP server surface or Codex tooling alignment.
- Gap: no bridge between Codex MCP expectations and OpenAgents MCP interfaces.

Feature: Notifications on turn completion
- Codex: notification hook + WSL fallback.
- OpenAgents: no Codex-compatible notification hook for Codex SDK turns.
- Gap: missing out-of-band notifications for long runs.

Feature: Multi-provider config (legacy CLI)
- Codex TS CLI: provider model configuration and base URLs.
- OpenAgents: OpenAgents has gateway + LM router, but no Codex CLI provider config compatibility layer.
- Gap: no migration path for Codex CLI provider config -> OpenAgents gateway.

Top integration priorities (recommended order)

1) Codex config bridge (read-only import + mapping)
   - Why: reduces friction for Codex CLI users and enables consistent settings for sandbox, approvals, MCP, and notifications.
   - Scope: parse `~/.codex/config.toml` into OpenAgents settings and display mismatches; no behavior change unless user opts in.

2) Non-interactive CLI parity (`codex exec` equivalent)
   - Why: unlocks automation/CI use and allows OpenAgents to replace Codex CLI in scripts without UI.
   - Scope: add `openagents exec` wrapper that invokes Codex SDK thread run with structured output + exit codes.

3) Explicit approval + sandbox mode parity
   - Why: Codex users expect suggest/auto-edit/full-auto semantics; mapping reduces confusion.
   - Scope: implement mode presets matching Codex CLI and surface them in Autopilot UI/CLI flags.

4) MCP interop layer (Codex-compatible MCP server)
   - Why: Codex ecosystem tooling expects Codex MCP semantics; bridging lets OpenAgents act as a drop-in.
   - Scope: add an MCP server facade that exposes OpenAgents sessions and tools in Codex MCP schema.

5) Notifications + AGENTS hierarchy toggle
   - Why: quality-of-life improvements for long runs and instruction scoping parity.
   - Scope: add a post-turn notification hook (configurable) and a hierarchical AGENTS mode flag.

6) Skills compatibility shim (optional)
   - Why: longer tail, but enables sharing skills/prompt packs with Codex ecosystem.
   - Scope: define import/export mapping between Codex skills and OpenAgents SKILL.md.

Notes / assumptions
- This analysis uses codex repo docs plus OpenAgents usage of `codex-agent-sdk`. If Codex features exist outside those docs (e.g., TUI2-specific UX or new config flags), they should be added as a follow-up pass.

Recent implementation check (last ~10 commits)
- App-server integration advanced: `9c8198cab`, `cd6d4d694`, `bf357856d`, `28ee37b89`, `55c0931c6`, `688c72bd4`, `41a49dd03` add Codex app-server transport + approvals wiring + event mapping + docs. This reduces the MCP/interop gap for running Codex-style workflows in OpenAgents, but it is not yet a Codex MCP server or config bridge.
- Codex naming sweep + fixes: `c77a55895` aligns naming and fixes build/test failures; no functional parity gains for Codex CLI features.
- Command palette disabled: `af4861f95` removes command palette access in Autopilot and Pylon Desktop; this widens the interactive UX gap relative to Codex CLI (by request).
- Gap analysis doc added: `4341566f5` is the base document; this section updates it.

Implications for priority order
- MCP interop remains a gap despite app-server progress; treat the new app-server transport as a precursor, not a replacement, for a Codex-compatible MCP surface.
- Config bridge + non-interactive CLI parity are still the fastest wins to close friction for Codex CLI users.
