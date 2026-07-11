# P0 TASK D2-A: freeze Desktop Effect service scopes and boundary oracles

- Issue: #8678
- Parents: #8574, #8566
- Depends on: #8676 D1-H request-processor shape; may proceed beside #8677 when paths
  and contracts are disjoint
- Live issue state: closed 2026-07-11 after the initial manifest and cache/
  freshness/disposal declaration slices
- Roadmap disposition: residual acceptance unproven; reopen #8678 or create one
  bounded successor before claiming the full architecture freeze
- Evidence:
  [`../../teardowns/2026-07-10-opencode-effect-architecture-teardown.md`](../../teardowns/2026-07-10-opencode-effect-architecture-teardown.md)

## Outcome

Before Desktop adds the D3/D4 editor, PTY, provider, MCP, permission, and
foreign-host breadth, make its Effect capability and lifetime topology
explicit and executable. Services are assigned to process, WorkContext,
conversation/run, request/command, or foreign-host/view scope; forbidden
dependency directions, ambient authority, duplicate Schema identities,
unowned fibers/resources, and internal runtime exits fail an architecture
oracle.

This is a bounded freeze of the current application graph. It is not permission
to copy OpenCode's custom `LayerNode` compiler or rewrite the app speculatively.

## Reconciliation note — 2026-07-11

The live issue closed after PRs #8679/#8680, but the issue's maintainer and
claim-release receipts explicitly exclude source-coupled `runPromise`/
`ManagedRuntime` and ambient-authority enforcement, service replaceability,
renderer/WorkContext/runtime/shutdown disposal oracles, structured trace
correlation, and the full Desktop verify/build/real-Electron acceptance below.
Closed state proves neither those items nor this dossier's complete exit. The
landed manifest/declaration slices remain valid baseline; the residual needs a
new live claim before mutation.

## Required topology

```text
process
  identity / encrypted storage / Sync / observability / component ledger
    └─ WorkContext
       repository / Blueprint action / policy / Pylon target / containment
         ├─ conversation or run
         │  model stream / captured tool generation / budgets / settlement
         ├─ request or command
         │  decode / idempotency / approval / transaction / event / receipt
         └─ foreign host or view
            PTY / editor / diff / preview / canvas / native capture
```

Narrower scopes may depend on wider scopes. Process services may not capture a
WorkContext; WorkContext services may not capture one conversation or view;
renderer/view state is never runtime authority.

## Scope

- Inventory current Desktop Runtime Gateway, Sync, workspace, history,
  provider/Pylon, command, renderer, and Effect Native host services with owner,
  dependency direction, cache key, freshness, and disposal rule.
- Add a small checked topology manifest/oracle. Prefer native Effect Layers and
  ordinary module-boundary checks. Add a graph IR only if a demonstrated
  replacement/hoisting problem cannot remain clear otherwise.
- Prove public command/event/projection contracts reuse one canonical Effect
  Schema identity; legacy compatibility contracts are explicitly named.
- Restrict `ManagedRuntime`, `runPromise`, Promise callbacks, Electron IPC, and
  provider/native callbacks to named perimeter modules. No ambient cwd,
  `AsyncLocalStorage`, renderer path, or module singleton may select authority.
- Attach long-lived fibers, subscriptions, registrations, native handles, and
  foreign hosts to the narrowest owning Scope and test disposal.
- Install structured trace/log correlation for owner-safe WorkContext, command,
  run, runtime generation, and receipt refs at the composition root.
- Record the failure taxonomy: recoverable domain refusal, dependency outage,
  interruption, invariant defect, and optional telemetry degradation.

## Acceptance

1. A checked topology artifact covers every current service and fails a
   process→WorkContext, WorkContext→view/run, or renderer→runtime authority
   dependency.
2. Architecture tests catch a cycle, wrong-scope replacement, duplicate public
   Schema identity, ambient path/ALS authority, unowned fiber/resource, and
   prohibited internal `runPromise` escape.
3. Filesystem, transport, identity, policy, provider, database, clock, and
   foreign-host services are replaceable in tests without Electron/global
   monkey-patching.
4. Renderer remount, WorkContext switch, runtime replacement, and app shutdown
   close exactly the resources they own while preserving wider scopes.
5. Existing Runtime Gateway, Codex history, Sync conversation, Desktop
   `verify`, build, and real-Electron smoke remain green.
6. Any necessary invariant or guarantee change lands with its test; no runtime
   policy is weakened to make the oracle pass.

## Non-goals

- copying OpenCode's LayerNode implementation;
- moving every direct Node/Bun call when it already sits behind a narrow owned
  construction boundary;
- changing Khala Sync, Pylon, Blueprint, or provider authority semantics;
- broad D3/D4 feature implementation.

## Close

Close when the topology is documented and mechanically enforced against the
current app, with concrete violations fixed and no speculative framework
rewrite left open.
