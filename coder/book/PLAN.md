# Proposed table of contents

Proposal, written 2026-11-01. This book builds Coder, OpenAgents' coding
agent, in Coax, and studies the coding agents other teams publish. It starts
from the harness chapters that began as Part XXI of
[Psionic: ML From Scratch](../../psionic/book/README.md).

## Where this book came from

Part XXI of the Psionic book ("A harness for Psionic models", chapters
200–214, 251 and 252) built an agent harness on Psionic's API, from the
[DeepSeek Harness notes](../../psionic/docs/harness.md). Most of that part is
not machine learning: session logs, tools, sandboxes, subagents, plugins,
protocols, routing many tasks and staying online. Coder also drives harnesses
that Psionic does not serve, such as Claude Code and Codex. So the harness
moves here, and Psionic keeps the parts where model and harness are designed
together (see [What stays in Psionic](#what-stays-in-psionic)).

The chapters moved here on 2026-11-01, renumbered: Psionic 200–207 are
chapters 1–8, 208–210 are 14–16, 251 is 17, 213 and 214 are 18 and 19, 252 is
20, and 211 and 212 are 21 and 22. Their code still lives in
`psionic/lib/harness/` with its examples and fixtures under `psionic/`, and
each chapter's registry entry names those paths; the code moves in a later
commit, checked with the compiler.

## OpenAgents and Coder

[Episode 289](../../docs/transcripts/289.md) describes OpenAgents as one
composable general agent: the same agent behind the web, terminal, desktop and
mobile apps, routing each request to its knowledge base, Coder, the wallet,
decks or plugins. The Super-agent design in [chapter 17](#iii-many-tasks)
gives that a structure, and this book uses it to split the work:

| | OpenAgents | Coder |
| --- | --- | --- |
| Role | The Router layer: one agent across every channel and the network | A Worker kind: one repository task, start to finish |
| Voice | "we" | "I" |
| Owns | identity, wallet, knowledge base, plugins, payments, event filtering and routing, every reply in shared channels | the repository, the working tree, the engine choice, tests and the pull request |
| Talks to people | yes, under per-channel rules | only through OpenAgents in shared channels; directly in a terminal the person opened |
| Personality | general, the same everywhere | opinionated about code: small diffs, tests first, honest failures |

Coder picks an engine per task: Microcoder (our loop on any model we can
reach), a Psionic-served model, or the Claude Code or Codex that the person
already uses. Driving those is a feature, not a competitor to them: a person
can use the subscription they pay for, and later lend its spare capacity to
the network.

## The CLI is the spine

Every chapter ends at an `openagents coder` verb that does what the chapter
built, so the book and the [CLI](../../docs/cli/2026-11-01-openagents-cli-v1.md#the-coder-domain-planned)
grow together:

| Verb | Chapters |
| --- | --- |
| `coder run "TASK" [--engine micro\|claude\|codex\|psionic] [--on mac\|cloud]` | 1, 9–13 |
| `coder ls`, `coder status ID`, `coder log ID` | 2, 14 |
| `coder attach ID`, `coder stop ID` | 14, 20 |
| `coder replay ID` | 2, 3 |
| `coder fork ID` | 14 |
| `coder watch THREAD` (for example `github:OpenAgentsInc/openagents#123`) | 17 |
| `coder policy [list\|add\|rm]` | 17 |
| `coder daemon` (attach this machine) | 20 |
| `coder bench SUITE` | 21, 22 |

## I. The loop

| Chapter | What we build and check |
| --- | --- |
| [1. The simplest loop](ch001-the-simplest-loop.md) — partly implemented | One loop: assemble the request, stream the answer, run the tools it calls, repeat until nothing is owed, with exact turn and step boundaries, a deadline per turn and cancellation. |
| [2. Model-visible means logged](ch002-model-visible-means-logged.md) — partly implemented | An append-only, versioned session log from which every request is rebuilt byte for byte and replayed. |
| [3. Keep the prefix stable](ch003-keep-the-prefix-stable.md) — partly implemented | Prompt assembly that only appends, with metadata outside the model input. The server-side measurement stays in Psionic. |
| [4. Tools as typed capabilities](ch004-tools-as-typed-capabilities.md) — partly implemented | Tool schemas from `proto/`, a guarded before/run/after pipeline and structured errors. |
| [5. A sandbox and approvals](ch005-a-sandbox-and-approvals.md) — partly implemented | Confined processes, an approval policy, per-tool timeouts and a guard against repeated failing calls. |
| [6. A document tries to become the system prompt](ch006-a-document-tries-to-become-the-system-prompt.md) — partly implemented | A replayable prompt-injection lab with zero unauthorized effects at the gate. |
| [7. Condense the history](ch007-condense-the-history.md) — partly implemented | One marked summary replaces older history, bracketed in the log, with its prefill cost measured. |
| [8. Programs as tool calls](ch008-programs-as-tool-calls.md) — partly implemented | A model-written program run once against bound host functions; limits refuse rather than truncate. |

## II. Engines

| Chapter | What we build and check |
| --- | --- |
| [9. Microcoder](ch009-microcoder.md) — partly implemented | Our own loop: state, one structured call per step, run the commands, repeat. Gateways to any model we can reach, and the endings it reports. |
| [10. Drive Claude Code](ch010-drive-claude-code.md) — partly implemented | The `claude` CLI's stream-json protocol as a Coax state machine: queries, control frames, in-process MCP servers and approvals, checked against recorded fixtures. |
| [11. Drive Codex](ch011-drive-codex.md) — partly implemented | `codex app-server` over JSON-RPC as a pull-based client: requests, server requests, approvals and recorded sessions replayed byte for byte. |
| [12. A Psionic-served model as an engine](ch012-a-psionic-served-model.md) — partly implemented | Microcoder's loop on a model Psionic serves on this machine or a Pylon, with the prefix-cache and constrained-tool gains measured against a hosted model. |
| [13. Choose an engine](ch013-choose-an-engine.md) — partly implemented | A System One decision picks the engine, model and effort for each task from the request and the machine, with a labelled set split into tuning and held-out halves. |

## III. Many tasks

| Chapter | What we build and check |
| --- | --- |
| [14. Forks, subagents and jobs](ch014-forks-subagents-and-jobs.md) — partly implemented | Forks at a turn boundary, scoped subagents, background jobs and goals. |
| [15. Plugins with owned lifecycles](ch015-plugins-with-owned-lifecycles.md) — partly implemented | Ordered profiles, owned registrations, visible states and third-party plugins in bounded Wasm signed as Nostr events. |
| [16. Speak the protocols](ch016-speak-the-protocols.md) — partly implemented | MCP, ACP, Open Responses function tools and System One decisions as a tool. |
| [17. One agent, many tasks](ch017-one-agent-many-tasks.md) — partly implemented | OpenAgents as the Router and Coder as Workers: Triggers, two-stage filtering, Thread subscription, batching, inline and spill, chain of recall and acting ahead. |
| [18. Consent before sharing](ch018-consent-before-sharing.md) — partly implemented | No session content leaves the machine without a per-session, revocable opt-in. |
| [19. Remember a fact, then revise it](ch019-remember-a-fact-then-revise-it.md) — partly implemented | Source-linked memory whose corrected facts never return in their old form. |

## IV. Staying online

| Chapter | What we build and check |
| --- | --- |
| [20. An agent that stays online](ch020-an-agent-that-stays-online.md) — partly implemented | Full duplex, channel rules, stateless compute over the log, per-machine daemons, a watchdog and credential handles. |

## V. Measure

| Chapter | What we build and check |
| --- | --- |
| [21. Measure the harness](ch021-measure-the-harness.md) — partly implemented | Harness benchmarks and end-to-end suites, one change at a time against the simplest loop. |
| [22. Generate candidates, then check them](ch022-generate-candidates-then-check-them.md) — partly implemented | One candidate, several with selection, and generate-run-repair under one budget. |

## VI. The command line

| Chapter | What we build and check |
| --- | --- |
| [23. `openagents coder`](ch023-openagents-coder.md) — partly implemented | The `coder` domain of the CLI: every verb above in the one command table, `--json` on all of them, exit codes, and the same verbs as MCP tools so another agent can drive Coder. |

## VII. What other coding agents teach

One chapter per agent. Each reads a pinned revision's source (cloned by
`projects/sync.sh`, read only) and records what it does, what Coder takes,
what it leaves and why, in the shape of the
[DeepSeek Harness notes](../../psionic/docs/harness.md). A claim about an
agent names the file and revision it came from. Where a design is worth
testing, the chapter names the chapter in Parts I–V that measures it.

| Chapter | Agent |
| --- | --- |
| [24. DeepSeek Harness](ch024-deepseek-harness.md) — written | The harness behind Parts I–IV: the session log, wire extensions outside the prefix, plugins, and the Super-agent design. |
| [25. Claude Code](ch025-claude-code.md) — written | Tools, permissions, hooks, skills, subagents, plugins and memory files. |
| [26. Codex](ch026-codex.md) — written | The Rust agent: sandboxing, approvals, the app-server protocol and non-interactive runs. |
| [27. OpenCode](ch027-opencode.md) — written | A provider-agnostic engine with a local HTTP control plane and several clients. |
| [28. Goose](ch028-goose.md) — written | Extensions over MCP, recipes, and testing across desktop and CLI. |
| [29. Aider](ch029-aider.md) — written | Repository maps, edit formats and a benchmark culture. |
| [30. mini-SWE-agent](ch030-mini-swe-agent.md) — written | The smallest honest baseline: a linear loop and independent commands. |
| [31. OpenHands Software Agent SDK](ch031-openhands-sdk.md) — written | An agent SDK with a clean split between fast tests and evaluation. |
| [32. Hermes Agent](ch032-hermes-agent.md) — written | Nous Research's agent runtime and its training-facing environments. |
| [33. Toad](ch033-toad.md) — written | A terminal host for agents over ACP, with an embedded shell. |
| [34. Background Agents](ch034-background-agents.md) — written | Durable remote sessions, prepared worktrees, a branch per task and pull-request delivery. |
| [35. Flue and Agent OS](ch035-flue-and-agent-os.md) — written | Programmable headless agents on Workers, and an in-process agent runtime with isolation. |

## VIII. The terminal

| Chapter | What we build and check |
| --- | --- |
| [36. A terminal UI](ch036-a-terminal-ui.md) — partly implemented | Coder inline in the person's terminal, as Claude Code and Codex run: a live region for status, the streaming reply and the composer, and finished turns committed to scrollback. Raw keys, a key decoder with bracketed paste, display width, a composer with history, and a pseudo-terminal smoke test. Code: `coax/lib/tui/tui.coax` and `coder/tui/`. |
| [37. A terminal inside the agent](ch037-a-terminal-inside-the-agent.md) — partly implemented | A VT emulator in Coax so Coder can run programs that draw to a terminal and read the screen as text, checked against libghostty as an oracle and never linked to it. |

## What stays in Psionic

Psionic keeps a short part on serving a harness, because these need the
model and the server: the prefix cache measured from the server side
([176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md)),
tool arguments constrained during generation
([53](../../psionic/book/ch053-choosing-the-next-token.md)), decision models
that route work, verifier and candidate economics, and what a harness costs a
served model in tokens. Those chapters link here for the harness itself.

## Rules for every chapter

- Harness logic is Coax; the host does processes, files and network
  ([tools in Coax](../../coax/tools-in-coax.md)).
- The default is the simplest loop that works. An addition stays only with a
  measured gain in chapter 21 on tasks it was not built from.
- No session content leaves the person's machines without chapter 18's
  opt-in. Money is Bitcoin-only.
- Routing and filtering by meaning go through a typed decision (System One),
  never keyword matching. Code may check bounded fields such as IDs, sources
  and allowlists.
