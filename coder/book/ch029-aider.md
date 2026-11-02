# 29. Aider

> **Status: written.** Read at Aider-AI/aider@5dc9490bb35f9729ef2c95d00a19ccd30c26339c on 2026-11-02.

Source: [`Aider-AI/aider`](https://github.com/Aider-AI/aider/tree/5dc9490bb35f9729ef2c95d00a19ccd30c26339c),
cloned shallowly into `projects/repos/aider` and read only. Nothing was
installed or run. Paths below are relative to that revision.

## Why study it

Aider is a pair-programming agent that edits files in a git repository from
the terminal. It does not run a free tool loop like the other agents in this
part. The model answers in text, and Aider parses file edits out of that text
in one of several edit formats, applies them, commits them, and then lints
and tests. Two of its designs matter to Coder:

- **The repository map.** Aider gives the model a ranked, token-budgeted
  outline of the whole repository on every request, built from tree-sitter
  tags and PageRank (`aider/repomap.py`).
- **Measuring edit formats.** Aider records, per model and per edit format,
  how often the model's reply could not be parsed or applied, and publishes
  those numbers next to the pass rate (`benchmark/benchmark.py`,
  `aider/website/_data/polyglot_leaderboard.yml`).

Coder's chapters 3, 7 and 21 ask what the model should see each step and how
to measure a harness change. Aider has answers to both, with public data.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| A repository map: tags from tree-sitter, a reference graph, PageRank personalized to the chat | `aider/repomap.py` (`get_ranked_tags`) | [3](ch003-keep-the-prefix-stable.md), [21](ch021-measure-the-harness.md) |
| The map fitted to a token budget by binary search | `aider/repomap.py` (`get_ranked_tags_map_uncached`) | [3](ch003-keep-the-prefix-stable.md) |
| A tag cache on disk keyed by file modification time | `aider/repomap.py` (`TAGS_CACHE_DIR`, `get_tags`) | [3](ch003-keep-the-prefix-stable.md) |
| Prompt sections in a fixed order with cache markers | `aider/coders/chat_chunks.py` | [3](ch003-keep-the-prefix-stable.md) |
| Edit formats: whole file, search/replace blocks, unified diff, patch, and an architect/editor split | `aider/coders/*_coder.py` | [4](ch004-tools-as-typed-capabilities.md), [21](ch021-measure-the-harness.md) |
| A failed edit explained back to the model with the closest lines | `aider/coders/editblock_coder.py` | [4](ch004-tools-as-typed-capabilities.md) |
| Per-model defaults: edit format, weak model, repo map on or off | `aider/resources/model-settings.yml` | [13](ch013-choose-an-engine.md) |
| Lint and test after an edit, errors fed back, at most 3 reflections | `aider/coders/base_coder.py` (`max_reflections`, `auto_lint`, `auto_test`) | [22](ch022-generate-candidates-then-check-them.md) |
| Chat history summarized by a weak model past a token limit | `aider/history.py`, `aider/coders/base_coder.py` (`ChatSummary`) | [7](ch007-condense-the-history.md) |
| A benchmark harness in Docker with two tries per exercise and error counters | `benchmark/benchmark.py`, `benchmark/README.md` | [21](ch021-measure-the-harness.md) |
| Published results keyed by model, edit format and Aider commit | `aider/website/_data/polyglot_leaderboard.yml` | [21](ch021-measure-the-harness.md) |

### The repository map

`RepoMap` parses each source file with tree-sitter queries from
`aider/queries/` and records two kinds of tags: definitions and references.
The tags are cached in `.aider.tags.cache.v{N}` and reused while a file's
modification time is unchanged (`aider/repomap.py`, `get_tags`).

`get_ranked_tags` builds a directed graph with one node per file. For each
identifier, it adds an edge from every file that references it to every file
that defines it. The weight starts at 1 and is multiplied by 10 if the person
mentioned the identifier, by 10 if it is a long snake, kebab or camel case
name, by 0.1 if it starts with an underscore, and by 0.1 if more than five
files define it. A reference from a file in the chat is multiplied by 50, and
the count of references is square-rooted so frequent names do not dominate.
PageRank then runs with a personalization vector that favours files in the
chat, files the person mentioned and files whose path contains a mentioned
identifier (`aider/repomap.py`, `get_ranked_tags`).

`get_ranked_tags_map_uncached` turns the ranked tags into an outline: file
names with the lines that hold the top definitions. It binary-searches the
number of tags to include so the outline's token count lands within 15% of
the budget without going over it when possible (`ok_err = 0.15`). The default
budget is 1,024 tokens. With no files in the chat, the budget is multiplied by
`map_mul_no_files` (8), capped at the context window minus 4,096 tokens, so a
new session sees more of the repository (`aider/repomap.py`, `get_repo_map`).
A `refresh` setting (`auto`, `always`, `files`, `manual`) controls when the
map is rebuilt and when the last one is reused.

### Prompt order and caching

`ChatChunks` holds the prompt in sections and always emits them in the same
order: system, examples, read-only files, repository map, earlier messages,
files in the chat, the current messages, and a reminder. For providers with
prompt caching it marks the end of the examples (or system), the end of the
repository map, and the end of the chat files as cache points
(`aider/coders/chat_chunks.py`). The stable parts come first and the parts
that change each turn come last.

### Edit formats

Each edit format is a `Coder` subclass with its own prompts and parser. The
`edit_format` names at this revision include `whole`, `diff` (search/replace
blocks), `diff-fenced`, `udiff`, `patch`, `architect` and the editor variants
`editor-diff`, `editor-whole` and `editor-diff-fenced`
(`aider/coders/*_coder.py`). `aider/resources/model-settings.yml` sets a
default format per model, along with a weak model and whether to send the
repository map.

The `diff` format asks the model for blocks fenced by `<<<<<<< SEARCH`,
`=======` and `>>>>>>> REPLACE` (`aider/coders/editblock_coder.py`, `HEAD`,
`DIVIDER`, `UPDATED`). `replace_most_similar_chunk` tries an exact match, then
a match that ignores leading whitespace, then one without a spurious leading
blank line, then a match across `...` elisions. A fuzzy edit-distance match
follows in the source but sits after a `return` and never runs. When a block
does not match, the error sent back names the file and lists the closest
actual lines under "Did you mean to match some of these actual lines"
(`aider/coders/editblock_coder.py`).

`ArchitectCoder` splits one request across two models. The main model
describes the change in prose; on confirmation, a second `Coder` runs with
the editor model and editor edit format, with no repository map and no
cache warming, and applies it (`aider/coders/architect_coder.py`).

### Lint, test and reflect

After applying edits, Aider lints the edited files when `auto_lint` is on (the
default) and runs the test command when `auto_test` is on (off by default).
Errors become a `reflected_message`, which Aider sends to the model as the
next request. It stops after `max_reflections = 3`
(`aider/coders/base_coder.py`). `aider/linter.py` falls back to a tree-sitter
syntax check (`basic_lint`) when no linter is configured.

### History

When the chat history passes the model's `max_chat_history_tokens`,
`ChatSummary` summarizes older messages, trying the weak model first and then
the main model (`aider/coders/base_coder.py`, `aider/history.py`). The same
models list writes commit messages (`aider/repo.py`, `get_commit_message`).

### The benchmark

The harness runs Exercism exercises inside Docker, because it executes
model-written code without review (`benchmark/README.md`). The polyglot set
covers C++, Go, Java, JavaScript, Python and Rust
(`aider/website/_posts/2024-12-21-polyglot.md`). Each
exercise gets `--tries` attempts, two by default: the second attempt sees the
failing test output (`benchmark/benchmark.py`). The summary reports
`pass_rate_1` and `pass_rate_2`, and also `percent_cases_well_formed`,
`num_malformed_responses`, `syntax_errors`, `indentation_errors`,
`lazy_comments`, `exhausted_context_windows`, `test_timeouts`,
`seconds_per_case` and `total_cost`. Every run records the Aider
`commit_hash` and marks a dirty tree with `-dirty` (`benchmark/benchmark.py`).
The published polyglot leaderboard has 69 entries, each with the model, edit
format, commit, command and all of those counters, over 225 test cases
(`aider/website/_data/polyglot_leaderboard.yml`).

## What Coder takes

- **Count the harness's own failures next to the pass rate.** A pass rate
  alone hides whether a loss came from the model's code or from the harness
  failing to parse it. Microcoder already ends a run as `bad_replies` after
  three schema failures ([loop.coax](../../coax/lib/microcoder/loop.coax)).
  Coder takes Aider's practice of reporting, per suite, the share of tasks with
  any malformed reply, the count of malformed replies, context exhaustion and
  timeouts, beside the pass rate. [Chapter 21](ch021-measure-the-harness.md)'s
  receipts are the place for these counters.
- **Record the harness revision with each result.** Aider stamps every run with
  its commit and a dirty flag. Chapter 21 already chains receipts to a
  configuration; Coder adds the repository revision and a dirty flag to each
  one, so a result can be traced to the code that produced it.
- **Two tries as a reported number.** `pass_rate_1` and `pass_rate_2` separate
  "right the first time" from "right after seeing the tests". This maps onto
  [chapter 22](ch022-generate-candidates-then-check-them.md)'s generate, run
  and repair under one budget, and Coder reports both numbers.
- **Stable sections first.** `ChatChunks` is the same idea as
  [chapter 3](ch003-keep-the-prefix-stable.md)'s fixed section order: put what
  rarely changes first and mark the cache boundary after it. Aider's order
  gives a concrete placement for a repository outline: after the system
  prompt and before anything per-turn.
- **Explain a failed edit with the nearest real text.** Microcoder edits
  through shell commands, so it does not parse edit blocks. When Coder adds an
  edit tool ([chapter 4](ch004-tools-as-typed-capabilities.md)), a failed match
  returns a structured error with the closest lines from the file, as Aider's
  `SearchReplaceNoExactMatch` message does.

## What Coder leaves, and why

- **Edits parsed out of free text.** Aider's formats exist because it asks for
  edits in prose. Coder's engines act through typed calls: Microcoder returns
  one `next_action` object checked against a strict schema, and Claude Code and
  Codex bring their own edit tools. Several text formats to choose between per
  model are a cost Coder does not need to pay, so it measures a structured edit
  tool against shell edits instead (below).
- **Identifier weights by naming style.** Multiplying weights for snake or camel
  case names longer than eight characters is a heuristic about meaning written
  as string rules. Coder routes by meaning through typed decisions, not keyword
  or string matching ([plan](PLAN.md#rules-for-every-chapter)). A repository
  outline in Coder ranks files from the graph structure and the files in the
  task; any learned ranking goes through a System One decision.
- **Auto-commit after every edit.** Aider commits each change. Coder owns the
  working tree and the pull request ([plan](PLAN.md#openagents-and-coder)) and
  commits when a task's tests pass, so the history a person reviews matches the
  tasks, not the steps.
- **A weak model for summaries by default.** Which model summarizes is an engine
  decision ([chapter 13](ch013-choose-an-engine.md)), and whether summaries help
  at all is [chapter 7](ch007-condense-the-history.md)'s measurement.

## Where a borrowed design is measured

- **A repository outline.** Microcoder today sees the environment, the task and
  its last 12 actions; it finds files with commands. Chapter 21 can measure one
  change: add a token-budgeted outline of definitions (Aider's default is 1,024
  tokens) as a fixed section, and compare task success, steps and input tokens.
  [Chapter 3](ch003-keep-the-prefix-stable.md) measures whether the outline
  stays byte-stable between steps; Aider's `refresh` modes are the variable to
  test.
- **Edit tool against shell edits.** Microcoder edits with commands such as
  `sed` or a heredoc. Chapter 21 measures a typed search-and-replace tool
  against that, with Aider's counters: well-formed rate, failed edits and
  syntax errors after an edit.
- **Two models for one edit.** The architect/editor split is a two-call design.
  [Chapter 22](ch022-generate-candidates-then-check-them.md) can compare one
  strong call against a strong planner plus a cheaper editor under the same
  budget.
- **Lint and test feedback.** Aider's three reflections are a fixed repair
  budget. Chapter 22's generate-run-repair loop measures how many repairs pay
  for themselves.

## Sources

- [`aider/repomap.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/repomap.py)
- [`aider/coders/chat_chunks.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/coders/chat_chunks.py)
- [`aider/coders/base_coder.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/coders/base_coder.py)
- [`aider/coders/editblock_coder.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/coders/editblock_coder.py)
- [`aider/coders/architect_coder.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/coders/architect_coder.py)
- [`aider/coders/`](https://github.com/Aider-AI/aider/tree/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/coders) (one file per edit format)
- [`aider/resources/model-settings.yml`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/resources/model-settings.yml)
- [`aider/history.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/history.py)
- [`aider/linter.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/linter.py)
- [`aider/repo.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/repo.py)
- [`benchmark/README.md`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/benchmark/README.md)
- [`benchmark/benchmark.py`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/benchmark/benchmark.py)
- [`aider/website/_posts/2024-12-21-polyglot.md`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/website/_posts/2024-12-21-polyglot.md)
- [`aider/website/_data/polyglot_leaderboard.yml`](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/website/_data/polyglot_leaderboard.yml)
- Coder: [`coax/lib/microcoder/loop.coax`](../../coax/lib/microcoder/loop.coax),
  [docs/coder/microcoder.md](../../docs/coder/microcoder.md).
- [Chapter plan](PLAN.md), Part VII; the shape follows the
  [DeepSeek Harness notes](../../psionic/docs/harness.md).
