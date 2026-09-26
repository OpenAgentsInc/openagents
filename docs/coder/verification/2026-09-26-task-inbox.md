# Durable local task inbox verification

Date: September 26, 2026. Scope:
[#9672](https://github.com/OpenAgentsInc/openagents/issues/9672), the first
M0/M2a/M3a slice in the [migration tracker](../migration-status.md).

**The local inbox passes its acceptance gate.** It records, lists, inspects,
and cancels queued requests through the real `coder task` CLI. Requests and
receipts survive process restart; identical retries return the original
receipt; conflicting command bytes and stale changes refuse. It does not
execute tasks, interrupt an agent, verify a candidate, or implement a Nostr
controller. Execution ownership remains
[#9673](https://github.com/OpenAgentsInc/openagents/issues/9673).

## Implementation and source identity

The implementation is fresh public Rust derived from the behavioral
requirements in the [migration assessment](../design/coder-suite-migration.md),
not copied private code. It follows the public scheduler's atomic-state
pattern and uses the public Nostr crate's strict JSON and digest primitives.
The local exact-byte command format is distinct from NIP-SESS and NIP-CTRL.

- [Task contract and store](../../../crates/coder/src/task.rs).
- [CLI dispatch](../../../crates/coder/src/task_cli.rs).
- [Persistence and fault tests](../../../crates/coder/src/task/tests.rs).
- [Independent-process CLI tests](../../../crates/coder/tests/task_inbox.rs).
- [Usage and recovery guide](../guides/tasks.md).

The [retained CLI proof](2026-09-26-task-inbox/cli-proof.json) records the base
commit, platform, Rust version, and SHA-256 hashes of 14 source, configuration,
and fixture files. These hashes identify the tested implementation independently
of later documentation edits. The base is `21a67d1bd3`; the implementation
was tested as a staged change against that revision. The
[manual gate receipt](2026-09-26-task-inbox/gate.json) retains the initial tree
diff digest, commands, phase results, and coverage exclusions.

## Acceptance evidence

| Check | Result |
| --- | --- |
| Core persistence/state tests | 17 passed: reopen, exact retries, global identity conflicts, revisions, closed commands, full-state reconstruction, bounds, private files, lock replacement/contention, concurrent initialization, and fault recovery. |
| Real CLI process tests | Seven passed: submit/show/list/cancel, restart, eight simultaneous submitters, competing cancellations, conflicting retry bytes, malformed input, corruption, private file modes, and no agent trace creation. |
| CLI argument tests | Two passed in the workspace suites: supported stdin/store syntax and rejection of ambiguous or unsupported operations. |
| Retained CLI demonstration | Seven commands: submit twice, list, show, cancel twice, show. One task remains; both retries return their original receipts; final state is cancelled, execution not started, checks not run. |
| Repository manual gate | All 12 standard phases passed, including PostgreSQL. The receipt labels the run `partial` because optional Metal and soak phases were not requested. Run `20260926T130323Z-9fe550`; elapsed 1003.7 seconds. See the exact phase results and exclusions in the receipt. |
| Documentation and fixtures | Local links and anchors checked; all 22 migration package IDs appear once in the tracker; JSON fixtures are exercised by the CLI tests. |

The manual command was the unscoped `./scripts/verify-rust.sh` on macOS with
the pinned Rust 1.97.1 toolchain, Homebrew Python 3.13 first on `PATH`, and a
separate target directory for this worktree. `CARGO_BUILD_JOBS=4`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, and
`CARGO_PROFILE_TEST_DEBUG=0` bounded build resources without removing gate
phases. The ordinary gate covers default and feature-enabled workspace
Clippy/tests, tooling checks, dependency policy, and disposable PostgreSQL
acceptance.

The optional Metal and relay-soak phases were not requested. Model-dependent
tests with unavailable helpers or weights do not establish live inference.
No paid model call, benchmark cohort, phone test, OS installation, distributed
ownership trial, or Linux run was performed for this inbox acceptance.

## Failure cases resolved during implementation

Independent source review found and resolved these persistence gaps before
final verification:

1. Opening an unsafe existing directory must refuse without changing its
   permissions. Test fixtures now create private directories explicitly.
2. A second process can acquire a new lock before its creator initializes
   state. It releases the lock and waits within a bound; it never treats
   missing state as permission to create a replacement inbox.
3. New directory entries require parent-directory sync. Reopening also
   completes file, lock, and ancestor-directory sync before returning an
   acknowledged receipt, including after a previous rename succeeded but
   its durability barrier failed.
4. Missing lock files must refuse repeatedly without recreating a lock or
   allowing a later attempt to bypass the refusal. The first-open check
   rechecks the lock when a concurrent initializer creates state.

Fault tests inject failures before and after rename and at the reopen
barrier. A failed write poisons that handle; reopening validates authoritative
state before retrying the exact command. Pending candidate files never
become accepted work. The tests exercise software-visible failure boundaries;
they are not a physical power-loss certification of every filesystem.

A preliminary gate run was stopped when review added the final initialization
check. The final receipt above is the release verification, not the interrupted
run. Source hashes were checked again before committing.

## Remaining boundaries

The authority boundary is one local OS user and a supported local filesystem.
The store detects malformed or internally inconsistent state; it is not
cryptographic attestation against an authorized user rewriting the whole
store consistently. It does not provide multi-host fencing or a network
filesystem guarantee.

Requested workspace, source revision, adapter, and model are inert intent.
They require separate source, grant, budget, capability, and effective-setting
admission before execution. Current terminal/headless conversations do not
consume this queue. The next owner issue must establish dispatch, uncertain
effects, cancellation versus confirmed stop, and recovery with a bounded
executor fixture before the Microcoder adapter or remote client can use it.
