# Coder: Coding Agents From Scratch

Building OpenAgents' coding agent in [Coax](../../coax/book/README.md), and
learning from every coding agent we can read.

Coder takes one repository task from start to finish: it reads the code, runs
commands, edits, tests and opens the pull request. It is one kind of Worker
inside OpenAgents, the general agent that people reach from the web, the
terminal, the desktop and mobile apps ([Episode 289](../../docs/transcripts/289.md)).
OpenAgents speaks as "we", owns identity, the wallet, the knowledge base,
plugins and routing, and writes every reply in shared channels. Coder speaks
as "I" and is opinionated about code. The [plan](PLAN.md#openagents-and-coder)
sets out the split.

The book builds the harness one measured piece at a time: a simple loop, a log
that rebuilds every request, typed tools, a sandbox, condensing, programs as
tool calls (Part I). It then adds engines: Coder's own pure-Coax runtime
(`openagents/core/` + `oa-worker`, superseding Inky and the deprecated
Microcoder loop), local and hybrid models that [Psionic](../../psionic/book/README.md)
serves, and the Claude Code, Codex, Cursor, or Antigravity CLIs a person
already uses (Part II & Part VIII). Parts III and IV turn one session into
one agent across many tasks and channels that stays online, following the
DeepSeek Harness team's Super-agent design. Part V builds measurement
machinery, a live repository pilot, gBench, Terminal-Bench 4.1, and ATIF-v1.8
session-history evals; comparative ablations of many additions remain. Part VI
collects the implemented paths into the `openagents coder` command line. Part
VII reads other coding agents' source, one agent per chapter, and records what
Coder takes from each. Part IX documents the unified native GPU desktop app
(`apps/openagents-desktop/`), its JSON-lines worker wire, and the Foreman.

The chapters target `openagents coder` verbs as their product interface.
The [plan](PLAN.md#the-cli-is-the-spine) lists those targets; chapter status
and the coverage matrix distinguish experimental components from live verbs.

Start with [guarantees and authorization](CONTRACT.md) for the distinction
between bounded experiments, live Coder behavior and engine-owned state.
For a practical path, run the [repository journey and coding pilot](../../docs/coder/repository-pilot.md),
then read chapters 1, 2, 9–11, 14 and 23. For the full architecture, follow
the numbered chapters and use chapter 21 to distinguish implementation
checks from measurements of real coding outcomes.

## Start here

- **What you build.** A coding agent's harness: the loop, the log, typed
  tools, a sandbox, engines, many tasks and an agent that stays online.
- **What you need.** The Coax book's Parts I–III. Psionic helps for the
  chapters that serve a model.
- **The core path.** Chapters 1, 2, 4 and 5 (the loop, the log, tools and
  the sandbox), 9–11 (engines), 14 (forks and jobs), 17 (many tasks), 20
  (staying online), 21 (measurement) and 23 (the command line).
- **Two books in one.** Parts I–VI, VIII and IX build Coder. Part VII is a
  field guide: it reads other coding agents' source at a pinned revision
  and records what Coder takes from each. Those chapters describe the other
  tool as it was at that revision.
- **Where this book sits.** Coder is the agent that reads the other books.
  See [the OpenAgents books](../../docs/books/README.md) for the whole library.

## Status

Written 2026-11-01; updated 2026-11-26. Of the 43 chapters, 11 are
implemented, 19 are partly implemented, 12 are written surveys and 1 is a
design draft. The
opening native profile composes the owned worker, durable journal and Linux
confinement; the finite teaching examples remain under `psionic/lib/harness/`.
The [registry](chapters.json) names each chapter’s sources and qualified targets.

| Status | Meaning |
| --- | --- |
| implemented | The chapter's examples run and its done criteria pass on a named host. |
| partly implemented | Some examples run; the chapter says what remains. |
| written | A survey chapter: its claims cite files in another project at a pinned revision. |
| design draft | The chapter says what will be built and how it will be checked. Code it names may already exist. |
| planned | A title and a scope only. |

## Chapters

| Chapter | Status |
| --- | --- |
| [1. The simplest loop](ch001-the-simplest-loop.md) | Implemented: cpu-harness-loop-v1, bounded CPU; coder-owned-v1, native Linux x86_64 |
| [2. Model-visible means logged](ch002-model-visible-means-logged.md) | Implemented: sha256-harness-log-v1, bounded CPU; coder-owned-v1, native Linux x86_64 |
| [3. Keep the prefix stable](ch003-keep-the-prefix-stable.md) | Partly implemented: px-prefix-v1, bounded CPU |
| [4. Tools as typed capabilities](ch004-tools-as-typed-capabilities.md) | Implemented: cpu-harness-tools-v1, bounded CPU; coder-owned-v1, native Linux x86_64 |
| [5. A sandbox and approvals](ch005-a-sandbox-and-approvals.md) | Implemented: cpu-harness-sandbox-v1, bounded CPU; coder-owned-v1, native Linux x86_64 |
| [6. A document tries to become the system prompt](ch006-a-document-tries-to-become-the-system-prompt.md) | Partly implemented: ij-inject-v1, bounded CPU, deterministic stand-in agent |
| [7. Condense the history](ch007-condense-the-history.md) | Partly implemented: cd-condense-v1, bounded CPU |
| [8. Programs as tool calls](ch008-programs-as-tool-calls.md) | Partly implemented: cpu-harness-program-v1, bounded CPU |
| [9. Microcoder](ch009-microcoder.md) | Implemented: deprecated `mcworker` (Vertex, Gemini API, OpenRouter; recorded streams replay); superseded by `coax/lib/inky/` and unified `openagents/core/` (`oa-worker`) |
| [10. Drive Claude Code](ch010-drive-claude-code.md) | Implemented: Claude Code 2.1.316 on the person's login; approvals by policy; Coder's tools in-process |
| [11. Drive Codex](ch011-drive-codex.md) | Partly implemented: codex-cli 0.163.0 app-server |
| [12. A Psionic-served model as an engine](ch012-a-psionic-served-model.md) | Partly implemented: `psionic-inkling-serve` local Metal 4 / CUDA serving (32K KV, 21-quant/LoRA, hybrid-exec-v1, $0 local tier) plus `--engine psionic` |
| [13. Choose an engine](ch013-choose-an-engine.md) | Partly implemented: `openagents.router.v2alpha1` `RoutePlan` + `coax/lib/agent/router/` (66/66 dev, 30/31 held-out) with opt-in external delegation gating |
| [14. Forks, subagents and jobs](ch014-forks-subagents-and-jobs.md) | Partly implemented: forks, jobs with deadlines and reports, CC-parity subagents (`oa-subagent.coax`, `openagents/core/agents.coax`, parallel/bg/fork, worktrees), `/goal` parity, and `tasks.coax` |
| [15. Plugins with owned lifecycles](ch015-plugins-with-owned-lifecycles.md) | Implemented: profiles, capabilities, draining, signed installs, Wasm plugins with admitted imports |
| [16. Speak the protocols](ch016-speak-the-protocols.md) | Partly implemented: MCP over stdio both ways and HTTP, `coder mcp` with resources, ACP, System One as a tool |
| [17. One agent, many tasks](ch017-one-agent-many-tasks.md) | Partly implemented: a Router with local, GitHub issue and CI sources, Workers, replies, issue context inline and recall through named sessions |
| [18. Consent before sharing](ch018-consent-before-sharing.md) | Partly implemented: cs-consent-v1 hash-chained ledger; `coder consent` gates remembered facts to every engine, and Worker logs, routing and Router decisions to System One |
| [19. Remember a fact, then revise it](ch019-remember-a-fact-then-revise-it.md) | Partly implemented: mm-memory-v1 hash-chained memory events; `coder remember`/`revise`/`forget`/`recall` on disk, ranked by Vertex embeddings, checked against the session line they came from |
| [20. An agent that stays online](ch020-an-agent-that-stays-online.md) | Partly implemented: a resumable loop, a watchdog with provider rechecks, machine daemons, credential handles, a read ceiling for unnamed authors (checked live against chapter 6's attacks), channel rules and mail replies as Gmail drafts |
| [21. Measure the harness](ch021-measure-the-harness.md) | Partly implemented: cpu-harness-bench-v1, coder-repository-pilot-v1, gBench, Terminal-Bench 4.1, ATIF-v1.8 session-history friction scorecard, and Gym A/B subagent bench |
| [22. Generate candidates, then check them](ch022-generate-candidates-then-check-them.md) | Partly implemented: cpu-int-candidates-v1, bounded CPU |
| [23. `openagents coder`](ch023-openagents-coder.md) | Partly implemented: every verb, `--on ssh:DEST`, `coder daemon`, offline bench journey; `--on cloud` not live |
| [24. DeepSeek Harness](ch024-deepseek-harness.md) | Written: deepseek-ai/deepseek-harness@ce7073d |
| [25. Claude Code](ch025-claude-code.md) | Written: AtlantisPleb/cc@abcc009 |
| [26. Codex](ch026-codex.md) | Written: openai/codex@a098129 |
| [27. OpenCode](ch027-opencode.md) | Written: anomalyco/opencode@ff90ed9 |
| [28. Goose](ch028-goose.md) | Written: block/goose@0909288 |
| [29. Aider](ch029-aider.md) | Written: Aider-AI/aider@5dc9490 |
| [30. mini-SWE-agent](ch030-mini-swe-agent.md) | Written: SWE-agent/mini-swe-agent@04d809c |
| [31. OpenHands Software Agent SDK](ch031-openhands-sdk.md) | Written: OpenHands/software-agent-sdk@9884945 |
| [32. Hermes Agent](ch032-hermes-agent.md) | Written: NousResearch/hermes-agent@4d26e0c |
| [33. Toad](ch033-toad.md) | Written: batrachianai/toad@dd4f90e |
| [34. Background Agents](ch034-background-agents.md) | Written: ColeMurray/background-agents@80426f4 |
| [35. Flue and Agent OS](ch035-flue-and-agent-os.md) | Written: withastro/flue@deee93b, rivet-dev/agent-os@c746292 |
| [36. A terminal UI](ch036-a-terminal-ui.md) | Implemented: three agents inline, themes, job status, macOS terminal |
| [37. A terminal inside the agent](ch037-a-terminal-inside-the-agent.md) | Partly implemented: vt-text-v1, bounded CPU |
| [38. Drive Cursor](ch038-drive-cursor.md) | Partly implemented: cursor-agent 2026.10.14-941492e ACP |
| [39. Drive Antigravity](ch039-drive-antigravity.md) | Implemented: `agy` ACP + `coder/tui/agyworker.coax` with T3 quirk fixes, prompt-cache expiry/idle compaction, and opt-in subagent delegation |
| [40. The desktop app](ch040-the-desktop-app.md) | Implemented: unified `apps/openagents-desktop/` (`OpenAgents.app`, `--minimal`/`--full`, macOS Metal + Linux/Windows Vulkan, modular `page/*.coax`) |
| [41. Worker protocol and sub-agents](ch041-worker-protocol-and-sub-agents.md) | Implemented: `mcworker-wire.coax` JSON-lines protocol driving `oa-worker`, `oa-subagent`, `inky`, `agyworker`, `claudeworker`, and legacy `mcworker` |
| [42. Beyond the desktop app](ch042-beyond-the-desktop-app.md) | Unimplemented: design draft |
| [43. The Foreman](ch043-the-foreman.md) | Partly implemented: Foreman supervisory chat in `apps/openagents-desktop/` (`Foreman.swift`, `Window+Foreman.swift`, `Window+ForemanAPI.swift`), feature-flagged under `foreman` |

## Contents

See [SUMMARY.md](SUMMARY.md) for the chapter list and [PLAN.md](PLAN.md) for
what each chapter builds and checks.

The [2026-11-03 review and roadmap](../../docs/coder/2026-11-03-book-review-and-roadmap.md)
assesses the book's guarantees, live integration and measurements, with
prioritized proposed work items and acceptance criteria.

The [structural follow-up suite](../../docs/coder/repository-pilot.md#structural-follow-up-suite)
adds provider availability checks, observed Codex metadata and interruptions
after real effects. It retains the v1 pilot as historical evidence.
