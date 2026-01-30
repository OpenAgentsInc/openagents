# Autopilot x OpenClaw integration ideas

This note is based on the OpenClaw repo at `~/code/openclaw` and highlights
integration surfaces that look stable and intended for extension. It focuses on
places where Autopilot can either:
- be invoked from OpenClaw (delegate a code task), or
- use OpenClaw as its messaging/control plane (deliver updates, ask for approvals,
  access nodes/tools).

## OpenClaw surfaces that are designed for integration

1) Gateway WS protocol (operator or node clients)
- Docs: `docs/concepts/architecture.md`, `docs/gateway/protocol.md`.
- OpenClaw already treats the Gateway as the control plane for operators and
  nodes. Autopilot can connect as:
  - operator: send `agent` requests, receive streaming `agent` events, and
    post messages to channels.
  - node: expose commands (like `autopilot.run`) and be invoked via the existing
    node tooling.

2) Gateway HTTP endpoints
- Tools invoke API: `docs/gateway/tools-invoke-http-api.md` (`POST /tools/invoke`).
- OpenResponses endpoint: `docs/gateway/openresponses-http-api.md`.
- This is the cleanest way for Autopilot to call OpenClaw tools without running
  a full agent turn (message sending, sessions, nodes, browser, etc.).

3) Plugin system (tools, gateway methods, HTTP handlers, services, hooks)
- Docs: `docs/plugin.md`, `docs/plugins/agent-tools.md`.
- Types: `src/plugins/types.ts`.
- Tools are collected in `src/agents/openclaw-tools.ts` and merged with plugin
  tools via `resolvePluginTools`.
- Plugins can also register Gateway RPC methods and background services.

4) Agent loop hooks
- Docs: `docs/concepts/agent-loop.md` (hook list).
- Hooks in code: `src/plugins/types.ts`.
- Useful for routing certain requests to Autopilot and for capturing/normalizing
  Autopilot outputs into OpenClaw sessions.

5) Sub-agents and session tools
- Docs: `docs/tools/subagents.md`, `docs/concepts/session-tool.md`.
- `sessions_spawn` is the supported way to run a background task and announce
  results back to the requesting chat. This is a natural fit for long Autopilot
  jobs.

6) CLI backends (text-only fallback path)
- Docs: `docs/gateway/cli-backends.md`.
- Code: `src/agents/cli-backends.ts`, `src/agents/cli-runner.ts`.
- This path disables tools and only supports text in/out, so it is a weaker fit
  for Autopilot, but it is a possible integration if Autopilot can run in a
  text-only mode and emit parseable JSON.

7) Skills (prompt-time instructions)
- Docs: `docs/tools/skills.md`.
- If Autopilot is integrated as a tool, a skill can teach the OpenClaw agent
  when and how to call it, with gating via `metadata.openclaw.requires`.

## Autopilot app surfaces in `apps/` (what to hook)

The only app under `apps/` is the desktop host:
- `apps/autopilot-desktop/src/main.rs` is the Winit/WGPU host that runs the
  Codex app-server client and the Full Auto guidance loop.
- `apps/autopilot-desktop/src/full_auto.rs` owns Full Auto state, decision
  summaries, and the DSRS-backed decision signature wiring.

The guidance loop already emits structured events to the UI layer via
`AppEvent::AppServerEvent` messages with method names like:
- `guidance/status`
- `guidance/step`
- `guidance/response`
- `guidance/user_message`
- `fullauto/decision`

Event payloads already include structured fields (thread id, signature, text,
model, signatures list), which makes them good candidates to mirror into
OpenClaw as progress notifications.

## Guidance mode and DSRS usage (today)

Guidance mode is already DSRS-first inside the Autopilot desktop app:

- `apps/autopilot-desktop/src/main.rs` uses DSRS `Predict` with:
  - `TaskUnderstandingSignature`
  - `PlanningSignature`
  - `GuidanceDirectiveSignature`
  - `GuidanceRouterSignature`
  in `run_guidance_super` and `run_guidance_followup`.
- `apps/autopilot-desktop/src/full_auto.rs` uses
  `FullAutoDecisionSignature` in `run_full_auto_decision`.
- `crates/autopilot-core/src/guidance.rs` uses
  `GuidanceDecisionSignature` and provides `GuidanceMode::Demo` with
  `ensure_guidance_demo_lm` (Ollama model by default) and shared guardrails.
- Guidance mode selection is env-driven:
  - `OPENAGENTS_GUIDANCE_MODE=demo` routes to the DSRS guidance pipeline and
    `OPENAGENTS_GUIDANCE_MODEL` (default `ollama:llama3.2`).
  - Legacy mode uses `OPENAGENTS_FULL_AUTO_DECISION_MODEL` (default
    `codex:gpt-5.2-codex`) for `FullAutoDecisionSignature`.
- `FullAutoTurnSummary` (in `full_auto.rs`) is the structured input to
  `FullAutoDecisionSignature` (plan, diff, tokens, pending approvals, etc.).
- `guidance/response` events already include the list of signatures used, which
  is a natural hook for progress streaming.

Takeaway: if OpenClaw integration needs to understand or reflect guidance
decisions, the DSRS signatures above are the definitive places to trace.

## Integration ideas (ranked by leverage)

### 1) Plugin tool: `autopilot.run` (most direct)
- Implement an OpenClaw plugin that registers an optional tool (allowlisted) that
  shells out to `autopilot run`.
- Pattern to follow: `extensions/llm-task`.
- Inputs might include:
  - `task` (required), `repoPath`, `access` (read-only/full), `model`,
    `autopilotLoop` (bool), `timeoutSeconds`, `lane`.
