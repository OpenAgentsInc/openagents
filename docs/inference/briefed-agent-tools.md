# Tools for the briefed agent: candidates and how to test them

Status: proposal, 2026-10-10. Companion to #11211 (briefed agent vs bare
Claude Code) and #11210 (context finder). The tools are built as in-process
tools on `crates/claude_agent_sdk` (`SdkMcpServer`, #11213).

The question: **which set of tools turns an issue into an accepted pull request
at the lowest cost?** This doc lists every tool worth trying, says what each
one takes and returns, why it might help, and what result would keep it in or
throw it out. Every one is an experiment, not a decision.

## The metric

- **Main:** cost per accepted PR = total dollars ÷ PRs that compile, pass the
  fix commit's own tests, and pass the blind judge. A cheap failure is not a
  saving.
- **Second:** median wall time to an accepted PR, and the spread across 3 runs
  of the same issue (reliability).
- **Per tool, logged on every call:** calls per run, share of runs that used
  it, tokens in and out per call, wall time per call, and whether the agent
  acted on the result (its next edit touched a file the result named).

A tool always costs something even when unused: its description is sent on
every turn. A 300-token description over 40 turns is 12k input tokens, which
caching cuts by a lot but not to zero. So a tool must pay for its description,
not only for its calls.

## Baseline arms

| Arm | Briefing | Tools |
|---|---|---|
| **A** bare | none, just "complete this issue" and the issue text | Claude Code defaults (Bash, Read, Edit, Write, Grep, Glob, Task, Web*, TodoWrite) |
| **B0** briefed minimal | briefing in the system prompt | Read, Edit, Write, Grep, Glob, **verify** |
| **B-bash** | briefing | Read, Edit, Write, Grep, Glob, Bash (no verify) |
| **C** oracle | briefing built from the real fix's files | same as B0 |

Every candidate below is tried as **B0 + that tool** (or B0 with a tool swapped
out), against the same issues, 3 runs each.

---

## Tier 1: try first

### 1. `verify`: the finish line

Runs the issue's own checks on the current working copy, deterministically, and
returns only what's wrong.

**Input**
```json
{ "scope": "auto | crate:<name> | all_touched", "tests": "auto | none | <filter>", "fmt": true }
```
- `scope: auto` takes the briefing's crates plus any crate the agent has
  edited.
- `tests: auto` runs the touched crates' tests, the fix-related tests first.

**Output**
```json
{
  "status": "pass | fail",
  "compile": [{ "file": "...", "line": 88, "code": "E0063", "msg": "missing field `cost`" }],
  "tests": [{ "name": "...", "file": "...", "line": 41, "assert": "left 2, right 1" }],
  "fmt": "ok | changed: [files]",
  "untouched_but_implicated": [{ "file": "...", "why": "co-changes with a.rs in 14/16 commits" }],
  "done_when": [{ "item": "Stop posts one result", "check": "fleet::stop_posts_once", "ok": false }],
  "elapsed_s": 21.4
}
```

**Behaviour:**
- `cargo check` runs before tests.
- One shared warm target dir.
- At most 20 errors returned, deduplicated by code and location.
- fmt is applied rather than reported, so it never costs the model a turn.

**Why it might win:**
- Raw `cargo test` output through Bash is the biggest token sink in bare
  runs.
- A stable shape lets the agent fix errors in one pass.
- `done_when` gives it a stopping rule.
- With `verify`, Bash can be turned off.

**Keep it if:** B0 beats B-bash on cost per accepted PR by ≥15%, or on
success rate by ≥10 points, without a slower median.

**Drop or rework it if:** agents call it more than ~6 times a run (it's being
used as a slow `cargo check` loop; consider `check_fast` below), or if
`untouched_but_implicated` is followed less than 30% of the time when shown
(cut that field).

### 2. `related`: the context finder on demand

Answers "what else goes with this?" mid-edit, from the #11210 indexes.

**Input**
```json
{ "path": "crates/x/src/a.rs", "symbol": "Board::stop", "kinds": ["callers", "co_change", "registry", "tests", "docs"], "limit": 10 }
```
- Either `path` or `symbol` is required.

**Output:** ranked `[{ "file", "kind", "why", "confidence" }]`.

**Why it might win:** misses that `verify` can't catch, such as docs, a
registry line, or a test that should exist but doesn't, are found before the
agent thinks it's done.

**Keep it if:** it lowers the "files opened outside the briefing" count, or the
judge's "incomplete" rate, and its calls cost less than the turns they save.
**Drop it if:** it's used in under 20% of runs (the briefing already covers it),
or results are acted on less than 25% of the time.

### 3. `outline` and `read_symbol`: read less

- **`outline`**: `{ "path": "..." }` returns signatures, structs, enums and
  impl headers with line numbers, and no bodies.
- **`read_symbol`**: `{ "path": "...", "symbol": "Board::stop", "context_lines": 3 }`
  returns just that item, with its doc comment.

**Why it might win:** agents read whole 2,000-line files to change 10 lines.
Reading tokens is the second-biggest cost after build output.

**Keep it if:** input tokens per run drop ≥20% with no fall in success. **Drop
it if:** agents call `outline` and then Read the whole file anyway in most
cases.

### 4. `finish`: an explicit "done"

`{ "summary": "...", "risk": "low | medium | high" }`. This runs `verify`
once more. If it passes, it writes the PR description and ends the run. If
it fails, it returns the failures, and the run continues.

**Why it might win:** bare agents keep going after the work is done (extra
reviews, re-reads, summaries). A hard stop removes tail turns, and the summary
feeds the PR body for free.

**Keep it if:** median turns after the last code change fall by ≥2 and the
judge doesn't mark more PRs incomplete. **Drop it if:** agents call it early and
loop (finish, fail, finish…) more than once per run on average.

---

## Tier 2: try after tier 1 is settled

### 5. `apply_patch`: one multi-file change

**Input:** a unified diff, or Codex-style `*** Begin Patch`. **Output:** the
files changed and the hunks that failed, with the surrounding lines.

**Why it might win:** one turn instead of five Edit calls for a cross-file
change. **Risk:** context lines that don't match cause failed hunks and retries.
**Keep it if:** turns per run drop and the failed-hunk rate stays under 10%.

### 6. `find_symbol`: definitions and references

**Input:** `{ "symbol": "...", "want": "definition | references | impls", "limit": 30 }`.
It's backed by rust-analyzer when one is warm, and otherwise by a ctags-style
index built from git.

**Why it might win:** Grep finds text, not meaning. Renames and new struct
fields need every real use. **Keep it if:** Grep calls fall and compile-error
loops shorten. **Drop it if:** rust-analyzer start-up makes it slower than Grep
in practice.

### 7. `example_change`: how we did this last time

**Input:** `{ "like": "add a field to a synced row and show it on Settings", "limit": 2 }`.
**Output:** the best matching past commits from history, each with its message,
the files it changed, and a trimmed diff.

**Why it might win:** our repo has strong conventions (bank answers, routes,
CSS budget, page tests). One good example often replaces many reads.
**Keep it if:** the judge's style and convention scores rise, or reworks after
review fall. **Drop it if:** the briefing's built-in example already does the
job, so the extra call adds nothing.

### 8. Codemods: deterministic edits the model shouldn't type

These are small, exact tools, each worth testing only if the matching kind of
issue is common:
- `rename_symbol { from, to, scope }`;
- `add_field { struct, field, type, default }`, which adds the field and fills
  every initializer the compiler names;
- `register { kind: "mod | route | tool | page_test | bank_entry", name, target }`,
  which adds the line in the right registry in the right order.

**Why they might win:** they are mechanical and error-prone for a model,
trivial for code. **Keep a codemod if** it is used in ≥10% of runs and those
runs have fewer compile loops. **Drop one if** it is rarely chosen.

### 9. `rules`: the repo rules that apply here

**Input:** `{ "paths": ["..."] }`. **Output:** only the AGENTS.md / INVARIANTS.md
/ promises lines that cover those paths.

**Why it might win:** the briefing can't know every path the agent will touch,
and rules broken in review are the costliest kind of failure. **Keep it if:**
judge rule violations fall. **Drop it if:** the briefing's rules section
already brings violations to near zero.

---

## Tier 3: speculative

### 10. `decide`: a cheap yes/no from Jev or Clef

**Input:** `{ "question", "options": [...], "context" }`. **Output:** a
probability for each option.

This is for small judgment calls such as "is this test failure flaky or
caused by my change?" or "is this file in scope?". It's cheap (cents per
thousand calls), but the agent model usually has the context already. **Keep
it** only if a clear class of question appears where it changes the outcome.

