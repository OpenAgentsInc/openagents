# 41. Compile and startup budgets

> **Status: partly implemented.** Phase and declaration reports, the Wasm
> build's per-step cache, split C emission, capacity diagnostics and native
> checks (behind `COAX_NATIVE_CHECKS=1`) run today. R22's general five-case
> kernel and startup budgets remain planned.

Measure the stage that slows an edit before changing its budget. The compiler
reports its phases and work; the browser build reports C compilation, linking
and Wasm optimization separately. Neither report proves program correctness.
[Chapter 19](ch19-builds-hot-reload.md) places these tools in the build loop.

## Phase and declaration costs

From the repository root, run:

```shell
coax/bin/coax check coax/book/examples/ch03_counter.coax \
  --timings /tmp/counter.json --timings-top 3 --timings-table
```

`--timings FILE` and `--timings-json FILE` write the same schema-1 JSON.
`--timings-table` also prints a table to stderr. `--timings-top N` selects
how many declarations the JSON retains, ordered by seconds and by work;
the table shows at most ten from each ordering. It does not enable timing
by itself. Without a report or table, phase clocks are off.

A run on the Mac at compiler-input fingerprint `037278f2a55d` ends with
these work rows (the elapsed times vary with machine load):

```text
  most work:
        0.000 s           38 work    0 inst  procedure main (coax/book/examples/ch03_counter.coax:14)
        0.000 s           26 work    0 inst  procedure advance (coax/book/examples/ch03_counter.coax:6)
        0.000 s            2 work    0 inst  struct Seat (coax/book/examples/ch03_counter.coax:3)
```

The `declarations` object has `by_seconds` and `by_work` arrays. Each entry
names the physical path and line, checked body nodes, generic instances and
their nodes, compile-time steps, emitted C bytes, and evaluation/emission
seconds. Work is nodes plus instance nodes plus evaluated steps. Declaration
seconds cover evaluation and emission; they are not per-procedure checker
wall times. Generic body work is charged to its template.

| Report field | Meaning |
| --- | --- |
| `seconds` | Load, lex, imports, libraries, parse, check, evaluate, authority, emit, and native compile/link when run; `total` is elapsed wall time. |
| `detail_seconds` | Parts of an enclosing phase, such as `check.checker`, `check.storage` and `check.transfer`; do not add them again. |
| `checks` | Each executed or reused check, its phase, seconds, steps, cases and result. |
| `libraries`, `library_checks`, `authority_checks` | Loaded-unit and authority work, including cache reuse. |
| `work`, `capacities` | Counted compiler work and bounded resource use, including top consumers. |
| `generated_bytes`, `instances`, `aggregate_traversal_work` | Output and evaluator costs apart from elapsed time. |

Authority checks remain mandatory. A matching checked-result cache can reuse a
passing verdict; enabling timings neither skips an obligation nor grants a
stale cache entry permission to run.

Storage measurements restart when evaluation enters a phase with different
limits. Live constants still count toward the new phase's storage use;
released temporary arrays do not. Each reported peak keeps the limit that
governs that phase, so an earlier allocation cannot produce a false note
against a later, smaller check budget.

## The Wasm build cache and report

`coax/web/wasm.sh APP.coax FACE.c OUT.wasm [LIBRARY.coax...]` reports each
`coax_to_c`, `clang`, `link` and `wasm_opt` step. The JSON contains the machine,
load, compiler hash, output bytes, elapsed seconds and a `steps` array. A
compiler step that actually runs embeds its own report under `coax`.
Steps can overlap; adding their durations is not the build's wall time.

```shell
COAXIAL_WASM_REPORT=/tmp/glow.timings.json COAXIAL_MODE=dev \
  coax/web/wasm.sh coax/book/examples/lib/ch17_glow.coax \
  coax/book/examples/lib/ch19_preview.c /tmp/glow.wasm
```

This builds a small module; it does not run the preview or open a page.
For a cache-invalidation exercise that also changes inputs, run the checked
Coax tool:

```shell
coax/web/tool.sh coax/tests/wasm-cache.coax "$PWD/coax/bin/coax"
```

