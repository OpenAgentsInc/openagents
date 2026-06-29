# Probe Retained Terminal-Bench Fixtures

Date: 2026-06-08

Probe now carries the first public-ref-only retained fixture package for the
Terminal-Bench failure families used by GEPA Stage 0 and Stage 1. The package
lives in `packages/runtime/src/benchmark/fixtures.ts` and is exported through
the runtime package entry point.

## Fixtures

The initial retained package covers:

- `configure-git-webserver` -> `service_readiness`
- `db-wal-recovery` -> `database_recovery`, `sqlite_wal_recovery`
- `filter-js-from-html` -> `parser_correctness`, `xss_sanitizer_policy`
- `gcode-to-text` -> `parser_correctness`, `gcode_parser_guard`
- `pypi-server` -> `package_indexing`, `python_package_index`
- `query-optimize` -> `query_optimization`
- `runner-stall-supervision` -> `runner_supervision`

Each fixture records the task id, public benchmark-suite ref, retained split
membership, expected failure-family enum values, expected Blueprint Program
Signature refs, tool-menu constraints, closeout requirements, and public-safe
verifier/scorer refs.

## Boundary

These fixtures intentionally do not contain task prompts, task solutions,
expected answers, hidden verifier content, private Harbor traces, raw logs, or
private repository refs. They are retained optimization handles only. Passing
them can support GEPA Stage 0/1 candidate selection, but it is not a public
Terminal-Bench score and does not authorize public claims.

## Tests

`packages/runtime/tests/benchmark-fixtures.test.ts` verifies that the fixture
package loads, every fixture maps to typed failure-family enum values, every
fixture has expected Blueprint signature refs, Stage 0/1 retained membership is
present, closeout bundle requirements match the normalized writer, and hidden
task or Harbor trace material is rejected.

The retained fixtures are also the first target for the GEPA candidate
execution adapter. `runProbeRetainedBenchmarkCandidate` uses fixture
signature and tool-menu constraints to compare a baseline assignment closeout
with a supplied candidate closeout without letting candidate text widen
Blueprint authority or tool access.
