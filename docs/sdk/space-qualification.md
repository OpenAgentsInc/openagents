# SPACE qualification and staged admission

2026-10-17. [#11816](https://github.com/OpenAgentsInc/openagents/issues/11816)
tracks the product qualification that follows the separate SPACE foundations.
Source completion does not qualify a customer rollout. Lane 0 owns shared
deployments and the live checks in the [master delegation](../audits/2026-10-16-master-delegation.md#after-close-checks).

## Status, 2026-10-18

Lane 0 ran the live qualification on the private candidate
(`space-private-11814`, private Computers revision with 100 ms polling) with two
synthetic test owners, then ran the public cases through staging with a
dedicated staging test account (`acct_4570d993d9897bdd`, not the owner). The
[packet](space-qualification-packet.json) pins the running identity, private
management, and the exercised rollback; `--test-accounts` passes. The
named-group assessment does not pass. Receipts and scrubbed evidence are in
[`evidence/space-qualification-2026-10-18/`](evidence/space-qualification-2026-10-18/).

| Result | Cases |
| --- | --- |
| Passed (22) | `account_isolation`, `actual_boot_and_shape`, `memory_and_workspace`, `attach_interruption`, `api_restart`, `reconciler_restart`, `lost_create_reply`, `delayed_lifecycle`, `worker_loss_and_single_writer`, `checkpoint_recovery`, `database_restore`, `storage_outage`, `egress_bypass`, `capacity_exhaustion`, `concurrent_wake`, `quotas_and_accounting`, `retention`, `account_deletion`, `disabled_placement`, `legacy_boat`, `unsupported_controls`, `monitoring` |
| Failed (10) | `public_sdk`, `web_task_and_followup`, and `stage_timings` ([#12146](https://github.com/OpenAgentsInc/openagents/issues/12146): SPACE placements refuse files and commands); `cancellation_and_children` and `upgrade_and_rollback` ([#12141](https://github.com/OpenAgentsInc/openagents/issues/12141)); `lost_command_reply` and `duplicate_delivery` ([#12142](https://github.com/OpenAgentsInc/openagents/issues/12142)); `provider_outage` ([#12143](https://github.com/OpenAgentsInc/openagents/issues/12143), fixed in source, not yet deployed here); `orphan_cleanup` and `fixture_cleanup` ([#12151](https://github.com/OpenAgentsInc/openagents/issues/12151)) |
| Pending (1) | `credential_revocation` (the credential adapter is not deployed) |
| Not required | `cross_host_memory_restore` (not advertised; one worker) |

Owner decisions, 2026-10-18:

- Alerts email the owner through the Resend integration. The control-node
  monitor sends one email when a condition opens and one when it clears.
- A dedicated staging test account may use SPACE. Staging Computers
  (`deploy/space-service/staging_binding.py`) keeps Boat as the default and
  binds only that account to `fast-sandbox`; pricing stays off.
- Cost is measured as an estimate when billing data is unavailable.
- Losing the last checkpoint of a computer that is awake when its worker fails is
  accepted for now. A suspended computer must lose nothing, and in the worker
  reset it lost nothing.

The packet's trials were rerun after the [#12144](https://github.com/OpenAgentsInc/openagents/issues/12144)
fix. Each net gateway resolved two local role accounts through OS Login and the
metadata server, which its sandbox network can't reach; locked local `dev` and
`deploy` accounts on the worker cut native boot-to-ready from about 10.6 s to
2.9 s. Create-to-verified-output on the private path (nearest rank, warm
template cache, concurrency 1 and 2):

| Kind | Attempts | Failures | p50 | p90 |
| --- | --- | --- | --- | --- |
| Cold create | 26 | 2 (uncertain turn, #12141) | 4,486 ms | 9,941 ms |
| Snapshot create | 9 | 9 (every boot was cold; #12145 fix not yet deployed) | none | none |
| Wake | 28 | 1 (uncertain turn) | 3,926 ms | 33,612 ms |

Before the fix the same arms measured 11,637 ms and 12,732 ms p50; those
series stay in `raw/trials`. On staging the public SDK workflow created and
readied a SPACE computer in 345 ms and 4.9 s before the refused upload. About one
wake in five still waits for a later Computers revisit (8 to 33 s). No sample has
a cold template cache.

The operator can't read the billing account or an export, so
`deploy/space-service/cost_estimate.py` multiplies the deployment inventory by
on-demand list prices from the Cloud Billing Catalog API: USD 0.80 an hour, or
USD 4.90 for this 6.1-hour window, about USD 0.096 for each of 51 verified
useful results. The packet records it as an estimate; it excludes discounts,
NAT data, object operations, and transfer. Boat stays the public default,
pricing stays off, and no customer account is admitted.

## Check a qualification packet

The Rust operator command checks finite account scope, pinned identities,
dated case receipts, local artifact checksums, measurement coverage, and explicit
latency, error, and service-cost budgets. It makes no network request and changes
no deployment, entitlement, or billing setting.

```sh
oa-computers-usage qualification-template > space-qualification.json
oa-computers-usage qualify space-qualification.json --test-accounts
oa-computers-usage qualify space-qualification.json
```

An incomplete template is valid input and fails both admission checks. Each
command prints its assessment before returning a nonzero exit status when its
requested stage is not ready. `--test-accounts` requires one or two explicitly
named test accounts, pinned identities, private management access, a prepared
rollback, and pricing off. The default command requires the complete named-group
qualification. It accepts at most eight exact account owners. Neither command
permits broad admission or customer pricing.

Account owners use the registry's `account:<64 lowercase hex characters>` form.
Do not use the owner's account or host. Keep credentials in the approved private
test-account file; never put credentials, transcripts, command output, or private
customer files in the packet. Only scrubbed receipts and artifacts belong in Git.

The packet identifies the actual web, Computers, SDK, and native commits; native
image, runtime, policy, and API-schema SHA-256 values; immutable template; and
observed guest CPU and memory. Digest fields contain 64 lowercase hex characters
without a prefix. Commit fields contain full 40-character lowercase Git hashes. The template uses `name@sha256:HEX`. Missing identities stay
unknown. Do not substitute an operator's intended image for the running image.

## Bind each result to its evidence

Every required case must have status `passed` and a relative artifact path and
SHA-256. `pending`, missing cases, and `failed` remain distinct; there is no
passing `skipped` status. The artifact is a JSON receipt with these fields:

| Field | Value |
| --- | --- |
| `schema` | `openagents.computers.space-case-receipt.v1` |
| `identity` | The exact identity object in the packet |
| `observed_unix` | The packet's observation time, in Unix seconds |
| `cases` | A map from reviewed case names to `passed`, `failed`, or `pending` |
| `source` | Relative `path` and `sha256` of the scrubbed raw check or archive |

Paths are relative to the packet's directory. Absolute paths, parent traversal,
and symlinks refuse. Receipts are at most 1 MiB, individual artifacts at most
16 MiB, and the assessment reads at most 128 MiB. Changed bytes, a different
deployment identity, or a different observation time cannot reuse a passing
receipt. The checker verifies declared evidence; an operator must review what
the raw check demonstrates. It does not attest a remote machine's state.

Copying a native benchmark into a public-workflow receipt does not establish the
public workflow. Record the API origin, actual account, computer, saved request
keys, source versions, scope of each measurement, and cleanup in the raw evidence.
Record recovery time and accepted data loss for each advertised recovery feature.

## Measure the public SDK flow

The existing Boat scenario also supports explicit SPACE placement:

```sh
SPACE_SDK_CHECKPOINT="$(openagents scratch)/space-sdk-qualification-01"
mkdir -m 700 "$SPACE_SDK_CHECKPOINT"
OPENAGENTS_TEST_API=https://staging.openagents.com/v1 \
OPENAGENTS_TEST_PLACEMENT=fast-sandbox \
OPENAGENTS_TEST_TOKEN="$OPENAGENTS_TOKEN" \
cargo run -p openagents-sdk --example computers -- "$SPACE_SDK_CHECKPOINT"
```

Load `OPENAGENTS_TOKEN` from the approved dedicated staging credential before
this command. Production uses `https://api.openagents.com/v1` and its dedicated
test credential. The example checks the configured dedicated account, sends
`X-OpenAgents-Test: 1`, and accepts only these two deployment origins. Admission
must already permit that account and environment on the selected deployment.
Running the example does not enable SPACE.

The workflow still creates, uploads binary bytes, runs and checks a command,
stops, wakes, reads the same bytes, removes the file, and observes deletion.
Requests remain saved before dispatch. A rerun reuses those requests; a changed
API origin or placement refuses the checkpoint. Use `our-cloud` to select Boat
explicitly, or leave placement unset to preserve the original scenario.

`timings.json` records the create response, readiness wait, upload or upload
observation, first command, first useful output, stop, wake, restored-file read,
post-wake command, and cleanup. First observations are preserved. The result
marks a resumed checkpoint ineligible as a fresh latency sample. Its first
useful-output measurement includes upload and checksum work; label that scope
when comparing it with a create-to-`true` benchmark. Reconcile, attach, repository,
and model stages that the example does not measure need separate observations.
Do not fill them with zero.

The real web task needs a dedicated account, a verified environment with a SPACE
artifact, and an isolated deployment configured to bind that account to SPACE.
Upload a synthetic repository, request a useful change, independently run its
tests through the SDK, request a follow-up, and verify both changes on the same
computer. Keep the commit ancestry and test results. A quoted success marker or
a successful process exit alone is not a useful result.

## Collect the required cases

| Cases | Action and required evidence |
| --- | --- |
| `public_sdk`, `web_task_and_followup`, `actual_boot_and_shape`, `stage_timings` | Complete the public SDK and web flows. Record the actual bound backend internally, snapshot boot mode, fixed guest shape, identities, stage timings, and independent useful results. |
| `account_isolation` | A second synthetic account cannot list or access the computer, command output, files, or credential references, or perform lifecycle changes. |
| `memory_and_workspace` | Compare process boot ID, PID, start ticks, memory nonce, and workspace checksums before suspension and after wake. Obtain fresh credential references. |
| `cancellation_and_children`, `attach_interruption` | Interrupt attachment and reconnect using the saved command. Cancel its process tree, wait for completion, and prove a child cannot make a delayed write. |
| `api_restart`, `reconciler_restart`, `lost_create_reply`, `lost_command_reply`, `duplicate_delivery`, `delayed_lifecycle` | Inject each failure on disposable infrastructure after saving the request identity. Observe the original operation; prove one remote command and no replacement under the old identity. Do not generate a new key to hide uncertainty. |
| `worker_loss_and_single_writer`, `checkpoint_recovery` | Stop the owned worker and inspect lease fencing, durable checkpoint state, recovery time, and data loss. Prove there are no simultaneously active restored copies. |
| `cross_host_memory_restore` | Required only if advertised. Physically replace the worker, restore from durable encrypted storage, and compare the same process and memory evidence. Same-host restore and a single-writer check do not establish this feature. |
| `database_restore`, `storage_outage`, `provider_outage` | Restore the backup and exercise bounded outages. Retain uncertain reservations, fence stale work, revoke old grants, and observe recovery without changing the computer identity. Record the recovery and data-loss boundaries. |
| `credential_revocation`, `egress_bypass` | Follow the [credential adapter runbook](../../deploy/space-credentials/README.md): real supported clients, expiry, stream closure, provider/broker outage, fresh wake grants, and host-control versus guest denial, including redirects, IP/DNS bypass, IPv6, metadata, and management endpoints. |
| `capacity_exhaustion`, `concurrent_wake`, `quotas_and_accounting` | Saturate bounded capacity, wake concurrently, read durable usage, and exercise byte/count quotas. Unknown provider usage stays held and unknown, not free. |
| `retention`, `account_deletion`, `orphan_cleanup`, `fixture_cleanup` | Expire fixtures, request account cleanup, and inspect provider/object inventory. Remove computers, chats, projects, credentials, processes, snapshots, and temporary infrastructure. State provider soft-deletion and retention separately. |
| `upgrade_and_rollback`, `disabled_placement`, `legacy_boat` | Deploy and roll back checked images. Stop new SPACE admission, restore Boat as default, and still observe, command, wake, and delete existing SPACE and Boat computers. Keep the previous connection and binding configuration. |
| `unsupported_controls`, `monitoring` | Verify actual advertised capabilities, private ingress, measured resource bounds, alerts, collection limits, and the operator runbook. Fork, terminal, and preview remain refused by this qualification version. |

The [maintained service runbook](../../deploy/space-service/README.md) has the
private helpers, encrypted lifecycle fixture, replacement sequence, cleanup,
monitoring, and rollback pins. Only lane 0 changes the maintained deployment.
Its existing memory and protected-endpoint evidence is useful within its stated
scope; repeat the public product checks after the combined deployment.

## Set budgets from measured results

The packet's limits are explicit operator acceptance targets. They do not become
customer promises. Supply all of the following before named-group admission:

- At least 20 successful cold trials, 20 successful snapshot trials, and 20
  successful wake trials, with
  failures retained, successful cold/warm cache coverage, and successful
  concurrency of at least two or the higher configured target.
- A snapshot first-useful-output p50 budget strictly below 2,000 ms, a p90 budget
  at least as large, and a separate positive wake p90 budget. Percentiles use
  nearest rank. Report the scope, counts, p90, failures, cache, and concurrency.
- An observed error budget in permille, checked for cold, snapshot, and wake separately
  as well as the whole sample. This is an observed rate, not an SLO confidence
  bound. Successful cold trials cannot hide failed wake trials.
- A complete [service economics](computers-usage-evidence.json) window, including
  shared idle capacity, database, disks, NAT, storage, and transfers. Supply a
  currency and maximum service cost per independently verified useful task.
  Missing cost, guest-capacity, or command-time measurements block admission.
- A maximum evidence age from one second to 24 hours. Future, missing, or stale
  observation times block named-group admission.

The earlier [SPACE measurements](../cloud/evidence/2026-10-16-space-computers.json)
have 20 warm-template snapshot runs, zero failures, p50 750.54 ms, and p90
811.17 ms through loopback timing proxies. They are not production public-SDK
latency measurements. The node's smaller benchmark number is not a user promise.

## Roll out and roll back

1. Keep Boat as the public default and use a finite dedicated test-account list
   on the private SPACE candidate. Complete the packet and remove its fixtures.
2. After a fresh passing named-group assessment and deployment review, configure
   a small explicit account list. Keep pricing off. Observe the same budgets
   and repeat qualification when the image, runtime, policy, or contract changes.
3. Broader customer entitlement and pricing require the owner's separate decision.
   The checker always returns `broad_admission: false` and `pricing_enabled: false`.

To stop new placement, set the SPACE deployment's `admit_new` to `false`, retain
`enabled: true`, and restore the previous Boat default. Removing rollout accounts
does not relabel existing bindings. Do not remove or retarget the old deployment:
its connection remains necessary to observe, cancel, wake, and delete its
computers. `enabled: false` also removes that connection from service and is not
the admission rollback control.

The retained [qualification packet](space-qualification-packet.json),
[assessment](space-qualification-assessment.json), and
[delivery evidence](space-qualification-evidence.json) record the current result.
Missing live results remain pending. Source completion under the release brief
does not turn an incomplete packet into a passing rollout assessment.
