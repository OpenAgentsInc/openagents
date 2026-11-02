# 32. Hermes Agent

> **Status: written.** Read at NousResearch/hermes-agent@4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8 on 2026-11-02.

Source: [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent/tree/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8)
at `4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8` (MIT, last commit 2026-11-01). A
shallow clone was read under `projects/repos/hermes-agent`; nothing was
installed or run. The workspace also tracks a one-covenant fork; this chapter
reads the Nous Research upstream. Every path below is relative to the
repository root at that revision.

## Why study it

Hermes Agent is a personal agent, not a coding agent. One agent core serves a
command line, a terminal UI, a desktop app and a messaging gateway for about
twenty chat platforms (`AGENTS.md`, "What Hermes Is"). Three things make it
worth reading for Coder:

- It states the same prefix rule as Coder chapter 3, as a project invariant:
  "Per-conversation prompt caching is sacred," and the one exception is
  context compression (`AGENTS.md`).
- It has a learning loop: memory files, skills the agent writes itself, a
  background review that decides what to keep, and search over past sessions.
  Coder chapter 19 builds memory with a stricter contract, so Hermes shows
  what the looser version looks like at scale.
- Nous Research trains models, and the repository keeps the training side
  next to the agent: a batch runner, toolset distributions, a trajectory
  format and a trajectory compressor. That is the path from a harness to a
  dataset that Coder chapters 18, 21 and 22 must also answer.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| One turn: build or reuse the system prompt, preflight compression, call the model, run tools, loop until a text reply | `agent/conversation_loop.py`, `agent/turn_*.py`, `website/docs/developer-guide/agent-loop.md` | 1 |
| Prompt caching as an invariant; prompt-changing commands wait for the next session unless given `--now` | `AGENTS.md` | 3 |
| Memory files (`MEMORY.md`, `USER.md`) that enter the system prompt as a frozen snapshot at session start | `tools/memory_tool.py` | 3, 19 |
| Skills as procedural memory the agent creates and edits (`SKILL.md` plus references, templates and scripts) | `tools/skill_manager_tool.py`, `tools/skills_tool.py` | 15, 19 |
| A background review: after a turn, a forked agent replays the conversation and decides whether to save memory or skills | `agent/background_review.py` | 14, 19 |
| Session search over SQLite FTS5, with no model calls; returns stored messages | `tools/session_search_tool.py`, `hermes_state.py` | 17, 19 |
| Tool registry; single calls run inline, several run on a thread pool and return in call order | `tools/registry.py`, `model_tools.py`, `website/docs/developer-guide/agent-loop.md` | 4 |
| Dangerous-command approval: pattern tables, an allowlist, a human prompt and an optional guardian model | `tools/approval.py`, `tools/approval_detection.py`, `tools/approval_smart.py` | 5 |
| Terminal backends: local, Docker, SSH, Modal, Daytona, Vercel Sandbox | `tools/environments/` | 5, 20 |
| Context compression with a model-visible marker built so the model cannot copy it into new calls | `agent/compression_marker.py`, `agent/context_compressor.py` | 7 |
| Programmatic tool calling: a Python script calls Hermes tools over RPC; only its stdout returns to the model | `tools/code_execution_tool.py`, `tools/code_kernel.py` | 8 |
| Subagents with a fresh conversation and the parent's toolsets minus blocked tools; the parent sees only the summary | `tools/delegate_tool.py` | 14 |
| An Agent Client Protocol (ACP) server | `acp_adapter/server.py` | 16 |
| Batch runs over a JSONL prompt set with checkpoints and per-tool statistics | `batch_runner.py` | 21 |
| Toolset distributions: each toolset enabled per prompt with a set probability | `toolset_distributions.py` | 21, 22 |
| Trajectories in ShareGPT-style JSONL, split into completed and failed files | `agent/trajectory.py`, `website/docs/developer-guide/trajectory-format.md` | 18, 21 |
| A trajectory compressor that fits a trajectory into a token budget | `trajectory_compressor.py` | 7, 21 |
| A SWE-style runner that writes the same trajectory format | `mini_swe_runner.py` | 21 |

### The loop and the prefix

`run_agent.py` is a thin facade over `agent/conversation_loop.py`, which
drives one user turn: the model call, tool dispatch, retries, fallbacks,
compression and post-turn hooks. The turn phases live in about thirty
`agent/turn_*.py` modules. The loop keeps history in the OpenAI message
format and converts it for Anthropic Messages or the Responses API at request
time (`website/docs/developer-guide/agent-loop.md`, "API Modes").