### 11. `self_review`: a second look before finishing

**Input:** `{ "focus": "completeness | rules | tests" }`. A different, cheaper
model reads the diff against the issue and the briefing, and returns a short
list of concerns.

**Why it might win:** it catches incomplete work, which `verify` can't.
**Risk:** false alarms cause churn. **Keep it if:** judge "incomplete" falls by
more than the tool costs, and acted-on concerns are right more than 60% of the
time.

### 12. `record_miss`: feed the finder

**Input:** `{ "file", "why" }`. It costs almost nothing and changes nothing in
the run. It turns the agent's discoveries into training data for #11210.
Better still, log misses automatically from Read and Edit outside the briefing,
and drop the tool. **Keep it** only if automatic logging misses the reasons.

### 13. `brief_more`: load briefing sections on demand

**Input:** `{ "section": "plan | files | examples | rules | checks", "file": "..." }`.
This is the opposite of putting the whole briefing in the prompt: start small,
fetch on demand. It trades prompt caching for fewer tokens per turn. **Keep it
if** issues with big briefings get cheaper and small ones don't get slower.

### 14. `run_allowed`: a narrow command line

**Input:** `{ "cmd": "one of the briefing's allowed commands", "args": [...] }`.
It's for issues that need a non-cargo step (a script, a codegen), where Bash
would otherwise come back. **Keep it** if B0 fails a class of issue only
because it can't run one specific command.

