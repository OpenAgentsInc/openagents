# Remote access closure verification — September 27, 2026

This record closes the
[remote access program, #9704](https://github.com/OpenAgentsInc/openagents/issues/9704).
It checks the program's five completion criteria against `main`, adds the
wire fixtures NIP-HOST and NIP-REACH lacked, and names where the remaining
work is tracked.

## Evidence class

All evidence here is synthetic, from one macOS computer. The end-to-end test
uses a local test relay and loopback sockets. No production relay, second
computer, LAN, tailnet, simulator, emulator, or physical device was involved
in this record's checks. Simulator and emulator evidence for the client
screens lives in the [screens](2026-09-26-computers-screens.md) and
[live client](2026-09-26-computers-live.md) records.

## Completion criteria

| Criterion | Result |
| --- | --- |
| Every NIP gap has a draft with fixtures, and the NIP index lists it with honest status. | Met. NIP-HOST, NIP-REACH, and NIP-TERM are new drafts; the activity-summary section is in NIP-WS and the SSH-launched hosts section is in NIP-ENV. The index rows name what is implemented and what is not, such as WebSocket transport. NIP-TERM already had wire fixtures. This change adds [`nip-host.json`](../../../crates/coder-access/fixtures/nip-host.json) and [`nip-reach.json`](../../../crates/coder-reach/fixtures/nip-reach.json). |
| Each child issue lands on `main` with targeted tests and a verification record that separates evidence classes. | Met. #9705–#9713 and #9715 are closed, each with a record under this directory. |
| A synthetic end-to-end test enrolls, discovers, connects directly and through a relay, opens a terminal, creates a task, loses the connection, reconnects, and is refused after revocation. | Met. `crates/coder-host/tests/end_to_end.rs` passed again for this record. See the [host serve record](2026-09-26-host-serve.md) for each step's assertion. |
| The roadmap, glossary, and migration tracker describe the program and its status. | Met. [Roadmap](../../roadmap.md), [glossary](../../glossary.md#remote-access-and-host-reach), and [tracker](../migration-status.md). |
| No hosted account on any route; no credential, invitation, or private content in a public event, URL, or log. | Met by design and by the child records: every route uses Nostr keys and private `3188` artifacts, invitations are shown only locally, the relay PL executor sends a fixed wake, and activity summaries are redacted and encrypted. No log audit of a production deployment was performed. |

## New fixtures

The NIP-HOST fixtures give a valid grant (by invitation and by approval), an
enrollment request, a request for each of the eleven operations, and replies
for device listing, dispatch, revocation, and two refusals. Twenty invalid
bodies each name the refusal code they must produce. The test also checks
that each reply's outcome is one its request's operation can produce, and
that a missing-right refusal names the right the operation requires.

The NIP-REACH fixtures give valid directory, presence (with and without
telemetry), and hint bodies; twenty-one invalid bodies with their codes; five
placement vectors covering the highest score, weights, stale, overloaded,
zero-weight, unadmitted, incompatible, and telemetry-less hosts, and the
tie-break; and a channel transcript vector. The transcript digest was
computed independently in Python from NIP-REACH's written construction and
matched the crate's output before it was pinned.

Each new test was checked by breaking a fixture on purpose: a
non-canonical rights list and a wrong expected code in NIP-HOST, and a wrong
placement choice and a wrong expected code in NIP-REACH. The tests failed and
named each case, and passed again once the fixtures were restored.

## Checks that ran

From a fresh worktree of `origin/main`, with its own target directory and
debug info off:

```sh
cargo test -p coder-access --no-default-features --test wire
cargo test -p coder-reach --test wire
cargo test -p coder-access -p coder-reach
cargo test -p coder-host --test end_to_end
cargo clippy -p coder-access --no-default-features --tests -- -D warnings
cargo clippy -p coder-reach --all-targets -- -D warnings
cargo fmt -p coder-access -p coder-reach --check
```

Results: 3 NIP-HOST and 4 NIP-REACH fixture tests passed; the full
`coder-access` and `coder-reach` suites passed; the end-to-end test passed in
4.2 seconds; Clippy and formatting reported nothing. The workspace gate did
not run.

## Remaining work

[#9719](https://github.com/OpenAgentsInc/openagents/issues/9719) tracks
WebSocket direct channels, the CAP/CJ binding of NIP-HOST, one host
generation counter, owner-directory discovery and SSH hosts in the clients,
an end-to-end headless approval test, Linux systemd, SSH, and PTY runs, a
push gateway for real APNs and FCM wakes, and physical-device and
production-relay checks. Internal TestFlight build 0.5.0 (45), built from
`0a3d495114`, already contains the live Computers screens, so the iPhone
checks can run now; the owner's steps are in the workspace `NEEDS_OWNER.md`
and the [live client record](2026-09-26-computers-live.md#physical-devices-and-production-relays).
Android has no distributed build yet.
