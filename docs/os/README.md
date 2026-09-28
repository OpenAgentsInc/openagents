# CoderOS

CoderOS is the NixOS description of a computer that runs Coder: a pinned
base system, an optional tiling desktop whose default window is Coder, and
optional capabilities such as screen recording and the Android emulator,
each off unless a host turns it on.

The working version lives in the private `~/coder` repository and describes
one machine, `coderos-4080`. This directory plans its move into this
repository as a generic release, with each person's own machine kept in a
private flake that imports it.

| Document | What it covers |
| --- | --- |
| [Audit of what moves (2026-09-28)](2026-09-28-coderos-audit.md) | Every module, script, and package in `~/coder/os`, what happens to each, the public and private layout, the edit loop for your own host, and the order of work. |
| [Camera and hands](camera-and-hands.md) | The camera daemon, hand tracking in the Coder compositor, the Jev seam's log, and how to measure it. |
| [Jev and hand tracking](hands-judge.md) | The design of the Jev seam beside the gesture rules: the trigger, the window, the state, the questions, and the rollout. |

Related requirements:

- The [migration assessment](../coder/design/coder-suite-migration.md#coderos-and-execution-environments)
  says to deliver the host before the custom desktop.
- Milestone M12 in the [migration tracker](../coder/migration-status.md) lists
  what a CoderOS profile must prove.
- The [portable host](../coder/runtime/portable-host.md) and
  [host service](../coder/runtime/host-service.md) are what a CoderOS host
  runs Coder with.
