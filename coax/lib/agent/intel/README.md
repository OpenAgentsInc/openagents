# Compiler questions for coding agents

A Coax library that lets an agent ask the Coax compiler about code, and edit
code by the hash of what it read (GitHub #13445 steps 1 to 3 and proposal 1,
part of #13444). It replaces "grep harder": the answers are the compiler's.

It holds no parser and no index. It runs `coax query`, `coax check`,
`coax deps`, `coax explain` and `coax vocab`, reads their JSON, and returns
typed records with paths relative to the root. `check` and `deps` run in the
resident compiler (`coax serve`), because the compiler sends them there
itself. `query` keeps its own index under `~/.cache/coax-query`.

| File | What |
| --- | --- |
| `answers.coax` | The records, the compiler's JSON read into them, hashes, and each answer as text. No effects |
| `compiler.coax` | Runs the compiler: the session (`Ix`), the questions, entry-file resolution, `ix_replace` |
| `intel.coax` | The command-line tool, the native check (`selftest`) and the timings (`measure`) |

## Use it from Coax

```coax
#load "../agent/intel/compiler.coax";

s: [1] Ix;                                   // about 50 MB: keep one for a run, never copy it
if ix_open(s[..], "/path/to/repo", "") {     // "": ROOT/coax/bin/coax, else $COAX, else coax on PATH
  decls: [8] IxDecl;
  a := ix_defines(s[..], "rp_put", decls[..]);          // a.n records, a.ms, s[0].version
  text: [8192] u8; diags: [16] IxDiag;
  shown := ix_show(s[..], "rp_lines", "", text[..]);    // shown.text (numbered), shown.hash
  done := ix_replace(s[..], IxEdit.{name = "rp_lines", seen = shown.hash, text = "..."}, text[..], diags[..]);
}
```

| Question | Signature | From |
| --- | --- | --- |
| Where is NAME declared | `ix_defines(s, name, out: [] IxDecl) -> IxAnswer` | `coax query ENTRY defines NAME --json` |
| Who uses NAME | `ix_uses_of(s, name, only, defs: [] IxDecl, out: [] IxUse) -> IxAnswer` | `coax query ENTRY uses NAME --json`, `coax deps ENTRY` |
| What a file declares | `ix_exports(s, file, out: [] IxDecl) -> IxAnswer` | `coax query FILE exports --json` |
| One declaration's text | `ix_show(s, name, file, out: [] u8) -> IxShown` | `defines`, then the file |
| Lines of a file | `ix_show_lines(s, file, from, to, out: [] u8) -> IxShown` | the file |
| A file's diagnostics | `ix_check(s, file, out: [] IxDiag) -> IxAnswer` | `coax check FILE --diagnostics json` |
| What a code means | `ix_explain(s, code, out: [] u8) -> IxText` | `coax explain CODE` |
| What a name is | `ix_vocab(s, name, out: [] u8) -> IxText` | `coax vocab NAME` |
| A procedure's effects | `ix_effects_of(s, file, proc, out: [] IxEffect) -> IxAnswer` | `coax deps FILE --effects --proc NAME` |
| What a file depends on | `ix_depends_on(s, file, out: [] IxDecl) -> IxAnswer` | `coax deps FILE --declarations` |
| The files of its program | `ix_loads(s, file, out: [] string) -> IxAnswer` | `coax deps FILE` |
| The files that load it | `ix_loaders(s, file, out: [] string) -> IxAnswer` | a text search, each confirmed by `coax deps` |
| Replace what was read | `ix_replace(s, e: IxEdit, out: [] u8, diags: [] IxDiag) -> IxReplaced` | `mc_edit_file`, then `coax check` |

`s` is `[] Ix`. Session options are fields: `entries` (compiler entries one
question may ask, 12), `timeout_ms` (one compiler run, 120,000), `run_steps`
(`--run-steps` for every run), `profile` (`"read-only"` refuses edits) and
`ix_env(s, "NAME=value")` for the compiler's environment.

Every answer is bounded by the arrays and buffers the caller passes: `n`
records were written of `total`, and `cut` says something was left out. Every
answer has `ms`, and `s[0].version` is the compiler that gave it. An answer's
strings last until the next question on the same `Ix`. `ix_decls_text`,
`ix_uses_text`, `ix_diags_text`, `ix_effects_text` and `ix_head` write an
answer as bounded text for a model.

### The records

- `IxDecl`: `kind`, `name`, `file`, `line`, `end_line`, `column`, `exported`,
  `generated`, and `hash`, the SHA-256 of the file's content.
- `IxUse`: `file`, `line`, `column`, `from` (the procedure or check it is
  written in), `kind`, `hash`, and `to_file` and `to_line`, the declaration it
  resolves to.
- `IxDiag`: `severity`, `code`, `message`, `file`, `line`, `column`, `offset`,
  `length`, `notes` and `fixes` (text, a line each).
- `IxEffect`: `proc`, `kind`, where the effect is, and `path`, one call path
  that reaches it.

### Finding the entry file

`coax query FILE ...` needs an entry file, and an agent often has only a
name. The library finds the entries:

- **`defines`**: a text search (git grep, or grep outside a repository) lists
  the files with a line that starts `NAME :`. Each is asked as its own entry.
- **`uses`**: the files that name NAME as a word are the candidates. Each one
  that no earlier run covered is asked as an entry. `coax deps` of that entry
  says which other candidates its program holds, and those aren't asked again.
- **A file that doesn't check alone** (it uses names from the unit that loads
  it) is asked through the files with a `#load` whose path, from their own
  folder, is that file.

