# Code mode in the OpenAgents SDK and mobile app

2026-10-17. Status: analysis and proposal. Nothing here is implemented
unless a row says it exists today. This document follows the research report
[Code mode for coding agents](chatgpt-codemode.md), especially its section 9,
"What I would build for OpenAgents", and maps each recommendation onto code
in this repository.

## Summary

We already have most of the parts that code mode needs, but they live in
separate places and don't share one contract. We have four embedded
interpreters, at least six tool registries, three permission models, a trace
format, and a task owner that already handles effects whose outcome is
unknown. The work is mostly joining these parts together:

1. Define one tool contract and one broker that both direct calls and
   program calls go through.
2. Run model-written programs as JavaScript on the `boa_engine` runtime that
   `crates/workflow-run` already uses. Run them next to the broker: on our
   servers for cloud chats, and in the engine on the person's computer for
   computer chats. Never run them on the phone.
3. Check permission on every nested call inside the broker, never only on the
   outer program.
4. Record each nested call as its own ATIF `Call`, linked to its parent
   program.
5. Show a program run on the phone as one Rust Native card that holds its
   calls, approvals, and result.

The first delivery can be server-side only: read-only programs over the
Google and GitHub tools in cloud chats. The phone card, the SDK event, and the
registry don't need a new Coder binary.

## What exists today

The report's reference architecture has six parts: discovery, a sandboxed
program, a tool broker, policy and budget, approval, and tracing. The table
maps each part to the code we have now.

| Part | What exists | Where |
| --- | --- | --- |
| Tool definitions | Hand-written `ToolDef` (name, description, input schema) for the engine's MCP server | `apps/openagents/crates/mcp/src/tools.rs:36` |
| Tool definitions | `ToolSpec` with id, HTTP method, input schema, OAuth scopes, and a default `Policy` | `crates/oa-connections/src/core.rs:198` |
| Tool definitions | Function declarations written separately by each web chat door | `crates/openagents-web/src/connections/chat.rs:226`, `connections/mail.rs:429`, `github_tools/issues.rs:96`, `agents_door.rs:118` |
| Tool definitions | Coder's plugin tool table and dispatcher | `crates/coder-new/src/plugin_tools.rs:438` (`defs`), `:744` (`execute`) |
| Tool definitions | The docs and keyed MCP server at `/mcp` | `crates/openagents-web/src/docs_mcp.rs:107` |
| Dispatcher | `Tools::call`, one `match` over tool names, plus batch tools (`create_chats`, `send_messages`, at most 32 items) | `apps/openagents/crates/mcp/src/tools.rs:827` |
| Interpreter | JavaScript workflow runs on `boa_engine`: `agent()`, `phase()`, `log()`, a session budget, pause, and reuse of unchanged steps | `crates/workflow-run/src/lib.rs`, `engine.rs:243` |
| Interpreter | Lua 5.4 code-as-action with a `Host` trait, instruction, depth, and time limits, and typed errors | `crates/voyager/src/interpret.rs:38`, `:54` |
| Interpreter | Sandboxed Luau for agent visuals, with no I/O, a memory cap, and a time budget; it runs on the phone | `apps/openagents/crates/visual/src/lib.rs:150` |
| Interpreter | Wasm plugin host with fuel, memory caps, cancellation, host calls, receipts, and replay | `crates/plugin/src/engine.rs:28`, `:44` |
| Execution boundary | Filesystem write boundary, optional network denial and read confinement | `crates/coder-boundary/src/lib.rs:35` |
| Execution boundary | Process-group supervisor with deadlines and output caps | `crates/supervise` |
| Execution boundary | Hosted eval runner with a fixed read and sandbox-write grant, never `exec` or `network` | `crates/eval-runner/src/lib.rs:15` |
| Policy | Connections policy: allow, require approval, or block, with inner scopes unable to weaken outer ones | `crates/oa-connections/src/core.rs:54`, `:421` |
| Policy | Approval bound to the exact card the person saw, with an HMAC confirm code | `crates/openagents-web/src/connections/mail.rs:175`, `:297` |
| Policy | Coder's turn permit, which only narrows | `crates/coder/src/permit.rs:49` |
| Policy | Program grant with six effect axes (reads, writes, delegation, network, subprocesses, spend) | `crates/coder/src/program_authority.rs:84` |
| Policy | Command effect classes (read-only, needs approval, denied) and the approval gate | `crates/coder/src/task/agent.rs:1372`, `crates/coder-new/src/approval.rs` |
| Policy | NIP-HOST device rights (observe, operate, terminal, review, access read, access admin, world) | `crates/coder-access/src/rights.rs:7` |
| Policy | Saved agent access (host default, read-only, workspace write) | `crates/agent-contract/src/lib.rs:45` |
| Tracing | ATIF `Call` with id, name, arguments, output, outcome, time, and an `extra` map; no parent field | `crates/atif/src/document.rs:81` |
| Durability | Effect intent before dispatch, `unknown` after a crash, no automatic retry of uncertain writes | `docs/coder/runtime/task-owner.md` |
| Durability | Idempotent publish with an operation id and read-back reconciliation | `crates/coder/src/task/publish.rs:188`, `:607` |
| Promotion | NIP-PRG programs, skill review stages, extension evaluation with baseline and subject arms | `programs/`, `crates/tenancy/src/skills.rs`, `crates/ext-eval` |
| Phone display | Tool groups with rails, file badges, and subagent cards | `apps/openagents/crates/mobile/src/layout/tools.rs` |
| Phone display | `MessagePart::Tool`, with output and diff sidecars and subagent status | `apps/openagents/crates/doc/src/parts.rs:155` |
| Phone display | Cloud chat stream with a text-only `Event::Tool` and an untyped `Event::Cards` | `crates/openagents-sdk/src/chats.rs:28` |
| Grouping | One grouping module for every surface: reads fold, actions stand alone | `crates/openagents-chat/src/tool_groups.rs` |