Its output includes:

```text
  ok   a cold build misses every step
  ok   the same build again reuses every step and writes the same bytes
  ok   a comment compiles the Coax again; the same C reuses the object and the module
```

`COAXIAL_WASM_CACHE` defaults to `~/.cache/coax-wasm`; set it to an empty
string for an uncached build. Each step has its own key. Coax emission keys
include the source closure, compiler bytes and options. Object keys include
C, included headers, compiler identity and flags. Link/optimization keys
include their inputs and tool identities. A changed Coax comment can miss
emission yet hit an unchanged C object's cache. An unchanged build can hit
every step. Entries unused for 14 days are removed.

`COAXIAL_WASM_REPORT` overrides the report destination; otherwise it is
`.c/MODULE.timings.json` beside the output. `COAXIAL_WASM_TIMES=0` silences
the summary line while retaining JSON. Use `COAXIAL_MODE=dev` for the
unoptimized-link edit loop; compare release mode separately.

## Split a large C unit

```shell
coax/bin/coax c coax/book/examples/ch03_counter.coax --library \
  --split 4 --split-unit 1 > /tmp/counter-split.c
```

The deliberately small byte threshold demonstrates the output on a small
program. `--split N` emits a common prelude followed by parts marked
`/* coax-unit ID */`, with a `shared` part when helpers need it. It is a stream for a
build driver to separate into translation units, not one C file to compile
as-is. Keep each part's prelude. `--split-unit BYTES` defaults to 512 KiB
of procedure C. A unit below four thresholds stays whole; the part count
is bounded by N. Only library C emission uses splitting.

The Wasm driver separates the stream and caches each object's inputs.
Renderer carriers default to `COAXIAL_SPLIT=16`; other units default to 0.
`COAXIAL_SPLIT_UNIT` overrides the byte threshold and `COAXIAL_SPLIT=0`
selects one unit. A local gameplay edit can then compile only its changed
parts. Splitting does not bypass Coax checking or authority checks and is
not a resident incremental compiler. Release linking uses 8 LTO partitions.

## Checks as native code

Most of a large unit's cold compile goes to its checks. The evaluator walks
the checked syntax tree and pays for every node it visits. With
`COAX_NATIVE_CHECKS=1`, the compiler instead emits the selected checks and
the procedures they call as C, compiles that C in parallel parts, and runs
the binary as several processes. A check that passes natively counts as
passed. Every other check (a trap, a refusal, a time limit, or one that
reaches an unevaluated `#run`) runs in the evaluator, so failures still get
the evaluator's diagnostic and `#forall` counterexample.
`coax/docs/native-checks.md` describes the
design and its settings.

Cold compiles with `--library` on one remote build host (a fresh check
cache for each run; 2026-11-09):

| Unit | Checks | Before | After | Check phase before | Check phase after |
| --- | --- | --- | --- | --- | --- |
| `apps/openagents-web/status.coax` | 90 | 84.6 s | 5.8 s | 83.0 s | 1.4 s |
| `apps/openagents-web/charactercreator.coax` | 439 | 68.3 s | 39.7 s | 26.0 s | 2.3 s |
| `apps/openagents-web/fivelab.coax` | 1,256 | 144.5 s | 44.5 s | 104.5 s | 7.2 s |

The check phase is the `authority` time. In fivelab's, the evaluator spent
1,382 s of CPU time across its parallel check workers. Native code spent
2.1 s compiling the C, 3.7 s running the checks and 0.6 s emitting. A warm
compile, where the check cache answers, takes the same time either way
(13.5 s for fivelab). Most of what remains of a cold compile is the loaded
units' standalone library checks, which load and check each unit again.

`COAX_NATIVE_CHECKS=compare` runs both for every check and reports any
check where they disagree. Native code has no step or traversal budget, so
a check that passes natively doesn't stop at E0400. `#steps` and
`#traversal` still govern checks the evaluator runs.

## Capacity refusals and near-limit notes

A refusal names the resource, use, limit, top consumers with physical source
locations, and a reduction to try. The stable code depends on the resource:
E0400 for evaluator steps/slots/traversal, E0339 for generic instances,
E0381 for checker capacity, and E0603 for source composition limits.

