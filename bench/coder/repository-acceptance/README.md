# Fresh repository adapter acceptance

These original Rust fixtures exercise public repository admission, model
execution, retained traces, and independent completion. They are not
Terminal-Bench tasks or a model quality benchmark. The initial sources contain
intentional bugs. Give the executor only one fixture's source and prompt.

| Fixture | Task prompt | Independent behavior |
| --- | --- | --- |
| `ceil` | Fix `ceil_div` in `lib.rs` for the documented contract over all u64 inputs. Preserve the public signature. | Zero capacity returns None; exact divisions remain exact; nonzero remainders round upward without overflow. |
| `range` | Fix `inclusive_span` in `lib.rs` for the documented contract over all i64 inputs. Preserve the public signature. | Inclusive length includes both endpoints; inverted intervals and a count greater than u64::MAX return None without signed overflow. |

The external [checker](../../../crates/coder/examples/repository_acceptance_check.rs)
compiles frozen tests from outside the candidate and runs the candidate under
the existing read-confined, read-only, offline filesystem boundary and process
supervisor. Only private scratch is writable. Each compile/run has a 20-second
wall limit, 1 GiB memory limit, and 16 KiB output cap. On macOS the existing
offline boundary allows localhost; it denies external IP connections. No
provider credential is forwarded into the checker children.

Build the checker with `OPENAGENTS_ACCEPTANCE_RUSTC` set to the absolute pinned
`rustc` executable. Install separate copies named `repository-check-ceil` and
`repository-check-range` outside both candidates, then pin each executable's
digest in its capability manifest and task requirements. The trusted role name
selects the compiled-in suite; a candidate file cannot choose a different suite.
The checker receives only the exact candidate snapshot digest and emits typed
`openagents.verification.v1` evidence with its own executable digest. Missing
prerequisites, changed snapshots, and confinement failures remain unverifiable.

Freeze the requirement, source, and checker identities before each model run.
Retain both success and failure, native model requests/responses without
credentials, complete ATIF events, costs with unknown totals explicit, artifact
bytes, and the separate check report. A successful model finish is not acceptance.
