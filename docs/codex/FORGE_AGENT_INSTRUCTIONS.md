# Forge Agent Instructions

This document is for agents operating inside the `openagents` repo on behalf
of OpenAgents engineers.

Use it when the task is about the current internal Forge shared coding lane:

- find a shared hosted coding session
- attach the local shell to that session
- inspect who currently controls it
- request or accept or force a handoff
- leave typed collaboration notes

Do not use the desktop UI as the primary automation path for those tasks.
Use the Forge CLI first.

## Why

The current internal Forge MVP is now exposed through a standalone CLI path:

- `autopilotctl forge ...`
- backed by the app-owned desktop-control contract
- with no-window autostart through `autopilot_headless_forge`

That means an engineer's agent can operate the shared-session layer directly
from a terminal without relying on a visible desktop window.

## What This Path Owns

Use Forge CLI for:

- hosted session discovery
- attach by shared-session id
- attach by Probe session id
- controller status
- controller handoff
- collaboration notes

Do not pretend it owns more than it does.

Do not use Forge CLI as the primary interface for:

- evidence bundle authoring
- delivery receipts
- campaign or bounty bookkeeping
- settlement state
- higher-level product-shell review flows

Those still live mainly in the desktop shell.

## Default Rule

If an engineer asks an agent to work with an existing shared coding session,
default to this path:

1. verify the Forge lane is reachable
2. discover visible sessions
3. attach to the intended session
4. read status back
5. only then mutate controller state

Always read back state after any write.

## Preferred Commands

If the binaries are not built yet:

```bash
cargo build -p autopilot-desktop --bin autopilotctl --bin autopilot_headless_forge
```

Smoke the standalone lane:

```bash
scripts/autopilot/headless-forge-smoke.sh
```

Discover sessions:

```bash
target/debug/autopilotctl forge hosted sessions --json
```

Attach by shared-session id:

```bash
target/debug/autopilotctl forge hosted attach-shared <shared-session-id> --json
```

Attach by Probe session id:

```bash
target/debug/autopilotctl forge hosted attach-probe <probe-session-id> --json
```

Inspect current status:

```bash
target/debug/autopilotctl forge status --json
target/debug/autopilotctl forge status --thread-id <probe-session-id> --json
```

Inspect handoff state:

```bash
target/debug/autopilotctl forge handoff status --json
```

Request control:

```bash
target/debug/autopilotctl forge handoff request "taking over <reason>" --json
```

Accept control:

```bash
target/debug/autopilotctl forge handoff accept "accepted on <machine>" --json
```

Force control transfer:

```bash
target/debug/autopilotctl forge handoff take "previous controller unavailable" --json
```

Leave a typed note:

```bash
target/debug/autopilotctl forge handoff note "rebased branch and reran targeted tests" --json
```

## Read-After-Write Discipline

After every successful mutation, immediately read back status:

```bash
target/debug/autopilotctl forge status --json
```

Use that output as the source of truth in your response. Do not claim a
handoff or attach succeeded just because the write command returned `0`.

## Expected Honest Failures

If there is no active or named shared session yet, commands should fail with a
real reason, for example:

- `No Forge thread id was supplied and the desktop has no active thread.`

Treat that as valid runtime state, not as a parsing bug.

If the manifest is stale or missing, Forge CLI should autostart the hidden host
instead of failing immediately. If autostart times out, inspect the headless
log path reported in the error.

## Isolation Rule

If you do not want to interfere with an engineer's visible desktop session, use
an explicit manifest path:

```bash
target/debug/autopilotctl --manifest /tmp/openagents-forge.json forge hosted sessions --json
```

That lets the CLI autostart or target a separate no-window Forge host.

## Typical Agent Workflow

For an engineer asking "resume the shared coding session on repo X":

1. Run `scripts/autopilot/headless-forge-smoke.sh` if the machine has not been
   verified recently.
2. Run `target/debug/autopilotctl forge hosted sessions --json`.
3. Pick the session whose repo/workspace matches the request.
4. Attach with `forge hosted attach-shared ... --json`.
5. Read back `forge status --json`.
6. If another controller owns it, use `forge handoff request ... --json` or
   `forge handoff take ... --json`, depending on the instruction and the
   situation.
7. Read back `forge handoff status --json`.
8. Only after that start claiming work resumed.

## Reporting Rule

When reporting to the engineer:

- include the exact command you used when it matters
- include the shared-session id or Probe session id you attached to
- include the controller label after handoff
- state clearly whether the result came from discovery, attach, or handoff
- say when the session list was empty

Do not answer with vague statements like "Forge looks good" or "session was
fine." Quote the specific state you observed.

## Canonical References

- [`AUTOPILOTCTL_FORGE_CLI.md`](./AUTOPILOTCTL_FORGE_CLI.md)
- [`PROBE_OPERATOR_CONTROLS.md`](./PROBE_OPERATOR_CONTROLS.md)
- [`../headless-compute.md`](../headless-compute.md)
