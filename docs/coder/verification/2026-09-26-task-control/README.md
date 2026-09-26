# Scoped task control: code verification

This record supports the bounded local-owner bridge in
[#9691](https://github.com/OpenAgentsInc/openagents/issues/9691), as described in
the [runtime guide](../../runtime/nostr-task-control.md). It records synthetic
code verification, not a model study, a production deployment, mobile platform
acceptance, or full CTRL/CTX/SESS/ENV/RUN conformance.

| Check | Result | Retained output |
| --- | --- | --- |
| Control library and CLI targets | 16 tests passed; this count includes the child entry point that the parent invokes in a separate process | [Control tests](control-tests.log) |
| Existing labor tests after extracting shared transport | 12 passed, including authenticated artifact reconnect and bilateral task/check acceptance fixtures | [Labor regression](control-labor-regression.log) |
| Strict Clippy, all targets: Nostr, control, transport, labor | Passed | [Clippy](clippy.log) |
| Client-only build and strict library Clippy, without host defaults | Passed | [Portable client](client-only.log) |

The final control fixture uses two authenticated identities and actual local
WebSocket connections. A client pairs, sends exact encrypted UTF-8 instruction
bytes, observes a bounded synthetic task, and cancels it. The test asserts that
the process group was cleared and the delayed write did not occur. A separate
child process then reopens the serialized host, resolves an existing finite-cut
cursor, and returns a signed page that the parent verifies. It does not use the
parent's host object or a replacement task.

Deterministic failure cases cover an uncertain local effect, a failure before
local dispatch, a failed final store write, exact retry at the source retention
limit, post-fetch command expiry, expired CJ dispatch, changed grant rights in a
valid-JSON store, stale generations, revoked grants, widened pairing rights,
changed original envelopes, and substituted instruction carriers. Refusals
preserve the local task revision; an uncertain result is not promoted to a
successful stop or a new attempt.

[manifest.json](manifest.json) pins the changed source files and these retained
outputs. The tests ran in the shared worktree on top of the stated base commit;
the source hashes identify the uncommitted implementation that was tested. The
full release gate was not run or claimed. The applicable focused checks follow
the current repository policy for day-to-day changes.

The [initial loopback receipt](initial-loopback/receipt.json),
[signed events](initial-loopback/signed-events.json), and
[ATIF](initial-loopback/task.atif.jsonl) retain the first fixture proof. That
proof predates the final text carrier, separate-process fixture, and durability
regressions. Its source was not separately sealed, so these earlier bytes must
not be presented as outputs of the final source hash set. The subsequent code
test logs provide the final implementation's verification evidence.

All fixture children were stopped and joined. The test relay ran within the
test process; its runtime exited with that process. No model calls, benchmark
cohort, background control service, or platform rehearsal was started by these
checks. Monetary cost remains unknown; zero model calls is not an invented
zero-dollar cost measurement.
