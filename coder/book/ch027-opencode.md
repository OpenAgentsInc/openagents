# 27. OpenCode

> **Status: written.** Read at anomalyco/opencode@ff90ed91607cabf420a369e9009539c1aa0932af on 2026-11-02.

OpenCode is an open coding agent with a terminal UI, a desktop app and a web
app, all clients of one local server. It is MIT licensed (`LICENSE`),
written in TypeScript on Bun and Effect, and talks to many model providers.
The revision read here is its second major version: `specs/v2/` documents
the contracts, and the packages split into Schema, Core, Protocol, Server,
Client and SDK (`AGENTS.md`).

The source was cloned read only into `projects/repos/opencode` at `ff90ed9`,
the head of its default branch, `dev` (committed 2026-11-02). Nothing was
installed or run. Paths below are relative to the clone.

## Why study it

Claude Code and Codex are each built by a model provider. OpenCode is not,
so its design has to work for any model, and its server has to serve
several clients at once. Three of its choices bear on Coder:

1. **The agent is a server.** The terminal UI, desktop app and SDK speak one
   HTTP API, generated from one definition.
2. **Admission is separate from execution.** A prompt is a durable inbox
   item before any model work starts, and crash recovery is explicit.
3. **Another agent as a provider.** OpenCode runs a person's own Claude Code
   as a model provider, with Claude Code's tools turned off and OpenCode's
   tools offered instead. Coder drives Claude Code too (chapter 10), with
   different choices.

## What it does

| Feature | File at `ff90ed9` | Coder chapter |
| --- | --- | --- |
| One authoritative HTTP API; generated Promise and Effect clients; an in-process SDK with no listener | `specs/v2/README.md`, `packages/client/README.md`, `packages/sdk/README.md` | [23](ch023-openagents-coder.md) |
| A local background service found through a registration file, with Basic authentication | `packages/client/src/service.ts`, `packages/server/src/auth.ts` | [20](ch020-an-agent-that-stays-online.md) |
| Prompts enter a durable inbox; `steer` and `queue` delivery; execution is process-local with write-ahead claims for recovery | `specs/v2/session.md` | [1](ch001-the-simplest-loop.md), [2](ch002-model-visible-means-logged.md), [20](ch020-an-agent-that-stays-online.md) |
| Narrow retries: the first request plus at most four, with jittered backoff | `specs/v2/session.md` ("Retry Is Narrow And Observable") | [1](ch001-the-simplest-loop.md) |
| Instructions as content-hashed deltas logged as durable events | `specs/v2/session.md` ("Instructions Are Value Deltas") | [3](ch003-keep-the-prefix-stable.md) |
| Compaction to a rolling summary; one overflow-triggered compaction, then a terminal error | `specs/v2/session.md` ("Compaction Rebuilds Active History") | [7](ch007-condense-the-history.md) |
| One tool shape: input and output schemas, model-facing content, UI metadata; scoped registrations where the latest wins | `specs/v2/tools.md` | [4](ch004-tools-as-typed-capabilities.md) |
| Permission rules of action, resource and effect; the last match wins; no match means ask | `packages/schema/src/permission.ts`, `packages/core/src/permission.ts` | [5](ch005-a-sandbox-and-approvals.md) |
| Build and Plan agents; Plan denies edits outside a plan folder | `README.md` ("Agents"), `packages/core/src/plugin/plan.ts` | [5](ch005-a-sandbox-and-approvals.md) |
| A step allowance per agent, with a text-only reply when it runs out | `packages/schema/src/agent.ts`, `packages/core/src/session/runner/max-steps.ts` | [1](ch001-the-simplest-loop.md) |
| Code Mode: a small interpreter for a JavaScript-like language that reaches the world only through tools | `packages/codemode/README.md` | [8](ch008-programs-as-tool-calls.md) |
| Claude Code as a provider through the Agent SDK | `specs/v2/claude-agent-provider.md` | [10](ch010-drive-claude-code.md) |
| One encoded event feed with a bounded queue per connection | `specs/v2/event-stream-architecture.md` | [20](ch020-an-agent-that-stays-online.md) |
| MCP, ACP and skills | `packages/core/src/mcp/`, `packages/cli/src/acp/`, `packages/core/src/skill/` | [16](ch016-speak-the-protocols.md) |
| A PTY daemon in Rust on `libghostty-vt` | `packages/process-daemon/README.md` | [37](ch037-a-terminal-inside-the-agent.md) |
| Many providers behind one schema-first API, including decision models | `packages/ai/README.md`, `packages/ai/src/providers/` | [12](ch012-a-psionic-served-model.md), [13](ch013-choose-an-engine.md) |