Two earlier findings point the same way. The OpenCode plugin study proposed
"Try Code Mode for MCP, behind a Gym measurement"
(`docs/research/2026-10-12-opencode-plugins.md:263`). The Connections model
was rebuilt from UsefulSoftwareCo/executor's design
(`crates/oa-connections/src/lib.rs:4`), which already separates tool identity,
credential custody, and policy, as the report's broker requires.

## What is missing

- **No output contracts.** No registry has an output schema, pagination
  rules, error retryability, or an idempotency field. `ToolDef` has only an
  input schema. A program that joins two tools has to guess their result
  shapes, which the report warns about in section 5.C.
- **No shared effect vocabulary.** Connections uses `Policy` per HTTP method.
  Coder uses `Effect` per command string. Program grants use six boolean
  axes. Device grants use seven rights. These answer related questions in
  different terms.
- **No shared dispatcher.** Each web door has its own model loop
  (`MAX_STEPS` of 6, 8, or 12) and its own `run` function. A program runtime
  that called these would have to copy their checks, which is the drift the
  report warns about.
- **No tool calls from scripts.** `workflow-run` scripts can start agents
  but can't call tools. Voyager's scripts call only Minecraft operations.
- **No parent links in traces.** ATIF records calls in a flat list.
- **No structured tool events for cloud chats.** The phone's cloud chat
  shows tool activity as secondary-colored text
  (`apps/openagents/apps/ios/OpenAgents/Session/CloudChatViewController.swift:416`).

## One canonical tool registry

Add a small crate, `crates/oa-tools`, with no I/O. It holds the contract that
every tool declares and the broker trait that every caller goes through.

