# Explicit root mutation scope — 2026-10-30

`coax mutate ENTRY --root-only` selects reachable procedure mutation sites
owned by the physical entry source. The parser already records that ownership
through source-unit/diagnostic provenance; no lexer, parser or type-checker
semantics changed. The small selection predicate is checked Coax in
`coax/lib/compiler/mutation-schedule.coax`, with regenerated C/header seeds.
The C++ driver observes that existing provenance, collects selected sites,
and labels the report and shard receipt.

All unmutated baseline checks still execute, including dependency checks.
Existing authority/execution-coverage requirements remain. The baseline,
per-mutant scheduling safeguards, check budgets and default reachable-graph
inventory are unchanged. A root-only zero-survivor score says nothing about
dependency mutation coverage; cite their separate passing gates. An empty
selected inventory still fails. Merge remains receipt aggregation; it binds
the complete loaded source text and explicit mutation scope rather than
rerunning checks. Pass `--root-only` to each shard and its merge.

## Focused qualification

A verified frozen compiler from main before this change (SHA-256
`df4d78ef4ddc8f788cb4a87da31d538a2ecf174af14085a5f34ab1d868a80fb0`)
served as the unchanged-behavior reference and generated the updated Coax
compiler core. The targeted development compiler is
`/tmp/coax-root-only-tools/coax`; its binary and changed source identities
are in [data/mutation-root-only/inputs.json](data/mutation-root-only/inputs.json).
This is not a fabricated fully checked bootstrap/release-cache record.
Existing native demo binaries remain frozen and were not rebuilt.

Actual changed-scope results:

- Coax scheduling unit: **11 checks passed, 63/63 mutants killed**. Its new
  predicate check first failed because the predicate did not exist, then
  passed after implementation. Logs are in
  [data/mutation-root-only](data/mutation-root-only/).
- **9 focused Python/compiler regressions passed** across targeted runs
  (the initial seven, then default-receipt compatibility and scoped parallel
  parity). The initial root-only test failed against the frozen compiler,
  which correctly did not know the new flag. The tests live in
  `coax/tests/mutation_scope.py` and the standard compiler test driver runs
  them. Set `COAX_TEST_COMPILER` to the candidate and optionally
  `COAX_BASELINE_COMPILER` to a frozen reference.
- Default report, first-killing-check trace, scheduling facts and exit status
  matched the frozen compiler byte for byte on a real composed fixture.
  Its default inventory remains **9 sites, 5 killed, 4 surviving**; the four
  dependency dead-branch survivors are intentionally visible in this test.
- Root-only selected exactly the **2 root sites, both killed**, excluding
  direct and transitive dependencies with the same basename as the root.
  Root defects and a failing imported check still fail the baseline; no-site
  filtering does not report success. Serial/parallel and shard/merge results
  matched within the scope.
- Default shard receipts matched the frozen compiler byte for byte and old
  default receipts still merged. Root-only/default receipts could not mix,
  including a no-import fixture with identical inventories in both modes.

Commands stayed below 15 minutes. No frozen model, crypto or unrelated
frontend mutation gate was repeated. The full bootstrap/release cache remains
separate qualification; the normal Python-free bootstrap consumes the updated
seed and checked Coax source.
