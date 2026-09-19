# training

Training harnesses that are not Rust.

This workspace is Rust, and that rule has one exception per reason rather
than per convenience. `lev-adapter` is Python because Apple's adapter
training toolkit is Python and there is no other way to produce a
`.fmadapter` package; `swift/lev-bridge` is Swift because Apple's
`FoundationModels` framework has no Rust binding.

`baseline` is Python for a different and weaker reason: it is a measurement
rather than a shipped path. It exists to answer whether frozen sentence
embeddings plus logistic regression beat the models we trained, and the
encoders and the regression both live in the Python ecosystem. Nothing in the
product depends on it. If the answer had been no, the directory would have
been deleted; the answer was closer than expected, so it stays as the record
behind [`docs/decision-models/2026-09-19-frozen-embedding-baseline.md`](../docs/decision-models/2026-09-19-frozen-embedding-baseline.md).
Its scorer is a port of `score` from `crates/gym/src/calibrate.rs`, verified
against that module's own test fixtures, so a baseline is never scored by a
different scorer than the doors it is compared against.

`score-probe` is Python for the same weaker reason, and a narrower one: it
never links a crate. It asks each running door the same Score questions over
HTTP and counts three things about the answers, to decide whether a Score's
weighted mean is a position at all. Its output is read by
[`docs/decision-models/2026-09-19-score-ordinality.md`](../docs/decision-models/2026-09-19-score-ordinality.md)
rather than by a binary. If one of its statistics earns a place in a gate, it
belongs in `crates/gym` in Rust.

Everything the adapter harness produces is consumed by Rust, and the
contracts between them are tested: `check_parity.py` proves the Python
renderer matches `crates/lev/src/schema.rs` character for character, and
`crates/lev/src/adapter.rs` reads the package the toolkit writes.
