# Computers: one public API over supported places work can run

2026-10-15. Status: delivery contract and tracked proposals, building on the
existing internal lifecycle service. Public endpoints and fields below require
implementation and qualification; a diagram or example does not advertise an
unimplemented capability.

Part of the [OpenAgents SDK](README.md). See the
[delivery plan](delivery-plan.md), [current lifecycle service](../cloud/computers.md),
and [SPACE production roadmap](../cloud/space-integration-roadmap.md).

## Current state

| Kind | Implemented path | Evidence and limits |
| --- | --- | --- |
| Our cloud, whole GCE VM | Boat driver through `oa-computers`; web chats use internal service calls | Earlier live check: about 77 s to first turn and 52 s resume; disk-only stop |
| Enrolled own machine | Existing Coder host/grants and product-specific access | No unified allocation adapter yet; online state and current rights determine availability |
| SPACE microVM | Optional Rust driver and completed GCE spike #11728 | HTTP cold p50 2,045.8 ms to first command; node snapshot p50 319.0 ms to readiness at a different shape; production still Boat |

The SPACE adapter declares Core and Suspend. Public Snapshot/Fork creation and
billable seconds remain unsupported on the pinned provider API. Same-host memory
restore preserved a full 1 GiB process buffer and took 9.293 seconds to answer;
cross-host recovery and a production chat flow remain unproved. See the
[measurement report](../cloud/space.md) for shapes, timing boundaries, p90,
source pin, and cleanup evidence.

A person's own computer is not an isolated allocated sandbox. Its availability
is not guaranteed instant, and a zero OpenAgents compute charge does not mean
hardware, electricity, or any other service cost is zero.

## Public contract and provider contract

```text
developers, our apps, agents
        |
        | account authentication and current grants
        v
api.openagents.com/v1/computers
        |
        v
oa-computers: ownership, intent, operations, Postgres, reconcile
        |
        v
registry: stored provider/deployment binding per computer
        |
        +-- Boat adapter -> Boat HTTP -> GCE
        +-- SPACE adapter -> native SPACE HTTP -> Firecracker
        +-- later: own-machine adapter -> enrolled host authority/transport
```

Public callers ask for requirements, environment, and placement intent. The
provider contract is `computer-driver` with declared capabilities and
`computer-conformance`. Boat-compatible HTTP is the existing out-of-process
provider option; SPACE's native HTTP adapter does not make SPACE a Boat server.

Internal service tokens and `X-OpenAgents-Account` are a trusted server protocol,
not developer inputs. The public boundary derives the owner from authenticated
account/workspace/grant data and ignores or strips spoofed internal headers.
The supported key issuers/scopes must be audited; prefix matching is insufficient.
Keep the named-agent/admin gate until broader admission is explicitly ready.

## Proposed resource and placement

