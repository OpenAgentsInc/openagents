+++
v = "openagents.eval-case.v1"
description = "A rustc type error pasted from cargo build: the function it is in is only in the source."
kind = "should-fire"
tags = ["rust", "compiler"]

[run]
allowed_operations = ["read", "write"]
+++

cargo build fails with this. What is wrong?

```
   Compiling ledger v0.1.0 (/home/dev/ledger)
error[E0308]: mismatched types
 --> src/ledger.rs:9:24
  |
9 |         let cap: u64 = limit;
  |                  ---   ^^^^^ expected `u64`, found `&str`
  |                  |
  |                  expected due to this

error: could not compile `ledger` (lib) due to 1 previous error
```
