# Full Auto dogfood on a shared Mac

- Date: 2026-07-16
- Class: operator runbook and incident reconciliation
- Status: active
- Issue: [#8928](https://github.com/OpenAgentsInc/openagents/issues/8928)
- Authority: the Full Auto ProductSpec and current implementation; this file
  narrows operator process handling and grants no new runtime authority

## Incident disposition

The 2026-07-16 dogfood proved real autonomous continuation work, but the stop
evidence originally combined two different events.

- The first unexplained `enabled: false` row remains unattributed historical
  evidence. The then-current registry stored no source for an ordinary
  disable, so it cannot honestly be assigned to a user click, hydration, or an
  outside process after the fact.
- The second stop was operator-caused, not a spontaneous Full Auto failure.
  The coordinator intentionally called the control API `disable` for thread
  `cc139f6f` at `2026-07-16T16:00:51Z`, then sent `SIGTERM` at approximately
  16:01 to the confirmed orphan process group `63256` that still held port
  5734. Those two observations therefore do not support a renderer-clobber or
  unexplained-kill claim.
- The proposed renderer hydration race does not reproduce in current source.
  Mount/selection hydration calls `fullAutoHost.get` and only updates renderer
  presentation state. It never calls `set(false)`. The #8928 regression test
  models a control-API enable while no renderer is attached, attaches/selects
  the thread later, and proves hydration performs no write; only the explicit
  toggle intent can disable it.

Current code also records `disabledBy` and `disabledAt` in every new disabled
registry row. The sources are `ui_toggle`, `control_api`, `workspace_guard`,
`continuation_cap`, and `dispatch_failure_limit`. Main appends a durable thread
note for the UI path, as it already did for control and policy paths. The
registry refuses a new unattributed disable instead of recreating the original
ambiguity. Pre-#8928 rows remain readable because the added fields are
optional on decode.

## Supported shared-Mac posture

Use a dedicated macOS login session or a VM for unattended Full Auto whenever
possible. A visible development Electron window in the owner's interactive
login is suitable for attended testing, not dependable AFK process isolation.
A windowless/remote-view mode is not implemented; do not describe it as the
current solution.

When a shared login is unavoidable, each lane must have all of the following:

1. a dedicated absolute `OPENAGENTS_DESKTOP_USER_DATA` directory;
2. a dedicated worktree and launch terminal/session;
3. the opt-in control server and its private `full-auto/control.json`;
4. one named operator responsible for that instance's lifecycle; and
5. an exact PID plus opaque server-instance receipt from that connection file.

The current writer includes `pid` and a cryptographically random
`serverInstanceId` minted once for that control-server lifetime. Authenticated
`list` and `status` responses echo the same opaque ID. Treat an exact match
between the private file and a fresh authenticated response as the minimum
ownership proof; the credential's issuance timestamp is deliberately not
called an OS process-start time. A log line, window title, port, `ps`
substring, or Electron process name is not ownership proof.

The additions remain optional when decoding the v1 connection-file schema so
an older live app is still discoverable. An old file with either ownership
field absent grants **no signal authority**: status may be inspected and the
app may be stopped through its already-owned launch session, but cleanup must
refuse to infer a PID owner or send a signal from that file.
Cleanup automation must call the shared
`verifyControlProcessIdentity` guard in
`scripts/full-auto-control-client.ts`; it fails closed on missing fields,
unreachable servers, non-200 responses, and identity mismatch. It returns an
exact PID only after the authenticated live echo agrees.

## Safe launch and cleanup

Before launch, verify the chosen userData has no responding control endpoint
and no still-owned PID. Do not reuse another lane's userData to work around an
Electron `SingletonLock`.

For a normal stop:

1. call `full-auto disable <threadRef> --user-data <dir>` and retain the JSON
   response with `disabledBy: "control_api"`;
2. request `list` or `status`, require its authenticated `serverInstanceId` to
   equal the connection file's `serverInstanceId`, and fail closed if either
   identity field is absent or differs;
3. stop the exact launch session or child process that this lane created; and
4. verify that its PID is gone and the control endpoint no longer responds.

For a stuck process, prefer stopping the exact child handle retained by the
launch session. If an exact PID signal is the only remaining option, read
`control.json`, require both `pid` and `serverInstanceId`, make a fresh
authenticated `list` or `status` request, require the echoed ID to match, then
immediately re-read the file and require the same tuple before signaling that
exact PID only. Missing, mismatched, unreachable, or changing evidence means
**do not signal**. This guard prevents stale-file identity reuse; no userspace
check can eliminate the final process-exit/PID-reuse race as strongly as
retaining the original child handle, which is why child ownership is the
preferred posture. Never use `killall Electron`, `pkill`, a broad
`pgrep | xargs kill`, a process-group kill, or “whatever owns this port” as
shared-Mac cleanup. Parallel Desktop tests must use temporary isolated
userData and terminate only child PIDs they spawned.

Remove `Singleton*` files only after the owned PID is dead and its control
endpoint is unreachable. A losing second-instance launch can briefly log that
the control server listened before quitting; CLI/API liveness, not that log
line, is the readiness oracle.

## Verification contract

- `src/full-auto-hydration.integration.test.ts`: the real bearer-gated control
  server enables the real durable registry before renderer construction; the
  registry-backed renderer then hydrates without any `set` call and only an
  explicit toggle disables. This composes the production modules but does not
  boot a real Electron window/preload, which remains a higher-rung E2E gap.
- `tests/full-auto-registry.test.ts`: disable provenance/time persist, clear on
  enable, and an unattributed disable is refused.
- `src/full-auto-control-server.test.ts`: API disable projects
  `control_api`; current files emit PID/opaque identity; authenticated
  list/status echo it; legacy v1 files still decode without granting signal
  authority, and the shared verification helper rejects absent/mismatched
  evidence.
- `tests/full-auto-restart.e2e.test.ts`: toggle-off remains durable across the
  Runtime A / Runtime B reopen.

This proves attribution and process handling contracts. It does not turn a
shared interactive login into strong process isolation, and it does not erase
the remaining owner/release gates in the ProductSpec.