The exact additive schema is settled in [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) and [#11804](https://github.com/OpenAgentsInc/openagents/issues/11804).
The intended fields are:

| Field | Meaning |
| --- | --- |
| `id` | Stable logical computer identity, independent of provider resource IDs |
| owner/project | Authorized account/workspace and optional project binding |
| environment/version | Immutable verified environment and compatible backend artifact |
| size | Supported named class, with actual vCPU/memory available for inspection |
| needs | Explicit capability requirements, such as memory-preserving suspension or fork |
| placement | `auto`, our cloud, fast sandbox, or an authorized enrolled host; exact wire names remain proposed |
| desired/observed state | Accepted intent and actual lifecycle progress, including failure and uncertainty |
| capabilities | What the bound backend/artifact actually supports, including suspension semantics and limits |
| lifecycle | Idle policy, absolute lifetime, and private-state retention |
| egress | Named policy pinned to an immutable version at creation |
| usage/cost | Attributable observed usage and stated price policy; unknown amounts remain explicit |

Driver and deployment identity belong on the internal record and authorized
diagnostics. They are not public create requirements or a vendor picker. An
internal operator may select an exact backend for qualification while the public
request still uses capability and placement intent.

Placement rules in [#11809](https://github.com/OpenAgentsInc/openagents/issues/11809):

1. Check account policy, grant/enrollment, artifact, capabilities, shape, quota,
   capacity, and data boundaries deterministically.
2. An explicit placement either satisfies the request or refuses before effects.
   Never silently redirect an own-machine or fast-sandbox request to whole-VM
   cloud execution or a contributor's host.
3. If an automatic choice needs judgment among eligible targets, use the typed
   Decisions API. Never use chat keyword matching or let a model override rights.
4. Persist selected driver/deployment/artifact/policy and operation identities.
   Changing defaults cannot move an existing computer or change its trust boundary.
5. Missing or disabled deployment configuration means unresolved availability;
   it does not prove deletion or permit recreation under the old identity.

## First public operations

| Operation | Proposed public behavior | Issue |
| --- | --- | --- |
| Create/list/detail | Account-scoped resource, intent accepted promptly, later readiness observed | [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) |
| Stop/wake/delete | Change intent; operation completion and observed effect stay distinct | [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) |
| Commands | Create identified session, attach/read bounded output, observe result, explicitly cancel | [#11802](https://github.com/OpenAgentsInc/openagents/issues/11802) |
| Files | Bounded upload/download in the authorized workspace | [#11803](https://github.com/OpenAgentsInc/openagents/issues/11803) |
| Capabilities | Additive discovery without breaking existing list/response shapes | [#11809](https://github.com/OpenAgentsInc/openagents/issues/11809) |
| Snapshot/Fork | Refuse unless the exact provider operation and ownership semantics are implemented and qualified | Later capability work |

The intended command family is `/v1/computers/{id}/commands`; existing turn
routes and chat task references remain compatible. Attaching must either start
execution or read already retained output. The specification must state which
behavior is implemented and prevent lost early output. Scoped attach authority
never grants lifecycle-management access.

File path and transfer rules cover traversal, absolute paths, symlink escape,
partial writes, overwrite policy, size/deadline bounds, and stopped-state effects.
Do not expose arbitrary host paths or implicitly wake/allocate a computer during
an ordinary file read unless that policy is explicitly part of the contract.

## Retry, cancellation, and state

- Generate one key for a logical create/mutation, retain it across attempts, and
  expose it to applications for recovery after restart. The API scopes it to the
  account and request; conflicting immutable settings refuse.
- Create acceptance is not readiness. A request deadline expiring is not proof
  the accepted operation failed. Inspect the same resource/operation.
- A command has a stable identity before execution. Unknown dispatch is observed
  or attached to, never replayed as a fresh command to obtain a clean answer.
- Stream disconnect/detach is distinct from command cancellation. Explicit stop
  confirms process/children termination or reports uncertainty.
- Retained output has cursors, bounds, expiry, and explicit gaps. Reconnect checks
  current account/grant authority, including expired attach tokens.
- Deletion is final only after the relevant provider and private storage cleanup
  are observed. Detaching an own-machine record has different physical effects.

## Suspension and recovery

Keep disk-only stop and memory-preserving suspension distinct. Boat's existing
Suspend profile remains valid; add an explicit memory-preservation capability
rather than redefining it incompatibly.

Choose suspend-after-turn or a bounded idle window from measured behavior.
Queued turns wait for checkpoint/teardown completion; a provider state alone
cannot prove the prior operation finished. Wake verifies readiness and renews
current grants/credentials before dispatch. Restored RAM may contain stale
sockets, clocks, and services that need recovery.

API-process recovery preserves identities and leases. Worker-loss recovery needs
a verified durable checkpoint and exclusive restore ownership; same-host memory
survival does not prove that. Document recovery time/data loss and refuse claims
that have not been exercised. Lifecycle implementation is [#11812](https://github.com/OpenAgentsInc/openagents/issues/11812).

## Credentials and egress

The current per-turn approach avoids deliberately saving values in our command
metadata; SPACE stdin also avoids its saved `env` field. The process can still
read a supplied key, and applications, logs, files, or memory snapshots may hold
it. Do not describe this as a universal no-secrets-on-disk guarantee. Keep private
checkpoints account-bound and separate from clean reusable base snapshots.

The target public surface uses scoped credential handles, for example:

```json
{
  "credentials": [
    {"name": "github", "reference": "credential-handle-example"}
  ]
}
```

This is a proposal, not an implemented request field. A handle identifies an
account-authorized source/grant and allowed computer/turn/destination/expiry.
It is not permission to resolve an arbitrary Google Secret Manager path.
The broker performs authorization before cloud-secret resolution and at use.

Qualify Google Secret Manager or our broker with SPACE's egress gateway; upstream
AWS support is not proof of GCloud support. Verify real Git/model clients, TLS,
streaming, redirects, subdomains, expiry, and revocation. If a client requires a
real guest key, record its restricted exception and snapshot handling honestly.

Pin named default-deny egress profiles at create. Test DNS bypass, literal IPs,
redirects, private networks, metadata, and IPv6 where enabled, with a reachable
host-side denial control. Credential provider failure must fail closed. This
work is [#11813](https://github.com/OpenAgentsInc/openagents/issues/11813) and precedes general customer access.

## SDK and internal consumers

The first Rust client [#11805](https://github.com/OpenAgentsInc/openagents/issues/11805) supplies typed lifecycle methods, bounded
operation waits, command streaming/reconnect/cancel, file transfer, and stable
retry handles. It does not require the host harness stack or a provider database.
A proposed facade name is a packaging goal, not publication permission.

The web proves the client contract first in [#11806](https://github.com/OpenAgentsInc/openagents/issues/11806). Solver
[#11807](https://github.com/OpenAgentsInc/openagents/issues/11807) and Environment builders/verifiers [#11808](https://github.com/OpenAgentsInc/openagents/issues/11808) migrate
separately, preserving their existing task/build/review owners and legacy cleanup.
Do not tie those migrations to unfinished SPACE support.

Own-machine support [#11817](https://github.com/OpenAgentsInc/openagents/issues/11817) uses current enrolled-host grants for each
operation. Create attaches authorized logical access; delete detaches it without
deleting files, shutting down the machine, uninstalling the host, or revoking
unrelated grants. Unsupported suspend/fork/snapshot needs refuse. Core-only host
support may require API admission changes instead of pretending Suspend works.

New CLI tools and compiled remote launch changes are tracked in [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823) and
[the next-release list](../coder/next-release.md). Compatible servers can improve
frozen Coder 1.0.0 now; new binary behavior cannot.

## Usage, rollout, and deferred capabilities

Durable usage/quota work [#11815](https://github.com/OpenAgentsInc/openagents/issues/11815) separates running guest capacity, suspended
private storage, shared pools/templates, worker idle cost, and network/storage
operations. Missing provider seconds is not zero cost. Preserve uncertain
reservations through restart and settle observed usage once. Account/service
limits precede unbounded allocation; retail pricing/entitlement is separately
approved and uses existing Bitcoin rails.

Deploy SPACE privately with durable state, compatible snapshots, bounded capacity,
monitoring, and rollback in [#11814](https://github.com/OpenAgentsInc/openagents/issues/11814); qualify the actual SDK/web path
in [#11816](https://github.com/OpenAgentsInc/openagents/issues/11816). Keep Boat serving existing/default computers until the named
rollout criteria pass. Disabling new SPACE placement must leave old computers
observable, wakeable, and deletable.

Snapshot/Fork, interactive terminals, previews, scheduled wake, external providers,
and automatic payment each require their own qualified capability and authority.
They are not implied by generic computer creation. Provider admission
[#11820](https://github.com/OpenAgentsInc/openagents/issues/11820) requires trust/consent/payout decisions; SDK auto-payment
[#11822](https://github.com/OpenAgentsInc/openagents/issues/11822) remains opt-in and deferred. These do not gate the first Boat flow.

See [delivery milestones and all implementation issues](delivery-plan.md).
