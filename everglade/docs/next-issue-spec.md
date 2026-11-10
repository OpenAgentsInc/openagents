# Next issue: pick what to work on, from caches, and measure every call (spec)

Written 2026-11-10. Status: **the first version is built** in
`apps/everglade-desktop/`. Sections marked **Planned** are not.

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
5. Records what you decided. If you said to start, the coding flow begins
   there. The coding flow is not built: the panel says so and nothing starts.

The same context is available from the command line, without the app:

```sh
coax/web/tool.sh apps/everglade-desktop/next-context.coax context --prompt "focus on the desktop app"
coax/web/tool.sh apps/everglade-desktop/next-context.coax status     # the caches and their ages
coax/web/tool.sh apps/everglade-desktop/next-context.coax usage      # the ledger, summed
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
| `start` | You agreed to an issue | The coding flow begins here. **Stub:** the panel says the coding flow is not built and nothing was started |
| `another` | You turned it down | The agent suggests the next best, using what you said |
| `none` | You want to stop | Nothing |

A question ("why that one?") is answered without the tool.

Every decision is appended to
`~/.openagents/everglade/next-issue/decisions.jsonl`:

```json
{"schema":"openagents.next-issue.decision.v1","at":1794321982,"action":"start","issue":13134,
 "reason":"Developer said \"ok sure, go ahead with that\" to starting #13134.",
 "session":"9b0adefa-...","coding_flow":"not built"}
```

That file is where the coding flow will pick up: it reads the newest `start`.

## 7. The ledger

Every model turn appends one line to `~/.openagents/usage/ai-calls.jsonl`
(mode 600):

| Field | Meaning |
| --- | --- |
| `at` | Seconds since the epoch |
| `app`, `purpose` | Who asked and what for: `next-issue.suggest`, `next-issue.reply` |
| `harness`, `model` | `claude-code`, and the model asked for |
| `session`, `turn` | The session and the turn within it |
| `input_tokens` | New input tokens, not from cache |
| `cache_read_input_tokens` | Input read from the provider's cache |
| `cache_creation_input_tokens` | Input written to the provider's cache |
| `output_tokens` | Output, thinking included |
| `cost_usd_e6` | This turn's cost in millionths of a dollar |
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

**The coding flow.** `start` hands the issue, its whole body from the cache,
and the files from the index to a coding agent. Separate work.

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
4. **The first known-answer set.** Which ten asks, and whose picks, do we
   hill-climb against?
