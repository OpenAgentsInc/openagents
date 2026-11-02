# A harness for Psionic models: notes from DeepSeek Harness

Read-only survey, 2026-10-29, of
[deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/f3803c96974ad4a94195fb97e5c637e9c2a3afe3)
at `f3803c96974ad4a94195fb97e5c637e9c2a3afe3` (MIT, developer preview, built on
the [Cordis](https://github.com/cordiverse/cordis) plugin framework). Its
source was read through the GitHub API; nothing was installed or run. A
pre-reset survey of an earlier revision is at
`git show 0b401561a0^:docs/coder-new/deepseek-harness-lessons.md`. Part XXI of
the [book plan](../book/PLAN.md) takes its scope from here.

## Why study this harness

DeepSeek publishes the whole stack: its models, its serving engine and article
([DeepSeek natively](targets/deepseek.md)), and this harness. The harness is
built for that stack, and the clearest place this shows is the model-visible
prefix. DeepSeek's serving reports a 96% input cache hit rate on 77K-token
average inputs, and the harness is designed to keep those hits: every harness
addition to a request (identity headers, plugin inventory, session metadata)
travels outside `messages`, system prompts and tool schemas, so it "does not
add model-input tokens or alter the model-visible prefix"
(`docs/deepseek-llm-api-wire-extensions.md`). Psionic owns both sides, so it
can design the harness's prompt assembly and the server's prefix cache
(chapter 176) together and measure the result (chapter 202).

## What it does, and what Psionic takes

| DeepSeek Harness | Where | Psionic chapter |
| --- | --- | --- |
| A turn is zero or more steps; a step is one model request plus its tools; exact durable `turn/*`, `step/*` events | `docs/architecture.md` "Turn flow" | 200 |
| "Model-visible means logged": every request reconstructable from an append-only session log; history derived from the log; versioned formats with readers for old ones | `core/session`, `docs/architecture.md` "Session log" | 201 |
| Extensions outside the model input so the prefix stays byte-stable | `docs/deepseek-llm-api-wire-extensions.md` | 202 |
| A scoped tool registry and guarded pipeline (`tools/pre-execute`, `execute`, `post-execute`); filesystem and process providers sharing one execution world | `core/tools`, "Capability seams" | 203 |
| Sandbox backends and policy; timeout policy; a reminder against repeating the same tool call | `packages/sandbox/`, `packages/guard/` | 204 |
| Compaction bracketed by `compaction/start` and `compaction/end` in the log, with a stable summary marker; tool-result pruning; overflow recovery | `packages/compaction/` | 206 |
| Programmatic tool calling: a model-written program run once against host-provided bindings, with typed failure kinds | `packages/ptc-runtime/` | 207 |
| Forks at a turn boundary from a seeded log; subagents; background jobs; goals | `packages/subagent/`, `jobs/`, `goal/` | 208 |
| Everything is a plugin: profiles of ordered bundles, service definition/provider/consumer, registrations owned and unwound by their plugin | `docs/architecture.md`, `packages/boot/` | 209 |
| MCP client and resources; ACP for editors | `packages/mcp/`, `packages/acp/` | 210 |
| Benchmarks for session open, conversation fold, long browser sessions, stream reconnect and agent continuation | `benchmarks/` | 211 |

## One agent across many tasks

Added 2026-11-01 from the DeepSeek Harness team's article "Exploring
Super-agent Design: Towards a More Autonomous Agent System" (Yujia Qiao et
al.), an architecture essay rather than a description of shipped harness
features. Parts of it are implemented in the `dsh-superagent-prototype`
plugin. The asynchronous `start`/`stop` task mechanism is in the harness but
not on by default.

The argument: a single task is limited by the model, but many tasks are
limited by the person, who can follow three to five sessions. So the harness,
not the person, should manage sessions. The article names four properties
(continuous, productive, proactive, persistent) and five modules that provide
them:

| Super-agent module | What it is | Psionic chapter |
| --- | --- | --- |
| Two levels | A long-lived Router that handles all human interaction and dispatch, with no shell tool, often on a lighter model; isolated Workers with full tools that never talk to people and report to the Router | 251 |
| Event-driven Triggers | Semi-structured events (chat, mail, calendar, review comments, CI) drive the agent instead of typed prompts | 251 |
| Two-stage filtering | A code fast path on source, group and allowlists, then a model slow path with policies written as prompts | 251 (slow path through System One, chapter 210) |
| Threads, subscription and pass-through | Workers subscribe to Threads (a pull request, chat topic or mail chain); follow-ups go straight to subscribers in code; many-to-many, so Workers on a shared Thread see each other's events | 251 |
| Router batching | Triggers queued during a step enter the next request together; long input and short output make the time per decision fall as the queue grows | 251 |
| Inline and spill; chain of recall | The context window as a cache: inline by locality, spill behind pointers; every object inlines its neighbours' IDs, so old context comes back hop by hop | 251 |
| Acting ahead | Speculative background work, such as "silently reproduce and fix", delivered only when confirmed | 251 |
| Full duplex | Long tools become `start`/`stop`; results return as events, so the session keeps answering | 252 (on chapter 208's jobs) |
| Multi-channel rules | Chat: short, only decisions and sign-offs, at most 100 words a reply; pull requests and mail: structured and complete | 252 |
| Compute, state and environment apart | Stateless loop compute, durable session state, and stateless daemons on each execution machine, including the person's own (BYOM) and ones that are only sometimes online | 252 |
| Watchdog | A scheduled session outside the main system that fixes infrastructure faults and judges whether each Worker progresses or drifts | 252 |

Its preliminary results, on synthetic workloads and marked indicative: with
about 300 messages and 20 work requests, Router and Workers finished tasks at
12.4 and 15.7 minutes (P50, P95) against 20.4 and 29.5 for one session with a
subagent tool and a neutral prompt (which never delegated) and 13.8 and 29.0
with a delegation-first prompt. Workers cost more cache-read tokens (23.2M
against 6.1M and 10.8M). Without pass-through, completion times held but
Router decisions rose about 30% and tokens about 37%. A Router stress test from
1 to 40 decision-needing Triggers a minute raised mean batch size from 1.0 to
2.8 and time per request from 2.5 s to 3.9 s, cutting amortized time per
decision from 6.4 s to 1.6 s. Chapter 251 reruns both on Psionic.

Its open problems are Psionic's too: when to shard the Router, when Workers
should talk directly, memory over months, how tasks appear once a
conversation is not one session, agent-to-agent work across context and
permission boundaries, and security. A cloud loop that takes in untrusted
events and reaches machines holding code and credentials can, if compromised,
take all of them, and credentials that enter model context can leak in normal
use. Chapter 252 answers with per-daemon grants and credential handles the
model never sees, and chapter 213's consent rule still applies.

## What Psionic does differently

- **The loop stays simple.** The harness exposes a great many extension
  points. Psionic starts from one loop (chapter 200) and keeps each addition
  only with a measured gain on tasks it was not built from, the rule that
  already governs Coder's agent.
- **Coax, not Node.** DeepSeek Harness is TypeScript on Node, and ordinary host
  plugins run with the host process's authority; the earlier survey noted that
  Cordis's scoped contexts are lifecycle composition, not containment.
  Psionic's harness logic is Coax; third-party plugins run in bounded Wasm with
  admitted capabilities, distributed as signed Nostr events (chapter 209).
- **Consent before sharing.** `dsh_session_log` is enabled by default and sends
  a contiguous suffix of the canonical session log with each model request to
  DeepSeek's API. Psionic sends no session content anywhere without the
  person's explicit, per-session, revocable opt-in (chapter 213).
- **Typed protocols from `proto/`.** Tool schemas, the session log's event
  types and any wire extensions are versioned contracts in `proto/`, not ad hoc
  JSON (chapters 201 and 203).
- **Bitcoin-only money.** Anything that meters or prices agent work uses the
  Spark wallet outside Psionic.

## Licensing

MIT. Porting designs needs no notice; copying code or text would carry the MIT
notice. Its third-party dependencies are listed in its
`THIRD_PARTY_NOTICES.md`; Psionic adopts none of them.
