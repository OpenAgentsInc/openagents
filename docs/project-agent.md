# Talking to a project's agent on its page (spec)

Status: draft, October 28. Goal: working end to end today for one approved
private project, then other approved projects.

A member opens a project page, types to the project's agent, and the agent
answers and makes targeted edits to the project. It can change milestones,
statuses, notes, plans, the project document itself and the files in the
project's linked repository.

Edits apply at once, with no per-edit approvals. Every agent turn can be
rolled back instead: one turn becomes one commit and one document version,
and UNDO TURN reverts both.

The agent runs on our hosted agent infrastructure. A Coax port of the Claude
Agent SDK drives the `claude` CLI. For now the agent is powered by our
Anthropic API key (Secret Manager `openagents-anthropic-api-key`) on Claude
Sonnet 5.5. Connecting a member's own Claude or Codex harness comes later.

## What a member sees

- **The AGENT panel**, drawn in Coax like the rest of the page:
  - the conversation, newest at the bottom;
  - a text box;
  - while the agent works, one line saying what it is doing ("reading the
    milestones", "editing day 10-29");
  - a STOP button.
- **Edits, as they happen.** Each one shows as a small diff card, with who
  asked and when:
  - a document edit, such as "day 10-29: *Start the deck* → doing";
  - a repository change, as the files changed and its commit.

  Each turn ends with an UNDO TURN button.
- **Goals keep their own rule.** The agent proposes a goal or a change to
  one, and it applies only when the members agree, as today. That is the
  project's rule, not a per-edit approval.
- **One shared conversation per project.** Both members see everything the
  agent was asked and did. It is the project's agent, not one person's.

## What the agent can do

Tools are offered to the model as in-process SDK MCP tools served by the
runner, not as shell access. Each tool calls oa-api with the project's agent
key, so the server enforces the same rules it enforces for any agent:

| Tool | What |
| --- | --- |
| `project_get` | The whole project as text, and as JSON with its version |
| `project_note` | A note on the thread |
| `project_plan` | The agent's plan for a milestone |
| `milestone_status` | todo / doing / done / dropped, with evidence |
| `milestone_edit` | Add, move, reword or carry over a milestone or backlog item |
| `checklist_status` | A checklist item's status and evidence |
| `goal_propose` | A new goal, or a change to one, as `proposed`; it applies when the members agree |
| `document_patch` | A targeted JSON patch to the project document, against the version it read |

- **Repository files.** If the project links a repository, the CLI's `Read`,
  `Glob`, `Grep`, `Edit` and `Write` tools are allowed. The permission
  callback limits them to paths inside that turn's checkout.
- **Everything else is refused:** Bash, web, Task and any path outside the
  checkout. The agent can change only its own project.
- Every write carries the version the agent read. A stale write is refused
  and the agent reads again.
- Each edit records the member message that caused it, so "why did this
  change?" has an answer.

## Rollback: one turn, one commit, one version

- **A fresh checkout per turn.** At the start of a turn, the runner makes a
  shallow clone of the project's repository's `main` into a fresh temporary
  directory. The model never runs git.
- **One commit per turn.** At the end of the turn, the runner commits every
  changed file as one commit and pushes to `main`. The message reads "agent
  turn <id>: <first line of the member's message>". The runner pulls with
  rebase first. If a member's own commit conflicts, the turn's changes are
  not pushed, and the panel says so and shows the diff.
- **Document edits** in the same turn are recorded under the turn's id;
  oa-api already keeps every document version.
- **UNDO TURN** does two things, and is itself one turn:
  - restores the document to the version before the turn;
  - pushes `git revert` of the turn's commit.
- **Write access.** It comes from a deploy key the runner reads from Secret
  Manager, one per linked repository, named in oa-api's approved-projects
  list. The key never enters the model's context or the checkout.

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
  2. The runner reads the queue, checks out the repository, runs one turn
     of the SDK, and streams events back to oa-api.
  3. It commits, pushes and exits idle.
  4. Permission requests are decided by the callback's rules alone; no one
     is asked.
- **The page** sends messages and UNDO TURN through oa-api with the
  member's existing session cookie. It gets events by polling oa-api every
  2 seconds while a turn runs, and every 30 seconds otherwise. A live socket
  can come later.
- **Approved projects only.** oa-api keeps a list of projects whose agent is
  turned on, starting with one. Any other project refuses with the same
  "not found" answer private projects already give.

## The runner and its engine

The runner (`coax/web/agent/runner.c`) is the host: the wake, the signed
calls to oa-api, the per-turn checkout, git, the deploy key and the runner's
Nostr key. The model's side is the engine, a separate process in the
checkout with only `ANTHROPIC_API_KEY`, `HOME` and `PATH`: the Coax SDK, the
project tools and the permission callback. They speak JSON lines:

| Direction | Line |
| --- | --- |
| runner → engine | `{"t":"turn","slug","turn":{id,kind,text,by},"history","version","model","budget_usd","repo","cwd","today"}` |
| engine → runner | `{"t":"call","id","tool","args"}`, one of the project tools; the runner makes it one oa-api op, adds the turn and signs it |
| runner → engine | `{"t":"reply","id","status","body"}` |
| engine → runner | `{"t":"event","type","data"}`: `working`, `agent.message`, `agent.tool_use`, `session.error` |
| runner → engine | `{"t":"stop"}` when a member pressed STOP |
| engine → runner | `{"t":"done","status","answer","error","cost_usd"}` |

The engine (`coax/web/agent/turn.coax`, with `engine-io.c` for its stdin)
serves the project tools as an in-process SDK MCP server named `project`.
The runner maps each to oa-api:

| Tool | oa-api op |
| --- | --- |
| `project_get` | `state` |
| `project_note`, `project_plan` | `note` (kind `note` or `plan`) |
| `milestone_status`, `checklist_status` | `status`, with the evidence as the note |
| `milestone_edit` | `milestone_edit`: add (a dated milestone, or a backlog item in a checklist), reword, move, carry over |
| `goal_propose` | `goal_propose`: a new proposed goal, or a change to a goal; a change to an agreed goal is a new proposed goal with `replaces`, and the agreed goal stays |
| `document_patch` | `patch`: RFC 6902 add, remove, replace, move and test |

`milestone_edit`, `goal_propose` and `patch` (migration 0018) carry the
version the agent read (`base`), build the new document and save it through
`push`, so the same rules hold: only an approved project's runner or agent,
only inside a running turn, never the title, the members or an agreed goal,
and each edit recorded under the turn. The CLI's `Read`, `Glob`, `Grep`,
`Edit` and `Write` are offered only when the project links a repository, and
the permission callback allows them only for paths inside the checkout; Bash,
web and Task are disallowed, no settings files load and no session is saved.
Milestones are todo, doing or done: the document has no dropped status yet.

Events go to oa-api in batches every 4 seconds, and each batch's answer
says whether STOP was pressed. The engine stops at the next event, and the
runner kills its process group 30 seconds after a STOP, or after 15
minutes. A turn that doesn't end done commits nothing.
`coax/web/agent/test-engine.py` is a stand-in engine for the runner's checks
(`coax/web/api/test_private_runner.py`); `coax/web/api/test_private_tools.py`
checks the document tools. The runner's keys are listed in
`coax/web/api/runners.conf` so their writes are limited at 600 a minute.

## The runner's seam

The runner is meant to serve more than project turns (a general chat with
read access to a repository is next). What is project-specific is kept to
three places:

- **The workspace.** The engine's first line carries
  `"workspace":{"kind","budget","tools","write"}`. Today the runner always
  sends `{"kind":"project","budget":"project:SLUG","tools":["project"],"write":true}`.
  The engine offers the project tools only when `tools` lists `"project"`,
  and Edit and Write only when `write` is true; Read, Glob and Grep are
  offered whenever there is a checkout, always limited to it.
- **The queue and the cap.** Turns come from `projects.api` `next`, and the
  daily cap is counted per project (`projects.spent`). Another kind of turn
  needs its own queue op and its own spend keyed by its budget id; the wake,
  the turn loop, the events batching, STOP and the 15-minute limit don't
  depend on the project.
- **The checkout.** `OA_AGENT_REPOS` maps a slug to a repository and deploy
  key. A read-only workspace would clone without a key and never commit.

/chat is the first other kind of turn ([chat.md](chat.md)). It uses the seam
as described: `coax/web/agent/chat-runner.c` (included by `runner.c`) takes
turns from `/api/chat/runner/next`, sends
`{"kind":"chat","budget":"chat","tools":["web"],"write":false}`, runs its own
engine (`chat.coax`, `/usr/local/bin/oa-chat-turn`) in a read-only checkout
of the public repository, and keeps its spend in `chat.turns`. The runner's
wake, events batching and STOP are shared.

## Deploying and checking it

- `coax/web/agent/deploy.sh` builds one image and deploys `oa-agent-staging`
  (Claude Haiku 5.5) and `oa-agent` (Claude Sonnet 5.5): scale to zero, one
  instance, invoked only by oa-api's service account. Which projects it
  serves, their repositories and deploy-key secrets come from Secret Manager
  at deploy time (`oa-agent-repos-staging`, `oa-agent-repos`), never from
  this repository. Then `coax/web/api/deploy.sh` points oa-api at it.
- `coax/web/agent/live-check.py` checks a deployed agent as a member would:
  `tools` asks for every tool and a file edit and checks the calls, the
  version, the edit log, the pushed commit and that UNDO TURN puts both back;
  `one` is a single change and its undo; `refused` checks a project without
  the agent answers 404 like no project; `capped` checks the day's limit.

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
  - $10 a day per project, from the CLI's reported cost, enforced by the
    runner;
  - max turns per message;
  - one turn at a time per project.
- **No shell and no network** for the model. Its only effects are:
  - the project tools above;
  - file edits inside its own checkout, which the runner commits.
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
5. The page's AGENT panel, with diff cards and UNDO TURN.
6. End-to-end on staging, then production, then a phone check.

Later:
- members connect their own Claude or Codex harness instead of our key;
- a live socket instead of polling;
- the native app view;
- end-to-end encrypted projects, where the runner holds a member-granted
  key.
