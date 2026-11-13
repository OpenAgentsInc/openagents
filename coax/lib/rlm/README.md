# Recursive Language Models in Coax

A Recursive Language Model (RLM; Zhang, Kraska and Khattab, [arXiv 2512.24601](https://arxiv.org/abs/2512.24601),
reference code [alexzhang13/rlm](https://github.com/alexzhang13/rlm)) answers a question about a
context far longer than its own window. The context is never put in the model's prompt. It lives
in an environment the model programs, turn by turn; the model's code slices, searches and chunks
it, and calls sub-models on the pieces. GitHub #13367. The experiment and its numbers are in
[docs/research/2026-11-12-recursive-language-models.md](../../../docs/research/2026-11-12-recursive-language-models.md).

## What the paper and the reference code do

| Piece | Paper and reference (`rlm/core/rlm.py`, `rlm/environments/local_repl.py`, `rlm/utils/prompts.py`) |
| --- | --- |
| Root model | One LM (GPT-5 in the paper). It sees a system prompt, the question, the context's type and length, and each turn the REPL's output cut to about 20,000 characters. Never the context itself. |
| Environment | A persistent Python REPL (`exec` in-process, or Docker, Modal, E2B and others). `context` holds the prompt (a `str` or `list[str]`). Variables persist across turns. |
| Operations | `llm_query(prompt)`: one sub-LM call (GPT-5-mini in the paper), which sees only that prompt. `llm_query_batched(prompts)`: the same, concurrently. `rlm_query(prompt)`: a child RLM with its own REPL whose `context` is the prompt; it falls back to `llm_query` at the depth limit. `SHOW_VARS()`. |
| Depth | `max_depth` 1 (the default and the paper's main setting) makes sub-calls plain LM calls; above 1, `rlm_query` starts children one level deeper. Children get what is left of the parent's budget and time. |
| Ending | The paper: `FINAL(answer)` or `FINAL_VAR(name)` written outside code. The current code: `answer["content"] = ...; answer["ready"] = True` in a block. After `max_iterations` (30) one more call asks for the final answer. |
| Limits | `max_iterations`, `max_budget` (USD), `max_timeout`, `max_tokens`, `max_errors`, a batch concurrency of 4 to 20. |
| Prompt | Look at the context first, plan the decomposition, chunk it into sub-calls of up to about 100K characters, batch them, print small results, use code where code decides exactly, and do not answer on the first turn. |
| Benchmarks | S-NIAH (constant work per query), BrowseComp-Plus over 1,000 documents (6 to 11M tokens), OOLONG trec_coarse (131K tokens, work linear in the context), OOLONG-Pairs (quadratic), LongBench-v2 CodeQA. With GPT-5: OOLONG 44.0 base, 56.0 RLM; OOLONG-Pairs 0.1 base, 58.0 RLM; BrowseComp-Plus 0.0 base (over its window), 91.3 RLM; CodeQA 24.0 base, 62.0 RLM; costs comparable to the base call ($0.11 to $0.99 a query). |

## The Coax version

We do not ship Python and won't run model-written code in one. The model writes the same thing it
would write for the reference, a ```` ```repl ```` block of Python, and `script.coax` runs a safe
subset of Python itself. That keeps the paper's essential property: the long context stays in
the environment as `context`, and the model programs over it and recurses. Models write Python far
more reliably than an invented language, so the subset is Python's own syntax and meaning; what it
lacks is refused with a message the model can act on.

**The language** (`script.coax`): `None`, `bool`, 64-bit `int` (no floats; `/` floors), `str`
(bytes), `list` and `dict` (shared by reference, insertion-ordered); f-strings, list comprehensions
and generator arguments, `a if c else b`; assignment (also `a, b = ...` and `x[i] = ...`), augmented
assignment, `if`/`elif`/`else`, `for` with one or two names, `while`, `break`, `continue`; the
common builtins (`len str repr int bool abs list dict range enumerate zip sum min max sorted`,
`key=` only as `d.get`) and string, list and dict methods; `re.search` and `grep(pattern, text)`
on `coax/lib/text/regex.coax`'s subset; and the RLM's own: `context`, `files` and `doc(path)` for a
folder, `chunk(text, size)` (cut at line ends), `llm_query`, `llm_query_batched`, `rlm_query`,
`rlm_query_batched`, `FINAL`, `FINAL_VAR`, `SHOW_VARS`. No `def`, `lambda`, `class`, `try`, `with`,
`import` (accepted, does nothing), `open`, `eval`: nothing the model writes reaches a shell, a file,
the network or the clock. Every error (a bad index, a missing name, an integer past 2^62, a step
limit, a full arena) ends the block with a message in the next turn; nothing traps.

**Model calls without re-entry.** The interpreter calls nothing outside itself. A sub-model call
takes its answer from the block's memo; when the memo has none, the call records its prompts and
suspends the block. The driver (`run.coax`) makes the calls (a batch's in flight together, up to
eight), adds the answers to the memo, puts the variables, lists and dicts back as they were when
the block started (a snapshot) and runs the block again. The interpreter is deterministic, so the
block reaches the same calls in the same order and gets their answers; what it did before a call
is never done twice. A block with k sequential calls runs k + 1 times; a batch costs one rerun.
(This also keeps the checker's analysis bounded: an interpreter that called the gateway, and a
driver that recursed, both ran it out of budget.)

**Recursion.** `rlm_query` at a depth below `max_depth` is a child level on the driver's stack,
not a recursive call: the child gets its own variable frame (it sees only its `context`, which is
the prompt), its own messages and the same root model, and its answer goes into the parent's memo.
At `max_depth` (1 by default, as in the paper) `rlm_query` is a plain sub-call.

**Ending.** `FINAL(x)` or `FINAL_VAR(name)` in code ends the run at once; a line `FINAL(...)` or
`FINAL_VAR(...)` in a reply with no code block ends it too (the paper's form). A reply with neither
gets a note. A root reply the provider refuses (Gemini's `MALFORMED_FUNCTION_CALL`) gets a note
and the turn goes again, up to three in a row, as the reference's `max_errors`. After `max_turns`
one closing call asks for `FINAL(...)`.

**Limits** (`policy.coax`, `#authority`, every mutant killed): before every call, root or sub, at
every depth, `rlm_may_call` refuses when the run has made `max_calls` calls, used `max_ms`, or when
what it has spent plus the least the call can cost (its prompt's input tokens at the rates card's
input price) would pass `max_usd_micros`; a model off the card is refused while a cap is set. A
refused sub-call ends its block with the limit's name, the model sees that output, and the run
ends there: a final answer the block already gave stands, otherwise the run has none (a closing
call would pass the limit too).
Per block: `max_steps` interpreter steps; per sub-call: `max_prompt_bytes`; per turn: `max_shown`
bytes of output shown to the root. The arenas the caller passes are the memory limit.

**Models** (`model.coax`): live, each call is a plan on the shared gateway
(`coax/lib/gateway/plan.coax`: retries, stall limits, the rates card's price on every done event);
scripted, each role answers from a list, with usage counted as four bytes a token and priced on
the same card, and a clock that moves a second a call. The checks run whole runs, limits and
recursion on the scripted model.

## RLM code

`code.coax` is a coding agent on the same driver, for Microcoder (GitHub #13325): the
repository is the environment, so file contents never enter the root model's prompt and are
never compacted and read again. The root model sees the task, a short coding prompt and, each
turn, what its blocks printed; a note ends each turn's feedback with the turn, the dollars spent
of the cap and the minutes used. With `env_on`, the language has the workspace's functions
(`script.coax` `RS_ENV`), all confined to `--cwd`:

| Function | What |
| --- | --- |
| `files(prefix="")` | tracked paths (`git ls-files`) |
| `read(path)`, `lines(path, a, b)`, `count_lines(path)` | the text; a range numbered; the line count |
| `search(pattern, prefix="", limit=200, regex=False)` | `git grep -n -I` matches as `"path:line:text"`; a fixed string unless `regex=True` (extended) |
| `edit(path, old, new)` | Microcoder's exact edit: the changed lines numbered, or `edit failed: ...` with the nearest lines |
| `replace_lines(path, a, b, text)` | lines a..b become text (`a = b + 1` inserts); the new lines numbered |
| `write(path, text)` | a whole file |
| `sh(cmd, seconds=120)` | `{"exit", "out", "timed_out"}`, the whole output; in Microcoder's sandbox profile (`--profile`, default `workspace-write`) |
| `outline(path)` | the file's top-level definitions, numbered as `lines` numbers them: in Coax the margin lines with `NAME :: ` (procedures, structs, constants) and `#test`/`#forall`; in Markdown the headings; elsewhere margin lines that start with a defining word, look like a C definition, or set a capitalised constant |
| `rlm_query(brief)`, `rlm_query_batched(briefs)` | at the root, a worker (below); in a worker, a plain sub-call |

**Effects ride the memo.** A call of one of them suspends the block as `llm_query` does
(`RS_ASK_FX`, the driver's phase `L_FX`): the request is the name and arguments joined by zero
bytes. `code.coax` does the effect once, adds its result to the block's memo and the block runs
again from its start, from its snapshot, taking that result from the memo; so nothing is done
twice, and a block with k effects runs k + 1 times. A refused effect (a path outside the
workspace or through a symlink out of it, a write under `.git`, a missing file) ends the block
with the reason. Limits: the run's `policy.coax` limits on model calls, time and money, and
Microcoder's counts (`native-profile.coax`): 512 reads (`files`, `read`, `lines`, `count_lines`,
`search`) and 128 commands and edits a run; past one, the effect is refused and the run stops.

**Workers** (GitHub #13367, #13325). One conversation that carries the whole task grows by a few
thousand tokens a turn, so its cost is quadratic in turns. `--max-depth` is 2 here, so `rlm_query`
at the root starts a worker: a child level on the driver's stack (as above) that is a whole coding
agent, with the same functions on the same workspace, a frame of variables and a conversation of
its own, the coding prompt with a worker's part ("do exactly the part in your brief, run the check
it names, end with FINAL(a short report)") and its brief as its first message, in the root's JSON
schema too. Its effects ride the same memo and rerun as the root's, in its own blocks' memo; its
FINAL text goes into the parent's memo as `rlm_query`'s value, so a rerun of the parent's block
takes it from there and the worker never runs again. The workers of a batch run one after another,
not at once (no two ever touch the workspace together). Every limit is the run's: calls, time,
money, and the 512 reads and 128 commands and edits are shared by the root and all its workers. A
worker a limit stops ends the parent's block at its `rlm_query` ("rlm_query stopped: ..."), and the
run ends there. In a worker `rlm_query` is a plain sub-call: workers start no workers.

**The manager** (`--manager`, off by default): the root plans and delegates. Its prompt
(`RC_MANAGER_SYSTEM`) gives it `files`, `count_lines`, `search`, `outline`, `sh`, `llm_query(_batched)`,
`rlm_query(_batched)` and `FINAL`; `read`, `lines`, `edit`, `replace_lines` and `write` fail at the
root, before any effect, with a note to give them to a worker (script.coax `root_deny`).

**Compiler questions and working memory** (GitHub #13444, #13445). Two flags, each off by
default, so a run with one can be compared with a run without. With neither, the functions,
the prompt, the turn notes and the report line are what they were. Their functions ride the same
request, memo and rerun as the rest (script.coax `env_more`), so none is asked or recorded twice.
Neither goes with `--manager`.

`--intel` (`[--coax PATH]`): the Coax compiler's answers, from `coax/lib/agent/intel/compiler.coax`.
The compiler is `--coax`, else the workspace's own `coax/bin/coax`, else the one of the checkout
the tool runs in.

| Function | Returns |
| --- | --- |
| `defines(name)` | where it is declared: kind, file, lines, the file's hash (16 records at most) |
| `uses(name, file="")` | each use with the procedure or check it is written in (40); `file` picks the declaration |
| `show(name, file="")` | one declaration numbered, under a line `FILE:A-B  KIND NAME  hash H` (12 hex digits) |
| `exports(path)` | what the file declares (80) |
| `check(path)` | the diagnostics as a list of dicts `code`, `line`, `message`, `fix` (20); `[]` is a clean file. A file that fails alone and has a loader is checked through the loader, and a first dict with code `note` says so |
| `explain(code)` | what a diagnostic code means |
| `effects(path, proc="")` | a procedure's may-effects with a call path to each (24) |
| `replace(name, seen_hash, text, file="")` | replaces the declaration through `ix_replace`. A hash that is not the text's: nothing is written, and it returns the text and hash as they are (`replace refused: ...`). Applied: the new lines numbered, their hash and the file's check |

An answer that left records out says how many. When the compiler can't answer (a program with a
tagged value, a file that does not compile, a crash, a run past its time, no compiler) the function
returns `no answer: the compiler could not answer F(ARG): WHY. Use search and lines for this.`;
`check` returns one dict with code `no answer` and that message. The block goes on. A question
counts as a read, `replace` as an edit, against the same 512 and 128. The compiler runs outside the
sandbox profile, as the tool does; `replace` writes through Microcoder's exact edit in the profile.

`--memory` (`[--session ID | --memory-dir DIR]`): the run store of `coax/lib/agent/workmem/`, in
`~/.openagents/coder/sessions/ID.workmem` (a new session each run unless `--session` names one;
a folder made for another workspace is refused; one opened again says what it holds in a line
before the task). The driver records with no action from the model:
`read`, `lines` and `show` as reads, `edit`, `replace_lines`, `write` and `replace` as changes, and
each `sh` as a run (the store looks at its files again first, so a file the command changed is seen).

| Function | Returns |
| --- | --- |
| `note(text, path="", a=0, b=0)` | records a finding with the lines it is about (the whole file when a and b are 0); `noted F3` |
| `known()` | findings, then reads, that are still current (6,000 bytes at most; the view says how many rows it left out) |
| `changed()` | every edit and the lines it wrote |
| `open_items()` | what stops `FINAL` now |

- **Repeat reads.** `lines` and `show` of lines the store already has at the file's current content
  return them under a first line `[already read at turn N: ...]`. `read` returns the file's text
  and nothing else (its value is often written back), and is counted. Each turn's note gains a line
  before it: `Reads this turn: 3; repeats (lines you already had): 2.`
- **The finish rule.** The task's commands are its spans in single backticks. An `sh` command is
  a run of the longest span that is the whole command, or that has a space and is part of it; the
  check's row is made at its first run. The root's `FINAL` is refused, with `wm_view_open`'s lines,
  while `wm_may_finish` says no: a check's latest run failed, or passed before a later edit. After
  two refusals it is accepted, and the report's `unproven` lists what was open. A check the model
  never ran has no row and stops nothing; a worker's `FINAL` is its report and is not held.

The report line gains `intel` (`asked`, `unanswered`, `stale_edits`, each function's calls) and
`memory` (`dir`, `reads`, `repeat_reads`, `finishes_refused`, `checks`, `runs`, `changes`, each
function's calls, `unproven`), each only with its flag. The trace has a `--- compiler: no answer`
line for each question the compiler could not answer and `--- final refused ---` for each refusal.

```sh
coax/web/tool.sh coax/lib/rlm/code.coax run --cwd DIR (--task TEXT | --task-file F) [--manager] [--max-depth 2] [--root-model gemini-4-argon] \
  [--sub-model gemini-3.8-flash] [--root-effort low] [--max-turns 40] [--max-usd 5] [--max-minutes 15] \
  [--profile workspace-write] [--trace F] [--log F] [--intel [--coax PATH]] [--memory [--session ID | --memory-dir DIR]]
coax/web/tool.sh coax/lib/rlm/code.coax selftest      # every function, confinement, reruns, a whole run, limits: a scratch git repo, the scripted model; then --intel and --memory on a scratch Coax repo with the real compiler
```

It prints one JSON line: `ending` (`final`, `limit`, `no answer`), turns, root and sub calls,
effects by function, input, cached and output tokens per role, the card's price, wall seconds and
the `FINAL` text; then `mode` (`manager` or `single`), `root_level` (the root's own calls, by role,
tokens and price) and `workers` (each one's depth, the parent turn that started it, turns, ending,
brief and report sizes, and the same use), which add up to the totals. `--trace` writes every
reply, block output and effect (with its result's size), each worker's indented under the call
that started it, with its brief and a line of its use when it ends; `--log` appends the line. The exit is 0 with a final answer, else 3.

## Files

| File | What |
| --- | --- |
| `policy.coax` | The limits: calls, time, money (floor priced on `coax/lib/gateway/card.coax`), prompt size, depth route. `#authority` |
| `script.coax` | The language: lexer, parser (nodes in the caller's arena), interpreter, builtins, suspension |
| `model.coax` | Root and sub models: gateway plans in up to eight slots, or a scripted model |
| `run.coax` | The driver: prompts, turns, blocks, snapshots and reruns, child levels, FINAL, accounting, the trace; and the checks |
| `rlm.coax` | The command line (below), the OOLONG-style task builder and its scoring |
| `live.coax` | The live models (Microcoder's Vertex credentials, or `GW_REPLAY`) and the limit flags, for both tools |
| `code.coax` | RLM code: the workspace's effects, their confinement and counts, the coding prompt, the run's report, and its self-test |
| `replay.sh`, `fixtures/replay/` | A recorded live run replayed through the gateway's `GW_REPLAY`, nothing sent; fails unless the answer matches |

## Use it

```sh
coax/bin/coax test coax/lib/rlm/run.coax          # the checks (about a second)
coax/bin/coax mutate coax/lib/rlm/policy.coax --root-only
coax/lib/rlm/replay.sh                            # the recorded run over the gateway's replay path
coax/web/tool.sh coax/lib/rlm/rlm.coax ask --dir coax/lib/microcoder --ext .coax \
  --question "Which limits does Microcoder enforce, and where?" --trace /tmp/rlm.trace
coax/web/tool.sh coax/lib/rlm/rlm.coax base --file ctx.txt --question Q --window 96000   # one plain call, cut to fit
coax/web/tool.sh coax/lib/rlm/rlm.coax full --file ctx.txt --question Q                  # one plain call, whole context
coax/web/tool.sh coax/lib/rlm/rlm.coax oolong --data train_5500.label --n 3000 --seed 7 --out DIR
```

Flags: `--root-model` (`gemini-4-argon`), `--sub-model` (`gemini-3.8-flash`), `--root-effort` and
`--sub-effort` (`low`), `--max-depth` (1), `--max-turns` (20), `--max-calls` (400), `--max-usd` (2),
`--max-minutes` (30), `--expect N` (scores the first number in the answer as OOLONG does,
0.75^|error|), `--log FILE` (appends the JSON line), `--trace FILE`. Credentials are Microcoder's
(`coder/tui/microcoder_keys.coax`): the service account `~/.openagents/coder/antigravity.env` names,
project `openagentsgemini`, location `us`; the token stays in curl's environment. `GW_RECORD=DIR`
records a live run's responses; `GW_REPLAY=DIR` answers from them, sending nothing.
