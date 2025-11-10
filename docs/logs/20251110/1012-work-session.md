# 2025‑11‑10 10:12 — Work Session Log (macOS Chat + FM → Codex Delegation)

## TL;DR
- Cleaned up macOS chat UI (black theme, centered transcript, simplified sidebar, composer‑only empty state).
- Improved tool call detail sheet (arguments/output, copy actions, themed UI).
- Enabled Apple Foundation Models (FM) to decide tool usage in chat and wired `codex.run` to trigger a real Codex run (no prompt heuristics).
- Added server reference to `AgentContext` so in‑process tools can invoke the desktop server.
- Wrote tests for the FM codex tool path and created a testing scenarios doc.
- Opened a GitHub issue to track the FM→Codex wiring.

## Changes by Area

### UI (macOS)
- Sidebar
  - Kept only “New Chat” and the sessions list; hid agent selector + search for now.
  - Restyled “New Chat” to OATheme gray (removed system blue): rounded rect, border stroke.
- Chat Area
  - Removed center placeholder; composer is the only visible element when empty.
  - Centered the transcript column (fixed max width ~820px).
  - Right‑aligned user messages (flush right). Assistant messages have no background.
  - Hid “Mode: …” rows in the transcript.
- Tool Call Detail sheet
  - Replaced generic NavigationView with a custom, all‑black sheet.
  - Header shows tool name/ID/status; Arguments and Output sections with pretty JSON; Copy buttons.
  - Falls back to timeline to render args/output if the current row is an update.

Commits: c277e110, d7a786e5, a2187053, bbd5ed95, 69ec15c7

### FM → Codex Delegation (Chat)
- Removed keyword heuristic in `OpenAgentsLocalProvider` (previously fired when prompt contained “codex”).
- Registered an FM tool `codex.run` in the chat FM session so the model chooses when to delegate.
- Implemented `FMTool_CodexRun`:
  - Emits ACP `tool_call` with structured args (provider, task, description, user_prompt, workspace_root, files_include_glob, summarize, max_files).
  - Applies `workspace_root` to `DesktopWebSocketServer.workingDirectory`.
  - Switches session to `.codex` and issues `localSessionPrompt` with a composed prompt including mapped parameters.
- Added `server` reference to `AgentContext` so in‑process tools can call server helpers.
- Desktop server now passes `server: self` in both RPC and local paths.

Commits: 5c2d6495, e7328b49

### Tests
- New test: `OpenAgentsLocalProviderFMToolTests.testFMToolCodexRunEmitsToolCallAndInvokesServer` (macOS)
  - Asserts ACP `tool_call` appears with `codex.run`.
  - Verifies `files_include_glob` present in tool args.
  - Verifies `server.workingDirectory` matches provided `workspace_root`.
  - Observes a follow‑up agent message (either real Codex output if available or a graceful provider‑not‑available error).

Commit: c39144c0

### Docs
- Testing scenarios doc: `docs/testing/011-codex-delegation-fm-tool-scenarios.md`
  - 18 scenarios covering model‑initiated delegation, parameter mapping, session/history semantics, UI, and edge cases.
  - Includes automation hooks (Combine publisher, History API) and pointers to code/tests.

Commit: d885949c

### Issue Tracking
- Opened: Wire FM `codex.run` tool to real Codex agent (model‑initiated)
  - https://github.com/OpenAgentsInc/openagents/issues/1469

### Ancillary
- Minor build/test deflakes and protocol fixes:
  - Adjusted tests for preferred mode mapping & flakiness.
  - Added Combine imports where needed.
  - Fixed optional‑chaining on non‑optional `OvernightTask` and related timeline polls.

Commits: 09f0e7b2, 739779d1, cbf224ca

## Rationale & Notes
- FM tool calling aligns with ADR‑0006 and removes brittle prompt parsing; the model decides if/when to delegate.
- Emitting `tool_call` before real execution preserves visibility and a consistent UI/inspector experience.
- Using `AgentContext.server` provides a clean seam for in‑process tools without introducing global singletons.
- For now, globs/summarize/max_files are embedded in the Codex prompt; we can later extend `CodexAgentProvider` to accept structured options.

## Follow‑ups
1) Codex structured options
   - Parse options from `AgentContext.metadata` or extend provider args to forward include globs/summarize/max files as flags/JSON.
   - Add a test asserting CLI args formation and output.
2) Timeline round‑trip test
   - Validate ordering: `tool_call` → `current_mode_update` → agent message stream → (optional) `tool_call_update`.
3) Claude Code parity
   - Implement `claude.run` FM tool mirroring `codex.run`.
4) UI polish
   - Syntax highlighting in JSON views; inspector “Run in Terminal” / “Open in Editor” developer affordances.

## Validation Checklist (Manual)
- New chat shows only the composer when empty; sending right‑aligns user messages.
- Tool call sheet shows arguments/output; copy actions work.
- Asking FM to perform repo‑level actions (grep/search) produces a `codex.run` `tool_call` and streams Codex output when available.
- Sidebar only shows “New Chat” + sessions list (no agent selector/search for now).

---

If something regresses, flip back to the heuristic path by re‑enabling the legacy branch in `OpenAgentsLocalProvider.streamResponse` (not recommended) or disable FM on incompatible systems (the deterministic fallback remains intact).