- Output should parse and return a stable summary. Ideal output:
  - `PR_SUMMARY.md` body, plus paths to `RECEIPT.json` and `REPLAY.jsonl`.
  - Reference: OpenAgents Verified Patch Bundle in
    `crates/dsrs/docs/ARTIFACTS.md`.
- The tool result can also include a short diff summary (line counts + files).
- Safety: mark tool optional and require explicit allowlist
  (`tools.allow` or `agents.list[].tools.allow`).

### 2) Plugin gateway method + service (long-running jobs)
- Register a Gateway RPC method like `autopilot.run` or `autopilot.jobs.*`.
- Use `api.registerService` to manage a local Autopilot worker and stream
  progress updates as Gateway events (similar to `agent` stream semantics).
- This keeps the heavy work out of the LLM tool call budget and allows
  progress updates in the OpenClaw UI/CLI.

### 3) Autopilot as a Gateway node (device-style integration)
- Autopilot connects to the Gateway as a `role: node` client (see
  `docs/gateway/protocol.md` for `caps/commands/permissions`).
- Autopilot exposes commands like:
  - `autopilot.run`, `autopilot.status`, `autopilot.cancel`.
- OpenClaw already has a `nodes` tool; a plugin tool could wrap node commands
  to make Autopilot feel first-class.
- Benefits: built-in pairing/approval flow, remote execution support, and
  clear device identity.

### 4) Autopilot uses OpenClaw as the messaging control plane
- Autopilot can connect as an operator client and call `agent` or `message`
  methods to deliver updates to user channels.
- Or use `POST /tools/invoke` to send messages, post to channels, fetch sessions,
  or trigger node actions.
- This keeps messaging/routing (WhatsApp, Telegram, Slack, etc.) in OpenClaw and
  leaves code execution in Autopilot.

### 4b) Progress notifications from Autopilot into OpenClaw
- Autopilot desktop already emits structured guidance events
  (`guidance/status`, `guidance/step`, `guidance/response`, `fullauto/decision`).
- Create a lightweight bridge that forwards those events to OpenClaw:
  - Option A: Autopilot calls OpenClaw Gateway WS `message`/`agent` to deliver a
    short progress line to the same user channel.
  - Option B: Autopilot calls `POST /tools/invoke` with `message` or
    `sessions_send` to keep delivery inside OpenClaw tool policy.
  - Option C: An OpenClaw plugin registers an RPC method like
    `autopilot.progress` and Autopilot sends JSON events that the plugin turns
    into outbound messages or status cards.
- Recommended throttling:
  - Only emit `guidance/status` on signature changes (or debounce to 1-2s).
  - Emit `guidance/step` and `guidance/response` immediately (low volume, high
    value).
  - Optional: drop `guidance/user_message` if OpenClaw is already showing the
    inbound prompt.
- Suggested payload shape (single line per event):
  - `run_id`, `thread_id`, `turn_id`, `phase`, `signature`, `summary`,
    `confidence`, `repo`, `timestamp`.

### 5) Sub-agent orchestration wrapper
- Build a plugin tool that calls `sessions_spawn` with an agent dedicated to
  Autopilot delegation (or calls Autopilot directly and then uses the announce
  step to post results).
- Use the built-in announce format to post a structured summary back to chat
  and include a pointer to Autopilot artifacts.

### 6) Hook-driven routing for code tasks
- Use `before_agent_start` to detect "code task" intents and prepend a short
  directive: "Prefer autopilot.run for repo edits." or even auto-dispatch
  when a command prefix is used (e.g., `/autopilot ...`).
- Use `agent_end` to attach Autopilot artifacts or to summarize tool output.

### 7) CLI backend (lowest leverage)
- If Autopilot can emit text-only JSON and avoid tools, it could be configured
  as a CLI backend under `agents.defaults.cliBackends`.
- This is less useful because the CLI path explicitly disables tools.

## Suggested MVP path (lowest effort, highest value)

1) Implement an OpenClaw plugin tool `autopilot.run` (optional/allowlisted).
2) Parse the Verified Patch Bundle artifacts and return:
   - PR summary text
   - receipt + replay paths (or hashes)
   - a brief file-change summary
3) Add a skill in OpenClaw workspace that teaches when to use the tool.
4) Optionally add a `/autopilot` command wrapper that routes directly to the
   tool (bypass the LLM if needed).

## Notes on safety and policy

- OpenClaw already has tool policy and sandboxing gates; keep the Autopilot tool
  optional and behind allowlists.
- If Autopilot is invoked via `exec`, remember that OpenClaw tool policies apply
  only to OpenClaw tools; Autopilot must enforce its own sandbox/approval rules.
- If Autopilot uses OpenClaw's `POST /tools/invoke`, those calls are gated by the
  Gateway auth token and tool allowlists.

## Code references (OpenClaw)

- Gateway protocol: `docs/gateway/protocol.md`
- Gateway architecture: `docs/concepts/architecture.md`
- Tools invoke HTTP API: `docs/gateway/tools-invoke-http-api.md`
- Plugin system: `docs/plugin.md`, `docs/plugins/agent-tools.md`,
  `src/plugins/types.ts`
- Tool registry merge point: `src/agents/openclaw-tools.ts`
- Agent loop hooks: `docs/concepts/agent-loop.md`
- Sub-agents + session tools: `docs/tools/subagents.md`,
  `docs/concepts/session-tool.md`
- CLI backends: `docs/gateway/cli-backends.md`, `src/agents/cli-backends.ts`

## Code references (OpenAgents)

- Autopilot CLI usage: `crates/autopilot/docs/MVP.md`
- Verified Patch Bundle: `crates/dsrs/docs/ARTIFACTS.md`