| Field | Meaning | Source today |
| --- | --- | --- |
| `id`, `version` | Stable identity, such as `drive.read@1` | `ToolSpec::id` |
| `input_schema`, `output_schema` | JSON Schema for both directions | Input exists; output is new |
| `effect` | `read`, `draft`, `write`, `spend`, or `delegate` | `ToolSpec::default_policy`, `Effect`, `Effects` |
| `idempotency` | `natural` (safe to repeat), `keyed` (takes a key), `reconcilable` (has a read-back check), or `unsafe` | New; `publish.rs` is the `reconcilable` model |
| `authority` | What must allow it: an OAuth scope, a NIP-HOST right, or a program grant axis | `ToolSpec::scopes`, `Right`, `Effects` |
| `pages` | Whether results page, and how the result says it's complete | New |
| `errors` | Typed refusal codes, and which ones are retryable | New |
| `limits` | Concurrency and cost hints | New |

The broker trait is one method:

```rust
trait Broker {
    async fn call(&self, ctx: &CallContext, call: ToolCall) -> Result<Outcome, Refusal>;
}
```

`CallContext` carries the account, the scope, the parent program run (if
any), the remaining budget, and the confirm code for an approved write.
Policy runs inside `call`. The direct path (the model calls a function) and
the program path (a script calls `tools.drive.read(...)`) both use the same
`call`, so a check added once applies to both.

Adopt it in this order, because each step is small and useful on its own:

1. Add `output_schema`, `effect`, and `idempotency` to `ToolSpec` as optional
   fields. This is an additive change, and Connections already serializes
   `ToolSpec` with `serde(default)`.
2. Generate the web doors' function declarations from the contract, instead
   of the four hand-written `tools()` functions.
3. Wrap `connections::run_tool` (`crates/openagents-web/src/connections/mod.rs:280`)
   and the mail `run` (`mail.rs:210`) as the first broker implementations.
4. Describe the engine's MCP tools with the same contract. A mapping from
   `ToolDef` to the contract can carry the effect and right per tool. For
   example, `list_chats` is a read that needs `Observe`, and `send_message`
   is a write that needs `Operate`.
5. Serve discovery from the contract: `GET /v1/tools` and a `search_tools`
   MCP tool on `/mcp`. Tool search is useful even without code mode, as the
   report's section 1 table notes.

## Where programs run

The report separates the orchestration runtime, which needs few
capabilities, from the development environment, which needs a shell and a
repository. In our system, the orchestration runtime runs wherever the broker
for its tools is.

| Place | Runs programs? | Why |
| --- | --- | --- |
| Our cloud (`openagents-web` on Cloud Run) | Yes, for cloud chats | The Connections broker, credentials, and policy are already here. These doors already run Gemini on Vertex AI here. |
| The person's computer (the engine in `apps/openagents`) | Yes, for computer chats | The engine is the authority for chats, devices, and projects (`apps/openagents/crates/mcp/src/lib.rs:6`). A program that fans out chats or reads changed files belongs next to it. |
| A cloud computer (Boat or SPACE) | Only as a tool target | A program calls tools that run commands on the computer. The script itself doesn't need a shell. |
| The hosted eval runner | No, but it evaluates them | Its fixed grant (read and sandbox write) is right for measuring candidate programs, not for running a person's live work. |
| The phone | No | See the next section. |
| A model provider's runtime (Anthropic PTC) | Not by default | Data would pass through the provider's execution host, and its tools can't call our MCP connectors directly (report section 5.D). Keep it as an evaluation arm only. |

### Why not on the phone

The phone has no broker. Its tools are the server's and the computer's, so a
program on the phone would make every nested call over the network anyway,
and it would run with the phone's background limits. iOS suspends an app
that leaves the screen, which would cut a program off between a write and the
record of that write. That is the unresolved-write problem from the report's
section 6, with a higher chance of happening.

The phone already runs model-written code safely: `openagents-visual` runs a
Luau script with no I/O for each `show_visual` card. That is the right shape
for the phone. A program runs where its tools are, and returns a bounded
result that the phone can draw. If we later want the phone to filter or sort
data it already holds, a Luau view function can do it without calling tools.

## Language and runtime