The search only chooses which files to hand the compiler. No answer comes
from it. A question asks at most `entries` entries, and the answer counts the
candidate files it didn't reach in `unchecked`.

Separate programs in one repository can each declare the same name. This
repository has three `rp_put`. `defines` returns all of them, and `ix_uses_of`
takes `only`, the file of the declaration you mean.

### Edits tied to what was read

`ix_replace` replaces a declaration (by name) or lines of a file. `seen` is
the hash the caller read: `IxShown.hash` (those lines) or `.file_hash` (the
whole file), in full or its first 12 hex digits or more.

- When the hash is no longer the text's, nothing changes. The answer has
  `stale` set and returns the text as it is now, with its hash.
- An applied edit goes through Microcoder's exact edit (`mc_edit_file`). The
  answer returns the new lines numbered, their hash, the file's hash and the
  file's `check` records. `IxEdit.check` names another file to check, for a
  part that checks only through its loader.
- A span's hash is the SHA-256 of its whole lines, newlines included. A new
  text with no last newline gets one. An empty text removes the lines.

## Try it from a shell

```sh
I="coax/web/tool.sh coax/lib/agent/intel/intel.coax"
$I defines rp_put                      # --root DIR for another tree
$I uses rp_put --file coax/lib/repo/lines.coax --entries 40
$I exports coax/lib/repo/lines.coax
$I show mc_edit_file                   # or: show coax/lib/repo/lines.coax:6-10
$I check coax/lib/repo/lines.coax
$I explain E0325
$I vocab rp_put
$I effects coax/lib/microcoder/tools.coax mc_edit_file
$I depends coax/lib/microcoder/tools.coax
$I loaders coax/lib/repo/lines.coax
$I replace rp_lines --seen HASH --text-file new.txt
```

Options: `--root DIR`, `--coax PATH`, `--entries N`, `--most N` (records),
`--bytes N` (text), `--run-steps N`.

## Checks

```sh
coax/bin/coax test coax/lib/agent/intel/answers.coax              # the pure parts
coax/bin/coax test coax/lib/agent/intel/compiler.coax --only unique
coax/web/tool.sh coax/lib/agent/intel/intel.coax selftest          # real compiler, files and edits
coax/web/tool.sh coax/lib/agent/intel/intel.coax measure           # the table below
```

