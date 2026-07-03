# BF-3.4 Private/Sovereign Compute Tier Spec

Date: 2026-07-02
Status: planned design and receipt gate for
[`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-3.4 / GitHub issue #8087. This flips
no product-promise state, grants no deployment authority, and broadens no
public copy.

Source material:

- [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-3.4: per-customer isolated
  workroom/VM lane with metering receipts.
- [`2026-07-02-business-fulfillment-engine-meditations.md`](./2026-07-02-business-fulfillment-engine-meditations.md):
  privacy tier for legal/health-grade work and the isolated-VM customer
  problem.
- [`../research/amp/2026-07-02-amp-orbs-adaptation-audit.md`](../research/amp/2026-07-02-amp-orbs-adaptation-audit.md):
  workroom lifecycle hooks, snapshots, pause/resume economics, and
  minute-metered receipts.
- Existing cloud-lane promise boundary:
  `autopilot.cloud_coding_sessions.v1` remains red until a real
  desktop-originated cloud session runs on GCE and produces a
  content-addressed artifact plus a dereferenceable usage receipt with owner
  sign-off.

## 1. Narrow Claim

The private/sovereign tier is a paid upgrade for regulated or
confidential business engagements: one customer workspace can be provisioned
onto an isolated workroom/VM lane, run fulfillment work inside that lane, and
emit usage receipts that reconcile the compute placement, runtime lifecycle,
and customer workspace ref without exposing client-identifying information in
the repository or public projections.

Until the receipt gate in section 5 passes, safe copy must say this is
**planned** or **private/operator-gated**. Unsafe copy includes "live private cloud",
"HIPAA-grade", "sovereign compute available now", "customer VMs are generally
available", or any claim that legal/health customer data can already be routed
through this lane without the redaction and cloud-lane gates.

## 2. Workspace Isolation Contract

Each isolated lane instance is bound to one opaque workspace ref, one vertical
profile, and one placement ref:

- `workspaceRef`: opaque customer workspace identifier, never a client name.
- `verticalProfile`: descriptor such as `legal`, `health`, `agency`, or
  `commerce`; no client-identifying strings.
- `placementRef`: cloud-lane placement id from the Cloud control plane or
  SHC lane.
- `trustTier`: `regulated_private` for this BF-3.4 lane.
- `corpusPolicyRef`: the BF-3.1/BF-3.2 corpus + redaction policy applied
  before any external inference.
- `meteringSessionRef`: the receipt lineage for VM/workroom seconds,
  storage/snapshot operations, and public-safe lifecycle events.

The lane may read only the workspace corpus, structured intake facts, approved
deliverable templates, and tool credentials explicitly bound to the workspace.
It must not inherit ambient operator credentials, broad GitHub credentials,
default local Pylon homes, raw customer identifiers, or unrelated workspaces.

## 3. Lifecycle Hooks

The lane adapts the orbs-style lifecycle, but every hook remains inside the
existing OpenAgents authority model:

| Phase | Required event | Gate |
| --- | --- | --- |
| Provision | `isolated_workroom.provision_requested` | Workspace has an opaque ref, vertical profile, promise object, and redaction policy. |
| Place | `isolated_workroom.placement_started` | Cloud lane is armed, trust tier is allowed, and no fallback to a shared/default lane is permitted. |
| Setup | `isolated_workroom.setup_completed` | Repo/workspace setup hook succeeds under timeout, network, and secret policy. |
| Snapshot | `isolated_workroom.snapshot_recorded` | Snapshot key is derived from setup inputs, lockfiles, image version, and workspace policy. |
| Run | `isolated_workroom.fulfillment_run_started` | The fulfillment contract names the deliverable and approval ladder. |
| Pause/resume | `isolated_workroom.paused` / `isolated_workroom.resumed` | Metering stops while paused and resumes only after the resume hook succeeds. |
| Closeout | `isolated_workroom.closeout_recorded` | Usage receipt, artifact refs, and approval/review decisions exist. |

Setup/resume hooks are repository or workspace input, so the same timeout,
network, secret, and trust-tier policy applies to them as to the main agent
run. Hook failures are typed lifecycle events, not silent broken workrooms.

## 4. Metering Receipt Shape

The first receipt can stay documentation-backed, but the implementation target
is a structured receipt row with these public-safe fields:

```yaml
receiptKind: business.isolated_workroom_usage.v1
workspaceRef: workspace.<opaque>
verticalProfile: legal | health | agency | commerce | other
placementRef: placement.<opaque>
meteringSessionRef: metering.<opaque>
trustTier: regulated_private
startedAt:
endedAt:
meteredVmSeconds:
pausedSeconds:
snapshotRefs: []
artifactRefs: []
approvalRefs: []
redactionPolicyRef:
costSource: resource_usage_receipt.v1
publicSafety: opaque_refs_only
```

The receipt must reconcile to the lower-level `resource_usage_receipt.v1`
or successor metering source. Public projections may expose opaque refs,
vertical descriptors, receipt timestamps, and totals. Owner/customer-private
views may dereference artifacts and raw logs under the workspace authority
boundary.

## 5. Green Gate For #8087

BF-3.4 is complete only when all of the following evidence exists:

1. One customer workspace is provisioned on the isolated lane with opaque refs
   only in committed docs, tests, and public proof.
2. The lane runs on a Cloud/SHC placement explicitly marked
   `regulated_private`; it does not fall back to local/default/shared compute.
3. The workspace has an attached corpus/redaction policy from BF-3.1/BF-3.2,
   or the run is explicitly marked as a no-corpus infrastructure smoke.
4. Setup, run, pause/resume if exercised, and closeout lifecycle events are
   recorded with workspace, placement, and metering refs.
5. A usage receipt reconciles VM/workroom runtime against the metering source
   and names the opaque workspace ref.
6. Artifact and review refs exist for the fulfillment work, but raw customer
   data, local paths, prompts, secrets, and client names remain out of public
   projections.
7. Product-promise state remains red/planned/yellow until owner review signs
   off on the exact claim and the registry/public API record is updated.

## 6. Implementation Order

1. Extend the cloud-lane placement contract with the `regulated_private`
   trust tier and fail-closed routing for BF-3.4 workspaces.
2. Add the isolated-workroom lifecycle event vocabulary and receipt schema in
   the same contract package or Worker authority that owns cloud sessions.
3. Wire workspace provisioning to request the isolated lane only when the
   workspace promise, vertical profile, and corpus/redaction policy are
   present.
4. Persist metering receipts by adapting the existing cloud resource-usage
   receipt flow rather than inventing a parallel money truth.
5. Add a no-client-data smoke fixture that proves placement, lifecycle,
   closeout, and receipt reconciliation end to end.
6. Run the first real workspace only after BF-3.1/BF-3.2 unblock regulated
   corpus handling and the owner approves the customer-private execution.

## 7. Non-Goals

- No HIPAA, legal-practice, or compliance certification claim.
- No customer names, client facts, raw documents, raw prompts, local auth
  paths, provider payloads, or secrets in this repository.
- No public self-serve launch, pooled marketplace capacity, or
  settlement-bearing third-party provider lane.
- No broad product-promise green flip from this design doc alone.
