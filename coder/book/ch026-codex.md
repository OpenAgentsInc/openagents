# 26. Codex

> **Status: written.** Read at openai/codex@a09812981de703f18f38015cd5d500ab373f616c on 2026-11-02.

Codex is OpenAI's coding agent. Its core, terminal UI, sandbox helpers and
servers are a Rust workspace in `codex-rs/`, licensed Apache-2.0
(`LICENSE`). Coder already drives it: the client in `coax/lib/agent/codex/`
speaks `codex app-server` over JSON-RPC and tracks codex-cli 0.163.0
([PARITY.md](../../coax/lib/agent/codex/PARITY.md)), and the Coder TUI runs
it as the `codex` engine (`coder/tui/codex.coax`).

The source was cloned read only into `projects/repos/codex` at `a098129`
(committed 2026-11-01), the head of `main`. The workspace version in
`codex-rs/Cargo.toml` is `0.0.0`; releases set it when they are tagged, so
this revision is newer than the 0.163.0 that Coder's client was recorded
against. Nothing was installed or run. Paths below are relative to the
clone.

## Why study it

Codex answers three questions that Coder must answer too, and its answers
are explicit in code:

1. How a command is confined on each operating system, not only judged by a
   rule.
2. How sandbox policy and approval policy combine into one decision.
3. How a client drives the agent over a typed protocol, so that the
   terminal UI, the IDE extensions and other programs are all clients.

The third matters most to Coder, because the app-server protocol is how
Coder drives Codex today (chapter 11) and a model for how other programs
drive Coder (chapter 23).

## What it does

| Feature | File at `a098129` | Coder chapter |
| --- | --- | --- |
| Sandbox modes `read-only` (the default), `workspace-write`, `danger-full-access` | `codex-rs/protocol/src/config_types.rs` (`SandboxMode`), `codex-rs/protocol/src/protocol.rs` (`SandboxPolicy`) | [5](ch005-a-sandbox-and-approvals.md) |
| Approval policies `untrusted`, `on-request` (the default), `granular`, `never` | `codex-rs/protocol/src/protocol.rs` (`AskForApproval`) | [5](ch005-a-sandbox-and-approvals.md) |
| Seatbelt through `/usr/bin/sandbox-exec` on macOS; bubblewrap with `no_new_privs` and seccomp on Linux; Landlock; a Windows sandbox | `codex-rs/sandboxing/src/seatbelt.rs`, `codex-rs/linux-sandbox/src/lib.rs`, `codex-rs/sandboxing/src/landlock.rs`, `codex-rs/windows-sandbox-rs/` | [5](ch005-a-sandbox-and-approvals.md) |
| Execution policy: Starlark `prefix_rule` with `allow`, `prompt`, `forbidden` and load-time examples | `codex-rs/execpolicy/README.md` | [5](ch005-a-sandbox-and-approvals.md) |
| An automatic reviewer that answers approvals instead of a person | `codex-rs/core/src/guardian/mod.rs`, `codex-rs/protocol/src/config_types.rs` (`ApprovalsReviewer`) | [5](ch005-a-sandbox-and-approvals.md) |
| Edits as one patch format with a published grammar | `codex-rs/apply-patch/src/parser.rs` | [4](ch004-tools-as-typed-capabilities.md) |
| `AGENTS.md` from the project root down to the working directory | `codex-rs/core/src/agents_md.rs` | [6](ch006-a-document-tries-to-become-the-system-prompt.md) |
| Sessions saved as JSONL rollouts under `~/.codex/sessions` | `codex-rs/rollout/src/recorder.rs` | [2](ch002-model-visible-means-logged.md) |
| The app-server: JSON-RPC 2.0 over stdio or WebSocket; threads, turns and items; server requests for approvals | `codex-rs/app-server-protocol/src/protocol/common.rs`, `codex-rs/app-server-transport/src/lib.rs` | [11](ch011-drive-codex.md), [16](ch016-speak-the-protocols.md) |
| Generated TypeScript and JSON Schema for the protocol | `codex-rs/cli/src/main.rs` (`AppServerSubcommand`) | [11](ch011-drive-codex.md) |
| `codex exec`: one task without a UI, JSONL events, an output schema, ephemeral runs | `codex-rs/exec/src/cli.rs`, `codex-rs/exec/src/exec_events.rs` | [23](ch023-openagents-coder.md) |
| Thread fork, compaction, steering, review and goals as protocol methods | `codex-rs/app-server-protocol/src/protocol/common.rs` | [7](ch007-condense-the-history.md), [14](ch014-forks-subagents-and-jobs.md) |