The runnable example deliberately copies a large value:

<!-- checked-source: examples/ch41_traversal.coax -->
```jai
Game :: struct { cells: [1024] int; tick: int; }
step :: (g: Game) -> int { return g.tick + 1; }
#test "copy budget" #traversal 3000000 {
  g: Game;
  total := 0;
  for i: 0..<2000 { total += step(g); }
  #assert total == 2000;
}
```

```shell
coax/bin/coax test coax/book/examples/ch41_traversal.coax --near-limit 60
```

The run passes, then reports:

```text
coax: note[E0400]: compile-time traversal work: 2055026 of 3000000 used (68%), above the near-limit threshold of 60%; more than 3000000 fails with E0400
  top users of compile-time traversal work:
    2054000 visits  coax/book/examples/ch41_traversal.coax:7:30  copying arguments into 'step' (2000 calls)
    1026 visits  coax/book/examples/ch41_traversal.coax:4:1  #test 'copy budget' itself
```

Its fix suggests passing a view, an index or only the needed fields, splitting
the check, or raising its traversal budget. Changing the annotation to
`#traversal 1000000` refuses with E0400: `1000001 value and field visits
needed, the limit is 1000000`. This is aggregate-copy work, not instruction
steps. `#steps N` and `#traversal N` govern different resources.

`--traversal N` sets the evaluator's traversal budget; a check's own
`#traversal N` overrides it. Use `#run_steps N;` for compile-time evaluation
steps and `#steps N` on a check for that check's steps. Near-limit reporting
defaults to 75 percent. Set `--near-limit P` or `COAX_NEAR_LIMIT=P` to a
whole percent from 1 to 100. Notes do not enlarge limits. Structured
`--diagnostics json` includes the capacity and proposed fixes.

## Bring a unit back under its limits

A large unit can sit near several limits at once. When it does, every new
feature fails with E0400 until something is removed. Raising a limit with
`#traversal` or `#steps` only postpones the next failure. Fix the cause.

The near-limit note names the top users. In practice they are these, most
costly first:

1. **A large struct passed by value.** Every call copies every field, and
   each field is one traversal visit. Pass a view of a one-element array,
   an index, or only the fields the procedure reads.
2. **A check that holds two large values at once.** Value slots count live
   storage. A check that needs two worlds can usually build one, assert,
   and then build the other.
3. **A copy taken to read one field.** `x := s[0].big;` copies all of
   `big`. Read the field through the view.
4. **One check that covers many cases.** Split it. Each check has its own
   budget, and unchanged checks are reused from the cache.
5. **A scratch value built in a loop.** Build it once outside the loop and
   reuse it.

The example above shows the first cause. Change `step` to take a view, and
the 2,054,000 visits for copying arguments go away:

```jai
step :: (g: [] Game) -> int { return g[0].tick + 1; }
```

This sketch is illustrative; it is not a checked example.

One Everglade unit shows the scale of the effect. It stood at 97% of its
traversal limit, 97% of its value slots and 84% of its steps, and each new
feature took several rounds of fixes to fit. Passing the character and
world records as views, holding one world per check and splitting the
largest checks brought value slots to 65% and every check's traversal
under 75%. Two raised `#traversal` limits were then removed (#13094).

Run with `--near-limit 60` while you work on a large unit, so a note
appears before a refusal does.

## What remains planned

An unchanged authority compile replays its near-limit notes from the cached
capacity facts. Changing the threshold selects another cache entry. After
source edits, checks that produce notes run again so their consumers point
to the current source; quiet checks still use per-check reuse. Cache reuse
does not suppress a capacity note.

R22's five kernel/startup cases are first compile, cached load, scene rebuild,
runtime parameter edit and structural edit. General content-keyed kernel
reuse and target-specific budgets still need consumer qualification.
The existing compile baseline lives in `coax/perf/baseline.json`; compare it
with `coax/web/tool.sh coax/perf/compile-times.coax` on the recorded machine.
Compiler reports and the Wasm step cache do not establish those future gates.