The prefix rule shapes the rest of the design. `AGENTS.md` says that
mutating past context, swapping toolsets, reloading memories or rebuilding
the system prompt mid-conversation breaks the cached prefix, so slash
commands that change system-prompt state defer to the next session. The
memory tool follows the rule: `MEMORY.md` and `USER.md` enter the prompt as a
frozen snapshot when a session starts, and writes during the session reach
disk but not the prompt (`tools/memory_tool.py`, module docstring). The same
file says every core tool is sent on every call, so the bar for a new core
tool is high, and new capability should arrive as a command plus a skill, a
gated tool or a plugin.

### The learning loop

Skills are directories under `~/.hermes/skills/` with a `SKILL.md` and
optional `references/`, `templates/`, `scripts/` and `assets/`; the agent
creates new ones and edits existing ones in place
(`tools/skill_manager_tool.py`). The background review decides when that
happens. After a turn, `run_conversation` may start a thread that replays a
snapshot of the conversation in a forked agent and asks whether any skill or
memory should be saved or updated. The fork inherits the parent's provider,
model and cached system prompt, so it hits the same prefix cache, and it runs
under a tool allowlist. Its writes go straight to the stores; the main
conversation is not touched (`agent/background_review.py`, module
docstring).

Recall uses the session database. `hermes_state.py` stores sessions and
messages in SQLite in WAL mode with an FTS5 index, tags each session with its
source, and splits sessions at compression through `parent_session_id`
chains. The session search tool has four shapes (discovery by query, scroll
around a message, read a session, browse), makes no model calls and returns
stored messages. It hides subagent and tool sessions and ranks scheduled
cron sessions below interactive ones, because cron vocabulary dominated plain
BM25 ranking (`tools/session_search_tool.py`).

### Tools, approvals and programs

`tools/approval.py` owns the approval gate: per-session approval state, a
"yolo" mode, gateway queues and a denial breaker. Command classification is a
separate module of normalization, tokenizing and pattern tables
(`tools/approval_detection.py`), and an optional guardian model can judge a
command (`tools/approval_smart.py`). Commands run through one of the terminal
backends in `tools/environments/` (`local.py`, `docker.py`, `ssh.py`,
`modal.py`, `daytona.py`, `vercel_sandbox.py`).

Programmatic tool calling lets the model write a Python script that calls
Hermes tools over RPC. On the local backend the script runs in a persistent
per-conversation kernel behind a Unix socket; only the script's stdout goes
back to the model (`tools/code_execution_tool.py`, module docstring).

Subagents get a fresh conversation, their own task ID, the parent's toolsets
minus child-blocked tools and a focused system prompt. The parent sees the
delegation call and the summary, never the child's tool calls
(`tools/delegate_tool.py`).

### The training side

