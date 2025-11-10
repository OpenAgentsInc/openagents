# Test Scenarios — FM Tool Calling → Codex Delegation

Purpose: make it easy to validate end‑to‑end flows now that the Apple FM session can decide to call `codex.run` and the desktop server dispatches a real Codex run.

Key code paths
- Chat provider + FM tool: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift`
- Server invoke + persistence: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/`
- Codex provider: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CodexAgentProvider.swift`
- Timeline + UI: `ios/OpenAgents/Bridge/TimelineStore.swift`, `ios/OpenAgents/Views/macOS/ChatAreaView.swift`

How this works
1) FM decides to use `codex.run` during a chat turn.
2) We emit an ACP `tool_call` (visible in UI/inspector) with structured args.
3) We switch the session’s mode to `.codex` and call `localSessionPrompt` to run Codex for real.
4) Codex streams updates; `SessionUpdateHub` persists to Tinyvex and publishes to UI.

Pre‑requisites
- macOS app build runs on macOS 13+.
- For “happy path” real execution, Codex CLI is in PATH; otherwise tests still pass by observing the provider‑not‑available error in the stream.

Test format
- For each scenario, we give: Setup → Action → Expect.
- Where applicable, note “Automated” pointers (Combine listener or unit test hook) and “Manual” steps (UI click path).

---

## A. Model‑initiated Delegation

1) FM tool call appears (no heuristics)
- Setup: Start new chat in default mode. FM available (iOS/macOS 26+), otherwise skip.
- Action: Prompt that requires workspace ops, e.g., “Search the repo for TODOs and list the files.”
- Expect: ACP `tool_call` with `tool_name = codex.run` is published; `tool_call.arguments` includes `user_prompt` and optional `workspace_root` if set.
- Automated: Subscribe to `notificationPublisher` and decode `ACP.Client.SessionNotificationWire`.

2) Mode switch to `.codex` when tool fires
- Setup: Same as above.
- Action: First Codex delegation of the session.
- Expect: A `current_mode_update` with `current_mode_id = codex` is present in the timeline near the tool call.
- Manual: Toggle inspector to confirm timeline ordering; Automated: parse Tinyvex timeline via `HistoryApi`.

3) Real Codex execution (CLI present)
- Setup: Codex CLI installed; `server.workingDirectory` set.
- Action: Prompt “Run a quick grep for ‘FIXME’ and summarize results.”
- Expect: After the tool_call, agent messages stream from Codex; no “provider not available” error.

4) CLI missing fallback
- Setup: Temporarily make Codex unavailable (rename binary in PATH or run on a machine without it).
- Action: Same prompt.
- Expect: We still see `tool_call`, followed by an assistant chunk indicating Codex isn’t available (error string). No crash.

---

## B. Parameter Mapping

5) Workspace root mapping
- Setup: Set `workspace_root` to the repo root.
- Action: FM emits `codex.run` with `workspace_root`.
- Expect: `DesktopWebSocketServer.workingDirectory` equals that path before Codex prompt; Codex args contain `--cd <root>`.
- Automated: Watch server’s debug logs or assert `server.workingDirectory?.path`.

6) Include globs
- Setup: FM tool args include `files_include_glob = ["**/*", "docs/**/*.md"]`.
- Action: `codex.run` fired.
- Expect: `tool_call.arguments.files_include_glob` matches; prompt text includes an “Include:” line with these patterns.

7) Summarize + max_files
- Setup: `summarize=true`, `max_files=250`.
- Action: `codex.run` fired.
- Expect: Prompt contains “Summarize: yes” and “Max files: 250”.

8) Description + user prompt composition
- Setup: Provide both.
- Action: `codex.run` fired.
- Expect: The composed prompt begins “OpenAgents → Codex delegation”, includes `Description:` and the `user_prompt` body afterward.

---

## C. Session + History Semantics

9) First run vs resume
- Setup: Two consecutive prompts that both trigger Codex; Codex CLI present.
- Action: Prompt A triggers new run; Prompt B triggers resume (Codex thread id is captured).
- Expect: Second run includes `resume <thread>`. UI shows continuous conversation.

10) Persistence
- Setup: Trigger Codex run, then restart app.
- Action: Load recent session from sidebar.
- Expect: Timeline shows `tool_call`, `current_mode_update`, and Codex messages from Tinyvex; inspector still shows arguments.

11) Cancel in progress
- Setup: Long Codex run (e.g., grep large patterns).
- Action: Issue `session/cancel` (UI: Cmd‑.` or dev tool).
- Expect: Running process terminates; timeline records cancellation; no zombie process.

---

## D. UI & Accessibility

12) Inspector detail
- Setup: Click a `codex.run` row.
- Action: Open Tool Call detail sheet.
- Expect: Header shows tool name/ID/status; “Arguments” pretty JSON; “Output” JSON if present; Copy buttons work.

13) Transcript layout
- Setup: Bubbles present (user right, assistant plain text).
- Expect: Centered message column; right‑aligned user; no mode banners.

14) Export
- Setup: Export JSON/Markdown transcript.
- Expect: Tool calls and updates included; readable summary for Markdown.

---

## E. Negative & Edge Cases

15) Path outside workspace
- Setup: `workspace_root` valid, but tool attempts `/etc/passwd` in description.
- Expect: Security rule applies; Codex (and any helper tools) must not read outside the workspace; error surfaced.

16) Massive globs
- Setup: `files_include_glob` includes overly broad patterns.
- Expect: Codex prompt is still formed, but provider enforces internal caps (max files) and produces a warning in output.

17) FM unavailable
- Setup: macOS < 26 or FM disabled.
- Action: Chat prompts that would normally trigger delegation.
- Expect: No `tool_call`; deterministic fallback assistant message.

18) Concurrent tool calls
- Setup: Fire two prompts quickly that both delegate.
- Expect: Two tool_call events, unique call IDs, interleaved but consistent agent output; no timeline corruption.

---

## Automation hooks & tips

- Programmatic listener: `DesktopWebSocketServer.notificationPublisher` → decode `ACP.Client.SessionNotificationWire`.
- Server helpers: `localSessionSetMode`, `localSessionPrompt`, `localHistorySessionTimeline`.
- Tinyvex queries: `HistoryApi.recentSessions`, `HistoryApi.sessionTimeline`.
- Tests live examples: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/OpenAgentsLocalProviderFMToolTests.swift`.

## Future scenarios (to add later)

- Claude Code parity: `claude.run` tool (same mapping and expectations).
- Structured hand‑off: pass a JSON block to Codex CLI with include/exclude patterns vs prompt text only.
- Orchestrated multi‑step flows: FM uses multiple tools before/after Codex delegation.

