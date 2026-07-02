# Formal Specs

This directory is the bounded TLA+ tier for `docs/fable/ROADMAP.md` task
T6.13 / GitHub issue #7857. The specs are design checks only: they inform the
runtime and fixture tiers, but they do not authorize runtime behavior or
weaken the workspace invariants.

## Running TLC

```sh
specs/run-tlc.sh
```

The runner expects a working standard `tlc` command. Each spec has a small
checked `.cfg` with bounded constants.

## Property Map

| Source | Spec | Checked properties | Source seam |
| --- | --- | --- | --- |
| `docs/fable/ROADMAP.md` WS-6 T6.13; QA design §9.3; fleet fanout §6 item 6 | `khala-fleet-delegate/FleetDelegateSupervisor.tla` | dead-end class unreachable; termination under bounded retries; active assignments never exceed advertised capacity; claim uniqueness under racing supervisors; paused runs claim nothing; drain terminates | `packages/khala-tools/src/fleet-delegate-program.ts`, `apps/pylon/src/orchestration/`, future `FleetRunSupervisor` |
| QA design §9.3 | `approval-protocol/ApprovalProtocol.tla` | no lost approvals; exactly one typed outcome per request; no stale-request forgery | desktop approval RPCs, Inbox `approval_required`, Codex/Claude approval bridges |
| QA design §9.3 | `session-thread-mapping/SessionThreadMapping.tla` | no orphan thread bindings; no double-bind across crash/reload; persisted bidirectional map stays consistent | Khala Code Desktop session store and thread list reload/reconcile path |

## Counterexample Fixtures

The JSON files under `fixtures/counterexamples/` are minimal bad traces that
the fixed specs exclude. They are intentionally public-safe and bounded so the
QA scenario/model tiers can port them into executable regressions:

- `fleet-supervisor-racing-claims.json`: second supervisor must be refused
  before claiming against the same Pylon.
- `approval-stale-request-forgery.json`: stale approval ids must be rejected
  after interruption or supersession.
- `session-thread-double-bind.json`: reload must reject duplicate thread
  binding and preserve the bidirectional map.
