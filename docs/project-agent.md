# Talking to a project's agent on its page (spec)

Status: draft, October 28. Goal: working end to end today for one approved
private project, then other approved projects.

A member opens a project page, types to the project's agent, and the agent
answers and makes targeted edits to the project: milestones, statuses,
notes, plans and the project document itself. Every edit it makes is shown,
signed and logged. Edits that need a member's yes wait on the page for it.

The agent runs on our hosted agent infrastructure. A Coax port of the Claude
Agent SDK drives the `claude` CLI. For now the agent is powered by our
Anthropic API key; connecting a member's own Claude or Codex harness comes
later.

## What a member sees

- **The AGENT panel**, drawn in Coax like the rest of the page:
  - the conversation, newest at the bottom;
  - a text box;
  - while the agent works, one line saying what it is doing ("reading the
    milestones", "editing day 10-29");
  - a STOP button.
- **Edits, as they happen.** Each one shows as a small diff card ("day 10-29:
  *Start the deck* → doing") with who asked and when.
  - An edit that needs approval shows ALLOW and DENY. The agent waits.
  - An edit to an agreed goal shows NEEDS BOTH and waits for both members'
    yes. Today's rule stays: agents propose goals, they never change agreed
    ones.
- **One shared conversation per project.** Both members see everything the
  agent was asked and did. It is the project's agent, not one person's.

## What the agent can do

Tools are offered to the model as in-process SDK MCP tools served by the
runner, not as shell access. Each tool calls oa-api with the project's agent
key, so the server enforces the same rules it enforces for any agent:

| Tool | What | Approval |
| --- | --- | --- |
| `project_get` | The whole project as text, and as JSON with its version | none |
| `project_note` | A note on the thread | none |
| `project_plan` | The agent's plan for a milestone | none |
| `milestone_status` | todo / doing / done / dropped, with evidence | none |
| `milestone_edit` | Add, move, reword or carry over a milestone or backlog item | one member |
| `checklist_status` | A checklist item's status and evidence | none |
| `goal_propose` | A new goal, or a change to one, as `proposed` | both members to accept |
| `document_patch` | A targeted JSON patch to the project document, against the version it read | one member |

- The CLI's built-in tools are off: no Bash, file, web or Task tools. The
  permission callback refuses anything not in the table, so the agent can
  edit only its own project.
- Every write carries the version the agent read. A stale write is refused
  and the agent reads again.
- Each edit records the member message that caused it, so "why did this
  change?" has an answer.

## How it fits together

```
page (Coax, wasm) ──► oa-api ──► agent runner (Cloud Run) ──► claude CLI ──► Anthropic API
     ▲  members'         │  queue, approvals,       │  Coax Claude Agent SDK
     │  cookie session   │  events, edits           │  + project MCP tools
     └──── events ◄──────┴──────── events ◄─────────┘
```

- **oa-api** (C, Cloud Run, Cloud SQL) owns everything durable:
  - the conversation;
  - the queue of member messages;
  - pending approvals;
  - session events;
  - the edits.

  Event shapes follow `openagents.agent.managed.v1` (`ManagedSession`,
  `SessionEvent`) in `docs/protocol/openagents-protocol-spec.md` §18.2, so
  this becomes the first piece of the hosted Managed Agents API rather than
  a one-off.
- **The runner** is a Cloud Run service, `oa-agent`, with scale to zero.
  1. When a member sends a message, oa-api calls the runner with a signed
     wake request.
  2. The runner reads the queue, runs one turn of the SDK, streams events
     back to oa-api, and exits idle.
  3. A permission request becomes a `requires_action` event. The runner
     waits on oa-api for the member's answer, with a time limit; no answer
     means deny.
- **The page** sends messages and approvals through oa-api with the
  member's existing session cookie. It gets events by polling oa-api every
  2 seconds while a turn runs, and every 30 seconds otherwise. A live socket
  can come later.
- **Approved projects only.** oa-api keeps a list of projects whose agent is
  turned on, starting with one. Any other project refuses with the same
  "not found" answer private projects already give.

## The Coax Claude Agent SDK

A port of the Rust crate `claude_agent_sdk` (removed in 0b401561a0; source at
`0b401561a0^:crates/claude_agent_sdk`), which tracks
`@anthropic-ai/claude-agent-sdk` 0.3.296. Like that crate, it ports the wire
protocol, not the npm package:

- **Transport:** start `claude` as a child process in its own process group,
  with stream-json in and out (`--output-format stream-json --input-format
  stream-json --verbose`). Read stdout line by line, keep stderr, and stop
  the whole group on kill or drop. The process code is a small C shim in
  `coax/platform/` next to `entropy-native.c`, imported with `#foreign`.
- **Messages:** every `SDKMessage` type and `system` subtype parses to a
  typed value. An unknown type becomes `Unknown` with its raw JSON, and the
  stream goes on.
- **Control protocol, both ways:**
  - `initialize`;
  - `can_use_tool`, the permission callback;
  - `hook_callback`;
  - `mcp_message`, for in-process SDK MCP servers;
  - `interrupt`, `set_model` and `set_permission_mode`.
- **Options:** `QueryOptions` becomes CLI flags and `initialize` fields:
  - model and system prompt;
  - allowed and disallowed tools;
  - permission mode;
  - MCP servers;
  - max turns;
  - cwd and env.
- **Credentials:** an `ANTHROPIC_API_KEY` given in the child's environment
  only, never a file and never logged. A Claude Code login also works for
  owner-local smokes.
- **Where it lives:**
  - library `coax/lib/agent/claude/`;
  - shim `coax/platform/process-native.c`;
  - tests on recorded stream-json fixtures, ported from the crate's
    `protocol/tests.rs`;
  - one live smoke against the installed CLI.
- **Parity:** a `PARITY.md` carried over from the crate, with the tracked
  version and what is deferred.

## Safety

- **The API key** lives in Secret Manager. Only the runner's service account
  can read it. It never reaches oa-api's responses, the page, logs or the
  model's context.
- **Spend:**
  - a daily cap per project in dollars, from the CLI's reported cost,
    enforced by the runner;
  - max turns per message;
  - one turn at a time per project.
- **No shell, no files, no network** for the model. Its only effects are the
  project tools above.
- **Private contents** reach Anthropic's API as model input. Members are
  told so on the panel once.
- **Logs** keep event types, tool names and costs. They never keep message
  text or document contents.

## Build order (today)

1. Coax SDK: transport and message parsing, with a live smoke of one query.
2. Coax SDK: control protocol, the permission callback and an SDK MCP
   server, with the smoke calling one in-process tool.
3. oa-api: an agent session table, events, a message queue, approvals, the
   approved-projects list and a signed wake call.
4. `oa-agent` runner: container with the CLI, the SDK binary, the project
   tools and the secret, deployed to staging.
5. The page's AGENT panel, with diff cards and ALLOW/DENY.
6. End-to-end on staging, then production, then a phone check.

Later:
- members connect their own Claude or Codex harness instead of our key;
- a live socket instead of polling;
- the native app view;
- end-to-end encrypted projects, where the runner holds a member-granted
  key;
- edits to a project's repository files.
