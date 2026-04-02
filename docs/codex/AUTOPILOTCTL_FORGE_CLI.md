# `autopilotctl forge`

This is the current programmatic control surface for the internal Forge MVP in
`openagents`.

Use it when you need to drive the shared Probe-backed coding session from a
script, another agent, or a second terminal without going through the desktop
chat UI.

It is intentionally narrow. It covers the shared-session seam that is already
real:

- discover visible hosted Forge sessions
- attach the current desktop shell to a hosted shared session
- inspect the current participant and controller state
- request, accept, force-take, or annotate controller handoff

It does not try to expose every future Forge object. Evidence, delivery,
campaign, bounty, and settlement flows still live primarily in the desktop
shell.

## Requirements

- a running `autopilot-desktop` instance with desktop control enabled
- a valid `autopilotctl` manifest or the default manifest path
- the Probe-backed Forge lane enabled in the desktop app
- shared hosted session state already visible to the app

If those conditions are not true, `autopilotctl forge ...` will fail honestly
instead of inventing hidden state.

## Quick Start

List visible hosted Forge sessions:

```bash
autopilotctl forge hosted sessions
```

Attach the current desktop shell to a shared hosted session:

```bash
autopilotctl forge hosted attach-shared forge-session-1
```

Inspect the active shared-session state:

```bash
autopilotctl forge status
```

Request control from another operator:

```bash
autopilotctl forge handoff request "taking over repo triage"
```

Accept an outstanding request on the current session:

```bash
autopilotctl forge handoff accept "accepted from desktop-b"
```

## Command Reference

### Status

```bash
autopilotctl forge status
autopilotctl forge status --thread-id <probe-session-id>
```

Shows the current shared-session view for the active or named Probe thread,
including:

- shared session id
- Probe session id
- current controller
- local desktop role
- participant roster
- pending handoff request
- recent collaboration timeline

### Hosted Session Discovery

```bash
autopilotctl forge hosted sessions
autopilotctl forge hosted sessions --json
```

Lists the hosted Forge sessions that the current desktop shell knows about.

Each entry includes the shared-session id, Probe session id, repo/workspace
context when available, participant count, controller label, and hosted runtime
status.

### Hosted Attach

```bash
autopilotctl forge hosted attach-shared <shared-session-id>
autopilotctl forge hosted attach-probe <probe-session-id>
```

Attaches the current desktop shell to an existing hosted Forge session and
loads the matching Probe session through the same app-owned attach flow used by
the UI.

Use `attach-shared` when you have the Forge shared-session id.

Use `attach-probe` when you only have the Probe session id.

### Handoff Control

```bash
autopilotctl forge handoff status
autopilotctl forge handoff request <summary>
autopilotctl forge handoff accept <summary>
autopilotctl forge handoff take <summary>
autopilotctl forge handoff note <summary>
autopilotctl forge handoff human <summary>
autopilotctl forge handoff agent <summary>
```

All handoff commands also accept:

```bash
--thread-id <probe-session-id>
```

Use them like this:

- `status`
  - read the current controller, pending request, and recent events
- `request`
  - ask for control without forcing it
- `accept`
  - accept the pending request and record the transfer
- `take`
  - force control transfer when recovery or operator intervention requires it
- `note`
  - append a collaboration note without changing control
- `human`
  - mark the session as explicitly human-controlled
- `agent`
  - mark the session as explicitly agent-controlled

## JSON Output

The Forge CLI supports the normal global JSON flag after the subcommand chain.

Examples:

```bash
autopilotctl forge hosted sessions --json
autopilotctl forge status --json
autopilotctl forge handoff status --json
```

Use JSON output when another agent or script needs to read the shared-session
state directly instead of scraping text output.

## Common Flows

### Discover And Attach

```bash
autopilotctl forge hosted sessions
autopilotctl forge hosted attach-shared <shared-session-id>
autopilotctl forge status
```

### Ask Another Operator To Hand Off

```bash
autopilotctl forge handoff request "taking over fix verification"
autopilotctl forge handoff status
```

### Accept A Handoff On The Current Desktop

```bash
autopilotctl forge handoff accept "accepted on laptop"
autopilotctl forge status
```

### Force Recovery When The Previous Controller Is Gone

```bash
autopilotctl forge handoff take "desktop-a offline; resuming incident response"
autopilotctl forge status
```

### Leave An Audit Note Without Changing Control

```bash
autopilotctl forge handoff note "rebased branch and re-ran targeted tests"
```

## Current Boundary

This CLI is the right interface for the current internal team use case:

- multiple teammates discover the same hosted coding session
- one teammate attaches from their desktop shell
- the team can see who currently controls the session
- the team can request or force a controller handoff
- the team can leave typed collaboration notes

If you need higher-level Forge product objects beyond that boundary, use the
desktop shell until those surfaces are promoted into `autopilotctl`.
