# Working memory for coding agents

A run store and an evidence record (GitHub #13444 parts 1 and 4, #13445
step 4). A coding agent's loop writes down what it read, learned, changed and
ran. Nothing is dropped. The loop shows the model the part that matters each
step, and the run may finish only when the record proves the task is done.

It is one library for both loops: Microcoder's JSON-action loop
(`coax/lib/microcoder/`) and the RLM coder (`coax/lib/rlm/code.coax`). See
[Wiring](#wiring) for the calls a loop makes.

| File | What | Effects |
| --- | --- | --- |
| `finish.coax` | The finish rule: may the run stop? | none (`#authority`) |
| `store.coax` | The rows, the log's one writer and one reader, staleness, the tree digest | none |
| `view.coax` | The views for the model, each inside a byte budget | none |
| `weak.coax` | A file's checks, asserts, `#authority` line and whether it is a compatibility baseline, counted from its text | none |
| `task.coax` | The checks a task names: its commands in single backticks | none |
| `file.coax` | The store's folder on disk: the log, the texts, each file's baseline | `coax/lib/sys/fs.coax` |
| `git.coax` | Baselines from git, for a loop that runs in a checkout | `git`, through `coax/lib/sys/proc.coax` |
| `workmem.coax` | The command line: `show`, `replay`, `profile`, `selftest` | files, one child process |

## What the store holds

| Row | Fields | Written by |
| --- | --- | --- |
| file | path, the content's SHA-256, bytes, lines | `wm_put_file`, `wmf_observe` |
| read | path, the content's hash, first and last line | `wm_put_read`, `wmf_read` |
| finding | a short note; `verified` (it names the run row that showed it) or a guess; sources (path, lines, hash) | `wm_put_finding`, `wmf_finding`, `wmf_finding_at` |
| change | path, the hashes before and after, the lines of the new content the edit wrote | `wm_put_change`, `wmf_change` |
| run | command, the check it is a run of, exit, timed out, milliseconds, the output's hash and size | `wm_put_run`, `wmf_run` |
| check | a check the task names: name and command | `wm_put_check`, `wmf_check` |
| requirement | text; status open, done or failed; the evidence rows that support it | `wm_put_requirement`, `wm_put_status` |

A text the store names by hash (a file's content at one version, a command's
whole output) is kept once, in `text/`. The same range of the same content is
one read row however often it is read: `uses` counts the reads.

### Staleness

Staleness is never stored. It follows from the hashes:

- A read is stale when its file no longer has the content it read.
- A finding is stale when the file of any of its sources has changed since.

Stale rows stay in the store and leave the "current" view. If the file comes
back to the old content, they are current again. A call that changes a file's
content (`wm_put_file`, `wm_put_change`, `wmf_observe`, `wmf_change`,
`wmf_refresh`) returns `stale_reads` and `stale_findings`, the number of rows
it made stale, and `file`; `wm_view_stale(s, file, ...)` names them.

### The tree digest

`wm_tree(s)` is the SHA-256 over the path and current hash of every file whose
content the run has seen change. A run row keeps the digest it ran on. An edit
after the run changes the digest, so a check that passed before the edit has
not passed on the tree as it is now. A file that changes and changes back
still counts as changed; the check must run again.

## The finish rule

`wmf_may_finish(w, reasons)` gives `WfVerdict{ok, count}`. `ok` is true only when:

- every requirement is `done` and at least one of its evidence rows still
  holds;
- every check the task names has run, and its latest run exited 0, inside its
  time, on the tree as it is now; and
- the run weakened no file that judges it (below).

`wm_may_finish(s, reasons)` is the same rule without the third part, for a
caller that has only the store.

An evidence row holds when it is:

- a run that exited 0, did not time out, and ran on the current tree;
- a finding that is verified and not stale; or
- a change whose text is still in the file.

A requirement marked done on the model's word alone does not hold. With no
requirements and no checks there is nothing to prove, and the answer is yes: a
loop that wants proof for every task adds the rows first.

Each reason has a code and a row: `WF_REQ_OPEN`, `WF_REQ_FAILED`,
`WF_REQ_UNPROVEN` (the requirement's row), `WF_CHECK_NEVER`,
`WF_CHECK_FAILING`, `WF_CHECK_STALE` (the check's row), and `WF_WEAK_BASELINE`,
`WF_WEAK_AUTHORITY`, `WF_WEAK_CHECKS`, `WF_WEAK_ASSERTS` (the file's row;
`wf_weakened(why)` is true for these four). `wmf_view_open` writes them as
lines for the model; `wm_view_open` writes the first six.

The rule is `finish.coax`, `#authority`, with zero surviving mutants (118 of
118 killed). It holds no store and reads no file: `store.coax` copies its rows
into the rule's facts, and `file.coax` counts each file, without judging them.

### A run may not weaken what judges it

An agent may not weaken a check, an `#authority` line or a compatibility
baseline to pass (#13445 step 4). For each file whose content is not what it
was when the run started, the rule is given what the file had then and has
now (`WfFile`). The file is a reason, one a file, the first of:

| Reason | When |
| --- | --- |
| `WF_WEAK_BASELINE` | The file was a compatibility baseline. Any change to one, or its removal |
| `WF_WEAK_AUTHORITY` | It had an `#authority` line and has none |
| `WF_WEAK_CHECKS` | It has fewer checks (`#test`, `#forall`) than it had |
| `WF_WEAK_ASSERTS` | It has fewer `#assert` than it had |

A file that is gone has no checks, no asserts and no line, so deleting a file
with checks is `WF_WEAK_CHECKS`. A file the run did not change is never a
reason. Adding checks is not one either, and a new file has nothing to lose.

The rule does not allow such a change on the model's word. A loop refuses the
finish a set number of times and then reports the reason to a person. The RLM
coder refuses twice, then accepts the answer with the line in its report's
`unproven`.

**What is counted** (`weak.coax`). The counts are of the file's text, in
`.coax` files only: each `#test`, `#forall` and `#assert` written outside a
comment, and a line that starts with `#authority`. A check that is commented
out is one check fewer. A `#test` inside a string (a fixture) counts as one.
Both sides are counted the same way, so only a difference matters. The
compiler is not asked: it names a file's checks only by running them, and a
baseline text is not a file on disk.

**A compatibility baseline** is a surface file: what `coax compat --emit
MODULE.coax` prints, kept so a later module can be compared against it
(`coax compat OLD.surface NEW.coax --baseline BASE.surface`,
[coax/checked-state.md](../../../checked-state.md)). It is known by its first
line, `coax.compat.v1`, whatever its name. This tree has two:
`coax/web/compat/everglade-wire.surface` and `character-checkpoint.surface`.

**A file's baseline** is its content when the run started, not when the store
first saw it: a shell command (`sed -i`) can change a file the run never
read. Baselines belong to the process and are not in the log, so
`openagents.workmem.v1` is unchanged.

- In a git checkout, `git.coax` gives them. `wmg_start` records the commit
  the workspace is at and, for each file that already differs from it, the
  file's content now. `wmg_refresh` asks `git diff --name-status START` and
  gives each file it reports the content `git show START:PATH` has (a file
  added since has none).
- Otherwise a file's baseline is the first content this process knew for it:
  what the folder held when it was opened, or the first content the loop gave.
- `wmf_baseline(w, path, text)` sets one directly.

Not seen: a file git does not track that a command changed before the store
saw it; a file over 4 MB; a path git quotes (a quote, a backslash or a control
byte in its name). `WmGit.skipped` counts the last two. A check moved from one
file to another is a reason for the file it left.

## The views

Each view is plain text in the caller's buffer, never longer than the budget
in bytes, and the same bytes for the same store.

| View | Shows |
| --- | --- |
| `wm_view_known(s, budget, out)` | Current findings, then current reads |
| `wm_view_findings`, `wm_view_reads` | Each of those sections alone |
| `wm_view_changed(s, budget, out)` | Every edit and the lines it wrote |
| `wm_view_open(s, budget, out)` | Each requirement and check that stops the run finishing, and why; empty when it may finish |
| `wmf_view_open(w, budget, out)` | The same, then each file the run weakened, with what it had then and has now (`file.coax`) |
| `wm_view_stale(s, file, budget, out)` | The reads and findings that are stale because that file changed |

```
Findings (current):
F1 guess: b has two lines [sub/b.txt:1-2]
F2 verified by run 1: line two is upper case now [a.txt:2-2]
Reads (current):
R2 sub/b.txt 1-2
```

A row is one line, shown whole or not at all. When a section's rows do not all
fit, the ones shown have the highest score, and a last line says how many were
left out (`[+5 more not shown]`). A finding's or an edit's score is how new it
is. A read's score is how new its latest use is, and each further use counts
as eight lines newer.

Rows are always in the order they were made, oldest first, and a row's text
never changes. So while everything fits, a new row only adds a line at the end
of its section, and a prompt cache serves everything before it. To keep a
whole prompt append-only, place `wm_view_findings` and `wm_view_reads` apart.
A view's start changes only when a row goes stale or the budget forces rows
out.

## On disk

One folder per run. For a Coder session it is
`~/.openagents/coder/sessions/ID.workmem` (`wmf_dir`).

```
DIR/log.jsonl     the store's lines, appended one at a time
DIR/text/SHA256   each text the store names, once
```

The log is schema `openagents.workmem.v1`, registered in
`proto/registry.json`. Its fixture is `proto/coder/workmem-v1.json`, and
`store.coax` is its one writer and one reader. Each line is one compact JSON
object with its keys in this order. A part in `[ ]` is left out when it does
not apply.

```
{"kind":"workmem","schema":"openagents.workmem.v1","run":ID,"root":DIR}
{"kind":"file","path":P,"sha256":H,"bytes":N,"lines":N}
{"kind":"read","path":P,"sha256":H,"from":A,"to":B}
{"kind":"finding","text":T,"verified":B[,"run":N],"sources":[{"path":P,"from":A,"to":B,"sha256":H}]}
{"kind":"change","path":P,"before":H,"after":H,"from":A,"to":B,"bytes":N,"lines":N}
{"kind":"run","command":C[,"check":NAME],"exit":N,"timed_out":B,"ms":N,"output":H,"bytes":N}
{"kind":"check","name":NAME,"command":C}
{"kind":"requirement","text":T}
{"kind":"status","requirement":N,"status":"open|done|failed","evidence":[{"kind":"run|finding|change","id":N}]}
```

- The `workmem` line is first, once. `root` is the workspace's folder; paths
  are relative to it.
- `H` is a SHA-256 as 64 lowercase hex digits. Lines are numbered from 1, and
  `from` and `to` are both included.
- A row's number is its place among the rows of its kind, from 0. `run`,
  `requirement` and an evidence `id` are row numbers.
- A `change` with `to` one less than `from` only removed lines.
- A `status` line replaces the requirement's earlier evidence list.

The rows in memory are what applying the lines in order makes, so a folder
opened again (`wmf_open`) is the store that wrote it. The store refuses a line
that is not one of these, names a row that is not there, or does not fit, and
the line changes nothing. A last line that a crash cut short is cut off when
the folder opens.

Limits per run: 1,024 files, 4,096 read rows, 1,024 findings, 2,048 changes,
2,048 runs, 128 requirements, 32 checks, 1 MiB of paths and notes, and 32 KiB
a line. A put past a limit returns `ok` false.

## The command line

```sh
coax/web/tool.sh coax/lib/agent/workmem/workmem.coax show DIR [--budget N]
coax/web/tool.sh coax/lib/agent/workmem/workmem.coax replay TRACE...
coax/web/tool.sh coax/lib/agent/workmem/workmem.coax profile proto/coder/workmem-v1.json
coax/web/tool.sh coax/lib/agent/workmem/workmem.coax selftest
```

- `show` prints a store's counts, its views at N bytes each (default 8192),
  its runs and requirements, and whether the run may finish.
- `replay` puts an RLM coder trace's effects (`--- effect read(...) -> N
  bytes`) through a store and reports how many reads the store already had.
- `profile` holds the codec to the fixture.
- `selftest` runs the store on a scratch tree with real files and a real
  command, opens the folder again, then weakens files outside a checkout and
  in one, and prints `workmem selftest: ok`.

## Checks

```sh
coax/bin/coax test coax/lib/agent/workmem/workmem.coax    # every check of the eight files
coax/bin/coax mutate coax/lib/agent/workmem/finish.coax   # the rule: zero survivors
coax/web/tool.sh coax/lib/agent/workmem/workmem.coax selftest
coax/web/tool.sh coax/lib/agent/workmem/workmem.coax profile proto/coder/workmem-v1.json
```

## Repeat reads on two recorded runs

Two failing RLM coder runs of task N4 (2026-11-13), replayed. A read counts as
answered when the store already had every line of its range at the file's
current content. The traces have no file contents, so every `edit` counts as
a change to its file, also one that failed; the real share is this or higher.

| Run | Reads | Answered by the store | Bytes read | Bytes the store had |
| --- | ---: | ---: | ---: | ---: |
| `n4-b` | 58 | 40 (68%) | 671,144 | 383,474 (57%) |
| `n4-c` | 61 | 26 (42%) | 492,723 | 169,550 (34%) |
| Both | 119 | 66 (55%) | 1,163,867 | 553,024 (47%) |

This is a count over recorded runs, not a measured gain: no run has used the
store yet.

## Wiring

The loop owns the workspace and already holds each text it reads, writes or
runs. It gives them to `file.coax`:

| When the loop | It calls | And uses |
| --- | --- | --- |
| starts a run | `wmf_open(w, dir, run, root)`, then `wmg_start(w, g)`; `wmf_requirement`, `wmf_check` for the task's rows | `wmg_start` is false outside a checkout, and nothing else changes |
| reads a file or a range | `wmf_read(w, path, whole_text, from, to)` | `known`: the model already had these lines |
| wants to show lines again | `wmf_recall(w, path, from, to, out)` | the lines from the store, with no read; `""` when they are not current |
| applies an edit | `wmf_change(w, path, before_text, after_text)` | the row's `from` and `to`; `stale_reads`, `stale_findings`, `wm_view_stale` |
| runs a command | `wmf_run(w, command, check, exit, timed_out, ms, output)`, then `wmf_refresh(w)` | the run row's number, for a verified finding or as evidence |
| records a note | `wmf_finding_at(w, text, run, path, from, to)`; `run` is -1 for a guess | |
| marks a requirement | `wmf_status(w, req, WF_DONE, evidence)` | |
| builds a prompt | `wm_view_known`, `wm_view_changed`, `wmf_view_open` | |
| is asked to finish | `wmg_refresh(w, g)`, then `wmf_may_finish(w, reasons)` | refuse with `wmf_view_open(w, budget, out)` when `ok` is false; `wf_weakened(reasons[i].why)` says a reason is a weakened file |
| ends | `wmg_refresh(w, g)`, then `wmf_view_open(w, budget, out)` | what is still open goes in the run's report, for a person |

`w` is a `[1] Wmf` (about 6 MB) and `g` is a `[1] WmGit` (about 5 MB): keep
both in the loop's memory struct, not on a small stack. `s` is `w[0].s[..]`.
`wmg_refresh` is `wmf_refresh` after the baselines, so it replaces that call
before the rule is asked; after a shell command `wmf_refresh` is enough. A
loop that loads `git.coax` gets `file.coax` with it.

The weakened rule is these three calls: `wmg_start(w, g)` after `wmf_open`,
`wmg_refresh(w, g)` before the rule is asked, and `wmf_may_finish` and
`wmf_view_open` in place of `wm_may_finish` and `wm_view_open`. The old calls
still work and give the rule without it.

The RLM coder is wired (`coax/lib/rlm/README.md`, "Compiler questions and
working memory"): `read`, `lines` and `show` call `wmf_read`; `edit`,
`replace_lines`, `write` and `replace` call `wmf_change`; `sh` calls
`wmf_run`; `note`, `known`, `changed` and `open_items` reach the findings
and views; the root's `FINAL` meets `wmf_may_finish`.
For Microcoder: the state rendered each step (`state.coax`) gains the views,
and `finished: true` is refused while `wmf_may_finish` says no.
