Yes. The Cloudflare-only sync plan is basically right. I’d add **nine things** based on the broader OpenAgents direction.

## 1. Make `OpenAgents Sync` serve both Autopilot and Nexus/Pylon later

Do not design this as only “Convex replacement for Autopilot UI.” Design it as the **canonical state/event substrate for accepted work**.

That means the same primitives should eventually cover:

```text
Autopilot workroom events
Probe/Forge run events
Pylon job assignment
Psionic execution summaries
Nexus public receipts
SHC/GCloud runner events
approval + payout state
```

The current audit already names scopes like `workspace`, `thread`, `agent-run`, `deploy`, `public-agent`, and `repo` . I would add future-compatible scopes now:

```text
pylon:{pylonId}
work-assignment:{assignmentId}
capacity-lot:{capacityLotId}
training-run:{runId}
training-window:{windowId}
settlement:{receiptId}
```

Even if you do not implement them in v0, reserving the naming model prevents a rewrite when Autopilot, Pylon, Nexus, and Psionic converge.

## 2. Add a first-class receipt ledger to the sync layer

Your plan has `sync_changes` and `sync_mutations`. Add a separate **receipt layer** now.

Minimum tables:

```sql
CREATE TABLE trust_receipts (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  subject_type TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  actor_id TEXT,
  summary_json TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  cost_json TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE failure_receipts (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  subject_type TEXT NOT NULL,
  subject_id TEXT,
  failure_code TEXT NOT NULL,
  failure_json TEXT NOT NULL,
  next_action_json TEXT,
  created_at TEXT NOT NULL
);
```

This matches your broader Blueprint direction: every mission ends with a Trust Receipt, and blocked work gets a Failure Receipt rather than disappearing into logs. The uploaded audit already keeps Trust/Failure Receipt language in the wider OpenAgents direction, and it should be pulled into the Cloudflare replacement instead of treated as later UI polish.

## 3. Add an explicit “source authority” layer before writes

This is critical for Autopilot.

Do not let sync mutations become generic writes. Every writeable object/property/action should have a declared authority:

```sql
CREATE TABLE source_authorities (
  id TEXT PRIMARY KEY,
  object_type TEXT NOT NULL,
  property_name TEXT,
  action_name TEXT,
  source_system TEXT NOT NULL,
  read_path TEXT NOT NULL,
  write_path TEXT,
  approval_policy TEXT NOT NULL,
  audit_policy TEXT NOT NULL,
  created_at TEXT NOT NULL
);
```

Rule:

```text
No source-system write without Source Authority.
No Source Authority -> Failure Receipt.
```

This keeps Autopilot from becoming a stale duplicate database or an unsafe agent write layer. It also preserves the broader architecture: Forge owns software-factory truth, Probe owns runtime truth, Nexus/Pylon own compute state, Psionic owns execution artifacts, and Autopilot/Blueprint owns the operating map.

## 4. Separate private sync from public publication

You need two event families:

```text
OpenAgents Sync
  private product/app state
  authenticated, scoped, operational
  D1 + DO + Worker

TRN / public receipt events
  public or semi-public network state
  signed, portable, mirrorable
  Nexus/Nostr-compatible later
```

Do not publish raw internal sync events to Nostr/TRN. Instead, add a projection boundary:

```text
private sync_changes
  -> projector
  -> public receipt / TRN event
```

Example:

```text
agent-run event stream
  -> internal patches: runner heartbeat, log ref, approval requested
  -> public receipt: accepted work, artifact hash, payout ref
```

That is important because your broader compute-market thesis depends on public evidence and settlement, but your Autopilot/agent runtime will contain private customer data.

## 5. Build “accepted work” as a native object type

The OpenAgents company thesis is not “nodes online.” It is **accepted work**.

So the sync schema should include this early:

```sql
CREATE TABLE accepted_work (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  work_type TEXT NOT NULL,
  assignment_id TEXT,
  worker_id TEXT,
  subject_ref TEXT,
  artifact_ref TEXT,
  validation_ref TEXT,
  settlement_ref TEXT,
  status TEXT NOT NULL,
  accepted_at TEXT,
  created_at TEXT NOT NULL
);
```

