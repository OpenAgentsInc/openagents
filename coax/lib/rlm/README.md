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
gets a note. After `max_turns` one closing call asks for `FINAL(...)`.

**Limits** (`policy.coax`, `#authority`, every mutant killed): before every call, root or sub, at
every depth, `rlm_may_call` refuses when the run has made `max_calls` calls, used `max_ms`, or when
what it has spent plus the least the call can cost (its prompt's input tokens at the rates card's
input price) would pass `max_usd_micros`; a model off the card is refused while a cap is set. A
refused sub-call ends its block with the limit's name; the run then stops with the answer it has.
Per block: `max_steps` interpreter steps; per sub-call: `max_prompt_bytes`; per turn: `max_shown`
bytes of output shown to the root. The arenas the caller passes are the memory limit.

**Models** (`model.coax`): live, each call is a plan on the shared gateway
(`coax/lib/gateway/plan.coax`: retries, stall limits, the rates card's price on every done event);
scripted, each role answers from a list, with usage counted as four bytes a token and priced on
the same card, and a clock that moves a second a call. The checks run whole runs, limits and
recursion on the scripted model.

## Files

| File | What |
| --- | --- |
| `policy.coax` | The limits: calls, time, money (floor priced on `coax/lib/gateway/card.coax`), prompt size, depth route. `#authority` |
| `script.coax` | The language: lexer, parser (nodes in the caller's arena), interpreter, builtins, suspension |
| `model.coax` | Root and sub models: gateway plans in up to eight slots, or a scripted model |
| `run.coax` | The driver: prompts, turns, blocks, snapshots and reruns, child levels, FINAL, accounting, the trace; and the checks |
| `rlm.coax` | The command line (below), the OOLONG-style task builder and its scoring |

## Use it

```sh
coax/bin/coax test coax/lib/rlm/run.coax          # the checks (about a second)
coax/bin/coax mutate coax/lib/rlm/policy.coax --root-only
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
