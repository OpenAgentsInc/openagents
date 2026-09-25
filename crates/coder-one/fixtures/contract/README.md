# Contract extractor fixtures

`instruction.md` is a synthetic task, not a benchmark task. The tests in
`crates/coder-one/src/checks/contract/tests.rs` pair it with an in-memory
untouched workspace, check every item the extractor draws from it, and
run those items against in-memory candidates.