### A server with many clients

Authority follows the concern: Protocol owns HTTP operations, Schema owns
public shapes and durable events, Core owns behavior and persistence
(`specs/v2/README.md`). Clients are generated from the assembled API, and
`bun run check:generated` catches drift between the API and the committed
clients (`packages/client/README.md`). The SDK "executes Server's assembled
HTTP router in memory, opening no listener and adding no network hop"
(`packages/sdk/README.md`), so an embedding program and a remote client call
the same operations.

The usual setup is one background server per machine. A client discovers it
through a registration file in the XDG state directory and can start it with
`opencode serve --service` (`packages/client/src/service.ts`). The server
accepts Basic authentication with a configured password
(`packages/server/src/auth.ts`). Events reach clients through one feed: the
server encodes each event once and offers the same frame to a bounded queue
per connection, 4,096 frames deep. A connection that falls behind is closed;
"publication and healthy connections continue in order"
(`specs/v2/event-stream-architecture.md`).

### Admission, execution and recovery

`specs/v2/session.md` is the most careful document in the repository.
`Session.prompt` first publishes a durable `session.inbox.enqueued` event;
the input stays outside the model-visible history until it is delivered. A
retried prompt with the same item ID is idempotent: "the first admission
wins". Delivery has two modes. `steer`, the default, delivers at the next
safe step boundary. `queue` waits until the session would otherwise go idle.

Execution is local to one process and keyed by session. When a busy period
starts, the runner commits a write-ahead claim; success, failure or a user
interrupt releases it, and a crash leaves it. On startup, claimed sessions
resume with "a durable continuation instruction". Before more model work,
tool calls still shown as running from an earlier process are marked
failed, and the document says what this cannot do: it "does not guarantee
exactly-once provider or tool behavior". Each tool call is durable "before
side effects begin".

Retries are narrow: rate limits, provider errors, unsent or unknown
deliveries and incomplete streams, "the initial request plus at most four
retries". If the provider reports a context overflow before any durable
output, the runner may compact once and rebuild the same step; "a second
overflow" is terminal.

### Instructions as deltas

Before each attempt, the runner reads every instruction source (built-ins,
`AGENTS.md` discovery, skills, references, MCP guidance), hashes each value
and logs one `session.instructions.updated` event mapping changed sources to
SHA-256 hashes. A later change is rendered once, frozen in the event, and
shown as a chronological system message, rather than rewriting the system
prompt (`specs/v2/session.md`). Global and per-directory `AGENTS.md` files
are found by `packages/core/src/config/plugin/instruction.ts`.

### Tools and permissions

A tool is one value with an input schema, an optional output schema and an
`execute` function. Its response carries three things: the validated
`output` that Code Mode receives, the model-facing `content` stored
durably, and optional `metadata` for the UI (`specs/v2/tools.md`). Every
call gets the session, agent, message and call IDs. Registrations are
scoped: "The latest active registration for a name wins", and closing it
reveals the previous one.

