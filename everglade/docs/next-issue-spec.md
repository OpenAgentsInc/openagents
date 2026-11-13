# Next issue: pick what to work on, from caches, and measure every call (spec)

Written 2026-11-10. Status: **built** in `apps/everglade-desktop/`,
including the coding flow that "start" begins. Sections marked **Planned**
are not.

The program answers one question: which issue should you work on next? It
reads your priorities from what you say and what you have been doing, gets
its facts from local caches, asks one small model, and lets you answer in
your own words. Every model call is written to a ledger with its model,
tokens and cost, so we can compare runs and cut tokens.

## Contents

1. [What it does](#1-what-it-does)
2. [Your priorities](#2-your-priorities)
3. [Caches](#3-caches)
4. [The message to the model](#4-the-message-to-the-model)
5. [The conversation](#5-the-conversation)
6. [The decision and the coding flow](#6-the-decision-and-the-coding-flow)
7. [The ledger](#7-the-ledger)
8. [What it costs today](#8-what-it-costs-today)
9. [Cutting tokens](#9-cutting-tokens)
10. [Planned](#10-planned)
11. [Open questions for the owner](#11-open-questions-for-the-owner)

## 1. What it does

In Everglade for desktop, the **Next issue** button at the top right:

1. Reads the open issues and recent commits from a local cache, and your
   recent prompts from Claude Code. Nothing is fetched when the cache is
   fresh.
2. Sends them to one Claude agent (Haiku 5.5, on your own Claude Code login).
3. Shows one suggested issue and why.
4. Takes your reply in your own words: "yes", "not that, something for the
   desktop app", "why that one?".
5. Records what you decided. If you said to start, a coding agent starts on
   that issue in a fresh worktree, in the same chat (section 6).

The same context is available from the command line, without the app:

```sh
coax/web/tool.sh coder/golemancy/next-context.coax context --prompt "focus on the desktop app"
coax/web/tool.sh coder/golemancy/next-context.coax work 13134 --repo-dir ~/work/openagents   # the worktree and the issue, whole
coax/web/tool.sh coder/golemancy/next-context.coax status     # the caches and their ages
coax/web/tool.sh coder/golemancy/next-context.coax usage      # the ledger, summed
```

## 2. Your priorities

Three signals, strongest first. The model is told this order.

| Signal | Where it comes from | Built |
| --- | --- | --- |
| What you say now | Your reply in the panel, or `--prompt` on the command line | Yes |
| What you asked for recently | Your last six prompts in the newest Claude Code transcript for this repository (`~/.claude/projects/*-openagents/*.jsonl`) | Yes |
| What you have been working on | The last 25 commit subjects on `main` | Yes |

A transcript line counts as your prompt only when you typed it. Tool
results, command output, and reminders the harness adds are left out. Each
prompt is cut at 320 bytes. Transcripts never leave your machine except
inside the message to your own Claude login.

## 3. Caches

### 3.1 GitHub (built)

```
~/.openagents/cache/github/OpenAgentsInc/openagents/
  issues.json     every open issue: number, title, labels, updatedAt and the whole body
  commits.txt     the last 25 commit subjects on main
  meta.json       when each was last fetched, and how many issues
```

`meta.json`:

```json
{"schema":"openagents.cache.github.v1","repo":"OpenAgentsInc/openagents",
 "issues":{"fetched_at":1794321688,"count":28,"file":"issues.json"},
 "commits":{"fetched_at":1794321688,"file":"commits.txt"}}
```

- **Freshness.** A file is fetched when it is missing or older than 600
  seconds (`--ttl SECONDS` changes that). `--refresh`, and the panel's
  **Refresh**, fetch now.
- **One request each.** Issues are one `gh issue list`; commits are one
  `gh api`. Both run on your own `gh` login.
- **A failed fetch keeps the old cache.** A fetch writes a temporary file and
  renames it over the old one only when it succeeded.
- **The cache holds whole bodies; the message carries the first 500 bytes of
  each.** A later step can read a whole body without a fetch.
- **Other programs can use it.** The folder and `meta.json` are the contract.

### 3.2 The conversation itself (built)

The agent's session stays open after its suggestion. Your reply is one short
message in the same session, so the issues are not sent again and the model
sees everything above it. Section 8 shows what that saves.

### 3.3 The provider's prompt cache (built, by layout)

The message is ordered so the part that seldom changes comes first
(section 4). Asking again within the provider's cache lifetime (five minutes)
reads the issues from the provider's cache at a tenth of the price.

### 3.4 The repository and its files (planned)

See [section 10](#10-planned).

## 4. The message to the model

One message, in this order:

| Part | Changes when | Why it is here |
| --- | --- | --- |
| The system prompt | We edit it | Stable |
| Open issues, **oldest first** | An issue opens, closes or is edited | A new issue is added at the end, so everything before it still matches the cached prefix |
| Recent commits, newest first | A commit lands | After the issues, so it can't spoil their cache |
| Your recent prompts | You type | Same |
| What you say now | Every time | Last |

The agent has **no built-in tools**: no Bash, no files, no web. The facts are
in the message, so it has nothing to fetch and makes no tool round trips to
read. It has one in-process tool, `choose` (section 6).

## 5. The conversation

- The panel shows the suggestion as it streams: the issue, up to three lines
  of why, then "Do it?".
- Click the panel and type. Enter sends. Escape, or a click on the world,
  gives the keys back to walking.
- While the agent answers the button says **Stop**. **Refresh** fetches the
  caches again and starts a new session. **Close** shuts the panel and keeps
  the session.
- The foot of the panel shows the last turn's tokens (new, cached, written),
  its cost, the session's cost, and how old the caches were.

## 6. The decision and the coding flow

Your words are never matched against keywords. The model reads your reply
and calls a typed tool:

```
choose(action: "start" | "another" | "none", issue: integer, reason: string)
```

| Action | When | What happens |
| --- | --- | --- |
| `start` | You agreed to an issue | The coding flow begins (below) |
| `another` | You turned it down | The agent suggests the next best, using what you said |
| `none` | You want to stop | Nothing |

A question ("why that one?") is answered without the tool.

Every decision is appended to
`~/.openagents/everglade/next-issue/decisions.jsonl` (`NextIssueDecision` in
`proto/openagents/coder/golemancy/v1alpha1/records.proto`, written by
`coax/lib/agent/ledger.coax` through the JSON mapper; a `start` adds `tier` and
the rubric's `scores`):

```json
{"schema":"openagents.next-issue.decision.v1","at":1794321982,"action":"start","issue":13134,
 "reason":"Developer said \"ok sure, go ahead with that\" to starting #13134.",
 "session":"9b0adefa-...","coding_flow":"not built"}
```

### The coding flow

When you say start, the picker finishes its turn, so its cost is recorded,
and its session ends. Then:

1. **A worktree.** `next-context work N --repo-dir CHECKOUT` fetches
   `origin/main` and makes `CHECKOUT-issue-N`, detached at `origin/main`. An
   existing worktree for the issue is reused as it is. `CHECKOUT` is the main
   checkout the app was built from. It prints the worktree's path and the
   issue whole, from the cache.
2. **A coding agent.** A second session starts in that worktree, in the same
   chat.
3. **You can steer it.** A reply now goes to the coding agent.

| Setting | Value |
| --- | --- |
| Model | The tier the picker's rubric gives (below): Haiku 5.5, Sonnet 5.5, Opus 5.5 or Fable 5.2. Opus 5.5 when no scores came. `EVERGLADE_CODER_MODEL` overrides it |
| Tools | Bash, Read, Edit, Write, Grep, Glob. Nothing else: no Task or Agent (no subagents), no web |
| Permissions | Edits and the shell run without asking, inside the worktree it starts in |
| Rules it loads | The repository's own (`AGENTS.md`, `CLAUDE.md`). Not your user settings |
| Its one job | That issue and nothing else. When its checks pass: commit its files, rebase on `origin/main`, push `HEAD:main`, close the issue with a comment naming the commit. If it can't finish: stop, say what blocks it, leave the issue open |

### The coding agent on Antigravity (built 2026-11-11, #13193)

The coding agent can be Google's Antigravity CLI (`agy`) instead of Claude
Code, in the normal flow and in Autopilot. The picker stays Claude Haiku.

| Setting | Effect |
| --- | --- |
| `EVERGLADE_CODER_ENGINE=antigravity` (or `agy`) | The coder runs on Antigravity, and the rubric names Gemini models |
| `EVERGLADE_TIER_FAMILY=gemini` | The same, said from the rubric's side |
| `EVERGLADE_CODER_MODEL=gemini-3.8-flash-low` | Any model `agy models` lists; one agy doesn't offer fails the run before any turn |
| Neither | Claude, as before. Two that disagree (`claude` with `gemini`) are refused with why |

It runs on the shared ACP engine with Antigravity's agent
(`coax/lib/agent/engine/antigravity.coax`, the one Coder runs), started in the
worktree with the credentials file `~/.openagents/coder/antigravity.env`, in
agy's `default` mode, where it asks before edits and commands. What the
Claude coder has, and how it maps:

| Claude coder | Antigravity coder |
| --- | --- |
| Rules in the system prompt (`NP_CODER_RULES`, `NP_CODER_REHEARSAL`) | The same text at the top of the first message, then a paragraph on agy's tools and the report line, then the same job and issue. agy advertises `_meta.antigravity.systemPrompt.append` at initialize, but its shape isn't documented, so the rules ride in the message |
| Bash, Read, Edit, Write, Grep, Glob; no subagents, no web | agy's own tools, limited by the panel's answers to its permission requests: a coding run allows edits and commands inside the worktree; rehearsal allows only `read` and `search` kinds. Refused always: kind `fetch`, a path outside the worktree or with `..`, and the tools `browser*`, `invoke_subagent`, `send_message`, `manage_subagents`, `define_subagent`, `search_web`, `read_url*`, `generate_image`, `swarm*`. agy runs some tools without asking (reads in the workspace, writes under the temporary folder), so a barred tool that starts unasked cancels the turn (`session/cancel`) and the run fails |
| The in-process `report` tool | A last line, `REPORT: done SUMMARY` or `REPORT: blocked SUMMARY`, read from the turn's words when it ends. agy takes no in-process tool, and an MCP server over ACP's `mcpServers` would be a second process to build, ship and wire back; a file can't be written in rehearsal. The line needs nothing and works read-only |
| Nudges (three) | The same, with the REPORT line named |
| Stop | Ends the agy process |
| Ledger line per turn | `harness` `antigravity`, `thought_tokens`, and `cost_priced_as`: agy reports tokens but no cost, so the turn is priced from the rates card (`coax/lib/gateway/card.coax`, Vertex): the model less its level (`gemini-3.8-flash-low` is `gemini-3.8-flash`); a model the card lacks (Gemini 4 Argon) at Carbon's row, so it is never free. Thought tokens are billed as output |
| The budget (`NP_BUDGET_E6`) | Counts those prices |

**Headless.** The Workshop window runs only on the Mac, so the flow has a
driver that calls `next.coax`'s exported calls as the Mac host does
(`next-face.c` under `Workshop.swift`'s frame loop): it presses Next issue,
runs `next-context` for the panel's requests, appends the ledger lines and
decisions where the app does, prints the chat, and stops after N coding turns.

```sh
EVERGLADE_CODER_ENGINE=antigravity EVERGLADE_CODER_MODEL=gemini-3.8-flash-low EVERGLADE_CODER_MODE=rehearsal \
  coax/web/tool.sh apps/everglade-desktop/next-drive.coax --laps 2 --minutes 14 --repo-dir CHECKOUT
```

Two laps rehearsed this way on coderos-4080 (2026-11-11, `gemini-3.8-flash-low`,
read-only) took about a minute. The picker chose #13218 (Sonnet tier), then
#13198 (Opus tier). Each time the coder first tried a shell command (`git show`,
`ls`), which rehearsal refuses; agy ends its turn after a refusal, so Autopilot
nudged it once, and its second turn read the files and ended with
`REPORT: done ...`. Antigravity's four turns: 61,397 input tokens, 194,146 read
from cache, 1,095 output, no thought tokens: $0.0647 at the card's Flash price.
The two picker turns cost $0.1231; Claude Code billed the first as Opus 5.5
(its `modelUsage`), though Haiku 5.5 was asked for.

### Which model works the issue (tiers)

With every `choose` call the picker also fills a rubric for the issue it
names: four scores from 0 to 3, each with written anchors, and whether an agent
should work it at all. The scores are the model's judgment. The thresholds are
fixed code in `coax/lib/agent/tier.coax`, shared so that any app handing an
issue to an agent (Coder, Foreman, a tool) can use the same rule.

| Score | 0 | 1 | 2 | 3 |
| --- | --- | --- | --- | --- |
| scope | one file, a few lines | a few files in one unit | several units, or a new module | many units or every host |
| ambiguity | the exact change is stated | a clear goal, small choices | design choices nobody has made | open-ended: research or a spec first |
| risk | docs, copy, tests, table numbers | one app's feature, easy to revert | shared engine, library or build code | production, money, auth, the release pipeline, data loss |
| depth | mechanical | ordinary app or UI code | engine systems | the compiler, GPU shaders, concurrency, verification |

The rule, first match wins:

| Condition | Model |
| --- | --- |
| Not delegable (a tracker, parked, owner-only, needs a device) | none: the picker is told to pick another |
| depth 3 and ambiguity 2 or more, or a sum of 9 or more | Fable 5.2 |
| a sum of 6 or more, risk 3, or depth 3 | Opus 5.5 |
| a sum of 3 or more, or risk 2 or more | Sonnet 5.5 |
| otherwise (housekeeping) | Haiku 5.5 |

**Gemini's models.** `EVERGLADE_TIER_FAMILY=gemini` (or the Antigravity
engine) makes the same tiers name Antigravity's models; the scores and the
thresholds don't change. Claude stays the default until the experiments in
#13193 say otherwise.

| Tier | Claude (default) | Gemini |
| --- | --- | --- |
| Haiku | `claude-haiku-5-5` | `gemini-3.8-flash-low` |
| Sonnet | `claude-sonnet-5-5` | `gemini-3.8-flash-high` |
| Opus | `claude-opus-5-5` | `gemini-4-argon` (the model agy's own `pro` subagent runs) |
| Fable | `claude-fable-5-2` | `gemini-4-carbon` (the newest) |

Flash's level is the tiers' choice, because it is the only difference
between Haiku and Sonnet. Argon and Carbon are named without a level, so agy
runs them at low thinking, the Antigravity engine's default (#13193): at
medium their long thinking made tasks minutes slower.

`tiers.coax --family gemini` prints the Gemini model for each issue.

The chat shows `> start #N on Sonnet`, and the decision line carries `tier`
and `scores`. To rate every open issue at once (one Haiku session, about
$0.01 for 27 issues):

```sh
coax/web/tool.sh coder/golemancy/next-context.coax context > ctx.txt
coax/web/tool.sh apps/everglade-desktop/tiers.coax ctx.txt [--model claude-sonnet-5-5]
```

Measured 2026-11-10 on 27 open issues: two Haiku passes gave the same model
for 26; Sonnet as the rater agreed with Haiku on 21, each difference one step
across a threshold. No open issue was housekeeping, so none went to Haiku.

**How a coder finishes.** The coder has one in-process tool, `report(status:
done | blocked, summary)`. It is told to end only by calling it, and never to end
its turn while something it started is still running: it waits in the
foreground. With Autopilot on, a turn that ends without `report` gets
"finish, or report blocked" up to three times before the loop moves on. The
chat shows `> done: ...` or `> blocked: ...`. This came from a Fable run on #13135
(2026-11-10). It started six check suites on the build hosts, said it was waiting
for them, and ended its turn. Autopilot took that as the end of the issue, and
the fix was left uncommitted.

**Why always a worktree.** The rule was: a fresh worktree unless that forces
an expensive rebuild. It doesn't. A worktree carries no build output, and
`coax/build.sh` installs the published compiler in seconds; heavy builds go
through `coax/remote/remote.sh` either way. So the flow never works in the
main checkout, which other agents share.

**What you see.** The agent's messages stream into the chat as plain text
(Markdown's bold marks and backticks are left out). Each tool call is one
line: `> $ git status`, `> Read coax/src/main.cpp`, `> Grep pattern`, with
paths shown from the worktree and the line cut at 96 bytes. A tool's result
shows only when it failed, as one line. A long run keeps the newest half of
the chat; the wheel scrolls it.

**Stop** ends the agent's process group. The worktree stays, so a new start
on the same issue picks it up.

### Autopilot

The **Autopilot** button sits beside Next issue and is on by default. With it
on, pressing Next issue starts a loop with no one in it:

1. The picker is told nobody will answer. It picks an issue and calls
   `choose` with `start` at once. If it asks anyway, Autopilot answers it
   once.
2. The coding agent works the issue.
3. When the coding agent's turn ends, the caches are fetched again (the issue
   may now be closed), and the picker picks the next. It is told which issues
   this run has already worked, and leaves them out.

The chat keeps the whole run, with a `> next` line between laps.

The loop stops when:

- you press **Stop** (it ends the agent and does not go round);
- an agent fails;
- the picker calls `choose` with `none`, because nothing is worth starting;
- the run has worked 32 issues.

Turning Autopilot off during a run lets the agent in hand finish and then
waits for you. Nothing starts when the app opens: the loop begins when you
press Next issue.

Two laps rehearsed on Haiku (read-only) took 54 seconds: it picked #13134,
then #13135, noting the first was already worked.

**One worktree, one agent.** The app records itself as the owner of an
issue's worktree, and `next-context work` refuses a worktree whose owner is
another process still running. Two windows, or a rehearsal beside a live run,
can't put two agents in one worktree.

**Rehearsal.** `EVERGLADE_CODER_MODE=rehearsal` gives the agent Read, Grep
and Glob only and tells it to say what it would change and stop. Use it with
`EVERGLADE_CODER_MODEL=claude-haiku-5-5` to try the flow without changing
anything:

```sh
EVERGLADE_CODER_MODE=rehearsal EVERGLADE_CODER_MODEL=claude-haiku-5-5 \
  build/Everglade.app/Contents/MacOS/Everglade --shot out.png --next 240 --autopilot-off --reply "yes, do it"

# Autopilot, two laps:
EVERGLADE_CODER_MODE=rehearsal EVERGLADE_CODER_MODEL=claude-haiku-5-5 \
  build/Everglade.app/Contents/MacOS/Everglade --shot out.png --next 400 --runs 2
```

A rehearsal of #13134 on Haiku took 31 seconds in all and cost $0.0116 for
the agent's turn (six tool calls, 178,841 tokens read from cache).

Not built: a worktree is not removed when the issue closes; two issues can't
run at once in one window; there is no cap on what a run may spend, which
matters more now that Autopilot goes round by itself (you can stop it, and
section 11 asks whether you want a limit you set).

## 7. The ledger

Every model turn appends one line to `~/.openagents/usage/ai-calls.jsonl`
(mode 600). The schema is `AiCall` in
`proto/openagents/coder/golemancy/v1alpha1/records.proto`; the one writer and
reader is `coax/lib/agent/ledger.coax`, and `proto/coder/golemancy-v1alpha1.json`
pins its lines byte for byte. The tier batch (`tiers.coax`, purpose `issue.tier`)
writes the same record with `issues_rated` and without the turn's fields.

| Field | Meaning |
| --- | --- |
| `at` | Seconds since the epoch |
| `app`, `purpose` | Who asked and what for: `next-issue.suggest`, `next-issue.reply`, `issue.code`, `issue.rehearse` |
| `issue` | The issue a coding turn worked on |
| `harness`, `model` | `claude-code` or `antigravity`, and the model asked for |
| `session`, `turn` | The session and the turn within it |
| `input_tokens` | New input tokens, not from cache |
| `cache_read_input_tokens` | Input read from the provider's cache |
| `cache_creation_input_tokens` | Input written to the provider's cache |
| `output_tokens` | Output, thinking included |
| `cost_usd_e6` | This turn's cost in millionths of a dollar (Antigravity's: the rates card's price, the row in `cost_priced_as`) |
| `thought_tokens` | Antigravity's thinking tokens, priced as output (only on its lines) |
| `session_cost_usd_e6` | The session's cost so far |
| `duration_ms`, `duration_api_ms`, `model_turns` | As the CLI reports them |
| `prompt_bytes` | Bytes we sent this turn |
| `caches` | How old the local caches were |
| `model_usage` | The CLI's per-model record, as it gave it (it includes thinking tokens) |

The numbers are the ones the CLI reports for the turn. Cost per turn is the
growth in the session's reported cost.

`next-context usage` sums the ledger by purpose and model:

```
purpose             model             calls  input  cache_read  cache_write  output  cost_usd  avg_ms
next-issue.suggest  claude-haiku-5-5  4      8      31834       30814        3913    0.006127  5085
next-issue.reply    claude-haiku-5-5  3      10     80267       3925         1717    0.002153  3265
```

**Rule for new AI features:** every model call an OpenAgents app makes writes
a line here, with its own `app` and `purpose`.

**Not in the ledger yet:** the Claude Code and Codex sessions that write this
code. See [section 10](#10-planned).

## 8. What it costs today

Measured on 2026-11-10 with 28 open issues (a 19.8 KB message):

| Call | New input | Cache read | Cache written | Output | Cost | Time |
| --- | --- | --- | --- | --- | --- | --- |
| First suggestion | 2 | 0 | 15,652 | 1,043 | $0.0025 | 5.4 s |
| A suggestion again within five minutes | 2 | 15,652 | 0 | 1,022 | $0.0007 | 5.4 s |
| A reply in the same session | 2 to 4 | 15,652 to 32,143 | about 1,000 | 150 to 760 | $0.0007 | 3 to 4 s |

What the numbers say:

- The provider's cache cuts a repeat suggestion to about a quarter of the
  price.
- A reply costs about the same as a cached suggestion, and the issues are not
  sent again.
- Of the first suggestion's 1,043 output tokens, 925 were thinking. The
  answer itself is about 120 tokens.
- Reading the local cache instead of fetching takes about a second, most of
  it reading the transcript.

## 9. Cutting tokens

The levers, cheapest first. Each is one change, measured with
`next-context usage` before and after.

| Lever | What it would save | Cost |
| --- | --- | --- |
| Lower effort for this call | Most of the 925 thinking tokens a suggestion | May pick worse; needs a set of asks with known good answers |
| Shorter bodies (500 bytes to 200), or titles and labels only | Up to two thirds of the input | The model loses the reason an issue matters |
| Rank locally first, send the top ten | Most of the input | Needs the ranking of section 10 |
| Keep one session across button presses | The cache write on each new session | The session must be told when the cache changed |
| A one-hour provider cache for the issues | The rewrite after five idle minutes | Cache writes cost more |
| A smaller model for replies that are a plain yes or no | Little; replies are already cheap | None worth it yet |

Do not cut what changes the answer without a way to check the answer. The
first thing to build before hill-climbing is a small set of asks with the
issue a person would pick.

## 10. Planned

**The repository, cached.** A clone at
`~/.openagents/cache/repos/OpenAgentsInc/openagents`, fetched on the same
freshness rule, so the program can read code without a network call and
without touching your working checkout.

**Facts attached to files.** An index keyed by commit, rebuilt only for files
that changed:

| Per file | From |
| --- | --- |
| Path, size, language, last commit, authors | `git` |
| What it declares (procedures, structs, checks) | `coax vocab` and the compiler's own tables |
| Which issues and commits touched it | Mined from `git log` and issue references |
| An embedding of its header comment and declarations | An embeddings model, cached by content hash |

Finding the files for an issue is then rules and the index first, and a model
only when they disagree (the deterministic-first rule, #11210). The same
index lets the suggestion say which files an issue touches and how big it is.

**Issue changes only.** Fetch issues changed since the last fetch
(`updated:>TIME`) and merge them, instead of the whole list.

**The files for an issue, handed to the coding agent.** Today it gets the
issue and finds the files itself. With the index it starts with them.

**All AI work in the ledger.** An importer that reads Claude Code and Codex
transcripts (each assistant message carries its usage) and writes ledger
lines with `app` set to the harness, so the work that builds this is measured
on the same table.

**More priority signals.** Issues you commented on, the project board's
order, and what is assigned to you.

## 11. Open questions for the owner

1. **The transcript signal.** It reads your most recent Claude Code session
   for this repository. Should it read more than one session, or other
   harnesses?
2. **The ledger's home.** `~/.openagents/usage/ai-calls.jsonl` is local.
   Should it sync anywhere?
3. **Freshness.** Is ten minutes right for issues, or should the panel always
   show the cache and fetch behind it?
4. **A spending limit.** A coding run on Opus has no cap. Do you want a
   limit you set (dollars or turns) that stops the run and asks?
5. **The first known-answer set.** Which ten asks, and whose picks, do we
   hill-climb against?