### Two policies, one decision

Codex keeps what a command may touch apart from when a person is asked.
`SandboxMode` has three values, and `read-only` is the default
(`codex-rs/protocol/src/config_types.rs`). `SandboxPolicy` carries the
details, such as `network_access`, which is off by default even in
read-only mode (`codex-rs/protocol/src/protocol.rs`). `AskForApproval`
decides when to ask. `on-request`, the default, lets the model ask for
escalation. `untrusted` asks for every command an exec policy rule does not
allow. `never` returns failures to the model and "never" escalates them.
`granular` turns each kind of prompt (sandbox escalation, exec policy
prompts and others) on or off, and a kind that is off is "automatically
rejected instead of shown to the user" (`GranularApprovalConfig`, same
file).

### Confinement per platform

The sandbox is enforced by the operating system. On macOS Codex runs
commands under Seatbelt and only trusts `/usr/bin/sandbox-exec`, so a
tampered binary earlier on `PATH` cannot stand in for it
(`MACOS_PATH_TO_SEATBELT_EXECUTABLE` in `codex-rs/sandboxing/src/seatbelt.rs`).
Its profiles are checked-in `.sbpl` files in the same directory. On Linux,
`codex-linux-sandbox` applies "in-process restrictions (`no_new_privs` +
seccomp)" and "bubblewrap for filesystem isolation"
(`codex-rs/linux-sandbox/src/lib.rs`). `codex sandbox` runs any command
under the same sandbox (`codex-rs/cli/src/main.rs`), so a person can test a
policy without the agent.

### Execution policy

`codex-rs/execpolicy/README.md` describes a small Starlark language.
`prefix_rule(pattern=[...], decision=...)` matches a command's tokens in
order; a pattern element may list alternatives; `decision` is `allow`,
`prompt` or `forbidden`. Each rule can carry `match` and `not_match`
examples that "are validated at load time (think of them as unit tests)",
and a `justification` that is shown when the rule blocks a command. An
approval can amend the policy: the v2 decision
`acceptWithExecpolicyAmendment` approves a command and adds a rule so
matching commands no longer ask
(`codex-rs/app-server-protocol/src/protocol/v2/item.rs`).

### Automatic review

`ApprovalsReviewer` is `user` or `auto_review`
(`codex-rs/protocol/src/config_types.rs`). With `auto_review`, a reviewer
called Guardian answers approval requests; core "enforces permissions and
mandatory review requirements" while the reviewer chooses policy and
evidence (`codex-rs/core/src/guardian/mod.rs`). A managed `prompt-user`
rule can still require "a fresh human decision", which session and
persistent approvals cannot satisfy (`codex-rs/app-server/README.md`).

### The app-server

`codex app-server` is JSON-RPC 2.0. Its transports are stdio and WebSocket
(`start_stdio_connection` and `start_websocket_acceptor` in
`codex-rs/app-server-transport/src/lib.rs`). The method table in
`codex-rs/app-server-protocol/src/protocol/common.rs` is large: thread
methods (`thread/start`, `thread/resume`, `thread/fork`,
`thread/compact/start`, `thread/revert`, goals and queues), turn methods
(`turn/start`, `turn/steer`, `turn/interrupt`), `review/start`, file system
and process methods, plugins, skills and accounts. A turn produces items,
and each item has a lifecycle: `item/started`, deltas such as
`item/agentMessage/delta` and `item/commandExecution/outputDelta`, then
`item/completed`. Item kinds include agent messages, reasoning, plans,
command executions, file changes and MCP tool calls
(`ThreadItemPayload` in `codex-rs/app-server-protocol/src/protocol/v2/item.rs`).

