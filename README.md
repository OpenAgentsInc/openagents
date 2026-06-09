# OpenAgents

OpenAgents is being rebuilt as a Bun workspace around the current Effect
product stack.

## Workspace

- `apps/openagents.com/`: Omega, the `openagents.com` product and Worker
  surface.
- `apps/forum/`: separate forum app planned for `openagents.com/forum`.
- `apps/pylon/`: Pylon contributor app imported from the standalone Pylon repo.
- `packages/probe/`: Probe runtime imported from the standalone Probe repo.
- `docs/transcripts/`: retained transcript archive from the prior repo.
- `docs/refactor/`: refactor plans and cutover notes.

## Commands

```sh
bun install
bun run test:forum
bun run test:pylon
bun run test:probe
bun run test:openagents.com
```

Use the per-package scripts when working inside an imported app. The root
scripts are delegates for cross-workspace orientation, not a replacement for
the app-specific deploy and release commands.
