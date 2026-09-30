# Desktop local Coder verification

Issue: [#10014](https://github.com/OpenAgentsInc/openagents/issues/10014).

The [local broker design](../../local-coder.md) selects the existing same-user
socket. The resident host signs requests with the portable NIP-HOST client and
passes them through normal admission and the durable task owner. The desktop
and phone use `coder_access::client::tasks::Tasks` for typed task dispatch and
receipt validation. History uses the same `coder_connect::client::Client` as
the phone, including source-bound cursors, signed replies, and direct page
bounds. Only the configured Coder source is admitted. No secret enters the
window process, and task creation changes no execution policy.

## Checks

Rust 1.97.1 on an M5 Max Mac:

- The new `coder` integration test runs the real desktop `SocketControl`
  against a scratch resident host and the real durable `task::remote::Inbox`.
  It creates an inert queued task, retries the exact request, restarts the
  host, and retries again with the same task ID. It lists the task's recorded
  ATIF fixture, reads its page, and cancels the actual inbox task twice.
- The same test rejects changed operation bytes, non-task broker operations,
  invalid identities, an unknown project, a failed encrypted journal, a
  signing key that differs from the established owner, and an outside source
  selector. Failed admission creates no second task.
- `coder-access`: 36 tests passed; `coder-computers`: 66 passed;
  `coder-connect`: 43 passed, 5 opt-in checks ignored;
  `openagents-connect`: 21 passed.
- Existing host control tests: 8 passed. Existing enrollment, discovery,
  fallback, catch-up, and revocation scenario: 1 passed.
- Desktop library: 81 passed; binary: 30 passed, 4 opt-in checks ignored.
  These include pairing, retained rendering, commands, and the Grid backdrop.
- Phone Rust consumer: 142 passed, 19 opt-in checks ignored.
- Targeted formatting and strict all-target Clippy passed. Strict Clippy for
  the new integration target passed. No full release gate was required.

The final scratch acceptance run passed in 0.46 seconds. Its key source,
socket, relay, inbox, observer, cache, and transcript all live under temporary
roots. No engine, owner home, keychain, service registration, enrolled device,
or persistent owner conversation participates. The recorded ATIF transcript
checks observation; it does not claim that the queued task executed. The
existing debug `coder` linker reports its large unwind-table warning.

The socket client API is ready for the chat dispatch issue (#10015). The
remaining native run against an installed host and OS key source is recorded
in NEEDS_OWNER.md and accompanies that integration. The earlier command
verification retains the current foreground latency run; this change adds no
foreground painting or UI-thread network operation.
