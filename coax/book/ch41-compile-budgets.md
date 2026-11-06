# 41. Compile and startup budgets

> **Status: partly implemented.** Phase and declaration reports, the Wasm
> build's per-step cache, split C emission and capacity diagnostics run
> today. R22's general five-case kernel and startup budgets remain planned.

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

## What remains planned

R22's five kernel/startup cases are first compile, cached load, scene rebuild,
runtime parameter edit and structural edit. General content-keyed kernel
reuse and target-specific budgets still need consumer qualification.
The existing compile baseline lives in `coax/perf/baseline.json`; compare it
with `coax/web/tool.sh coax/perf/compile-times.coax` on the recorded machine.
Compiler reports and the Wasm step cache do not establish those future gates.