### 15. `ask`: a question instead of a guess

**Input:** `{ "question" }`. In the harness it records the question and ends
the run as "needs clarification", which counts as neither success nor failure.
Ambiguous issues are a real share of failures. A clean "I'd need X" may be
worth more than a wrong PR. **Keep it** if the judge agrees the question was
necessary in most cases where it was used.

---

## Built-in tools to test removing

| Built-in | Hypothesis | Signal |
|---|---|---|
| Bash | Replaced by `verify` (+ `run_allowed`) | B0 vs B-bash |
| Task (subagents) | Adds cost; rarely needed for a scoped issue | Success unchanged with it off |
| WebFetch / WebSearch | Repo issues rarely need the web | Off unless the issue's kind needs it |
| TodoWrite | Noise for small issues; may help large ones | Compare by issue size |
| Write | Edit + `apply_patch` cover most; Write needed for new files | Keep; check for misuse |

## How to run the experiments

1. **Tier 1, one at a time:** B0, then B0 + `related`, then B0 + `outline`/`read_symbol`, then
   B0 + `finish`, each against B-bash and A. Use about 20 closed issues × 3
   runs, the same issues and seeds for every arm, compared pair by pair per
   issue, with bootstrap confidence intervals on cost per accepted PR.
2. **Keep the winners:** the new baseline is B1 = B0 + the tier 1 tools that
   passed their keep rule.
3. **Tier 2, one at a time on B1.** Codemods are only tried on issues of the
   matching kind, picked by the triage classifier.
4. **Combine:** a small fractional factorial over the surviving tools, to catch
   pairs that only help together (likely `related` × `example_change`).
5. **By issue kind:** report results per kind (bug, feature, docs, refactor,
   config) and per size. The best tool set will likely differ, and the triage
   step can then pick the set per issue.
6. **Write it down:** every run records its full tool set and settings, so any
   table here can be recomputed. Results go in `briefed-agent-ab.md`.

## Guesses to check

Written before the data, so they can be proved wrong:
- `verify` replacing Bash is the largest single gain: 20–35% cheaper per
  accepted PR, mostly from smaller tool output and fewer turns.
- `outline`/`read_symbol` is next: 10–20% fewer input tokens.
- `finish` is a small but reliable gain: 1–3 fewer turns.
- `related` helps only on issues where the briefing missed something, so it's
  worth more while the finder is weak and less once it is strong.
- `self_review`, `decide` and `brief_more` won't pay for themselves on small
  issues.

### What the data says so far (2026-10-10)

From [briefed-agent-ab.md](briefed-agent-ab.md). The frozen tool ablation
(`plans/ablate-tools.json`: B0 against Bbash, Brelated, Boutline and
Bfinish on S2's 21 issues) is running; each line below is updated from it.

- **Briefing plus minimal tools plus `verify` vs bare Claude Code:
  confirmed large.** S2: 53.9% lower cost per accepted change (CI 44.3% to
  60.4%) and a median of 121 s against 284 s, at the same observed success.
  Most of the saving comes from a smaller context, not from fewer turns:
  bare runs read 1.02M cached tokens a run against B0's 0.46M (Claude Code's
  own prompt and tool definitions, plus about 23 Bash outputs of 650 tokens
  each), while B0 takes more turns (23 vs 15).
- **`verify` replacing Bash is the largest single gain: not supported yet.**
  In the exploratory round before the #11229 fix, briefed-with-Bash cost no
  more than briefed-with-`verify` ($15.82 vs $19.62 over 33 and 34 trials;
  medians $0.50 vs $0.42) and passed the fix's tests as often (13 vs 12).
  So the gain is the briefing and the lean prompt, not `verify` by itself.
  The frozen B0-vs-Bbash comparison will settle it.
- **`verify` usage:** 1.5 calls a run, about 200 tokens out per call, a
  median 39 s each; it is not being looped as a slow check, so its
  drop-or-rework rule (over about 6 calls a run) is not triggered.
- **`outline`/`read_symbol`, `finish`, `related`:** untested so far; in
  the running ablation.
- **`self_review`, `decide`, `brief_more`:** not tried.
