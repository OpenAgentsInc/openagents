# Apple serving verification: partial matrix

The #9426 Apple verification run at
`65ca0b7d62b2779527f7e07782f57e40e39f4c0b` passed both Lev test configurations
and strict Lev and Kev Clippy. The requested weighted Kev `serve,metal` test
command failed. **#9426 remains open.** No test, timeout, gate, or model input
was changed to obtain these results.

## Machine and environment

Apple M5 Max, macOS 26.4 (25E246), with the real Swift helper present. The
controller ran from 2026-09-20 20:20:42 UTC to 20:40:59 UTC. Its separate Cargo
target was `/tmp/openagents-supervision/root-target`. The tracked checkout was
clean; `.claude/` was untracked. These are functional checks, not a quiet
latency experiment. Initial load averages were 5.77, 7.49, and 8.37; final
averages were 7.76, 4.88, and 5.37.

The controller removed artifact, variant, and bridge override variables before
setting `LEV_BRIDGE_BIN` to this checkout's real helper and `LEV_OS_BUILD` to
`25E246`. Only the final Kev test command received `KEV_TEST_DEVICE=metal`.
`codesign --verify --strict` passed for the helper, whose SHA-256 was
`c0a3d48cedc2214fd87bdfd8f2c7d5097088192d6f165327c6b1b50ecc7a764d`.
The run metadata retains commands, timestamps, load, exit codes, and hashes.

## Observed matrix

| OS and device | Command and feature coverage | Result | Scope |
| --- | --- | --- | --- |
| macOS, Apple FoundationModels | `cargo clippy -p lev --all-targets --features serve -- -D warnings` | Passed | Compile and lint; not inference evidence |
| macOS, Apple FoundationModels | `cargo test -p lev -- --nocapture` | Passed | Default features, real helper available |
| macOS, Apple FoundationModels | `cargo test -p lev --features serve -- --nocapture` | Passed | Real-helper conformance, isolation, pool, manifests, and fake-helper admission and supervision |
| macOS, Metal feature enabled | `cargo clippy -p kev --all-targets --features serve,metal -- -D warnings` | Passed | Compile and lint; not inference evidence |
| macOS, Metal selected for conformance | `KEV_TEST_DEVICE=metal cargo test -p kev --features serve,metal -- --nocapture` | Failed, exit 101 | All four variants completed Metal conformance; the CPU HTTP round-trip test timed out |
| macOS, bf16 | No bf16 command in this run | Unmeasured | The tested model loader defaults to F32 |

No executed test log reported a skipped or ignored test. That does not imply
coverage beyond the invoked features, devices, and test targets. The failed
Kev command prevents a green full-command result.

### Lev evidence

The `serve` run passed nine fake-helper admission tests, including the
observer's success and failure paths. The supervision test
`faults_retire_a_helper_and_the_lane_recovers` passed in 4.01 seconds, with
hang, stderr flood, oversized frame, wrong ID, crash, malformed response, and
replacement-helper recovery coverage. The real isolation test ran and reported
sibling 0.75, absent 0.75, and state 1.00 over eight seeds. These are this test's
rates, separate from the startup observations in
[`2026-09-20-admission-live.md`](2026-09-20-admission-live.md).

### Kev evidence and failure diagnosis

The weighted conformance target passed all six tests in 1,003.41 seconds.
Its output names `kev-0.5b`, `kev-0.6b`, `kev-4b`, and `kev-8b`, including
golden probabilities, packed versus separate requests, isolation, and
permutation observations. The shared loader in `tests/common/mod.rs` honors
`KEV_TEST_DEVICE=metal`; `DecisionModel::load` selects F32. This establishes
actual Metal F32 inference, not merely a Metal-feature build and not bf16.
Artifact paths came from the existing local fallback directories. This run
did not retain a separate full checkpoint-file digest inventory; fixture
conformance is not a substitute for such an inventory.

The API, encoding, and synthetic refusal targets also passed. The serving
target then passed three tests and failed one:

```text
thread '<unnamed>' panicked at crates/kev/tests/serve.rs:116:57:
system_one: Timeout { timeout: 10s }
thread 'jev_client_round_trips_all_types' panicked at crates/kev/tests/serve.rs:83:34:
client thread: Any { .. }
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

The source explains an important distinction: `tests/serve.rs` constructs its
model with `Device::Cpu` and labels that serving state `cpu`. It does not use
the conformance loader's `KEV_TEST_DEVICE` setting. Its blocking Jev client
keeps the default 10-second timeout, and this command builds the debug profile.
The observed failure is therefore a CPU HTTP round-trip deadline failure
inside a command with Metal features enabled. It does not show a Metal
inference deadline failure. Debug CPU execution is consistent with the known
limitation described in the #9426 Linux comment, but the log alone cannot
attribute all elapsed time to inference or quantify the cause.

The serving target took 79.88 seconds overall; that is not one request's
latency. The six preceding conformance tests took much longer and had already
finished. Cargo stopped on the failed serving target; no successful Kev doc-test
summary appears afterward. No release-profile rerun is claimed here.

## Remaining acceptance

The real Lev helper and fake-helper Mac checks are recorded. The requested Kev
command is not green, bf16 remains unmeasured, and this record does not certify
every possible OS/device combination. Keep #9426 open with the exact failure.
Do not increase the test timeout, hide weights, change its device, or relabel a
release-profile result as a pass of this debug command. The final workspace
`verify-rust.sh --with-metal` run is separate and is not established by this
matrix.

## Retained evidence

These files preserve the captured bytes. Logs contain local build paths and
fixed test inputs, not credentials. Each command log's digest was checked
against the controller result before copying.

| File | SHA-256 |
| --- | --- |
| [matrix-kev-clippy.log](2026-09-20-apple-serving-matrix/matrix-kev-clippy.log) | `5e2d225c0fb3fefc2b6a3e57c3cf8d1cf14f572b998901ac8a265a5a84d0c6e3` |
| [matrix-kev-metal.log](2026-09-20-apple-serving-matrix/matrix-kev-metal.log) | `53752cf9cf41945658490d663f8fe77103602a10beb780f79e235bcc4b1f4ac6` |
| [matrix-lev-clippy.log](2026-09-20-apple-serving-matrix/matrix-lev-clippy.log) | `08e7edd8b3a4dec5ea9cc1223a06c4e3b8ef08302de2485f8adff894a3fa9987` |
| [matrix-lev-default.log](2026-09-20-apple-serving-matrix/matrix-lev-default.log) | `a86916ba512a7364bbe477e310f3abf6820f7309166757767a9a82b51a68078b` |
| [matrix-lev-serve.log](2026-09-20-apple-serving-matrix/matrix-lev-serve.log) | `d9d571707c4994c4c6ebde219358ef5629ffdcd8f97de9112c9e886d40f23d8c` |
| [run.json](2026-09-20-apple-serving-matrix/run.json) | `a75c65fd65e8d4d35a778bbe9a1dfb4ca6a51be425df0bfaccf122ceebd1c96c` |