Statuses:

```text
assigned
running
submitted
validating
accepted
rejected
paid
failed
superseded
```

This gives you one object that can later unify:

```text
Autopilot managed mission outcome
Forge work order delivery
Probe code task
Pylon compute job
Psionic eval/training contribution
SHC runner execution
```

That is much more aligned with your “machine work market” direction than only syncing messages and UI state.

## 6. Add capability and routing events, not just user-facing patches

For the compute-network direction, you need to remember why a job went where it went.

Add event types like:

```text
capacity.reported
capacity.admitted
runner.heartbeat
runner.unhealthy
assignment.created
assignment.accepted
assignment.rejected
artifact.submitted
validation.started
validation.accepted
validation.rejected
settlement.queued
settlement.paid
settlement.failed
```

These can all flow through the same SyncRoom/D1 outbox model. But they should be modeled as **market memory**, not just UI e[118;1:3uvents.

The moat you keep circling is accumulated routing memory: which machines finished work, which validators caught problems, which job classes were profitable, and which contributors returned. That needs to exist in the data plane from the beginning.

## 7. Design for “durable replay packets,” not only reconnect replay

Cursor replay for WebSockets is necessary, but not enough.

You also want **exportable replay packets**:

```text
scope snapshot
+ sync_changes range
+ artifact refs
+ receipts
+ source authority refs
+ schema versions
= replay packet
```

Use cases:

```text
debugging a failed mission
investor proof packet
customer audit export
runner dispute
training-window verification
migration parity from Vortex/Convex
```

This connects directly to your Trust Receipt / public proof / investor diligence strategy. The sync layer should make replay packets cheap to create.

## 8. Add policy gates as data, not hardcoded conditionals

Define policies in data early:

```sql
CREATE TABLE policies (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  scope_pattern TEXT NOT NULL,
  action_pattern TEXT NOT NULL,
  decision_mode TEXT NOT NULL,
  rule_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
```

Decision modes:

```text
allow
deny
approval_required
budget_required
source_authority_required
human_review_required
```

This will matter for:

```text
external email
CRM writes
Forge work order creation
production deploys
paid compute dispatch
training run launch
payout settlement
public publication
```

Without a policy table, every domain will reinvent permission checks. With it, Autopilot’s Action Center, Trust Receipts, and Failure Receipts can all explain why something did or did not happen.

## 9. Make Foldkit’s Model a projection, not the source of truth

The audit correctly says the browser store is the Foldkit Model and not TanStack DB . I would add a stricter rule:

```text
Foldkit Model is a projection of server-authoritative sync state.
It may hold optimistic state, but every committed fact must arrive from a server SyncPatch.
```

That means:

```text
HTTP accepted != committed
WebSocket patch with seq + mutationId = committed
receipt generated = auditable
settlement receipt = payable/paid
```

That pattern should be enforced everywhere.

## The revised “must add” stack

I’d update the canonical target from:

```text
D1 source of truth
Durable Objects realtime
Foldkit client
R2 artifacts
Queues/Workflows async
```

to:

```text
D1 source of truth
D1 sync outbox
D1 receipt ledger
D1 source authority registry
D1 accepted-work ledger
Durable Objects for scoped realtime
Agents SDK for agent-shaped coordination
R2 for artifacts and replay packets
Queues for ingest/fanout/repair
Workflows for long-running closeout
Foldkit for deterministic client projection
OpenAuth for identity
Policy gates for all writes
Public TRN/Nostr projector for selected receipts
```

## My strongest recommendation

The v0 should still stay narrow: **live thread workroom**.

But define it as:

```text
live thread workroom
+ runner event ingest
+ artifact refs
+ approval request
+ Trust Receipt / Failure Receipt
+ accepted-work object
```

Not merely:

```text
messages + events over WebSocket
```

That makes the first slice prove the actual OpenAgents operating model:

```text
agent does work
state streams live
human can approve
artifact is retained
outcome is accepted or rejected
receipt is generated
future routing improves
```

That is the spine of the whole company.