A permission rule is an `action`, a `resource` pattern and an `effect` of
`allow`, `deny` or `ask` (`packages/schema/src/permission.ts`). `evaluate`
in `packages/core/src/permission.ts` takes the last matching rule across the
rulesets, and a request that matches nothing gets `ask`. The configured
rules are checked first, and a configured deny wins over any saved "always
allow" answer. A session whose agent cannot be resolved denies everything
(`missingAgentPermissions`). When a request would ask and nobody is present,
as in a scheduled run, it is denied unless an `Unattended` ruleset allows it:
"Permission ... needs approval, and nobody is present to give it" (all in
`packages/core/src/permission.ts`). The Plan agent is a plugin that adds rules: questions allowed, edits
denied, edits under its own plan directory allowed
(`packages/core/src/plugin/plan.ts`). An agent can set `steps`
(`packages/schema/src/agent.ts`); when the allowance runs out, the runner
disables tools and asks for a text summary of what was done and what
remains (`packages/core/src/session/runner/max-steps.ts`).

### Code Mode

Code Mode is OpenCode's version of programs as tool calls. It is "a
lightweight, pure interpreter for a JavaScript-like language". "Rather than
trying to sandbox arbitrary JavaScript, CodeMode only runs the language
features we implement"; programs reach the outside world "only through tools
provided by the host, which can also limit execution time, tool calls,
output size, and data" (`packages/codemode/README.md`).

### Claude Code as a provider

`specs/v2/claude-agent-provider.md`, a draft, describes running the person's
own `claude` through Anthropic's Agent SDK. OpenCode never reads Claude
Code's saved login. Claude Code's built-in tools are off; Claude gets
OpenCode's tools, and OpenCode runs each call "like any other model's tool
call, including permission prompts". The reason given: "the history looks
the same whichever model wrote it", and "there is also only one permission
system". Claude Code's settings files, `CLAUDE.md`, hooks, MCP servers,
skills and automatic compaction are all turned off.

## What Coder takes

- **One API, generated clients** ([chapter 23](ch023-openagents-coder.md)).
  The proposed Coder contract puts verbs, `--json` output and MCP tools in
  one definition in `proto/`, with a drift check as `check:generated` does.
  Chapter 23's shared-catalog work remains open; the existing MCP dispatch
  running CLI verbs is not that generated definition.
- **Admission before execution** ([chapter 2](ch002-model-visible-means-logged.md),
  [chapter 20](ch020-an-agent-that-stays-online.md)). A message to Coder is
  logged as admitted before any model work, with an ID that makes a retry
  idempotent. `steer` and `queue` map onto chapter 17's Router batching.
- **Honest recovery** ([chapter 20](ch020-an-agent-that-stays-online.md)).
  A write-ahead claim per busy period; on restart, calls left running are
  marked failed, never replayed; and the docs say exactly-once is not
  promised.
- **Logging the tool call before it runs** ([chapter 4](ch004-tools-as-typed-capabilities.md)).
  Chapter 5 already logs every call and answer; Coder adds the rule that the
  call event is durable before any side effect.
- **Instructions as hashed deltas** ([chapter 3](ch003-keep-the-prefix-stable.md)).
  A changed `AGENTS.md` appends a new message instead of rewriting the system
  prompt, so the prefix stays stable and the change is visible in the log.
- **A bounded queue per client** ([chapter 20](ch020-an-agent-that-stays-online.md)).
  A slow terminal or phone never slows the agent or other clients.
- **Deny when nobody can answer.** An unattended request that would ask is
  denied, as Coder's clients already decline unanswered approvals
  ([PARITY.md](../../coax/lib/agent/codex/PARITY.md)).
- **A text-only reply when an allowance runs out**. Coder sets no step limit of its own
  (`coax/lib/microcoder/loop.coax`), but a person can set one, and at the
  limit Coder reports what it did and what remains.
- **Code Mode's stance** ([chapter 8](ch008-programs-as-tool-calls.md)).
  Chapter 8's programs already run once against bound host functions, with
  limits that refuse rather than truncate. OpenCode reaches the same design
  from the other side: implement only the language you can bound.

## What Coder leaves, and why