At this revision the repository holds no reinforcement-learning environments.
The learning-path page sends readers to the external Atropos repository for
environments (`website/docs/getting-started/learning-path.md`, "I want to
train models"), and the Dockerfile's comments describe an `[rl]` extra that
pulls `atroposlib` and `tinker` from Git (`Dockerfile`, the build-env
comment). What remains in the tree is data generation:

- `batch_runner.py` runs the agent over a JSONL prompt set on a process pool,
  writes `batch_N.jsonl` files, checkpoints for `--resume`, and aggregates
  per-tool success and failure counts. It derives the set of valid tool names
  from `model_tools.TOOL_TO_TOOLSET_MAP` and uses it to drop entries with
  hallucinated tool names.
- `toolset_distributions.py` maps toolset names to the percentage chance each
  is enabled for a prompt, sampled independently; `terminal_tasks` enables
  terminal and file tools 97% of the time.
- Trajectories go to `trajectory_samples.jsonl` when completed and
  `failed_trajectories.jsonl` otherwise; batch entries add `api_calls`,
  `toolsets_used`, `tool_stats` and `tool_error_counts`
  (`website/docs/developer-guide/trajectory-format.md`).
- `trajectory_compressor.py` protects the head and the last N turns and
  replaces only as many middle turns as needed with one summary turn, never
  splitting a tool call from its response.
- `mini_swe_runner.py` runs tasks in the local, Docker or Modal environments
  and writes the same format.

## What Coder takes

**Freeze what the prompt holds at session start.** The frozen memory
snapshot is the simplest way to keep memory from moving the prefix: a write
lands in the store at once and in the prompt at the next session. Coder
chapter 19 already logs every memory write as an event; chapter 3's prompt
assembly reads memory once per session and treats later writes as data the
model reaches through a tool. Hermes's `--now` escape hatch becomes an
explicit, logged prefix break in the same shape as chapter 7's condensation.

**Review after the turn, in a fork that shares the prefix.** Running the
memory decision in a fork after the reply keeps it off the person's critical
path, and inheriting the cached system prompt keeps it cheap. Coder chapter 14
forks at a turn boundary from the log, so a review fork is a fork with a
narrow tool grant. The difference Coder keeps: the fork writes chapter 19
events (remember, revise, forget) with provenance, not free edits to a file.

**Recall without a model.** Session search returns stored messages and makes
no model calls, and ranks automated sessions below interactive ones. Coder's
chain of recall in chapter 17 reads the log the same way: objects by ID,
neighbours by pointer, no summarizer on the read path.

**Tool statistics per run, keyed to the registry.** Deriving the valid tool
set from the registry and counting success and failure per tool is cheap and
catches a model that names tools that do not exist. Chapter 21's harness
benchmarks record the same counts per run.

**Toolset distributions as an experiment variable.** Sampling which toolsets
a run gets is a direct way to measure what each tool adds. Chapter 21 can use
the same shape to compare runs with and without a tool on held-out tasks.

## What Coder leaves, and why

**Channels in the agent core.** Hermes runs about twenty messaging platforms
from one gateway process. In OpenAgents the Router owns channels, identity and
every reply in shared channels; Coder is a Worker and talks to people only
through it ([plan](PLAN.md#openagents-and-coder)).

**Skills the agent writes into its own prompt, unmeasured.** Self-written
skills change what later sessions see. Coder adds nothing to the prompt
without a measured gain in chapter 21 on tasks it was not built from. A skill
Coder learns is a chapter 19 fact with a source until a measurement promotes
it.

**Pattern tables as the approval gate.** `tools/approval_detection.py`
classifies commands by normalized patterns, and a guardian model can decide
in their place. Coder chapter 5 keeps the decision in a policy that is data:
a ceiling, the effects that run without asking, and typed answers recorded in
the log. Patterns can still label a command for display, but they do not
decide. A model judgment, if one is added, goes through a typed System One
decision (chapter 16), not a free-form guardian prompt.

**A Python kernel for programs.** Hermes runs model-written Python with full
language power and returns its stdout. Coder chapter 8 checks a program
against the grant and its worst-case cost before anything runs, and stops at
the exact statement that asks or fails. Coder keeps that check.

**Trajectory export by default.** Hermes writes trajectories to the working
directory as a matter of course. Coder sends no session content anywhere
without chapter 18's per-session, revocable opt-in, and its log already
rebuilds every request, so an export is a view of the log, not a second
format.

## Where a borrowed design is measured

- The frozen snapshot: chapter 3 measures prefix stability across a session
  with memory writes; the server-side cache numbers stay in Psionic.
- The review fork: chapter 19 checks that reviewed facts carry provenance and
  that revised facts never return in their old form; chapter 21 measures
  whether review-written facts improve held-out tasks.
- Tool statistics and toolset sampling: chapter 21.
- Trajectory compression against chapter 7's condensation: chapter 21
  compares the token cost and task outcome of both on the same logs.

## Sources

- [AGENTS.md](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/AGENTS.md)
- [agent/conversation_loop.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/agent/conversation_loop.py)
- [agent/background_review.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/agent/background_review.py)
- [agent/compression_marker.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/agent/compression_marker.py)
- [agent/trajectory.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/agent/trajectory.py)
- [tools/memory_tool.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/memory_tool.py)
- [tools/skill_manager_tool.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/skill_manager_tool.py)
- [tools/session_search_tool.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/session_search_tool.py)
- [hermes_state.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/hermes_state.py)
- [tools/approval.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/approval.py)
- [tools/approval_detection.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/approval_detection.py)
- [tools/approval_smart.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/approval_smart.py)
- [tools/code_execution_tool.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/code_execution_tool.py)
- [tools/delegate_tool.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/delegate_tool.py)
- [tools/environments/](https://github.com/NousResearch/hermes-agent/tree/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/tools/environments)
- [acp_adapter/server.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/acp_adapter/server.py)
- [batch_runner.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/batch_runner.py)
- [toolset_distributions.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/toolset_distributions.py)
- [trajectory_compressor.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/trajectory_compressor.py)
- [mini_swe_runner.py](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/mini_swe_runner.py)
- [website/docs/developer-guide/agent-loop.md](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/website/docs/developer-guide/agent-loop.md)
- [website/docs/developer-guide/trajectory-format.md](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/website/docs/developer-guide/trajectory-format.md)
- [website/docs/getting-started/learning-path.md](https://github.com/NousResearch/hermes-agent/blob/4d26e0cceaed0a08ede833caec0ec6d6ca77a4b8/website/docs/getting-started/learning-path.md)
- [Chapter plan](PLAN.md), Part VII; [DeepSeek Harness notes](../../psionic/docs/harness.md) for the shape of this survey.
