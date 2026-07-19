# SBX-06 managed IDE placement

Date: 2026-07-19
Issue: [#9027](https://github.com/OpenAgentsInc/openagents/issues/9027)
Status: implemented as a default-off deterministic Desktop component. SBX-09
still owns live GCP, packaged-app, cleanup, cost, rollback, and rollout
acceptance.

## Outcome and claim boundary

OpenAgents Desktop now has one typed OpenAgents-managed placement consumer in
the existing IDE project and agent graph. The main process can refresh
admission, create, inspect, dispatch, interrupt, stop, resume, and delete the
canonical managed-sandbox resource. The renderer exposes create, inspect,
interrupt, stop, resume, and delete controls, but it receives only a bounded
public projection.

This packet supplies the managed target that IDE-13 and IDE-17 can compose. It
does not claim the broader IDE-13 project-capability symmetry or IDE-17 Agents
Window exits. IDE-10 and IDE-12 remain dependencies for their terminal/task
and SCM/worktree/delivery contracts. SBX-07 still owns the authenticated
server broker used by Desktop and Sarah, while SBX-09 remains the only live
release gate.

## One identity graph

The placement binding retains the exact existing project, root, worktree,
session, agent attachment, attachment generation, and placement generation.
It adds stable work-unit, placement, and sandbox refs without minting a second
project or agent identity. Every attached command carries the full expected
agent attachment. Main re-reads the current agent-code snapshot and refuses a
stale grant, ref, or generation before calling the managed-sandbox gateway.

Gateway results must preserve the admitted target, owner scope, attachment,
sandbox, resource generation, command, receipt, and turn. Target substitution,
wrong-sandbox responses, stale generations, mismatched receipts, or malformed
native results fail closed. Stop/resume can advance the native resource
generation without changing the project, worktree, session, work-unit, or
agent attachment identity.

## Effect and Electron ownership

`IdeManagedSandboxService` is a `Context.Service` acquired through
`Layer.effect`. Commands are named `Effect.fn` operations, expected refusals
are a `Schema.TaggedErrorClass`, all gateway and renderer inputs decode through
Effect Schema, state lives in an Effect `Ref`, and scope teardown clears the
service state.

The Electron ownership split is:

| Layer                   | Owns                                                                                                     | Must not own                                                          |
| ----------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| managed-sandbox service | identity continuity, admission, lifecycle projection, generation checks, receipts                        | GCP credentials, HTTP, UI mechanics                                   |
| main host               | encrypted-session credential read, authenticated broker call, private persistence, agent-host comparison | renderer state or provider-local fallback                             |
| preload                 | fixed snapshot and command IPC methods with schema decoding                                              | bearer, generic IPC, GCP client, filesystem root                      |
| renderer                | visible public facts and typed owner intent                                                              | credential, raw root, shell, Box SDK, GCP client, lifecycle authority |

The feature is default-off unless
`OPENAGENTS_DESKTOP_MANAGED_SANDBOX=1`. Signed-out and disabled hosts return
typed unavailable/refused projections without opening a network path. When
enabled, the access token exists only in main and is placed only in the HTTP
`Authorization` header. It is absent from request bodies, persisted snapshots,
IPC values, renderer state, receipts, and test output.

## Visible placement truth

The Files workspace contains a compact OpenAgents-managed placement strip. It
shows the effective Google Cloud target and isolation class, region,
image digest, profile, regional custody, freshness, latency class, native
generation and version, lease state and expiry, maximum USD cost, capability
states, lifecycle, and the last public-safe refusal.

Controls enable only for valid projected states. Create requires an admitted
target and exact current agent attachment. Inspect, stop, resume, interrupt,
and delete require the same attachment and sandbox binding. Interrupt also
requires the exact current running turn. An unavailable host stays visibly
unavailable. It cannot silently substitute local, fake, Box-owned, or weaker
isolation.

The resource projects into the existing IDE capability shape as kind `agent`
with managed evidence. This is the narrow adapter later IDE packets consume.
It is not a new capability authority.

## Broker boundary and next packet

The main-owned HTTP gateway is intentionally narrow:

- `POST /api/managed-sandboxes/desktop/admission`
- `POST /api/managed-sandboxes/desktop/commands`

Both carry native OpenAgents schema values. They are not Box-v1 routes and do
not expose a generic cloud or shell client. SBX-07 implements the authenticated
Worker broker and gives Sarah typed access to the same lifecycle authority.
Until that server path and SBX-09 live evidence land, Desktop remains a
default-off component rather than a live product claim.

## Verification

The deterministic corpus proves:

- one identity across create, dispatch, interrupt, stop, resume, and delete.
- stale attachment refusal before gateway mutation.
- admitted-target substitution refusal.
- bearer isolation to the main-owned header and absence from public bytes.
- default-off behavior with zero network calls.
- exact rendered target, custody, freshness, latency, generation, lease,
  cost, capability, lifecycle, and lifecycle-valid actions. And
- schema-first service and renderer boundary enforcement.

The canonical evidence record is
`docs/sol/evidence/2026-07-19-sbx06-managed-ide-placement.json`. The packet does
not contain a live provisioner receipt, packaged Desktop journey, real spend,
or cleanup receipt. Those omissions are deliberate and remain release
blockers owned by SBX-09.
