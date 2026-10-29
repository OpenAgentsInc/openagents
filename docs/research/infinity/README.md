# Infinity (Hydro project): agents as data between turns

Written 2026-10-28. Sources:

- the repo, https://github.com/hydro-project/infinity, read at `1abc6fc` (2026-10-28);
- the launch post, "Introducing Infinity: a runtime for extreme agent concurrency" (AWS Open Source blog, Shadaj Laddad);
- the docs in the repo under `docs/docs/` (runtime architecture, threading, the RAP spec, Infinity Code).

Infinity is Apache-2.0 Rust from the Hydro project (AWS with UC Berkeley). It is
an early research release.

## What it is

Three things in one repo:

1. **Infinity Runtime** (`infinity-agent-core`): a Rust library that runs agent
   loops the way `async` runs functions. A tool call is an await point. While
   an agent waits, it is only its conversation history in a store: no task, no
   stack and no open connection.
2. **RAP, the Reactive Agent Protocol**: an open HTTP protocol for tools that
   answer later. The runtime POSTs an invocation with a `callback_url` and a
   `group_id` (the thread). The tool acknowledges with 200 at once and POSTs
   the result back whenever it is ready, 100 ms or three days later. MCP
   servers run through a compatibility bridge.
3. **Infinity Code**: a coding agent built on the runtime:
   - a daemon, a TUI and a web UI;
   - edits happen in a sandbox (a jj workspace or git worktree, on a
     `sandbox-{thread_id}` branch);
   - parallel child threads, background sessions, and remote sessions over SSH;
   - a Slack bot.

The crates include:

- `infinity-agent-core`, `infinity-agent-lambda`, `infinity-daemon` and `infinity-agent-cli`;
- the providers `infinity-provider-bedrock` and `infinity-provider-rig`;
- `rap-protocol`, `rap-client`, `infinity-rap-bridge` and `infinity-mcp-bridge`;
- `sandbox-local` and `sandbox-remote`;
- `rap-github-event-poller`.

## How it works

**The invariant: a slice never blocks on anything external.** Each run of an
agent is a bounded *slice*:

1. Load the history and the IDs already processed.
2. Turn the new inputs into history. Redelivered messages are skipped, and
   messages for closed threads are dropped.
3. Stream one completion.
4. Persist the turn.
5. Dispatch at most one asynchronous tool call, then end.

The tool's answer comes back later as a new message and starts a new slice.

**Everything is one message type.** User text, tool results, subscription
events, reports from child threads, OAuth prompts and timers are all an
`InputMessage` with a `group_id`. Each thread's messages are FIFO, and
different threads run in parallel. That behaves like an actor per
conversation, with the ordering held by the queue rather than by locks:

- **On AWS:** an SQS FIFO queue with `MessageGroupId` per thread, Lambda per
  step, Aurora DSQL for histories, DynamoDB for dedup, and EventBridge for
  `sleep`.
- **Locally:** one channel and worker per thread.

**Durability.**

- Streamed output goes into a turn buffer and is committed only when the turn
  ends. A stream that fails partway is thrown away and retried.
- A turn that ends in a tool call is persisted *before* the call is
  dispatched.
- Every input has a stable ID, and the state store records which IDs each
  thread has processed. That makes at-least-once delivery effectively
  exactly-once.

**Synchronous tools loop back.** Cheap tools such as `spawn_thread` run inline
and the completion continues in the same slice. An unknown tool name gets an
injected error result, so a hallucinated tool costs one completion instead of
a stuck thread.

**Events the model wasn't trained for.** An event that arrives
asynchronously is written into history as a tool call the model never made,
`receive_event__injected`, followed by its result. The authors rejected:

- a second result on an old call, which breaks the one-result rule;
- a user message, which misattributes the event;
- the system prompt, which loses ordering.

The model can't call the `__injected` name itself.

**Subscriptions and threads.**

- A tool can mark its first result as the start of a subscription. Later
  events for that call wake the agent.
- `spawn_thread` makes a child thread. By default it inherits the parent's
  history up to the spawn; `fresh_context` starts it empty.
- A child reports with `report_to_parent` and finishes with `close_thread`.
- By default each subscription event is handled in a short-lived child
  thread, so the parent's context stays small. *Associative* events, such as
  background test output, go straight into the subscriber's history.
- Compaction summaries stand in for the prefix of history they cover, across
  ancestors too.

**Numbers (from the post):**

- 80,000 agents with a simulated model, 20 turns each with two completions
  (3.2 M completions), on one thread of an AMD EPYC 9R14 with in-memory
  stores.
- Memory grew linearly at about 100 KB per idle agent, mostly history: about
  75,000 agents in the 8 GB of a Raspberry Pi 5.
- On Lambda, an agent waiting three days for CI uses no compute.

**Open questions they name:**

- how interleaved events change what models decide, given they were trained
  on synchronous traces;
- priority scheduling (today it is first come, first served);
- compaction across a tree of agents.

## What it means for us

We already do pieces of this by hand:

| Ours | Infinity's name for it |
| --- | --- |
| `/chat`'s turn queue in oa-api (`chat.turns`, `queued`/`running`; the runner polls `next`) | slices off an input queue |
| Project agents and Foreman waiting on `release.sh`, CI, TestFlight builds | long-running RAP tool calls |
| The Mac app's planned room (`ask_agent`, `tell_agent`, `room_read`, docs/desktop/coax-desktop.md) | subscriptions, `spawn_thread`, `report_to_parent`, associative events |
| Projects on the Nostr relay: tasks, plans and claims as signed events | an event log that wakes agents |
| "No courtesy loops": an answer returns as the asker's tool result | `ask_agent` as a subscription whose result comes back once |

### Ideas to take now (no dependency)

1. **The slice invariant for our runner.** A turn never holds a process
   open waiting on a build or a person. It persists, dispatches and ends. A
   `/chat` or project-agent turn that waits on CI should become a callback,
   not a sleeping process. This matters for the $1,500/month GCP rule: idle
   agents should cost nothing.
2. **Commit only finished turns, and dedup by message ID.** Our `chat.turns`
   runner should buffer streamed output and commit at turn end. It should
   also record processed input IDs per conversation, so a redelivered `next`
   or a runner restart can't double-append.
3. **Per-thread FIFO, parallel across threads**, enforced by the queue rather
   than by locks:
   - In Postgres: `SELECT … FOR UPDATE SKIP LOCKED` keyed by conversation.
   - On GCP: Pub/Sub with ordering keys, or Cloud Tasks with one queue per
     group.
4. **Injected events in the Mac app's room.** When Codex tells Claude
   something mid-turn, the event goes into Claude's history as an injected
   tool call and result (`receive_event__injected`). It doesn't go in as a
   user message, which is the misattribution Infinity calls out. `tell_agent`
   maps to an associative event and `ask_agent` to a subscription that
   returns once. Their reasoning supports the rule already in our spec.
5. **Events in short-lived child threads by default.** Project agents that
   watch the relay or GitHub would get a short-lived child per event, which
   reports up. This keeps the long-lived agent's context small.

### RAP over Nostr

RAP is three verbs:

- `invoke`, with a `callback_url` and a `group_id`;
- an ack;
- callbacks of type `tool_result`, `subscription_event`, `oauth`,
  `user_choice` and `view_update`.

Our relay could carry them:

- the invocation is a signed event addressed to the tool's key;
- results and events are signed events tagged with the thread (`group_id`)
  and the call `id`;
- "the callback URL" is a relay subscription.

That gives us properties the spec leaves to implementations: callbacks are
authenticated by signature, replay is caught by event ids, and the log
survives a restart. It also fits the Nostr identity we already use for
`/login` and projects. Worth a short spec if the room, or project agents,
need tools that answer later. MCP compatibility would come from the same kind
of bridge Infinity ships.

### What doesn't fit

- **It drives model APIs, not Claude Code.** Infinity's providers call Bedrock
  (or `rig`) with API credentials. The Mac app's first milestone runs the
  user's own `claude` CLI on their own login. That CLI owns its agent loop,
  its tools and its session, so it can't be cut into Infinity slices.
  Infinity fits our *server* agents (the /chat runner, project agents), not
  the local harnesses.
- **AWS-shaped deployment.** The reference stack is SQS FIFO, Lambda, DSQL,
  DynamoDB and EventBridge. We run on GCP, and Vertex is our first provider.
  The core library is storage- and queue-agnostic (`ConversationStore`,
  `StateStore` and `InputSender` are traits), so a Postgres plus Pub/Sub port
  is plausible, but nothing for GCP exists today.
- **Early research.** The APIs will move. The post's own open questions
  (event interleaving, priorities, tree compaction) are exactly what a
  many-agent product hits first.
- **Language.** Our agent code is Coax (`coax/lib/agent/`). Infinity is Rust.
  Taking it as a dependency would put a second agent runtime next to ours. Its
  value to us is the design, not the crate.

## Recommendation

- **Don't adopt the crate.** Port the design into `coax/lib/agent`:
  1. a slice runner with a turn buffer, per-thread FIFO and dedup by
     message ID, used first by the `/chat` runner;
  2. injected events for the Mac app's room (M4);
  3. a RAP-over-Nostr note when a tool first needs to answer hours later, the
     first candidate being "tell me when release.sh / CI finishes".
- **Try Infinity Code locally as a reference**, the way we read Codex and
  OpenCode. Its daemon and background sessions, sandboxes per thread on
  jj/git, and diffs to review overlap with the Mac app's M3 to M5. Install:
  `cargo install infinity-agent-cli --git https://github.com/hydro-project/infinity --features bundled-web`.
  It needs a Bedrock provider (AWS credentials) before it can run a model.
- **Track the open questions.** Their eventual answer on how models behave
  with interleaved events is directly relevant to the room. If they publish
  an eval, run it against Opus 5.5 and GPT 6.1 Sol.