Approvals are server requests that the client must answer:
`item/commandExecution/requestApproval`, `item/fileChange/requestApproval`
and `item/permissions/requestApproval`. A command decision is `accept`,
`acceptForSession`, `acceptWithExecpolicyAmendment`,
`applyNetworkPolicyAmendment`, `decline` or `cancel` (`item.rs`). New
methods sit behind an `experimentalApi` capability the client opts into at
`initialize`, and a client can opt out of notification methods it does not
want. The CLI generates TypeScript bindings and JSON Schema for the whole
protocol (`GenerateTs` and `GenerateJsonSchema` in `codex-rs/cli/src/main.rs`).

### Non-interactive runs

`codex exec` runs one task and exits. With `--json` it prints one JSONL
event per line: `thread.started`, `turn.started`, `item.started`,
`item.updated`, `item.completed`, `turn.completed`, `turn.failed` and
`error` (`ThreadEvent` in `codex-rs/exec/src/exec_events.rs`).
`--output-schema FILE` asks for a final answer in a given JSON shape, and
`--ephemeral` keeps no session file (`codex-rs/exec/src/cli.rs`). The
TypeScript SDK wraps this mode: it "spawns the CLI and exchanges JSONL
events over stdin/stdout" (`sdk/typescript/README.md`).

### Sessions and instructions

Every session is saved as a rollout, `rollout-<timestamp>-<id>.jsonl`, under
`~/.codex/sessions`, "so sessions can be replayed or inspected later"
(`codex-rs/rollout/src/recorder.rs`). A revert writes a new rollout file
next to the old one rather than truncating it (same file). Instructions come
from `AGENTS.md`: Codex walks up to the project root (marked by `.git` by
default), collects every `AGENTS.md` from the root down to the working
directory, concatenates them in that order, and does not walk past the root
(`codex-rs/core/src/agents_md.rs`).

## What Coder takes

- **Two policies, not one** ([chapter 5](ch005-a-sandbox-and-approvals.md)).
  Chapter 5 already separates an effect ceiling from what runs without
  asking. Codex's split names the same idea for real processes: a sandbox
  mode for what a process may touch, an approval policy for when a person is
  asked. Coder uses Codex's three sandbox names, which DeepSeek Harness
  also uses (chapter 24), so one Coder policy maps onto either engine.
- **Network off unless granted.** `network_access` defaults to false in
  every confined mode. Coder's real-process sandbox does the same.
- **A trusted path to the sandbox binary.** Coder's host resolves
  `sandbox-exec` and `bwrap` from fixed paths, never from `PATH`.
- **Rules with examples** ([chapter 5](ch005-a-sandbox-and-approvals.md)).
  A Coder policy rule carries examples that must and must not match, checked
  when the policy loads, and a justification shown when it blocks. An
  approval that adds a rule is an event in the log, so a replay rebuilds the
  policy.
- **Refuse when a kind of prompt is off.** `granular` rejects instead of
  showing a prompt that nobody will answer. Coder's TUI client already
  declines unanswered approvals rather than allowing them
  ([PARITY.md](../../coax/lib/agent/codex/PARITY.md), "The wire").
- **Items with a lifecycle** ([chapter 2](ch002-model-visible-means-logged.md)).
  Started, deltas, completed: Coder's log records the started and completed
  events and keeps deltas out of the durable log, so a replay is short and
  a live client still streams.
- **A protocol with generated schemas** ([chapter 23](ch023-openagents-coder.md)).
  `openagents coder` exposes its verbs with `--json` and as MCP tools. Its
  message types live in `proto/`, and the JSON Schema is generated from
  them, as Codex generates its own.
- **`exec` as the shape of `coder run --json`**: one JSONL event per line,
  a final answer that can be constrained by a schema, and an ephemeral flag
  for runs that keep no log. The ephemeral flag still obeys chapter 18:
  ephemeral means nothing is written, not that something is sent.

## What Coder leaves, and why

