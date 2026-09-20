# Apple workspace gate: feature-enabled Coder tests failed

The required `./scripts/verify-rust.sh --with-metal` run ended with exit 101.
The default workspace test command passed, including all six Kev conformance
tests in 5724.45 seconds. The next, feature-enabled workspace test command
failed two Coder library tests. The gate stopped there; its later Metal phase
was not reached.

This record retains the [complete log](2026-09-20-final-workspace-gate/gate.log),
[run metadata](2026-09-20-final-workspace-gate/run.json),
[controller](2026-09-20-final-workspace-gate/controller.py), and
[script as invoked](2026-09-20-final-workspace-gate/verify-rust.sh).
It reports this actual run, not a rerun or the later progress-reporting script.

## Invocation and environment

Command: `./scripts/verify-rust.sh --with-metal`.
Source at launch: `4d8d3dd787e934f9ad26ef1151dbd11ea66d8c5d`.
Launch status contained only the untracked `.claude/` directory. Runtime:
2026-09-20T21:26:14Z to 23:05:02Z, 5927.405 seconds overall. The pinned gate
identified Rust 1.97.1 and rustfmt style edition 2024.

The controller used `/tmp/openagents-supervision/root-target`, explicitly
selected the real repository Swift bridge, and set `LEV_OS_BUILD=25E246`.
It removed inherited Kev device, variant, and artifact-directory overrides,
and Lev unsigned-helper and deadline overrides. In particular,
`KEV_TEST_DEVICE` was unset: the completed default Kev conformance phase was
CPU execution, not evidence that the later Metal phase ran.

Load averages at launch were 2.916, 2.832, and 3.445; at completion they were
10.892, 10.661, and 14.825. These endpoint readings do not establish load
throughout each test and do not prove why either assertion failed. The
source hash records only the launch checkout. The retained
[shared-checkout reflog](2026-09-20-final-workspace-gate/shared-checkout-reflog.txt)
shows changes during this run: `75068dece6` at 21:28:08 UTC, `638b6722a4` and
`6ee4af70fb` at 22:05:49 UTC, further commits from 22:12 to 22:30 UTC,
`d725754bd4` at 22:37:57 UTC, and `36e6649b65` at 22:50:19 UTC. The reflog's
local timestamps use UTC−05:00.

This was therefore **not a single-revision verification run**. The controller
did not snapshot sources or record a revision before each Cargo invocation.
The log proves the commands' observed outcomes, but it cannot establish the
exact source revision used by every later compiled phase. Neither the launch
revision nor the final checkout can be credited with a complete gate result.

## Completed and incomplete phases

| Phase | Observed result |
| --- | --- |
| Artifact acquisition Python tests | Passed, nine tests |
| `cargo fmt --all --check` | Passed; the fail-fast script continued |
| Default workspace strict Clippy | Passed |
| Feature-enabled workspace strict Clippy | Passed |
| Default workspace tests | Command passed; standard ignored doctests remain identified in the log |
| Feature-enabled workspace tests | Failed in Coder library: 168 passed, two failed |
| Rust 1.95 workspace check | Not reached |
| Rust 1.94 Kev check | Not reached |
| Dependency policy | Not reached |
| PostgreSQL acceptance | Not reached |
| Metal strict Clippy and tests | Not reached |
| Relay soak | Not requested; its final reporting branch was not reached |

Both feature-enabled commands used
`kev/serve,lev/serve,gym/tui,jev/blocking`. A failure in the feature-enabled
workspace command means later crates in that command are not established as
passing by this run. Separately recorded checks are separate evidence and do
not turn this gate into a pass.

## Exact failures

`delegate::tests::a_timed_out_delegation_ends_its_descendants`, at
`crates/coder/src/delegate.rs:2217`, reported:

```text
assertion `left == right` failed
  left: ""
 right: "partway through"
```

`delegate::tests::the_fan_out_is_concurrent_under_its_bound`, at
`crates/coder/src/delegate.rs:2310`, reported:

```text
a width of one is the sequential case: 2.531683875s against 3.7654655s
```

The Coder library result was:

```text
test result: FAILED. 168 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.67s
error: test failed, to rerun pass `-p coder --lib`
```

No assertion, timeout, feature, or device setting was changed to make this run
pass. No rerun was performed to prepare this record.

Complete-log SHA-256:
`ee684f02eec098e448b0b321e740338ab1892e21c131b3f2ca4a09b67f7b66c7`.
The [hash inventory](2026-09-20-final-workspace-gate/sha256.json) also covers
the exact metadata, controller, and invoked gate script.
