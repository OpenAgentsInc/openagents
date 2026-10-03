# C/D eligibility follow-up

The targeted public test passes for C and D. Their first full historical-reference
checks still do not qualify either task: C reaches the unchanged 240-second
budget, and D fails a different ordinary integration test. The independent
checker runs and passes for D; C's independent checker is not reached.

The [complete timing record](eligibility-cd.json) retains both targeted runs and
both full attempts. These Linux checks make no model calls and produce no scored
executor outcomes. Historical source, independent checkers, and acceptance
requirements remain unchanged. The earlier [A/B preflight](acceptance-preflight.md)
remains a separate record.

## Follow the helper before inferring a defect

An initial source inspection matched B's ordering: `answer_in` calls `boundary`
before `answer_wrapped` creates the artifact directory. That suggested the old
missing-directory failure remained in C/D. Following the called helper corrects
that inference: C/D's
[`boundary` already creates the artifact directory](https://github.com/OpenAgentsInc/openagents/blob/e11b84c3adde42755c32fb478cc1895d13149042/crates/coder-delegate/src/terminal.rs#L356)
before adding it as writable. The relevant source files are identical between
each task's base and reference.

The same existing public test then passes for both untouched bases, with no skip
message. C takes 104.391 seconds to compile the library test target and 0.415
seconds for the selected test command; D takes 97.369 and 0.365 seconds. Total
setup and checking take 130.860 and 123.819 seconds, respectively. These runs
inject no independent checker and do not run the entire crate suite.

This is a concrete reason to include relevant helper behavior when preparing
source evidence. It is a corrected static inference, not a measured benefit
from a briefing or a claim that a targeted pass establishes full acceptance.

## Full reference checks

| Reference | Total acceptance | Ordinary compile | Ordinary test command | Independent check | Verdict |
| --- | ---: | ---: | --- | --- | --- |
| C | 240.002 s | 116.763 s | Unfinished; exact command duration not checkpointed | Not reached | Nonaccepted at deadline |
| D | 173.561 s | 117.416 s | Failed in 28.924 s | Passed; 1.117 s compile and 0.616 s test | Nonaccepted |

Both scope and formatting checks pass. Setup takes 24.748 seconds for C and
23.957 seconds for D. C's completed timings remain intact, but its outer
watchdog terminates the worker before the active ordinary command writes its
final duration. That duration is unknown, not zero. The raw log names
`task::owner::tests::a_live_owner_refuses_competitors_and_recovery_never_reexecutes`
as running for more than 60 seconds.

### C's deadline

Public-source review identifies a bounded scheduling stall. The reference makes
a competing `execute` wait synchronously for an owner lock for up to 120 seconds.
The existing `#[tokio::test]` uses Tokio's default current-thread runtime: while
that competing call waits, the first owner future cannot be polled on the same
thread. About 142 seconds of measured setup, formatting, and compilation precede
the ordinary test command, leaving too little of the 240-second cap to finish
that wait.

This explains the observed timeout; it does not establish an infinite deadlock
or eventual ordinary-suite success. A proposed 360-second qualification is canceled before launch after the
source comparison below identifies the same later stale fixture as D. The
original timeout stays nonaccepted. The increase is withdrawn before launch or scoring; the original 240-second
cap remains. No checker, quality requirement, or executor budget changes here.

### D's stale integration expectation

D fails `desktop_creates_retries_reads_and_cancels_through_the_phone_clients` at
[`desktop_local_coder.rs:119`](https://github.com/OpenAgentsInc/openagents/blob/4eee1cbf3ecc844f89d42abc2b7df7be4411c846/crates/coder/tests/desktop_local_coder.rs#L119).
The fixture replaces its scratch owner key and expects `create_task` to refuse;
it receives an accepted synthetic task ID. Line 135 propagates the worker panic.
The integration target reports two passes and one failure.

Read-only review traces this to the intentional same-account local-authority
policy change in commit `7310fc3bc1`, which left this fixture's earlier refusal
expectation in place. The client reads the current key from disk; the mismatch
selects the local-owner path for the authenticated same-account socket. The
relevant public fixture and implementation are unchanged between D's base and
reference. This is not a stale-key cache explanation. The strict ordinary gate
still fails: no fixture patch, skip, or relaxed verdict is applied.

C's base and reference have the same relevant desktop fixture, task-control, and
key-source blobs as D. A separate existing host-control test also expects the
new local-owner behavior. This predicts that C would encounter the same stale
expectation after its owner-lock wait finishes. That is source-based evidence,
not a measured C integration failure. It makes another full C run unnecessary
for the current strict-gate eligibility decision.

## Disk and cleanup

After D's checks finish and before scratch cleanup, a read-only size probe
observes:

| Item | Allocated bytes |
| --- | ---: |
| Acceptance target | 6,082,457,600 |
| Acceptance workspace, including its Git snapshot | 2,365,603,840 |
| Available disk at that point | 5,638,746,112 |

The size probe takes 0.099 seconds outside the acceptance timer. Only the private
preflight driver adds this observation; the frozen acceptance coordinator and
its deadline stay unchanged. The JSON binds both driver versions.

Native and acceptance scratch coexistence is **unverified**. A seed builder's
headroom check does not reserve disk for a later complete trial. The observed
acceptance scratch is material to runtime admission and cannot be treated as
just the size of the exported library seed.

All four completed or timed-out runs retain receipts and logs. After process
closure and hash verification, their reconstructible workspace/home directories
are removed; trusted seeds, source archives, and the shared target remain.
C/D full-check cleanup takes 1.400 and 1.479 seconds outside acceptance timing.
A subsequent read observes 14,088,126,464 free bytes, and the agent slot lock is
independently acquired and released. The private evidence archive is verified
locally; its digest is published without exposing reference patches or checkers.
