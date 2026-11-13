# The Coax compiler for agents: what it answers today

Written 2026-11-13 for GitHub #13445 (the compiler side of #13444). This is
the capability audit that issue asks for: what `coax0` can already tell a
coding agent, what is partial, what is only planned, and what each of the
issue's ten proposals would take.

Every command below was run on real files of this repository. Nothing in the
compiler or any library was changed.

## How it was measured

| | |
| --- | --- |
| Compiler | `coax0 0.52.0`, checked build `f8f8e9f66610`, installed with `COAX_MUTATION=0 coax/build.sh` (12 s, from the local cache) |
| Tree | `2e3860a6f8`; rebased to `e1b747b7cd` before writing, with no change to `coax/src` or `coax/lib/compiler` between them |
| Machine | coderos-4080: Intel i7-14700K, 28 threads, Linux 6.18. Other agents were using it |
| Times | Wall time of one run, not a best of several. "Cold" is `COAX_SERVE=0`. "Warm" is a second run in the resident compiler (`coax serve status` showed it up, with the units' lexes kept) |
| Small file | `pay/voice-meter.coax`: 1 unit, 190 lines, 6 checks, `#authority` |
| Medium program | `coax/lib/rlm/code.coax`: 57 units, 1.45 MB of source, 967 procedures, 302 checks |
| Errors | Deliberate errors were made in scratch copies outside the repository, or in the worktree and restored |

## 1. The capability table

State: **works** (does what it says on the files tried), **partial** (works,
with a gap an agent will hit; the gap has a number in section 2), **planned**
(not built).

| Command | What an agent gets | Schema | Cold | Warm | State | Source |
| --- | --- | --- | ---: | ---: | --- | --- |
| `coax query FILE defines NAME [--json]` | Each top-level declaration named NAME: kind, file, line, column, end line, exported, generated | `coax.query.v1` | 224 ms | 230 ms | partial (G1 to G6) | `coax/src/query.cpp`, `query_command` in `coax/src/main.cpp` |
| `coax query FILE uses NAME [--json]` | Each place NAME is written and resolves to a declaration, with the procedure, struct, constant or check that contains it | `coax.query.v1` | 276 ms | 227 ms | partial (G1 to G8) | same |
| `coax query FILE exports [MODULE] [--json]` | A module's public declarations, or the file's own | `coax.query.v1` | 229 ms | 224 ms | partial (G1 to G5) | same |
| `coax deps FILE` | The source units and descriptors a program reads, and its limits | `schema_version: 1` | 379 ms | 50 ms | works | `compile` in `coax/src/main.cpp`, `coax/src/sources.cpp`, `coax/lib/compiler/sources.coax` |
| `coax deps FILE --declarations` | Every struct, enum, constant and procedure with its byte span, line range and the SHA-256 of its text; every resolved reference | `coax.declarations.v1` | 251 ms | 52 ms | partial (G1, G2, G9 to G11) | `declarations_report` in `coax/src/main.cpp` |
| `coax deps FILE --modules` | Module content and dependency revisions | `coax.modules.v1` | 239 ms | 59 ms | partial (G1, G2); empty for a program with no `#module` | `modules_report` in `coax/src/main.cpp` |
| `coax deps FILE --effects [--proc NAME]` | For each procedure, a mask of may-effects and one witness call path for each | `coax.effects.v1` | 241 ms | 55 ms | partial (G12, G13) | `coax/src/effects.cpp`, `coax/lib/compiler/effects.coax` |
| `coax deps FILE --lowering [--proc NAME]` | The ordered operations, traps and sequencing of a procedure | `coax.lowering.v1` | 267 ms | 55 ms | works (one procedure tried) | `lowering_report` in `coax/src/main.cpp` |
| `coax check FILE --diagnostics json` | One object for each error on stderr: code, message, span (file, line, column, byte offset, length), notes, fixes with edits | `coax.diagnostic.v1` | 79 to 111 ms with an error; 242 to 311 ms clean | 53 to 147 ms | partial (G14 to G16) | `error_at`, `diagnostic_json`, `fix_edits` in `coax/src/main.cpp`; messages in `coax/lib/compiler/checker/` |
| `coax check FILE --json` | The older one-line form: file, line, col, code, message, fix | none | | 125 ms | works; superseded | same |
| `coax test FILE [--only SUBSTR] [--list]` | Each check's result, with a counterexample for a failing `#forall` | text; `coax.diagnostic.v1` for failures | 123 ms | 52 to 198 ms | partial (G17, G18) | `coax/src/verify.cpp`, `coax/src/native_checks.cpp` |
| `coax test FILE --evidence OUT` | A record of the run: compiler hash, every unit's SHA-256, budgets, each check's outcome, a verdict | `coax.evidence.v1` | | 52 to 63 ms | partial (G19, G20) | `write_evidence` in `coax/src/main.cpp`, `coax/src/verify.cpp` |
| `coax mutate FILE [--unit PART] [--evidence OUT]` | Mutants killed and survived; survivors and unreached procedures as `E0414` and `E0415` diagnostics | text; `coax.evidence.v1` | | 261 ms (72 mutants, one unit); 158.6 s (153 mutants of one unit, 447 checks) | partial (G21, G22) | `coax/src/verify.cpp`, `coax/lib/compiler/mutation-schedule.coax` |
| `coax mutate FILE --killers OUT` | For each mutant: its procedure, what changed, and the check that killed it | tab-separated, no schema name | | 15 ms after a first run | works; the only printed link from a procedure to a check | `coax/src/verify.cpp` |
| `coax compat OLD NEW [--baseline BASE]` | Which published structs, fields and procedures changed incompatibly, including a procedure whose effects grew | text, "reason N" | 14 to 71 ms | not served | partial (G23) | `coax/src/compat.cpp`, `coax/lib/compiler/compatibility.coax` |
| `coax compat --emit FILE` | The published surface as rows: structs with size and alignment, fields with offsets, arguments, returns, each procedure's effect mask | `coax.compat.v1` (tab-separated) | 3 ms | not served | works | same |
| `coax layout FILE` | Struct sizes, alignment, offsets and padding | text | 270 ms | 65 ms | works; text only | `coax/src/emit_c.cpp` |
| `coax c FILE --source-map OUT` | Each procedure and local: Coax name, C name, type, file, line; source hashes | `schema_version: 1` | | 191 ms | works; not accepted by `layout` or `check` | `coax/src/emit_c.cpp` |
| `coax check FILE --timings OUT` | Phase times, check cache counts, peak storage | `schema_version: 1` | | 68 ms | works | `coax/src/main.cpp` |
| `coax explain CODE` | A title and one or two sentences for each of 231 codes | text | 1 ms | not served | partial (G24) | `coax/src/errors.cpp` |
| `coax vocab NAME...` | What a library procedure, type, keyword, error code or foreign spelling is: signature, doc comment, file and line; nearest names when unknown | text | 166 to 348 ms (1.5 s the first time) | not served | partial (G25) | `coax/web/vocab.coax`, run through `coax/web/tool.sh`; `vocab_command` in `coax/src/main.cpp` only starts it |
| `coax status --json` | Every language feature with its state (works, planned) | `schema_version: 1` | 1 ms | | works | `coax/src/release.h` |
| `coax serve` | The resident compiler. `c`, `check`, `test`, `lib`, `header`, `build`, `deps`, `layout` and `checks-c` run in it without being asked | status text | | | partial (G3, G26) | `coax/src/serve.cpp`, `coax/src/skeleton.cpp` |
| `coax/web/tool.sh fmt --write --revision REV FILE` | A write that is refused (exit 3) when the file no longer has the revision the caller read | text | not measured | | works; the one write in the tree tied to what was read | `coax/web/fmt.coax` |
| Replacing a procedure body while it runs | | | | | planned (book chapter 45, R26) | none |
| A resident cache of the typed graph, by declaration | | | | | planned (book chapter 46, R27) | none |
| A self-hosted evaluator and emitter | | | | | planned (book chapter 44, R25) | none |

On the medium program (57 units):

| Command | Time | Note |
| --- | ---: | --- |
| `check`, clean | 557 ms cold, 390 to 416 ms warm | |
| `check --diagnostics json`, a type error inside one procedure body | 386 ms cold, 214 ms warm | The resident checked the changed body first (`skeleton.cpp`); `serve status` counted it |
| `deps --declarations` | 425 ms | 2.0 MB of JSON: 1,562 declarations, 9,942 references |
| `deps --effects` | refused | `effects inventory refused graph/capacity status 1` (G12) |
| `query exports --fresh` | 705 ms | |
| `query defines`, index cached | 254 ms | |
| `query defines`, first after an edit | 709 ms | The index is rebuilt whenever any unit's hash changed |
| `test --only` one check | 395 to 456 ms | |
| `test`, all 302 checks | 1.08 s | |

Two larger runs: `coax test coax/lib/lnaddr/lnurl.coax` (447 checks) took
2.9 s. `coax mutate coax/lib/lnaddr/lnurl.coax --unit coax/lib/lnaddr/name.coax`
(153 mutants) took 158.6 s, and 158.7 s when run again: a mutation run is
not remembered.

Three library files under `coax/lib/compiler/` have names that suggest this
work and aren't part of it. `query-index.coax` and
`query-projection-cache.coax` are the borrow checker's private query rows.
`provenance.coax` compares build-record digests. None of them serves
`coax query`.

## 2. The gaps

Each was hit while running the commands. "Refused" means exit status 1 with
a message. Nothing here was root-caused past what is written.

| # | Gap | What was seen |
| --- | --- | --- |
| G1 | **`query`, `deps --declarations` and `deps --modules` refuse any program that uses a tagged value.** | `E0911: coax.query.v1 has no tagged-value or switch inventory; no index was published`. An optional, a `Result`, `null`, `try` or `or_else` anywhere in the program is enough (`LegacySumInventory` in `main.cpp`). Of 1,823 files tried (every fourth tracked `.coax` file, without fixtures), 77 were refused this way (4%). They include `pay/lnaddr-policy.coax`, `pay/makerpay-policy.coax` and `apps/openagents-web/fivelab.coax` (refused after 3.3 s) |
| G2 | **`query` and `deps --declarations` crash on two files.** | Exit 139 (segmentation fault) on `psionic/lib/shadow.coax` and `metaverse-engine/tests/feet-fairy.coax`. `coax check` passes on both. A larger stack didn't help. Not root-caused; one small attempt to reproduce it failed |
| G3 | **`query` is not served by the resident compiler, and costs about 225 ms at least.** | `query_command` hashes the compiler binary (43 MB) on every call before it reads its cache; `sha256sum` of that file takes 103 ms. A cached answer on a 190-line file takes the same 225 ms as a fresh one. `check` on the same file takes 53 ms warm |
| G4 | **`id` and `module` are null unless the unit has `#module`.** | By design (`declaration_id` in `sources.cpp`: "no stable identity is claimed"). `coax/lib/json/json.coax` gives `coax.decl.v1:coax.json:proc:jv_parse`; `pay/` and `coax/lib/rlm/` give null. A use's `to` is then null too, so when two declarations share a name (`wrap_angle` is defined in two engine files) the answer doesn't say which one a use resolves to |
| G5 | **A query needs the right entry file, and answers for that program only.** | Of the 1,823 files, 1,319 (72%) answered when given as the entry; 426 (23%) don't compile alone (`E0302`, `E0315`, `E0601` and others) and need the file that loads them. `uses bounded_push` from `coax/lib/bounded.coax` lists 30 uses, all in that file; uses in the programs that load it are found only from their entries. No command says which entries load a unit |
| G6 | **An unknown name is an empty answer with exit 0.** | `defines vm_cost` returned `"definitions":[]`. No nearest name is offered. With `--json`, a compile error comes back in the older one-line JSON form, not `coax.diagnostic.v1` |
| G7 | **`uses` lists direct uses only.** | `uses over` names `proc owed` and two checks. `#forall "what is owed"` reaches `over` through `owed` and isn't listed. A caller has to walk upward itself, one call for each name |
| G8 | **A check is a place a use is found in, never a thing to ask about.** | `uses` reports `#test "NAME"` and `#forall "NAME"` in `from`. `defines` of a check name is empty, `deps --declarations` lists no checks and no span for one, and `test --list` prints kind and name with no file or line |
| G9 | **`deps --declarations` has no filter.** | 2.0 MB for the 57-unit program, 11.5 KB for the smallest. A model can't be shown it; a library has to cut it down |
| G10 | **References there have null `from` and `to` without `#module`.** | In `pay/voice-meter.coax` every reference is `{"from":null,"to":null,"name":...,"line":...}`. With a module both are IDs |
| G11 | **A declaration's span starts at its name.** | `owed` spans lines 58 to 60; its comment on line 57 is outside. Replacing by span keeps a comment that may now be wrong |
| G12 | **`deps --effects` refuses a mid-sized program.** | 57 units, 967 procedures: `refused graph/capacity status 1`, with or without `--proc`. `EF_capacity` in `effects.coax` admits at most 4,096 procedures and 65,536 edges and sites |
| G13 | **`coax.effects.v1` can't say "this procedure does I/O".** | Its kinds are `host_io`, `foreign_call`, `unknown_effects`, `may_trap`, `borrowed_read`, `borrowed_write` and three more. `owed`, which only adds and multiplies, reports `host_io`, `unknown_effects` and `may_trap` at a checked subtraction, because a trap logs. The enforced `#effects(...)` vocabulary (`io`, `foreign`, `clock`, `reads`, `writes`, `traps`) is a separate policy (`effect-contract.coax`) and no command prints a procedure's set in it, except as a number in `compat --emit` |
| G14 | **Most fixes can't be applied by a tool.** | Only "add 'X'" fixes carry edits (`fix_edits`). `E0315` carries "find where it is declared: coax vocab NAME" with no edit. `E0325` and `E0316` carried no fix in the trials. Of 243 error sites in `coax/lib/compiler/checker/`, 136 pass an empty fix |
| G15 | **`E0316` (wrong argument count) doesn't point at the declaration.** | Nine objects, one for each call, none with a note giving the signature or where it is |
| G16 | **A diagnostic's `file` is the path as typed; `query` gives the canonical path.** | `"file":"e1.coax"` against `"file":"/home/.../pay/voice-meter.coax"`. A span on an operator has length 1 |
| G17 | **`--only` takes one substring.** | A second `--only` is a usage error. Running four chosen checks takes four runs or a shared substring. There is no "the checks of this unit" or "the checks that reach this procedure" |
| G18 | **The first `coax test` after an edit can build native check code.** | A failing run without `--evidence` took 3.4 s on the 190-line file; the same with `--evidence`, which runs the checks in the evaluator, took 180 ms |
| G19 | **A run with `--only` never has the verdict `passed`.** | Its record says `"verdict":"not-checked"` because the other checks are listed as `not-checked`. A finish rule has to read each claim, not the verdict |
| G20 | **Two fields of the evidence record shift meaning.** | A passed claim's `at` is the check (line 120, column 1); a failed claim's `at` is the failing `#assert` (line 124). With `mutate --unit`, `selection` doesn't name the unit; only `mutation.scope` says `unit` |
| G21 | **`mutate` knows which checks enter each procedure and doesn't print it.** | The baseline run records the procedures every check entered (`verify.cpp`), to skip checks that can't kill a mutant. Only counts come out (`mutation-scheduling selective checks=78 skipped=39`), plus the killers file |
| G22 | **`mutate --unit` can be slow, and a second run costs the same.** | 158.6 s and 158.7 s for 153 mutants, with `"scheduling":"full"`: every mutant ran against every check |
| G23 | **`compat` prints a reason number and nothing else.** | `coax compat: incompatible Meter.ms (reason 2)`. No JSON, no reason text, no witness. An added field isn't named; the fields after it are. Both sides must compile |
| G24 | **`explain` has no example.** | 231 codes, 158 characters each on average; 2 mention an example. `coax/tests/fail/` has 146 failing programs covering 57 codes that could supply them |
| G25 | **`vocab` doesn't know a program's own names.** | `vocab owed` (defined in `pay/voice-meter.coax`): "no library procedure ... has this name; nearest: seed, Noted, SwAd". The compiler's own `E0315` fix sends the agent there |
| G26 | **No command takes source from memory.** | `coax check /dev/stdin` is `E0601`; there is no overlay flag. The resident compiler reads every unit from disk on each request and remembers only lexes, hashes and the last checked skeletons |

## 3. The ten proposals

"Library" means Coax over the commands above (the compiler-questions library
is being built in `coax/lib/agent/intel/` by another agent, and
`coax/lib/agent/workmem/finish.coax` landed while this was written; neither
was audited). "Compiler change" means an edit under `coax/src`,
`coax/bootstrap`, `coax/lib/compiler` or `coax/lib/proto`. Those are the
compiler's inputs (`provenance.sh`), so any such edit gives the tree a new
compiler: about 90 minutes to bootstrap on this machine with the gates, 84
of them in `mutate lib/compiler/checker.coax` (`coax/RELEASE.md`, "Published
builds"), and every other machine waits for the release hosts to publish it,
about an hour and a half. `coax/web/vocab.coax` and `coax/web/fmt.coax` are
tools, not compiler inputs.

Size: **small** is days and one file or one library; **medium** is a week or
several files or sources joined; **large** changes the language or joins
many sources.

### 1. Read and edit by declaration

| | |
| --- | --- |
| Exists | `deps --declarations` gives every declaration's byte span, line range and the SHA-256 of its text, in 52 ms warm. `query defines` gives file, line and end line. `fmt --write --revision` is a write refused when the file changed |
| Missing | Checks have no span (G8). No filter (G9). Comments above a declaration are outside its span (G11). Refused or crashing on some programs (G1, G2) |
| Where | Library first: find the declaration, read its bytes, compare the hash, splice, write, run `check --diagnostics json`, put the old bytes back if asked. Later in the compiler: checks as declarations with spans and a `--name` filter (`declarations_report` in `main.cpp`, `QueryBuilder` in `query.cpp`), and G1 |
| Compiler change | Not for procedures, structs, enums and constants. Yes for checks and for G1 |
| Size | Small as a library: every fact it needs is already printed. Medium in the compiler |
| Measurement | Failed edits for each edit (23% on the unseen tasks, 7% on the tuning set). Share of reads that are repeat reads (66% on Carbon). Bytes of source shown for each task. Calls before the first edit |

### 2. Which checks reach this code

| | |
| --- | --- |
| Exists | `query uses NAME` names the procedures and checks that write NAME. Coax has no procedure values, so the written call graph is the whole call graph. `mutate` records which procedures each check entered and prints only the killers file. The check cache keys each `#authority` check by the declarations it reaches (`ReuseIndex` in `verify.cpp`) |
| Missing | The transitive answer (G7). Running a chosen set of checks in one call (G17). Checks as things with a place (G8) |
| Where | Library today: walk upward with `uses` from the changed procedure until the names are checks; about 230 ms a step until G3 is fixed. In the compiler: a `query FILE checks NAME` form that walks the index it already holds (`query_answer` in `query.cpp`, about 40 lines), and a repeatable or exact `--only` (`main.cpp`, `verify.cpp`) |
| Compiler change | Not for the walk. Yes for the one-call form and for `--only` |
| Size | Small either way |
| Measurement | Seconds from an edit to the first relevant check result. Checks run for each edit against checks in the program. Runs that hit a time limit while verifying. Failures the whole suite finds that the chosen set missed: this must be zero |

### 3. Check an edit before writing it

| | |
| --- | --- |
| Exists | The resident compiler checks a changed procedure body first: 214 ms on 57 units against 390 ms for the whole check. It reads from disk only (G26) |
| Missing | Any way to hand the compiler bytes that aren't in the file |
| Where | Library today: write the candidate, check, put the old bytes back when it fails. That is one candidate at a time, and the file is briefly wrong on disk. In the compiler: an overlay (`--overlay PATH=FILE`) where a unit's bytes are read (`sources.cpp`). The resident's memo is keyed by bytes and stays correct; the query cache compares files on disk (`query_current`) and must not answer under an overlay |
| Compiler change | Not for write, check and roll back. Yes for several candidates at once with nothing written |
| Size | Small as a library; medium for the overlay, because each cache that trusts a path must be shown to see the overlay |
| Measurement | Milliseconds from an edit to its diagnostics. Share of applied edits that type-check the first time. Steps in a run where the tree doesn't compile |

### 4. Refactors the compiler performs

| | |
| --- | --- |
| Exists | `query uses NAME --json` gives every resolved use with the column of the name itself, after any alias. That is what a rename edits. After a signature change, `check --diagnostics json` lists every call (`E0316` at each, nine in the trial) |
| Missing | Uses in other programs (G5) and a way to find those programs. Which declaration a use resolves to when names collide (G4). Fields, enum members and locals: the index holds top-level declarations only. C that calls an `#export_c` procedure is outside the graph |
| Where | Library: a unit-to-entries index built from `coax deps` (50 ms a file warm, kept by hash), then rename as uses plus definitions, written as hash-tied edits, then a check of every affected entry. Refuse when an entry doesn't answer (G1, G2) or a name collides. One line in `query.cpp` would add the target's file and line to each use |
| Compiler change | Not for a rename in one program. Small for G4. Field renames need the checker's field resolution in the index |
| Size | Medium: the edits are easy, the entries index and the refusal rules are the work |
| Measurement | On rename and signature tasks: failed edits, calls and seconds against the plain loop, and whether every affected entry checks clean. `grep -w` gives an upper bound on the sites to compare with |

### 5. What changed, in meaning

| | |
| --- | --- |
| Exists | `compat OLD NEW` for the published surface, including effects that grew. `compat --emit` as rows that can be compared. `deps --declarations` hashes, which give the changed, added and removed declarations exactly. `test --list` for checks added or removed. `layout` |
| Missing | Reason text and JSON from `compat` (G23). The old tree has to be on disk, whole, because loads are relative. Whether a unit is `#authority` isn't in any JSON output seen |
| Where | Library: a worktree of the base commit, the same four commands on each affected entry in both trees, and a comparison. No compiler change. `compat --json` with named reasons would be a small one (`compat.cpp`) |
| Compiler change | No |
| Size | Medium: four formats to read, two trees, and the list of affected entries |
| Measurement | Every statement in the summary is a compiler fact, so it is checked by construction. For the rule in #13445 step 4: runs that removed a check, an `#authority` line or a baseline, and the share of them flagged, which must be all. Seconds to produce |

### 6. A compiler that teaches the language

| | |
| --- | --- |
| Exists | 231 codes with `explain`. Repairs for foreign spellings (`elif`, `let`, `a ? b : c`: the idiom table in `errors.cpp`). `vocab` with signatures and doc comments for library names. Counterexamples for `#forall`. Fix text on 107 of 243 checker error sites |
| Missing | Edits in fixes (G14). Examples (G24). Program names in `vocab` (G25). A note on `E0316` (G15). A count of which diagnostics agents hit: the transcript analysis has none |
| Where | First the count, from recorded sessions, with no compiler involved. Then tool-side: `vocab` over the program's declarations, and examples from `coax/tests/fail/` joined to `explain` in the library. Then the compiler, most-hit codes first: fix strings in `coax/lib/compiler/checker/`, edits in `fix_edits` (`main.cpp`), text in `errors.cpp` |
| Compiler change | Not for the count, `vocab` or examples. Yes for each message and fix, through the checker's mutation gate |
| Size | Small for the tool-side half. Medium to large for the messages, a few codes at a time |
| Measurement | For each code: calls between its first appearance and a clean check. The top ten codes by count before and after |

### 7. Typed holes

| | |
| --- | --- |
| Exists | Nothing. A wrong guess already returns both types (`E0325: 'x' is bool but is initialized with u64`) and the argument count (`E0316`) |
| Where | Lexer and parser (a hole token), checker (the expected type and the names in scope that fit: `coax/lib/compiler/checker/`), `errors.cpp`, the spec, the grammar and the book |
| Compiler change | Yes, and a language change with a version and a changelog entry |
| Size | Large: listing the names in scope that fit a type is new work in the checker, and the checker's gate is the slowest in the build |
| Measurement | Before building: count how often a run stalls on "what goes here" (the count in proposal 6). The transcripts show none. Build it only if that count is high |

### 8. Effects as the agent's limits

| | |
| --- | --- |
| Exists | Enforced `#effects(...)` since 0.52: an undeclared effect is `E0941` with the call path (`effect contract of 'bad' excludes traps; call path: bad -> plain`). `compat OLD NEW` reports each published procedure whose effects grew: adding a clock call to `owed` gave "reason 4" for `owed` and its three callers. `compat --emit` carries each procedure's mask (8 before, 1034 after) |
| Missing | The mask as names, and the path, in `compat`'s output (G23). `deps --effects` is a different vocabulary and refuses mid-sized programs (G12, G13). The capabilities are `io`, `foreign` and `clock`; file and network aren't told apart. 24,294 of the tree's 24,479 `#effects` annotations are `all`, so the declared bound refuses almost nothing; the inferred mask is what carries information |
| Where | Library: compare the masks of `compat --emit` on the base and current trees, decode the bits from `effect-contract.coax`, flag a growth, and treat it as a refusal for an `#authority` unit. In the compiler: names and a witness path in the reason-4 line (`compat.cpp`) |
| Compiler change | Not for the flag. Small for names and paths |
| Size | Small as a library on top of proposal 5's two trees |
| Measurement | This is a guard, not a way to pass more tasks. Count how often it fires on real issues and how often the growth was intended |

### 9. A data set from our own history

| | |
| --- | --- |
| Exists | Evidence records with source hashes and counterexamples. Diagnostics as JSON. Declaration hashes. Killers files. Commits with `Agent:` trailers. Session readers in `coax/lib/agent/observe/`. 146 failing programs in `coax/tests/fail/` |
| Missing | The join. Commits are usually green, so "a diagnostic and the edit that fixed it" is in session records, not in git. Old commits may not compile with today's compiler |
| Where | A Coax tool under `coax/lib/agent/`: for each commit, both trees, changed declarations by hash, their checks by proposal 2; the diagnostic-and-fix pairs from session journals |
| Compiler change | No |
| Size | Large: four sources, old trees, and a review of what transcripts may be kept |
| Measurement | Valid records produced. An agent's pass rate on held-out items made from them |

### 10. Runs that replay

| | |
| --- | --- |
| Exists | `GW_RECORD` and `GW_REPLAY` in the gateway record a live run's model replies and answer from them with nothing sent. `coax/lib/rlm/replay.sh` replays one recorded run and fails unless the answer matches. Compiler answers are a function of the compiler's hash and the units' hashes, and the evidence, effects and query records carry those hashes |
| Missing | Tool results in the record. The tree at each step (hash-tied edits from proposal 1 give it). Test output prints milliseconds, so it isn't byte-stable. A harness change that changes the request no longer matches the recorded replies; replay tests parsing, tools and state, not prompts |
| Where | Library in the harness: record each tool result with its command, compiler hash and input hashes; a check that drives the loop from the record |
| Compiler change | No |
| Size | Medium: the model half exists |
| Measurement | Harness changes tested against recorded runs with no model call. Regressions caught there. Replays that diverge |

### The three to build first

From `docs/research/2026-11-12-microcoder-transcript-analysis.md`, what
Microcoder lost passes and time on:

- It forgets what it read (W1): 66% of Carbon file reads were repeat reads;
  three of its four failures on the unseen tasks.
- Its edits fail on text it wrote itself (W3): 23% of edits on the unseen
  tasks, 30 of 39 failures on a file the run had already edited.
- It pages files by guessed line ranges (W4): 72 `sed` reads of one file in a
  run; nine greps for one name after the first output was cut.
- It rewrites whole blocks from memory (W6c): 60 to 105 s each.
- It invents names: 315 s on `rs_reset`, `rs_setup` and `rs_seed`, none of
  which exist.
- Verification is unbounded: Antigravity ran `coax mutate` in 28 of 86 runs,
  which cost it three of four Carbon limit hits and one unseen task.
  Microcoder ran the task's checks after its last edit in 28 of 42 runs.

**First, proposal 1 (read and replace by declaration, with the check result
returned).** It answers W3, W4 and W6c directly and part of W1. The agent
reads the 18 lines of `rc_outline`, not a 749-line file. An edit names the
hash of those lines, so a stale edit is refused with the current text, and
an applied one returns what it wrote and its diagnostics in 53 to 400 ms.
Every fact it needs is printed today, so it is small and needs no release.

**Second, proposal 2 (which checks reach this code).** It bounds
verification, the cost that showed on both harnesses, and step 4 of #13445
needs it: "the checks the task names have passed" requires knowing which
checks concern a change. The walk over `uses` works today. One check runs in
52 to 450 ms; a guessed suite runs for minutes (158.6 s for one unit's
mutation run).

**Third, the tool-side half of proposal 6.** Count the diagnostics agents hit
in recorded sessions, make `vocab` answer for a program's own names with the
nearest real ones, and attach a failing example to `explain`. The invented
setup API is the clearest single loss to not knowing the code, and today
the compiler's own hint for an unknown name (`coax vocab NAME`) answers
"nearest: seed, Noted, SwAd" for a name defined one file away. None of this
needs a release. This choice has the thinnest evidence of the three: the
transcript analysis doesn't count diagnostics, so the count comes first and
decides the rest.

Not first: proposal 3's overlay (its write, check and roll back form is part
of proposal 1), proposals 4, 5 and 8 (useful, with no loss in the transcripts
that they would have prevented), 7 and 9 (large), and 10 (worth having once
edits carry hashes; it tests fewer harness changes than it seems to).

Before any of them: G1 and G2. A tool that is refused or crashes on 4% of
entry files, among them two of the eight money files in `pay/`, will send
the agent back to grep on exactly the code that most needs a certain answer.

## 4. Other ideas (the author's, not in the issue)

Ordered by what they would save.

| # | Idea | Why | Where | Size |
| --- | --- | --- | --- | --- |
| A | Let `query` and `deps --declarations` describe programs with tagged values (G1) | Coverage falls as code adopts `Result` and optionals | The two `LegacySumInventory` gates in `main.cpp`, and a schema `coax.query.v2` if the answer's shape must change | Medium |
| B | Find and fix the crash (G2) | A crash is read as "no answer"; an exit-status check is the only thing that told it from an empty one | Shared by `query` and `deps --declarations`, so before either builder | Small once found |
| C | Run `query` in the resident compiler and take the compiler's hash from its memo | 225 ms falls to about 50 ms; the walk in proposal 2 is many calls | The command list in `serve.cpp`, `query_command` in `main.cpp` | Small |
| D | A whole-index answer: `query FILE index --json` with every declaration and reference | One call, then the library answers `defines`, `uses`, callers and reaching checks in memory. The index already exists privately as `coax.query-index.v2` | `query.cpp` | Small |
| E | A unit-to-entries index | Every command takes an entry file; an agent holding a library file has no way to find its programs (G5) | Library over `coax deps`, kept by hash | Small |
| F | Checks as first-class: a span and a place for each, `test --list --json`, exact and repeated `--only` | Proposals 1 and 2 and the finish rule all name checks | `verify.cpp`, `main.cpp`, `query.cpp` | Small |
| G | Print each unit's SHA-256 in `coax.query.v1` answers | #13445 step 1 wants the file's content hash in every typed result. The index has it and doesn't print it. `fmt` uses a different hash (FNV-1a 64); pick one | `query_answer` in `query.cpp` | Small |
| H | A note on `E0316` and `E0325` with the declaration's place and signature (G15), and canonical paths in diagnostics (G16) | The agent gets the fact it needs without a second question | `coax/lib/compiler/checker/`, `main.cpp` | Small each, through the checker's gate |
| I | Fix the two shifting fields of the evidence record (G20), and a verdict for the selected checks alone (G19) | A finish rule reads these records | `write_evidence` in `main.cpp` | Small; a schema change, so `coax.evidence.v2` |
| J | Filters on `deps --declarations`: `--name`, `--unit` | 2 MB is not an answer (G9) | `declarations_report` | Small |
| K | Remember mutation runs by input hash | A second identical run cost 158.7 s (G22); the 15-minute rule and "never repeat a passing result" both want this | `verify.cpp`, beside the check cache | Medium |
| L | Raise or stage the effects inventory's capacity (G12) | It refuses the RLM coder's own program | `effects.coax`, `effects.cpp` | Small to medium |

C, D, F and G are all compiler changes. They are small and would fit in one
release, taken together.

## 5. What was not checked

- **macOS.** Everything ran on Linux x86-64. The resident compiler's times
  and the 43 MB binary's hash time will differ on the Mac.
- **A quiet machine.** Other agents were building. Times are single runs.
- **The largest programs.** `fivelab.coax` (294 units) was refused by `query`
  (G1); `check`, `test` and `deps` weren't timed on it.
- **The cause of G2**, and whether more files crash: the survey covered a
  quarter of the tree and found one; the second was found by hand.
- **The survey's 23%.** Files that don't compile as an entry weren't sorted
  into "needs its loader" and "is a fixture or broken".
- **`--baseline` on `compat`, `--shard` and `--merge` on `mutate`,
  `checks-c`, `--timings-table`, `coax status --jobs`.** Not run.
- **`coax c --source-map`** was run once and its start read; nothing was
  built on it.
- **`deps --lowering`** was run for one procedure and not studied.
- **`coax/web/tool.sh fmt --revision`** was read, not run.
- **The spec** was read where it covers tooling (section 17.3), effects
  (`coax/checked-state.md`) and diagnostics; not the whole 2,363 lines.
  Book chapters 22, 45 and 46 were read in full; 27, 41 and 44 in part.
- **`coax/lib/agent/intel/`, `coax/lib/agent/engine/` and
  `coax/lib/agent/workmem/`.** Other agents' work in progress; not read.
- **Recorded sessions.** No session journal was opened, so the diagnostics
  agents hit are still uncounted.
- **Any model.** No model call was made and no credits were spent. Each
  "measurement" above is a proposal for one, not a result.
- **The size estimates.** They come from reading the code, not from trying
  the change.
