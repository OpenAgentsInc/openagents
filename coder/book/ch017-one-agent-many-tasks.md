# 17. One agent, many tasks

> **Status: partly implemented.** A Router runs in Coder. Events come from
> `coder trigger` and from GitHub sources (new labelled issues and new
> comments). `coder router` decides each event's fate: first in code on
> bounded fields, then by a System One decision. Workers are detached Coder
> sessions, a later event on a subscribed Thread passes straight to its
> Worker, and when a Worker ends the Router writes its reply to the outbox
> and, for a GitHub source that asks for it, posts it on the issue. The
> offline smoke, a live System One check and a live read-only GitHub poll
> cover it. Still to come: chat and CI sources, a Router model for replies
> that need more than the Worker's result, batching, inline and spill, chain
> of recall, acting ahead, many-to-many subscriptions, and the ablation.

Chapters1–19 build one session well. A person can follow only three to
five active sessions at once, so past that point the limit is their attention,
not the model. This chapter turns the harness into one continuous agent that
takes work from chat rooms, issues, pull requests and CI, runs many tasks at
once, and acts on events without waiting to be asked. The design follows the
DeepSeek Harness team's Super-agent article
([notes](../../psionic/docs/harness.md#one-agent-across-many-tasks)).

The person never picks a session. Sessions become an implementation detail of
the harness, and the agent they talk to is the same agent in every channel.

## What this chapter covers

- **Two levels.** One long-lived Router session and many Worker sessions:
  - The Router receives every human message and every admitted event, keeps
    the global picture, dispatches work and writes every reply. It has no
    shell or file tools and does not execute tasks, so it stays fast. A
    smaller model can serve it.
  - Each Worker owns one task, has an isolated context and the full tool grant
    of chapters4 and 5, and never talks to people. It reports status and
    conclusions to the Router. A Worker can use chapter14's subagents
    internally; the Router does not see them.
- **Triggers.** The agent's input is events from its environment: chat
  messages, issue and review comments, CI results, calendar items and mail.
  Each Trigger is a typed record from `proto/` with a stable ID, a source, a
  Thread and a summary.
- **Two-stage filtering.** A fast path and a slow path, as in a network stack:
  - Fast path: code admits or drops events on bounded fields only (source,
    room or repository ID, author and bot allowlists). Each source is set to
    one of `explicit_requests_only`, `candidate` or `ignore`. No keyword
    matching decides intent.
  - Slow path: admitted candidates go to a model with written policies. The
    first check is a System One decision through the Decisions API (chapter
    210), and only events it cannot settle reach the Router. Policies are
    prompts that a person can add, change or turn off at any time.
- **Threads and subscription.** Consecutive events on one pull request, chat
  topic or mail chain form a Thread. When the Router starts a Worker for a
  Thread's first event, the Worker subscribes to it:
  - Subscribed events pass straight through to their Workers in code, without
    a Router decision.
  - The Router still sees every event, but decides only on events that no one
    subscribes to.
  - Subscriptions are many-to-many. Workers that share a Thread all receive
    its events, so they can notice and settle overlaps without messaging each
    other. A Worker can unsubscribe or hand an event back to the Router.
- **Batching in the Router.** Triggers that arrive during a Router step queue
  and enter the next request together. The Router's steps have long inputs
  and short outputs, so prefill computes the batch in parallel and the time
  per decision falls as the queue grows, up to the context and accuracy
  limit. Chapter3's stable prefix keeps those long inputs cached.
- **Inline and spill.** The context window is a cache:
  - Start an injected object with its ID and a short summary.
  - Inline what the model is likely to need next by locality: IDs of related
    objects, the message it replies to, neighbouring messages and review
    comments, the first characters of long content.
  - Spill the rest. Truncate at a fixed cap and leave a pointer that a read
    tool follows on demand.
- **Chain of recall.** Triggers, Threads, Workers and tasks all have IDs, and
  every injected or recalled object inlines the IDs and summaries of the
  objects next to it. The Router recovers an old task's context by following
  references hop by hop through the reference graph, from chapter2's log,
  instead of keeping every task in its window.
- **Acting ahead.** A policy can start a Worker before anyone asks: reproduce
  a reported bug, check a proposed change, draft a reply to an invitation. The
  Worker delivers only a confirmed result, such as a reproduced bug with a
  small fix as a pull request, and otherwise stays silent. Discarded work is
  logged with its cost.

## What runs

### Triggers

`coder trigger "TEXT" [--thread T] [--source S] [--author A] [--to-agent]`
writes one Trigger record to `~/.openagents/coder/inbox/ID.json`:

```json
{"id":"20261102-073134-163","source":"local","thread":"ci","author":"me","to_agent":true,"text":"$ echo first"}
```

Any program that can write a file can be a source: a chat bridge, a webhook
receiver or a CI step. `to_agent` is a bounded field the source sets when
the event is addressed to the agent, for example a mention or a direct
message. The Router never guesses it from the text.

### GitHub sources

A source named `github:OWNER/REPO` is polled on every Router pass through
`gh api` (`rt_poll_github`):

```json
"github:OpenAgentsInc/openagents":{"mode":"explicit_requests_only","authors":["AtlantisPleb"],
  "label":"coder","engine":"claude","deliver":true}
```

- An open issue with the source's label becomes an event addressed to the
  agent. The label is the bounded field: whoever added it asked for the work.
- A new comment becomes an event on its issue's Thread,
  `github:OWNER/REPO#N`. On a Thread with a Worker it goes to that Worker;
  otherwise the source's mode decides.
- The first poll records the time in `router/since-OWNER_REPO` and takes
  nothing older. `router/seen-OWNER_REPO` keeps each issue and comment from
  becoming an event twice.
- Comments the Router posted itself are listed in `router/posted` and never
  come back as events, so a reply can't start a loop.

In the live check, the Router read six hours of comments on this repository
and dropped each one with `dropped: author not allowed`, because that
source's author list allowed no one. A second pass found nothing new.

`coder policy add` writes these entries, and `coder watch THREAD` makes
every later event on a Thread addressed to the agent (chapter 23).

### Sources and the fast path

Each source has a policy in `~/.openagents/coder/router/sources.json`:

```json
{"sources":{
  "local":{"mode":"explicit_requests_only","authors":["me"],"engine":"echo"},
  "chat":{"mode":"candidate","authors":["*"],"policy":"A team chat. The agent works on requests about this repository; ignore social talk."},
  "spam":{"mode":"ignore","authors":["*"]}}}
```

`coder router` ([router.coax](../../coder/tui/router.coax), `rt_fast`)
decides in code, on the source, the author and the Thread only:

| Event | Fate |
| --- | --- |
| Unknown source, or mode `ignore` | dropped: source ignored |
| Author not on the source's list | dropped: author not allowed |
| A Worker subscribes to the Thread | passed to its Thread's Worker |
| `to_agent` is true | addressed to the agent: new Worker |
| `explicit_requests_only` and not addressed | dropped: not addressed to the agent |
| `candidate` and not addressed | the slow path |

### The slow path

A candidate goes to System One (mammoth-1) as a typed yes-or-no question:
does the event ask the coding agent to do work now, under the source's
written policy? The state System One sees is the event's source, Thread,
author and text, and the policy. A probability of 0.5 or more starts a
Worker. With no System One key, the event is dropped and the log says why.
No keyword decides intent.

In the live check, with the `chat` policy above, "the build on main is
failing in the parser tests, can someone fix it?" started a Worker, and
"anyone want lunch at noon?" was dropped.

### Workers and subscription

A Worker is a background Coder session (chapter14): the Router runs `coder
run TEXT --detach`, on the source's engine when the policy names one. The
Thread then subscribes to that session, in `router/threads` (one Thread and
session ID per line). A later event on the Thread passes to its Worker in
code, as `coder fork SESSION TEXT --detach`, so the new session carries the
Thread's conversation. The subscription then moves to the new session. A
Worker never writes to people; the Router's log records each event's fate:

```json
{"kind":"trigger","id":"20261102-073137-458","source":"local","thread":"ci","fate":"passed to its thread's worker","session":"20261102-073137-636","at_us":1793604697637825}
```

Decided events move to `inbox/done/`, so each event is decided once.

### Replies

When a Thread's Worker ends, the Router writes the reply once
(`rt_report`), under the source's channel (chapter20's channel rules):

- `chat`, the default: the last turn's result in at most 100 words, cut with
  " …". A Worker that didn't finish says how it stopped: `Stopped (lost): …`.
- `issue`, the default for GitHub sources: structured, with what happened
  first and then the full result:

  ```text
  **Coder** finished in 3 steps (session `20261102-073134-333`).

  Fixed the parser test: …
  ```

The reply goes to `~/.openagents/coder/outbox/SESSION.json` with its Thread,
source and channel, where any bridge can pick it up. A GitHub source with
`"deliver": true` also posts it on the issue with `gh issue comment`; without
it, nothing leaves the machine. `router/reported` keeps each Worker from
replying twice.

## Checks

- `coax test coder/tui/coder.coax`: the fast path on every combination of
  mode, author, Thread and `to_agent`. Also source and author lookup, and the
  Thread table's read and write.
- `python3 coder/tui/smoke.py`, offline:
  - An addressed event starts a Worker.
  - An ignored source and an unaddressed event are dropped.
  - A later event on the Thread goes to its Worker as a fork whose header
    names the first Worker.
  - A finished Worker's reply goes to the outbox under the chat rules.
  - Against a stand-in `gh`: the first poll only sets the cursor; the next
    turns a labelled issue into a Worker; the Worker's reply is posted on the
    issue in the issue format; the Router's own comment is not an event.
- Unit checks for the last turn's result, the chat word cap and the issue
  format, and GitHub thread and cursor parsing.
- Live: the two `chat` events above, decided by System One, and the
  read-only GitHub poll.

## Dependencies

- [Chapter1](ch001-the-simplest-loop.md) and [Chapter2](ch002-model-visible-means-logged.md): the loop each session runs, and the log that every object ID points into.
- [Chapter3](ch003-keep-the-prefix-stable.md): the Router's long, stable prefix.
- [Chapter4](ch004-tools-as-typed-capabilities.md) and [Chapter5](ch005-a-sandbox-and-approvals.md): Worker tool grants; the Router has none that execute.
- [Chapter6](ch006-a-document-tries-to-become-the-system-prompt.md): Triggers are untrusted input.
- [Chapter14](ch014-forks-subagents-and-jobs.md): subagents and jobs inside a Worker.
- [Chapter16](ch016-speak-the-protocols.md): System One decisions for the slow-path filter.
- [Chapter21](ch021-measure-the-harness.md): the measurements below.
- [Chapter20](ch020-an-agent-that-stays-online.md): the asynchronous loop and channel rules the Router uses.

## Done means

- **Ablation.** A replayable synthetic stream of about 300 chat and CI
  messages, about 20 of them work requests and the rest chatter, follow-ups,
  cancellations and CI noise, drives four designs with simulated work tools
  that only consume time, five runs each:
  1. Router and Workers with pass-through.
  2. Router and Workers without pass-through.
  3. One session with a subagent tool and a neutral prompt.
  4. One session with a subagent tool and a delegation-first prompt.
  
  Each run reports task completion time at P50 and P95 (request to result
  message), cache-read tokens across all sessions and Router decisions. The
  chapter records whether design 1 beats designs 3 and 4 on P95 and whether
  pass-through cuts Router decisions and tokens, with Psionic chapter 240's noise
  floor. The DeepSeek team's indicative result was 12.4 and 15.7 minutes
  against 13.8 and 29.0 minutes, with pass-through cutting Router
  cache-read tokens by about 37%; Psionic records its own numbers.
- **Batching.** With a real Psionic-served Router model and mock Workers, raise
  the arrival rate of Triggers that need a decision from 1 to 40 per minute.
  Report mean batch size, time per request and amortized time per decision at
  each rate, and the rate where decision accuracy on a labelled set first
  drops.
- **Filtering.** On a labelled event stream, report the fraction the fast path
  drops, the slow path's misses and false starts, and model calls per hundred
  events.
- **Recall.** A follow-up about a task that left the Router's window long ago,
  sent from a different channel, recovers the task through the reference
  graph. Report hops and tokens read.
- **Acting ahead.** Report delivered and discarded speculative runs and their
  token cost. No speculative run posts anything before its result is
  confirmed.
- A Worker never writes to a channel directly, checked against the captured
  outbound sink.
- Each mechanism passes chapter21 against the simpler design on tasks it was
  not built from, or it is removed.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- DeepSeek Harness team, "Exploring Super-agent Design: Towards a More
  Autonomous Agent System" (Yujia Qiao et al.), and its
  `dsh-superagent-prototype` plugin.