Product code in this repository is Rust, and we don't add TypeScript. A
model-written program is not product code: it's runtime data that a host
bounds, as `crates/voyager/src/interpret.rs:1` says of its Lua scripts.
`workflow-run` already runs model-written JavaScript on that basis, with its
authoring guide in `plugins/skills/openagents-workflow/SKILL.md`. The
question is which interpreter, not whether we may embed one.

| Option | In the tree today | Strengths | Weaknesses |
| --- | --- | --- | --- |
| JavaScript on `boa_engine` | `crates/workflow-run` | Models write JavaScript well, and Cloudflare, Codex, Pi, Docker Agent, and Claude Code workflows all use it. Pure Rust, async native functions, already used with our budget and pause rules. | No in-process heap cap; only loop, recursion, and stack limits. Slower than QuickJS, which matters little for programs that wait on I/O. |
| JavaScript on QuickJS (`rquickjs`) | No | Memory limit and interrupt handler in process. Pi uses QuickJS. | A C dependency. A second JavaScript engine beside `boa`. |
| Luau (`mlua`) | `apps/openagents/crates/visual` | Designed as a sandbox, with memory and time limits; already builds for iOS. | Models write it less reliably than JavaScript. It would add a third action language beside JavaScript and Lua. |
| Lua 5.4 (`mlua`) | `crates/voyager` | Simple host bridge and a working code-as-action precedent. | Same model-skill concern. Not sandboxed by design. |
| Starlark | No | Deterministic and restricted, which suits replay (Bifrost uses it). | No async, and models write it less well. A new dependency. |
| Wasm components (`wasmtime`) | `crates/plugin` | The strongest isolation we have, with fuel, memory, and receipts. | Models don't write Wasm. A guest language would still need an interpreter compiled to Wasm. |

**Recommendation:** Use JavaScript on `boa_engine`, and extend
`workflow-run` rather than starting a second runtime. Add a `tools`
namespace generated from the contract next to the existing `agent()`. Close
`boa`'s missing heap cap with a process boundary. On a computer, run the
interpreter as a child process under `supervise`, with `coder-boundary` set
`offline` and no writable paths, because the script needs no filesystem.
On Cloud Run, run it in a worker process with an address-space limit. If a
measurement shows the per-run process costs too much, try `rquickjs` behind
the same `Host` trait before changing languages.

Keep Wasm for reviewed plugins. A plugin's Wasm operation becomes a tool in
the contract (`effect: read`, `idempotency: natural` for a `Pure` profile),
so a program can call `explain-error` or `repo-map` like any other tool.
Keep Luau for visuals.

## Permission on every nested call

The report's section 8.2 describes systems that check only the outer script.
We shouldn't do that. Instead, the broker checks each call against the policy
that already governs it:

| Nested call | Check inside the broker | Existing code |
| --- | --- | --- |
| A Google or GitHub read | Connection grant, then `effective_policy` | `connections/mod.rs:290` |
| A Gmail send or Calendar event | `RequireApproval`: suspend and show a card; run only with a confirm code bound to that exact card | `mail.rs:186`, `:297` |
| An engine tool on a computer | The calling device's NIP-HOST right (`Observe` for reads, `Operate` for chats) | `crates/coder-host` rechecks the grant before every channel message |
| A shell command in a cloud computer | `Effect` class; `Approval` waits for CONFIRM or REJECT | `crates/coder/src/task/agent.rs:1388`, `crates/coder-new/src/approval.rs` |
| A delegated agent (`agent()`) | Program grant `delegation` and `spend` axes, and the session budget | `program_authority.rs:84`, `workflow-run` budget |
| A saved agent's program | `Access::ReadOnly` allows only `read` tools | `agent-contract/src/lib.rs:45` |

Two rules apply on top of the per-call check:

- **Admission is a ceiling, not a grant.** A program declares the tools it
  may call in its `meta`, as workflow scripts already declare phases. The
  host refuses a program whose declared tools exceed the session's ceiling
  before it starts. The broker still checks each call, because arguments
  are computed at run time.
