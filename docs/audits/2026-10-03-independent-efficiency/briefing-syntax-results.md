# Tree-sitter excerpt selection experiment

Recorded October 2, 2026 (America/Chicago).
Tracking issue: [#10255](https://github.com/OpenAgentsInc/openagents/issues/10255).
Input source revision: `43709aba33bab17d3aa6ab42102097c9d32db760`.

This is the first implementation experiment from the
[System One preparation audit](system-one-briefing.md). It tests whether cached
Rust syntax helps a brief show the relevant part of a selected file. It does
not call a model or change production routing.

## Question and comparison

The original prototype often finds a relevant file but starts its excerpt at
a generic word near the top. A cached declaration can locate an exact function
or qualified method farther down the file without another agent search.

The `--syntax` treatment changes excerpt selection only. Both arms retain the
same file scores, top-eight candidate pool, ancestor context, history, and
64-line excerpt cap. The treatment matches exact Rust identifiers or lexical
qualified names against cached declarations. Missing matches use the baseline
excerpt rule. It cannot recover a file absent from the candidate pool.

Three comparisons separate the effects:

1. **Baseline:** ordinary metadata index and ordinary selection.
2. **Syntax:** syntax metadata index and structural excerpt selection.
3. **Index control:** syntax metadata loaded with selection disabled. Its
   selected source must equal the baseline; this exposes metadata overhead.

The [evaluator](evaluate_briefing_retrieval.py) rotates the three arms through
each run position and launches a fresh process each time. It checks that
evidence stays identical across repeats and independently verifies each final
selection against Git. Index construction is timed separately, once per index
in baseline-then-syntax order; those are ordered observations rather than
paired build estimates. The OS cache stays warm; no GitHub request, model call, build, or test execution is
inside the preview timer.

## Fixture labels and measurement limits

The [fixture panel](briefing-retrieval-fixtures.json) contains narrow inspection
queries and public issue records, with exact source ranges and rationales.
Only each fixture's `issue` object is sent to preview. Expected paths, ranges,
case IDs, and rationales remain evaluator inputs.

The hand-authored development/evaluation split is a convenience for diagnosis.
It is too small and too closely tied to this repository to establish a
population effect. Real issue rows test source retrieval, not whether an agent
can solve those issues. Later fixes and their descriptions are absent from
inputs.

The evaluator reports necessary-file hits separately from complete labeled
spans and covered source bytes. It also applies the same **16 KiB global source
budget** to each arm, consuming excerpts in returned order. This is an offline
comparison budget; the preview still writes its ordinary bounded excerpts.
The final admitted partial line contributes only its actual source bytes.
Original issue text remains complete and is outside this optional-source budget.
Untrimmed span scores are retained too. Preview latency measures the ordinary
rendered output; the 16 KiB scoring cut happens afterward.

A necessary file counts once per case even if it has several labeled spans.
A complete-span hit requires every byte of that labeled range under the budget.
This avoids crediting a file header as the implementation it contains. A
negative case has no positive-recall denominator; its no-match behavior and
unrelated source cost are recorded separately.

## Implementation and boundaries

Tree-sitter and its Rust grammar are existing repository dependencies. The new
extractor records parser, grammar, and extractor versions with declarations,
signatures, bodies, exact byte/line ranges, and parse limitations. These are
syntax facts, not resolved types, calls, trait implementations, or macro
expansions. The distinction follows the parser's
[syntax-tree API](https://tree-sitter.github.io/tree-sitter/using-parsers/2-basic-parsing.html).

The extractor supports functions, methods, structs, enums, traits, impls,
modules, type aliases, constants, statics, and macro declarations. Complete
declarations outside a damaged region can still be selected. Error-bearing
declarations are excluded from structural selection. Ambiguous exact names
retain an ambiguity count and use deterministic source order. Oversized
selected declarations stay capped and are marked partial.

A syntax preview requires a compatible syntax index. Source path/blob
bindings, exact bytes, and cached coordinates are checked before rendering.
The baseline remains the default.

## Run the comparison

On the approved build host, using its existing Cargo target directory:

```sh
cargo build -p briefing-lab --release --locked
export BRIEFING_LAB_BIN="$CARGO_TARGET_DIR/release/briefing-lab"

scripts/briefing-preview.sh --repo "$PWD" \
  --rev 43709aba33bab17d3aa6ab42102097c9d32db760 --syntax \
  --issue https://github.com/OpenAgentsInc/openagents/issues/10239 \
  --output-dir /tmp/briefing-syntax-preview

python3 docs/audits/2026-10-03-independent-efficiency/evaluate_briefing_retrieval.py \
  --repo "$PWD" --binary "$BRIEFING_LAB_BIN" \
  --fixtures docs/audits/2026-10-03-independent-efficiency/briefing-retrieval-fixtures.json \
  --output /tmp/briefing-syntax-study --repeats 6 --source-byte-budget 16384
```

The first command through the wrapper builds its index and acquires the issue.
Use `--index` and `--issue-file` for a warm local preview. Rebuild the index
when its revision, parser, grammar, or extractor changes. The wrapper never
builds the Rust binary on the owner's Mac.

## Results and verification

**Keep syntax selection opt-in.** It improves two exact-symbol inspection
queries, but the baseline retrieves too few necessary files, and an incidental
word match makes one real-issue excerpt worse. This is evidence for targeted
structural selection plus better retrieval and intent selection; it does not
justify making the current selector a default agent briefing.

The [raw results](briefing-syntax-measurements.json) retain all 180 timings,
per-case evidence ranges, syntax choices, budgeted and untrimmed scores,
index/source/binary hashes, and split summaries. The
[watched-dispatch example](briefing-syntax-example.md) shows one successful
structural selection. Inputs and policy were frozen before this run; there
was no retuning after inspecting the results.

### Coverage

| Panel | Labeled spans | Baseline complete spans | Syntax complete spans |
| --- | ---: | ---: | ---: |
| Hand-authored development | 4 | 0 | 1 |
| Hand-authored evaluation | 8 | 0 | 2 |
| Two real-issue diagnosis slices | 6 | 1 | 0 |
| All diagnostic cases | 18 | 1 | 3 |

Both arms retrieve **4 of 13 required case/file locations**. A file counts
once per case. The untrimmed results have the same complete-span counts as the
16 KiB comparison, so the shared byte budget does not explain these misses.
Overall labeled-byte recall rises from 1.98% to 13.55%, but remains low. Two
of nine positive cases contain all their labeled spans under syntax selection;
the baseline contains none. These are diagnostic counts, not agent pass rates.

The case details explain where to invest next:

- **`Grammars::classes`:** both arms retrieve the highlighting implementation.
  The baseline excerpt misses the method's guards; syntax selects the method
  and includes the required bytes at lines 204–215.
- **Watched dispatch:** both arms retrieve `coder-delegate/src/delegate.rs`.
  Syntax finds the requested method and covers both the input contract and
  pre-dispatch guard. The baseline starts at an unrelated match.
- **First-call bonus, snapshot opening, and several symptom queries:** the
  required files are absent from the selected pool. Moving excerpt boundaries
  cannot repair those nine missing case/file locations.
- **#10239 regresses:** the baseline covers one labeled amount-definition
  span. The syntax selector interprets the ordinary word `fixture` in the issue
  as an exact request for the function of that name and returns lines 397–401.
  The source bytes are correct, but their association with the request is poor.
  This is an identifier-intent failure, not a parsing error. Separately, the
  issue's required regions at lines 84–804 cannot all fit in one 64-line
  excerpt from that file.
- **#10248 remains incomplete:** the pool includes the chat client but misses
  two other required files; neither excerpt policy delivers a labeled span.
- **Absent-symbol case:** both arms still retrieve unrelated candidates and
  neither reports no match. They render approximately 33.4 KB and 28.9 KB of
  source, respectively. The tool makes no verified existence claim, but this
  is avoidable context that a presence test or stricter candidate policy could
  remove.

The syntax-index control returns exactly the baseline evidence. File order is
identical across all arms. Those checks separate the observed within-file
changes from retrieval changes.

### Time and preparation cost

The isolated Boat host used Linux x86-64, eight visible logical CPUs, pinned
Rust 1.97.1, and a release binary. Six repetitions per case produce 60 fresh
process measurements per arm, rotating run position equally.

| Arm | p50 | p95 | Maximum | At or above 1 second |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 273.0 ms | 319.2 ms | 363.1 ms | 0/60 |
| Syntax selection | 337.8 ms | 386.2 ms | 395.9 ms | 0/60 |
| Syntax index, baseline selection | 341.8 ms | 382.9 ms | 412.2 ms | 0/60 |

The richer-index control is close to syntax selection. This supports targeting
metadata loading/representation before optimizing the selector loop. It does
not establish a precise causal allocation of every millisecond. Preview timing
includes process exit and output writes; internal phase timers are nested.

Both indexes inspect the same 5,281 files and 67,108,849 source bytes. They
omit 200 entries under the budget and 22,182 generated/archive/unsupported
entries. The ordinary index is 19.82 MB; the syntax index is 43.66 MB. Syntax
metadata covers 2,485 Rust files, with one file reporting parse errors.
Ordered, single-build observations are **1.62 seconds** for the baseline and
**7.00 seconds** for syntax. They exclude clone and toolchain setup. These
numbers make upfront preparation cost visible; repeated build or update-cost
studies are still needed for amortization claims.

### Verification

On Boat:

- `cargo test -p briefing-lab --locked`: 7 parser tests and 16 integration
  tests pass.
- `cargo fmt -p briefing-lab -- --check`, release build, and shell syntax pass.
- All 180 preview processes succeed. Evidence fingerprints remain identical
  within each case/arm. Each final selection is independently verified against
  Git, including exact source bytes and the full original issue.
- Baseline evidence matches the richer-index control; syntax uses the same
  ordered file pool. Fixture and implementation hashes match retained inputs.

The cases cover qualified names, ambiguous trait methods, multiline signatures,
comments and macro tokens, incomplete parses, corrupt cached coordinates,
partial declarations, and unchanged baseline behavior.
No Mac Cargo build, paid inference, live engine run, or deployment was needed.

## Next isolated components

1. **Exact identifier-to-file lookup before lexical ranking.** Keep identifiers
   intact alongside split terms. Union their defining files with the lexical
   pool and measure necessary-file recall under the same evidence budget.
2. **Select intended identifiers.** Compare explicit quoted/code identifiers,
   caller-provided focus symbols, and a small typed semantic selection. The
   `fixture` regression is a fixed development example; use fresh evaluation
   cases after changing the policy.
3. **Pack several relevant spans from one file.** Preserve signatures,
   attributes, required fields, and relevant assertions under one global byte
   budget. Test #10239's distant regions without giving it extra budget.
4. **Load syntax metadata only for shortlisted files.** Compare compact or
   sharded metadata with the current JSON load. Preserve the same selections
   so representation cost is measured independently of retrieval quality.

A System One ranker should come after candidate recall is measured: it cannot
choose any of the nine required file locations missing from the current pool.
Once that pool is useful, narrow semantic questions can distinguish requested
behavior from incidental identifiers and combine spans around requirements.
The present experiment gives those next changes measurable inputs and failures.
