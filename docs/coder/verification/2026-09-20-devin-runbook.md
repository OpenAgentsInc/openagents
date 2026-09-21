# Workstation validation of the Devin delegation runbook

The [runbook](../guides/devin-delegation-runbook.md) was exercised on an Apple silicon
Mac with Devin CLI `3000.10.31`, the `sandbox-exec` boundary, and hosted Jev
reporting `jev-1.13.0`. The Coder runtime and delegated baseline were
`db22fead007a3bffd43842bdd7639b270da4b732`. The new runbook, smoke list,
checker, and test-client fix were developed during this observation.

The result supports immediate **supervised dogfooding**, with the supervisor
responsible for disjoint task selection, independent artifact review, serial
integration, and retry accounting. It does not establish unattended backlog
execution, a global account limit, or automatic acceptance of arbitrary patches.

## Earlier experience used

The supplied Devin Cloud parent session was retrieved through the organization
session API, along with all 75 messages returned by the paginated message API.
Operational delegation excerpts informed this procedure. The raw session and
authentication material remain outside the repository; they are not published
as evidence here.

That history contained two different mechanisms: hosted child sessions created
by a Cloud supervisor, and Coder programs spawning the local Devin CLI, later
through `coder-worker`. This runbook tests the second mechanism on an operator's
computer. Lessons carried forward include selecting disjoint ready work,
separating executor state from approved binaries, using the correct trust store,
retaining writing worktrees, reviewing only commits after the scratch root, and
checking the actual delegation transport. Routing generation over a relay did
not by itself establish that the delegated work crossed it.

Historical proofs remain in the [observed fan-out record](../measurements/2026-09-20-observed-fanout.md)
and [relay transport record](../measurements/relay-transport.md). They are distinct from the
new runs below.

## New runs

| Run | Observed result |
| --- | --- |
| Initial direct six-task attempt | Six boundary refusals because the writable state was inside the approval store's protected parent. No executor ran. Coder still returned an answered turn with exit `0`. |
| Direct six-task retry after separating approvals | Six real Devins answered. The prompt supplied no explicit expectations, so all six completion verdicts were unverifiable. The old model-card count question was ambiguous after the document gained more revisions. This was transport evidence, not a graded pass. |
| First writing plan | Independence `0.67`, below the unchanged `0.7` floor; no task ran. |
| Revised six-task writing batch | Six answered, six text expectations matched, and six direct Devin processes were observed concurrently. Total invocation wall time: **807.15 s**. |
| Additional test-client repair | One additional Coder-delegated Devin produced the macOS HTTP test-client fix in a retained scratch commit. Its delegation took **319.13 s**. |
| Two-task relay check | Two answered and both expectations matched in **22.22 s**. |
| Local smoke, first run | Six answered and six expectations matched in **67.13 s**; six direct Devin processes observed concurrently. |
| Local smoke, consecutive repeat | Six answered and six expectations matched in **57.39 s**; six direct Devin processes observed concurrently. |
| Six-task relay check | Six answered and all expectations matched in **20.42 s**; six direct Devin processes observed concurrently under the worker. |

The revised writing tasks all created separate new artifacts against the same
fixed baseline. No task consumed another task's output. Five supplied runbook
sections; one supplied the checker and its tests. Each task committed only its
assigned files, and every scratch repository was clean when inspected. The
supervisor fetched all six tips, reviewed their diffs against their seeded
roots, condensed the prose into one runbook, and strengthened the checker.
The additional repair occupied a verified free slot while the original batch
was finishing. Coder did not schedule that refill automatically.

The two local smoke runs used [the explicit smoke work list](../examples/devin-smoke.work-list.json).
The six independently checked answers were `5`, `calibration, development,
locked`, `L1, L2, L3`, `2024`, `30182`, and `3`. SHA-256 hashes of those six
input files were unchanged before and after both runs. This is an input-file
comparison, not a full filesystem snapshot or a claim of no external effects.

The relay test used a disposable PostgreSQL cluster, a loopback relay with
NIP-42 authentication required, distinct coordinator and worker identities,
and a worker allowlist containing only the coordinator. The worker ran the
same approved local executor. The coordinator received a minimal environment
with no Devin login, no local capability approval, and no Devin on `PATH`.
The trace identified `devin-relay` and recorded request IDs corresponding to
the worker's job logs. This tests the remote-worker protocol on one physical
host; it does not measure a WAN deployment or remote patch transfer.

## Verification and fixes

The new checker passes **22 standard-library Python regression tests**. It
accepts successful local and relay records and rejects malformed or interrupted
logs, incorrect programs or counts, duplicate calls, empty outputs, missing
expectations, mismatched answers, and unsuccessful delegations. It rejected the
actual boundary-refused, ungraded, and independence-declined records above.
It checks recorded execution evidence and recomputes the stated text match;
it does not build or inspect a delegate's patch.

The original manual Rust gate passed formatting, both Clippy configurations,
default and feature tests, minimum compiler checks, and dependency policy, then
failed PostgreSQL gateway acceptance. An isolated retry reproduced macOS
`ConnectionReset` in the test HTTP client's `read_to_end`. The server had
closed a refused upload without draining its body. The reviewed repair accepts
that reset only after the complete declared response has arrived. Truncated,
malformed, and inconsistent responses still fail; production relay behavior
and the existing protocol assertions are unchanged.

The final integrated `./scripts/verify-rust.sh` run **passed** after rebasing
onto `82cafc0bbb`, including the newer gateway commits on `main`. It passed
all 22 checker tests, formatting, both Clippy and workspace-test configurations,
Rust 1.95 workspace and Rust 1.94 Kev checks, dependency policy, and the full
disposable PostgreSQL acceptance suite. PostgreSQL acceptance took 55.9 seconds
and included the gateway contract, multiprocess, backup/import, release-load,
and binary deployment checks. Optional Metal and long-running soak checks
were not run; conditional external-model tests do not establish live inference.
The hosted Jev and Devin observations above are separate live measurements.

The worker setup documentation now uses the same isolated `XDG_DATA_HOME` for
trust and execution, handles optional startup-state files, and keeps the
approval store in its own protected directory.

The temporary relay, worker, and PostgreSQL cluster were stopped after the
live tests. No per-task entries remained in the isolated Devin trust list.
The configured local executor state and private evidence were retained for
the operator's next supervised run.

## Evidence and limits

The [derived observation summary](2026-09-20-devin-runbook.evidence.json)
contains statuses, answers, timings, and SHA-256 digests of the original ATIF
logs. Raw logs, CLI state, work-list captures, process samples, and downloaded
Cloud transcripts remain in the operator's private storage. The summary is a
derived report, not a signed receipt or an independently replayable snapshot.

Process samples count direct Devin children of the owning Coder or worker
process, excluding the CLI's own child wrapper. The writing batch's Coder
summary reported 798.6 seconds of fan-out wall time and 3508.8 seconds of
summed delegation time. That ratio is not a measured speedup against a
sequential baseline. These are bounded workload observations, not throughput
or reliability guarantees for arbitrary tasks.

No native Devin Cloud session adapter, durable scheduler, remote artifact
transport, program-wide authority policy, or independent automatic artifact
verifier was added. The open implementation dependencies remain listed in the
runbook.