- **Text from a tool result never approves anything.** The mail door already
  checks that the confirmation came before anything was read in that turn
  (`mail.rs:16`). A program can't produce a confirm code, because only the
  person's tap on the card produces one.

## Tracing every nested call in ATIF

A program run is one ATIF `Call` named `program`. Its `arguments` hold the
source text, the source digest, the runtime name and version, and the
declared tools. Each nested call is its own `Call`, and its `extra` map holds
these fields:

- `parent`: the program call's id.
- `index`: the call's position in the run, which replay needs.
- `tool`: the contract id and version.
- `policy`: the decision (allowed, approved with which card, refused and
  why).
- `idempotency_key`: the key sent downstream, if any.
- `attempt`: the attempt number, and whether the outcome is `unknown`.

This fits ATIF-v1.8 now, because readers keep `extra`. Add a first-class
`parent` field in a later ATIF version only if a reader needs to filter on it.

One exception matters for privacy. ATIF keeps output in full
(`document.rs:89`), but the mail door logs no message, subject, or address
(`mail.rs:21`). For Connections tools, the server trace records counts,
digests, and citations, not content. The program's returned value, which the
model and the person see, stays in the chat as it does today.

## Suspension and unresolved writes

`boa` can't serialize a running program, so we use journal replay, as
Cloudflare does. This already matches `workflow-run`, which reuses a step
"whose prompt and options are unchanged."

1. Before each nested call, the host records an effect intent: run id,
   index, tool, and argument digest. This is the task owner's rule that a
   durable effect intent comes before dispatch.
2. When a call needs approval, the run pauses (`workflow-run` already has
   `pause`, at `lib.rs:702`). The card goes to the person. Nothing else runs.
3. After approval, the host runs the program again from the start. A call
   whose index, tool, and argument digest match a recorded result gets that
   result without running again.
4. If the replay reaches a different call at a recorded index, the run stops
   with a plain error. It doesn't guess.

Replay order must be deterministic. Allow `Promise.all` over `read` tools
only. A program that issues writes in parallel is refused at the first
parallel write. Date and random values come from host functions that are
recorded.

For a crash between a write and the record of its result, use the contract's
`idempotency` field:

| `idempotency` | After a crash | Precedent |
| --- | --- | --- |
| `natural` | Run it again. | Reads |
| `keyed` | Run it again with the same key; the service drops the duplicate. | A key built from run id, index, and argument digest, as `publish.rs:188` builds `operation_id` |
| `reconcilable` | Read back first; run it only if it didn't happen. | `publish.rs:607` checks the remote before pushing again |
| `unsafe` | Mark it `unknown`, stop, and tell the person. Never retry it automatically. | The task owner's recovery rule |

The host owns the run's lifetime, not the model. `workflow-run` already caps
open runs per session and run lifetime (`lib.rs:29`, `:33`). Cancellation
stops new calls. It doesn't claim that a call already sent has stopped.

## How the phone shows a program run

A program run is one card. It isn't a wall of text, and it isn't a stream of
separate tool lines. The card is a Rust Native view, like every other chat
card, so the phone, desktop, and web share one layout.

The card has four parts:

1. **A title and status.** The program's own short name, such as "Checked
   pull requests," with a spinner while it runs and a check mark or error
   when it ends.
2. **Folded calls.** The calls group by verb through `tool_groups`, the same
   module the Coder run view uses: "Read 12 files, listed 3 folders." Each
   write stands alone on its own row. The rows use the existing rail, icons,
   and file badges from `layout/tools.rs`. Tapping the group opens the rows.
3. **Approval, inline.** A write that needs approval shows its exact card,
   such as the draft or the event, with **Send** and **Don't send** buttons.
   The rest of the card shows "Waiting for you."
4. **The result.** The bounded value the program returned, with links to
   evidence: the files read, the pull requests checked, the chats started.
   A **Show program** control opens the source, for people who want it.

