# Parity Fixture Corpus

Issue coverage: `VCAD-PARITY-006`

## Purpose

Create a shared, deterministic parity fixture corpus and seed set derived from the
machine-readable gap matrix.

Input:

- `crates/cad/parity/vcad_openagents_gap_matrix.json`

Output:

- `crates/cad/parity/fixtures/parity_fixture_corpus.json`

The corpus includes seeded `matched` and `missing` fixtures across:

- `docs`
- `crates`
- `commands`

## Commands

Generate/update fixture corpus:

```bash
cargo run -p openagents-cad --bin parity-fixture-corpus --
```

Full drift check (crawlers + matrix + scorecard + fixture corpus):

```bash
scripts/cad/parity-fixture-corpus-ci.sh
```

## Test Coverage

- `crates/cad/tests/parity_fixture_corpus.rs`
  - fixture schema/invariant checks
  - generation parity check against committed fixture
- unit tests live in:
  - `crates/cad/src/parity/fixture_corpus.rs`
