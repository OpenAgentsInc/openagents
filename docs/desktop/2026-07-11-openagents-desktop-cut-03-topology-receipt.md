# OpenAgents Desktop CUT-03 source-coupled topology receipt

Date: 2026-07-11

Issue: [#8683](https://github.com/OpenAgentsInc/openagents/issues/8683)

Implementation: `4d875dcb4b`

## Result

The Desktop topology oracle no longer accepts its typed manifest as sufficient
evidence. Each service entry now identifies its implementation module, real
construction symbols, composition module, installation scope, authorities,
cache key, freshness rule, and disposal owner. The normal package test sweep
reads those checked-in sources and fails when a module, constructor, or
composition reference drifts.

The source oracle also derives protected filesystem, network, process, and
secret authority from implementation code. Renderer ownership of any of those
authorities fails. Ambient cwd/`AsyncLocalStorage`, an unnamed
`Effect.runPromise`/`ManagedRuntime` exit, or a session/project service installed
at process scope also fails.

The audit exposed and corrected concrete manifest drift:

- false workspace dependencies were removed from the request-scoped legacy
  chat and Fleet request services;
- the stateful Codex account-connect host is now truthfully process-scoped;
- a selected workspace now constructs one explicit root-bound WorkContext
  service instead of passing a process-global root through every operation;
- the persistent Codex-history worker is represented by a process-owned host,
  tested for response correlation and pending-read settlement, and disposed on
  app shutdown; and
- cache/freshness/disposal declarations now sit on the same typed entries as
  the checked production construction evidence.

## Verification

```bash
bun run --cwd apps/openagents-desktop verify
```

Passed from the clean worktree:

- TypeScript typecheck;
- 176 tests across 24 files, 979 expectations, zero failures;
- mutation coverage for removed/uncomposed constructors;
- negative fixtures for renderer filesystem/process/network/secret authority,
  ambient cwd/`AsyncLocalStorage`, unnamed runtime exits, and wider
  session/project installation;
- production bundle; and
- deterministic real-Electron smoke/reload acceptance.

This closes only CUT-03. It does not claim CUT-04 service replacement, complete
scope-disposal matrices, structured correlation, or full architecture-freeze
host acceptance; #8684 and parent #8678 remain open for those guarantees.