The data path takes three additive changes:

- **SDK.** Add `Event::Program { request_id, run, update }` to
  `crates/openagents-sdk/src/chats.rs`, carrying typed rows: id, tool, verb,
  target, status, and approval card. Old clients ignore an event kind they
  don't know.
- **Cloud chat state.** Give `CloudChatLine` in
  `apps/openagents/crates/mobile/src/client_ffi/cloud_chat.rs` an optional
  program card, instead of folding tool text into a line.
- **Computer chats.** Add `ToolCall::Program` to
  `apps/openagents/crates/proto/src/agent.rs:310`. Link the nested calls the
  way `subagent_ref` links a subagent's document today
  (`parts.rs:191`). iOS drops unknown kinds, and old desktop builds fall back
  to empty text, so older apps show nothing rather than something wrong.

The text people see follows our copy rules. Use "Checked 20 pull requests.
3 have failing checks.", not "Program executed 21 nested calls." Use "This
may have already been sent. We stopped so it isn't sent twice.", not
"Unresolved write; outcome unknown." Add the program words that people must
not see (nested call, dispatcher, journal, replay, broker) to the `oa-copy`
lexicon, and run its guard over the card's strings.

## Promoting programs into reviewed procedures

A program that worked once is a candidate, not a procedure. The promotion
path reuses what we have:

1. **Candidate.** "Save as a workflow" on a finished run's card writes a NIP-PRG
   program draft with the source, the inputs it read from `args`, the
   declared tools, and their contract versions.
2. **Review.** The draft goes through the skill directory's review stages in
   `tenancy::skills` (static, decision, and reasoning review), and through
   `ext-eval` with a test set: a baseline arm without the program and a
   subject arm with it.
3. **Admission.** An admitted program still runs only under an operator
   grant (`docs/coder/guides/program-authority.md`). Selection proposes;
   it doesn't grant.
4. **Invalidation.** A program pins its tools' contract versions. When a
   tool's major version changes, the program becomes unavailable until it's
   tested again.

Promotion is always a person's action or a reviewer's action. The model never
promotes its own program, which is the rule the report credits to
Cloudflare's runtime.

## Evaluation plan

Measure each part separately, as the report's section 9.6 says, with the
model, tools, tasks, permissions, and success checks held the same.