- **Most of the protocol surface.** The app-server carries accounts, rate
  limits, realtime voice, projects, plugins, marketplaces and file system
  methods. Coder's client sends what a task needs (`initialize`, threads,
  turns, interrupt, approvals, `model/list`) and passes everything else
  through `cx_request`. The PARITY file lists what is not done: steering,
  `/compact`, `/review` and subagent threads.
- **A model as the approver.** Guardian answers approvals with a model.
  Coder keeps the person or the written policy as the approver, the same
  choice it makes for Claude Code's `auto` mode (chapter 25).
- **The default engine settings in the TUI.** The `codex` engine runs with
  approval policy `never` and full access unless `--ask` is passed
  (`coder/tui/codex.coax`). That suits a person at their own terminal.
  [Chapter 11](ch011-drive-codex.md) replaces it with chapter 5's policy, so
  an unattended Coder run starts at `workspace-write` with approvals
  answered by the policy.
- **Codex's patch format as Coder's edit tool.** Microcoder edits through
  commands so every write shows in the transcript
  (`coax/lib/microcoder/tools.coax`). The `apply_patch` grammar
  (`codex-rs/apply-patch/src/parser.rs`) is a candidate edit tool, kept only
  if it wins in chapter 21.

## Where a borrowed design is measured

- Sandbox modes and approval answers: [chapter 5](ch005-a-sandbox-and-approvals.md)
  checks that every outcome is logged and that approval never passes the
  ceiling; the real-process checks remain to be written.
- Recorded sessions replayed byte for byte: [chapter 11](ch011-drive-codex.md)
  runs Coder's client against two scrubbed 0.163.0 recordings
  (`coax/lib/agent/codex/fixtures/`). A new recording at a release built
  from this revision is the check that the client still holds.
- A structured edit format against edits through commands:
  [chapter 21](ch021-measure-the-harness.md).
- Codex against Microcoder and Claude Code on the same task:
  [chapter 13](ch013-choose-an-engine.md).

## Licensing

Apache-2.0 (`LICENSE`, `NOTICE`). Coder ports designs and copies no code.

## Sources

- [openai/codex at a098129](https://github.com/openai/codex/tree/a09812981de703f18f38015cd5d500ab373f616c)
- [codex-rs/protocol/src/protocol.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/protocol/src/protocol.rs)
- [codex-rs/protocol/src/config_types.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/protocol/src/config_types.rs)
- [codex-rs/sandboxing/src/seatbelt.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/sandboxing/src/seatbelt.rs)
- [codex-rs/linux-sandbox/src/lib.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/linux-sandbox/src/lib.rs)
- [codex-rs/execpolicy/README.md](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/execpolicy/README.md)
- [codex-rs/core/src/guardian/mod.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/core/src/guardian/mod.rs)
- [codex-rs/apply-patch/src/parser.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/apply-patch/src/parser.rs)
- [codex-rs/core/src/agents_md.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/core/src/agents_md.rs)
- [codex-rs/rollout/src/recorder.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/rollout/src/recorder.rs)
- [codex-rs/app-server-protocol/src/protocol/common.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/app-server-protocol/src/protocol/common.rs)
- [codex-rs/app-server-protocol/src/protocol/v2/item.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/app-server-protocol/src/protocol/v2/item.rs)
- [codex-rs/app-server-transport/src/lib.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/app-server-transport/src/lib.rs)
- [codex-rs/app-server/README.md](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/app-server/README.md)
- [codex-rs/cli/src/main.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/cli/src/main.rs)
- [codex-rs/exec/src/cli.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/exec/src/cli.rs)
- [codex-rs/exec/src/exec_events.rs](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/codex-rs/exec/src/exec_events.rs)
- [sdk/typescript/README.md](https://github.com/openai/codex/blob/a09812981de703f18f38015cd5d500ab373f616c/sdk/typescript/README.md)
- Coder's client: [PARITY.md](../../coax/lib/agent/codex/PARITY.md); the TUI
  engine in `coder/tui/codex.coax`.
- [Chapter plan](PLAN.md), Part VII.
