# 34. Background Agents

> **Status: written.** Read at ColeMurray/background-agents@80426f42e72abf3f76f71d00045b0e8b77224224 on 2026-11-02.

Source: [ColeMurray/background-agents](https://github.com/ColeMurray/background-agents/tree/80426f42e72abf3f76f71d00045b0e8b77224224)
at `80426f42e72abf3f76f71d00045b0e8b77224224` (MIT, last commit 2026-10-31).
The project calls itself Open-Inspect, after Ramp's Inspect (`README.md`). A
shallow clone was read under `projects/repos/background-agents`; nothing was
installed or run. Paths below are relative to the repository root.

## Why study it

Open-Inspect is not a new agent loop. It runs existing harnesses (OpenCode,
the Claude Agent SDK and the Codex CLI) inside cloud sandboxes, and builds
everything around them: durable sessions, prepared environments, a branch per
session, pull requests attributed to the person who asked, and review
comments fed back as follow-up prompts (`docs/HOW_IT_WORKS.md`). That is the
part of Coder that chapters 14, 17, 20 and 23 describe and that Coder has not
built yet. Its design is written down in detail, including the failure
paths, so the chapter can compare it with the plan line by line.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| A control plane on Cloudflare Workers; one Durable Object with its own SQLite database per session | `packages/control-plane/src/cloudflare/durable-object.ts`, `docs/HOW_IT_WORKS.md` "Architecture" | 2, 20 |
| A durable prompt queue per session; prompts run in order, and a follow-up waits for the running turn | `packages/control-plane/src/session/message-queue.ts`, `docs/HOW_IT_WORKS.md` "Prompt Queuing" | 17 |
| Events (tokens, tool calls, status, artifacts) stored in the session and broadcast to every connected client | `packages/control-plane/src/session/event-stream.ts`, `docs/HOW_IT_WORKS.md` "Real-time Events" | 2, 20 |
| A bridge in the sandbox: a WebSocket to the control plane, a heartbeat, boot phases, prompt and stop commands, git identity per prompt author | `packages/sandbox-runtime/src/sandbox_runtime/bridge.py` | 20 |
| One harness seam; OpenCode, Claude Agent and Codex emit the same events, and the bridge owns turn completion | `packages/sandbox-runtime/src/sandbox_runtime/harness/`, `docs/HOW_IT_WORKS.md` "The Agent" | 10, 11, 13 |
| Boot: clone, `.openinspect/setup.sh`, `.openinspect/start.sh`, managed skills, harness start, ready | `packages/sandbox-runtime/src/sandbox_runtime/repository_boot.py`, `docs/HOW_IT_WORKS.md` "The Sandbox Lifecycle" | 20 |
| A connect watchdog, a boot budget and a heartbeat timeout; no snapshot from a failed boot | `docs/HOW_IT_WORKS.md` "How long a boot may take" | 20 |
| Filesystem snapshots after each completed prompt and before an idle shutdown; prebuilt images per repository | `docs/HOW_IT_WORKS.md` "When Snapshots Are Taken", `docs/IMAGE_PREBUILD.md` | 14, 20 |
| A branch per session, `open-inspect/{session-id}` | `packages/shared/src/git.ts` (`generateBranchName`) | 23 |
| Commits authored by the prompting person; pull requests opened with that person's GitHub token, or the App's for unattended runs | `packages/control-plane/src/session/pull-request-service.ts`, `docs/HOW_IT_WORKS.md` "Creating Pull Requests" | 18, 23 |
| Autofix: review feedback on an agent's pull request becomes a prompt in the session that opened it, with a cap per pull request per day | `packages/control-plane/src/autofix/handler.ts`, `packages/control-plane/src/autofix/service.ts` | 17 |
| Child sessions in their own sandboxes: spawn, follow up, check status, cancel | `packages/sandbox-runtime/src/sandbox_runtime/tools/spawn-child.js`, `send-child-prompt.js`, `cancel-child.js` | 14 |
| Untrusted GitHub fields wrapped in review and comment prompts | `docs/integrations/GITHUB.md` "Prompt Safety" | 6 |
| Sandbox providers: Modal, Daytona, Vercel, E2B, Boat | `packages/*-infra/`, `docs/HOW_IT_WORKS.md` "Data Plane" | 20 |

### Sessions live in the control plane

A session is the unit of work. Each one gets a Durable Object with its own
SQLite database holding messages, events, artifacts, participants and a
reference to the sandbox and its snapshot (`docs/HOW_IT_WORKS.md`, "What's
Stored in a Session"). The control plane runs no code; it keeps state, routes
messages and starts, snapshots and restores sandboxes. Because state lives
there, a prompt sent from Slack shows up on the web, and several people can
watch and prompt the same session.

Prompts enter a durable queue in the session's object and run one at a time.
A prompt sent while the agent works waits; the person can also stop the
running turn (`docs/HOW_IT_WORKS.md`, "Prompt Queuing").

### The sandbox and its boot

`packages/sandbox-runtime/src/sandbox_runtime/bridge.py` is the process that
connects a sandbox to its session. It opens a WebSocket to the session's
Durable Object, sends a heartbeat, forwards harness events, handles prompt,
stop and snapshot commands, and sets the git identity for each prompt's
author. It never speaks a vendor protocol; the agent sits behind the harness
seam in `harness/`.

A fresh boot reports phases as it goes: `starting`, `sync` (clone with
brokered credentials), `setup` (`.openinspect/setup.sh`), `start`
(`.openinspect/start.sh`), `skills`, `harness`, and then `ready`. A prompt
sent during boot waits for `ready`. Three bounds apply: a 4-minute watchdog
from sandbox creation to the bridge's first connect, a boot budget of 30
minutes by default from creation to `ready`, and a 90-second heartbeat
timeout during boot. A boot that hits any of them is failed, its credentials
are revoked, and no snapshot is taken, "so a half-provisioned workspace never
becomes the restore point" (`docs/HOW_IT_WORKS.md`, "How long a boot may
take").

Snapshots are taken after each completed prompt and before an idle shutdown.
A restore fetches the session's branch and leaves the checkout as it was, so
uncommitted work survives (`docs/HOW_IT_WORKS.md`, "Restore").

### Three harnesses behind one seam

The Codex harness runs `codex app-server` and speaks JSON-RPC over standard
input and output. It always runs the image's pinned binary, and the process
does not inherit the sandbox's `OPENAI_*` or `CODEX_*` variables; the bridge
hands it one credential fetched from the control plane. The Claude Agent
harness starts the `claude` binary with a clean environment carrying exactly
one Anthropic credential. All three harnesses emit the same session events,
and the bridge emits the single `execution_complete` event that ends a turn
(`docs/HOW_IT_WORKS.md`, "The Agent").

### Branches, pull requests and review feedback

The default head branch is `open-inspect/{session-id}`
(`packages/shared/src/git.ts`); a create-PR request may name another branch,
which is sanitized first (`packages/control-plane/src/source-control/branch-resolution.ts`).
Commits carry the prompting person as author and the bot as committer. The
agent pushes with brokered credentials, and the control plane opens the pull
request with the person's GitHub OAuth token, so it appears as theirs and
they cannot approve it themselves. Unattended automation runs open pull
requests as the GitHub App instead, so the owner can review them
(`docs/HOW_IT_WORKS.md`, "Creating Pull Requests").

Autofix closes the loop. When a reviewer, a pull-request comment or an
allowed review bot leaves feedback on an agent's pull request, the feedback
becomes a prompt in the session that opened it, under settings that include
`maxAttemptsPerPrPer24Hours` (`packages/control-plane/src/autofix/handler.ts`).
Each piece of feedback gets a receipt: received, queued, skipped or failed
(`packages/control-plane/src/autofix/service.ts`). An Autofix turn is not the
owner's prompt, so it cannot read or save the owner's personal memories
(`docs/integrations/GITHUB.md`).

### Children

An agent can start a child session in its own sandbox with `spawn-child`,
send it follow-ups with `send-child-prompt`, read its state with
`get-child-status` and stop it with `cancel-child`. A follow-up enters the
child's normal queue and does not interrupt it. The parent's token is never
exchanged for the child's sandbox token; the control plane checks the
parent-child link in its database and again in the child's Durable Object
(`docs/HOW_IT_WORKS.md`, "Parent-to-Child Follow-Ups").

## What Coder takes

**The session's state outside the sandbox.** Open-Inspect's split matches
Coder chapter 20's "compute, state and environment apart": the sandbox can
die, the session cannot. In Coder the session is chapter 2's log, and the
sandbox is replaceable environment.

**Readiness from the runtime, not the socket.** The bridge reports each boot
phase and sends `ready` only after the harness attaches. Chapter 20's
per-machine daemon reports the same phases, so a long boot and a dead one look
different to the watchdog.

**No restore point from a failed boot.** A snapshot taken only after a
completed prompt, never after a failed or timed-out boot, keeps a broken
workspace from becoming the base for the next run. Chapter 14's background
jobs and chapter 20's restarts follow the same rule.

**Credential handles, one per process.** Each harness process gets exactly
one credential, does not inherit the sandbox's provider variables, and runs a
pinned binary. Chapter 20's credential handles do the same for Claude Code
and Codex, which Coder already drives through `coax/lib/agent/claude/` and
`coax/lib/agent/codex/`. Open-Inspect's README also says brokered credentials
are cached on disk and can end up in snapshots (`README.md`, "How It Works");
Coder treats that as a defect to avoid, not a property to copy.

**A branch per task and a pull request in the person's name.** One branch per
session, named from the session ID, keeps concurrent runs apart and makes the
branch findable from the session and the reverse. Opening the pull request
with the person's own token keeps review honest: the person who asked cannot
approve their own agent's change. Automatic branch/PR delivery from
`coder run` is a proposed design here; chapter 23's complete repository
journey was still open at this survey's revision. The offline journey now
captures the reviewed diff locally, without claiming live PR delivery.

**Review comments as a Thread.** Autofix is chapter 17's Thread subscription
for one source: the pull request is the Thread, and the Worker that opened it
receives its follow-ups. Coder takes the receipt per event (received, queued,
skipped, failed), so every review comment has a visible fate.

**One seam for all engines.** The bridge owns turn completion and every
harness emits the same events. Coder's terminal program already has three
engines behind one composer (`coder/tui/coder.coax`); chapter 13's engine
choice depends on the engines reporting the same endings.

## What Coder leaves, and why

**Keyword routing rules.** A Slack routing rule can launch an environment
from a keyword, and an LLM classifier picks repositories from free text
(`docs/HOW_IT_WORKS.md`, "Session Targets"). Coder routes by meaning through
a typed System One decision (chapters 13 and 17); code checks only bounded
fields such as repository IDs and allowlists.

**Single-tenant trust.** Open-Inspect is built for one organization whose
members all trust each other (`README.md`, "Security Model"). Coder runs for
many people on shared infrastructure. Access checks and consent therefore
need explicit scope; chapter 18 does not yet gate every byte leaving a
machine. The [shared contract](CONTRACT.md) lists current enforcement and
remaining outbound paths.

**Rented sandbox vendors.** Open-Inspect runs on Modal, Daytona, Vercel, E2B
or Boat. Coder runs on the person's own machine or on OpenAgents' own cloud
machines behind its daemon (chapter 20), so the boot phases and watchdog
carry over and the provider adapters do not.

**A queue that runs prompts strictly one at a time.** Open-Inspect queues
follow-ups behind the running turn. The Super-agent design that chapter 17
follows batches events queued during a step into the next request. The two
differ in latency and cost, and chapter 21 decides between them.

**Boot budgets as built-in defaults.** Open-Inspect fails a boot after 30
minutes by default. Coder adds no time limit of its own to a task; limits are
the person's to set. A watchdog that detects a dead daemon by missing
heartbeats is a different thing, and Coder keeps that.

## Where to measure a borrowed design

- Boot phases, heartbeats and no-snapshot-on-failure: chapter 20's watchdog
  checks, with a daemon killed at each phase.
- Strict queueing against batching of follow-ups: chapter 21, on the same
  recorded streams of follow-up events.
- Autofix as Thread subscription: chapter 17 checks that every review comment
  on a Coder pull request reaches its Worker with a receipt; chapter 21
  measures whether the follow-up turns land fixes that pass review.
- Branch per task and attribution: chapter 23's end-to-end suite opens a pull
  request from `coder run` and checks the branch name, the commit author and
  the pull request's author.

## Sources

- [README.md](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/README.md)
- [docs/HOW_IT_WORKS.md](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/docs/HOW_IT_WORKS.md)
- [docs/integrations/GITHUB.md](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/docs/integrations/GITHUB.md)
- [docs/IMAGE_PREBUILD.md](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/docs/IMAGE_PREBUILD.md)
- [packages/control-plane/src/cloudflare/durable-object.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/cloudflare/durable-object.ts)
- [packages/control-plane/src/session/message-queue.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/session/message-queue.ts)
- [packages/control-plane/src/session/event-stream.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/session/event-stream.ts)
- [packages/control-plane/src/session/pull-request-service.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/session/pull-request-service.ts)
- [packages/control-plane/src/source-control/branch-resolution.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/source-control/branch-resolution.ts)
- [packages/control-plane/src/autofix/handler.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/autofix/handler.ts)
- [packages/control-plane/src/autofix/service.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/control-plane/src/autofix/service.ts)
- [packages/shared/src/git.ts](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/shared/src/git.ts)
- [packages/sandbox-runtime/src/sandbox_runtime/bridge.py](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/sandbox-runtime/src/sandbox_runtime/bridge.py)
- [packages/sandbox-runtime/src/sandbox_runtime/repository_boot.py](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/sandbox-runtime/src/sandbox_runtime/repository_boot.py)
- [packages/sandbox-runtime/src/sandbox_runtime/harness/](https://github.com/ColeMurray/background-agents/tree/80426f42e72abf3f76f71d00045b0e8b77224224/packages/sandbox-runtime/src/sandbox_runtime/harness)
- [packages/sandbox-runtime/src/sandbox_runtime/tools/spawn-child.js](https://github.com/ColeMurray/background-agents/blob/80426f42e72abf3f76f71d00045b0e8b77224224/packages/sandbox-runtime/src/sandbox_runtime/tools/spawn-child.js)
- Coder's side: [coder/tui/coder.coax](../tui/coder.coax), [coax/lib/agent/codex/](../../coax/lib/agent/codex/), [coax/lib/agent/claude/](../../coax/lib/agent/claude/).
- [Chapter plan](PLAN.md), Part VII; [DeepSeek Harness notes](../../psionic/docs/harness.md) for the Super-agent modules referred to above.