| Arm | Configuration |
| --- | --- |
| A | Direct calls, every tool declared up front (today's doors) |
| B | Direct calls with tool search and parallel calls |
| C | Programs, with the same tools declared up front |
| D | Programs with tool search |

Task families, in rollout order:

1. **Cloud chat reads.** Summarize a Drive folder, join Gmail threads with
   Calendar events, and list a repository's issues by label. These are
   arms A and C over the existing doors.
2. **Repository and CI inspection on a computer.** Failing checks across
   open pull requests, and changed files across chats (`changed_paths`).
3. **Fan-out.** Start and collect several chats, compared with the
   `create_chats` batch tool, which is already a fixed program.
4. **Interrupted writes.** A Calendar event or chat creation with a crash
   injected after dispatch. The pass condition is no duplicate and an
   honest `unknown`.

Record accepted-task rate, completeness (did it drop pages), cost per
accepted task, p50 and p90 latency, model turns, nested calls, retries, and
policy refusals. Run the suites through the Gym so results land in the
receipt-chained store with a digested gate. Adopt a configuration only when
paired runs show equal or better accepted-task rate and lower cost or fewer
turns, which is the bar the OpenCode study set.

## Phased plan

Each row is about one issue.

| Phase | Step | Ships through | Needs a new Coder binary? |
| --- | --- | --- | --- |
| 0 | Add `crates/oa-tools`: contract, broker trait, refusal codes | Library | No |
| 0 | Add `output_schema`, `effect`, `idempotency` to `ToolSpec`; fill them for Drive, Gmail, Calendar | `oa-connections` | No |
| 0 | Generate the web doors' function declarations from the contract | `openagents-web` | No |
| 1 | Add a `tools` namespace and per-call broker hook to `workflow-run` | Library | No |
| 1 | Run read-only programs in the Drive and mail doors, in a worker process | `openagents-web` | No |
| 1 | Write ATIF nested calls with `parent` and policy in `extra` | `atif` consumers | No |
| 1 | Add `Event::Program` to the chats stream and the SDK | `openagents-web`, `openagents-sdk` | No |
| 1 | Add the program card as a Rust Native view; show it in the phone's cloud chat | `apps/openagents` | No |
| 1 | Run the arm A and arm C suites for cloud chat reads in the Gym | `gym`, `ext-eval` | No |
| 2 | Add `GET /v1/tools` and `search_tools` on `/mcp` | `openagents-web` | No |
| 2 | Suspend for approval and resume by replay; Gmail send and Calendar create inside programs | `openagents-web`, `workflow-run` | No |
| 2 | Add idempotency keys and read-back checks for Connections writes | `oa-connections` | No |
| 3 | Add a `run_program` tool to the engine's MCP server, with NIP-HOST rights per call | `apps/openagents` (`openagents` binary) | Yes |
| 3 | Add `ToolCall::Program` and nested-call documents for computer chats | `apps/openagents` | Yes, for the engine; the phone reads it without one |
| 3 | Run the interpreter under `supervise` and `coder-boundary` offline on computers | `apps/openagents`, `coder-boundary` | Yes |
| 4 | "Save as a workflow" to a NIP-PRG draft; review and `ext-eval` | `openagents-web`, `tenancy` | No |
| 4 | Contract version pinning and invalidation for promoted programs | `oa-tools` | No |

## Coder 1.0 and what can ship now

The 1.0 freeze in `docs/coder/1.0-freeze.md` was lifted on 2026-10-16, and
one Coder 1.0.0 ships on 2026-10-19 from `main`
(`docs/audits/2026-10-16-coder-1.0-big-bang.md`). `docs/coder/next-release.md`
currently lists 1.0.0 scope. Code mode isn't part of that release, so:

- **Phases 0, 1, 2, and 4 ship from the server, the SDK, and the apps.**
  They change no Coder binary. They work with any 1.0.0 client, because the
  new API fields and events are additive, and old addresses, model ids, and
  error codes keep working.
- **Phase 3 needs a new `openagents` binary**, because the engine and its MCP
  server are compiled into it. Add its rows to `docs/coder/next-release.md`
  after 1.0.0 publishes, when that page goes back to listing work for the next
  release. Don't add them before, or they would read as Monday's scope.

## Risks

- **Correct-looking wrong answers.** A program can finish cleanly and join on
  the wrong id or miss a page. Output schemas with `complete` flags and
  counts, plus the Gym's completeness check, catch some of this. They don't
  catch all of it.
- **Filtered-away evidence.** The result must carry counts, scope, and links
  back to the source material, so the model and the person can check what the
  program left out.
- **More spend, not less.** A program that calls `agent()` many times spends
  much more than one turn. The session budget and the `spend` axis apply to
  every nested call, and the card shows the cost, as the Drive door already
  does (`connections/chat.rs:308`).
- **`boa` without a heap cap.** A runaway allocation could exhaust a server
  worker. The process boundary is required, not optional.
- **Replay surprises.** Programs that depend on timing or parallel writes
  break replay. Refuse parallel writes, and record time and random values.
- **Two code-mode layers.** If a third-party MCP server offers its own code
  execution, a program calling it adds a second language boundary. Prefer our
  broker as the one layer where we compose tools.
- **Drift during migration.** Until every door uses the broker, a tool can
  have different checks on its direct and program paths. Move a tool into
  programs only after its door uses the broker.
- **Prompt injection.** Tool results can still steer the model. Per-call
  policy and card-bound approval limit what an injected instruction can do,
  but they don't stop it from shaping the answer.
