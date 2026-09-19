# training

Training harnesses that are not Rust.

This workspace is Rust, and that rule has one exception per reason rather
than per convenience. `lev-adapter` is Python because Apple's adapter
training toolkit is Python and there is no other way to produce a
`.fmadapter` package; `swift/lev-bridge` is Swift because Apple's
`FoundationModels` framework has no Rust binding.

Everything either harness produces is consumed by Rust, and the contracts
between them are tested: `check_parity.py` proves the Python renderer matches
`crates/lev/src/schema.rs` character for character, and
`crates/lev/src/adapter.rs` reads the package the toolkit writes.