- **Last match wins.** Within the configured rules, OpenCode's outcome
  depends on rule order, so a rule written later can open what an earlier
  rule denied. Coder keeps
  Claude Code's fixed deny, ask, allow order (chapter 25), where no rule can
  carve an exception out of a deny.
- **Replacing the driven agent's tools.** OpenCode turns off Claude Code's
  tools and offers its own, so the history is uniform. Coder drives Claude
  Code and Codex with their own tools, because people choose those engines
  for how they work, and answers their approval requests with chapter 5's
  policy. Coder's in-process MCP tools (`coax/lib/agent/claude/mcp.coax`)
  add to Claude Code's tools rather than replace them. Which choice gives
  better results is a question for chapter 13's held-out set, not a matter
  of taste.
- **A password-protected local HTTP server as the only control plane.**
  Coder's local verbs are a CLI and MCP over stdio. A network listener comes
  with chapter 20's daemons, with per-daemon grants and credential handles.
- **TypeScript plugins sharing the host's runtime.** OpenCode loads plugins
  into the host process and shares one `effect` instance with them
  (`packages/plugin/README.md`). Coder's third-party plugins run in bounded
  Wasm ([chapter 15](ch015-plugins-with-owned-lifecycles.md)).
- **Many providers as a goal.** Coder reaches models through its gateway
  plan and Psionic. Its engine choice is a typed System One decision
  ([chapter 13](ch013-choose-an-engine.md)), not a provider list.
- **Linking libghostty.** OpenCode's PTY daemon links `libghostty-vt`.
  Coder's terminal emulator is Coax and uses libghostty only as an oracle in
  tests ([chapter 37](ch037-a-terminal-inside-the-agent.md)).

## Where to measure a borrowed design

- Narrow retries and one overflow compaction: [chapter 21](ch021-measure-the-harness.md)
  compares completion and cost against the simplest loop's retry rule.
- Instruction deltas against rewriting the system prompt:
  [chapter 3](ch003-keep-the-prefix-stable.md) checks the prefix bytes;
  Psionic measures the cache hits.
- Recovery after a killed process: [chapter 20](ch020-an-agent-that-stays-online.md)'s
  watchdog tests kill a run mid-tool and check that the call is marked
  failed, not replayed.
- Uniform tools against each engine's own tools:
  [chapter 13](ch013-choose-an-engine.md).

## Licensing

MIT (`LICENSE`). Coder ports designs and copies no code or text.

## Sources

- [anomalyco/opencode at ff90ed9](https://github.com/anomalyco/opencode/tree/ff90ed91607cabf420a369e9009539c1aa0932af)
- [AGENTS.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/AGENTS.md)
- [README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/README.md)
- [specs/v2/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/specs/v2/README.md)
- [specs/v2/session.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/specs/v2/session.md)
- [specs/v2/tools.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/specs/v2/tools.md)
- [specs/v2/event-stream-architecture.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/specs/v2/event-stream-architecture.md)
- [specs/v2/claude-agent-provider.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/specs/v2/claude-agent-provider.md)
- [packages/client/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/client/README.md)
- [packages/client/src/service.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/client/src/service.ts)
- [packages/sdk/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/sdk/README.md)
- [packages/server/src/auth.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/server/src/auth.ts)
- [packages/schema/src/permission.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/schema/src/permission.ts)
- [packages/core/src/permission.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/core/src/permission.ts)
- [packages/core/src/plugin/plan.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/core/src/plugin/plan.ts)
- [packages/core/src/session/runner/max-steps.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/core/src/session/runner/max-steps.ts)
- [packages/core/src/config/plugin/instruction.ts](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/core/src/config/plugin/instruction.ts)
- [packages/codemode/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/codemode/README.md)
- [packages/plugin/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/plugin/README.md)
- [packages/process-daemon/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/process-daemon/README.md)
- [packages/ai/README.md](https://github.com/anomalyco/opencode/blob/ff90ed91607cabf420a369e9009539c1aa0932af/packages/ai/README.md)
- [Chapter plan](PLAN.md), Part VII.