`selftest` builds a scratch tree (five files that load each other, a
definition used in three files, a part that doesn't check alone, a file with
a type error, a procedure with an effect), asks every question and makes
every kind of edit. Inside the checkout it also asks about real files,
read-only. It prints `intel selftest: ok`.

## What it costs

Measured on coderos-4080 (i7-14700K) with coax0 0.52.0 on 2026-11-13, by
`intel.coax measure`. Cold is a new resident compiler with empty query and
check caches, in a scratch folder. The questions run in the order listed, so
a later cold question can reuse what an earlier one built.

| Question | Compiler runs | Records | Cold (ms) | Warm (ms) | Warm, no resident compiler (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| defines rp_put (three programs declare it) | 4 | 3 | 1498 | 1302 | 1321 |
| defines mc_edit_file | 1 | 1 | 526 | 400 | 434 |
| uses rp_put, of `lines.coax`'s | 24 | 235 | 12153 | 5446 | 7722 |
| uses mc_edit_file | 9 | 14 | 3352 | 2061 | 2896 |
| exports microcoder/tools.coax | 1 | 62 | 245 | 248 | 243 |
| show mc_edit_file | 1 | 55 | 386 | 387 | 409 |
| check repo/lines.coax | 1 | 0 | 50 | 61 | 6 |
| check microcoder/tools.coax | 1 | 0 | 3782 | 140 | 311 |
| check rlm/code.coax | 1 | 0 | 1848 | 396 | 571 |
| explain E0325 | 1 | 1 | 9 | 3 | 9 |
| vocab rp_put | 1 | 1 | 196 | 193 | 207 |
| effects tools.coax mc_edit_file | 1 | 6 | 577 | 159 | 368 |
| depends microcoder/tools.coax | 1 | 49 | 215 | 212 | 418 |
| depends rlm/code.coax | 1 | 82 | 759 | 755 | 1006 |
| loaders repo/lines.coax | 3 | 3 | 590 | 448 | 1019 |

An edit and its check on a copy of `lines.coax`: `show` 256 ms; `replace` by
name 292 ms in all, of which the check is 49 ms.

Where the time goes:

- One `coax query` takes about 230 ms however small the file and however warm
  its index. It doesn't run in the resident compiler. `uses` asks one per
  entry, so a name used across many units costs seconds.
- A text search with git grep takes about 100 ms in this repository.

## What the compiler can't answer yet

Found while building this, for the compiler's owners:

1. `coax query` isn't served by the resident compiler and has a floor of
   about 230 ms a call.
2. `coax query` takes one entry file. There is no query over a folder or a
   repository, so finding entries is done here by search and trial.
3. `id` and `module` are `null` in `coax.query.v1`, and a use's `to` is
   `null`. `from` and `to` are `null` in every `coax.declarations.v1`
   reference. A use can't be tied to its declaration except by the entry
   having only one of that name.
4. `coax query` gives a declaration's line, column and end line, but no end
   column, byte span or hash. `coax deps --declarations` has the byte span and
   its SHA-256. Neither gives the comment above a declaration.
5. `coax deps FILE --effects` fails on a large unit (`coax/lib/rlm/code.coax`:
   "effects inventory refused graph/capacity status 1"), with and without
   `--proc`.
6. Effect kinds are too coarse to act on. A procedure with checked arithmetic
   reports `may_trap`, `host_io` and `unknown_effects`. A foreign procedure
   declared `#effects(foreign, clock)` reports `foreign_call` and no clock.
7. A file that needs a larger `--run-steps` to check doesn't say so
   (`coax/lib/bitcoin/header.coax` fails alone with E0400). The caller has to
   know the number.
8. A part that checks only inside its loader can't be checked, queried or
   listed alone, and nothing names its loader.
9. `coax vocab` answers only inside an openagents git checkout.
10. With `--json` and no `--diagnostics json`, `coax query` prints errors in
    the older JSON shape, not `coax.diagnostic.v1`.
11. No command checks a proposed text without writing the file (proposal 3).
