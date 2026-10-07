# Operating the retail cloud service

Status: the supervised package and native recovery controls are implemented;
production paid capacity is closed. The service
advertises no paid computer until the owner confirms the [retail
contract](retail-contract.md) and a funded qualification of the exact
supported configuration is retained ([qualification](retail-qualification.md),
`NEEDS_OWNER.md`).

## What the service publishes

[`retail_qualify::launch::advertise`](../../crates/retail-qualify/src/launch.rs)
decides, and fails closed:

| Gate | Closed reason when it fails |
| --- | --- |
| The owner confirmed the contract and the price book | `contract_unconfirmed` |
| A funded qualification receipt exists | `no_funded_qualification` |
| The receipt is funded (never fake), qualified, and for the supported plan's digest | `qualification_not_valid` |
| Fewer than 4 retail sandboxes run and the Boat plan has starts left | `no_capacity` |

Every advertisement names the service (`openagents.cloud.retail-service.v1`)
and the price book, and labels two things separately, always: sponsored
hosted inference fallback for local Coder turns, and the operator's own Boat
and GCE placements (`chat work --on boat|gce`). Neither is a paid computer,
and neither is for sale.

```sh
cargo run -p retail-qualify -- advertise                      # closed: contract_unconfirmed
cargo run -p retail-qualify -- advertise --contract-confirmed --receipt FUNDED_RECEIPT.json
```

These commands render gate records. The deployed runtime additionally rebuilds
native funding, delivery, cleanup, and accounting evidence before it admits
paid work.

The published customer terms are the [retail contract](retail-contract.md)
(supported class and task, credentials, retention) and the [retail
prices](retail-prices.md) (charges, holds versus refunds, unknown costs).
The [customer transport and worker](retail-service.md#customer-transport-and-resident-worker)
mount these records over loopback HTTP. The selected native
[`retail-client`](../../crates/compute-workbench/src/bin/retail-client.rs)
uses that transport. The production package terminates TLS separately;
client read access grants no spending or execution authority.

## Service configuration

The `retail-service` binary reads one mode-0600 JSON file. Its `customer`
object contains the schema `openagents.cloud.retail-customer.v1`, absolute
private `state` and central `ledger` paths, the supported `template`, explicit
`grants` (principal, account, ledger generation, observe/execute/disclose),
`contract_confirmed`, the retained `qualification`, `supported_plan` digest,
and `plan_starts_left`. Each accepted confirmation consumes one of that
configured start allowance for this state; an owner must reconcile any
increased allowance with the provider plan. Existing compute accounts and
bearer principals are created through the central ledger's operator paths;
the HTTP service cannot enroll or grant them.

The versioned host schema is `openagents.cloud.retail-host.v1`.
`mode` defaults to `closed`. Only an explicit `development` configuration
accepts synthetic qualification fixtures; it is never production evidence.
The host fields are `listen` (loopback only), `boat_api_base`, `boat_org`,
`boat_key_file` (absolute private file for the dedicated retail account),
`wallet_home` (explicit resident receiver socket), and `poll_seconds` (1–30).
No live configuration, secret, owner home, or paid qualification is bundled.
Run the built binary as `retail-service --config /absolute/private/config.json`.
It owns `transport.sqlite`, `lifecycle.sqlite`, the private credential vault,
and the Boat index under `state`; the money ledger remains at `ledger`.
Use the state directory's `lifecycle.sqlite` with `retail-qualify health`.
Revoke or rotate a central principal to stop its active execution; changing
configured retail grants requires a service restart. Restart resumes the
same records. Existing paths must be private, owned, unshared, and free of
symlinks. The runtime refuses replaced original state descriptors before
any subsequent mutation or external effect. It does not adopt shared files
by changing their permissions.

## Deploying

[`deploy/retail/`](../../deploy/retail/) contains a closed configuration
example, separate receiver and service systemd units, a Caddy TLS ingress
configuration, and the build script. The script requires a clean checkout
at current `origin/main`, builds through the lease with an external retained
Cargo target, and records executable hashes and the source commit. It
packages `retail-service`, `retail-qualify`, `retail-client`, and `openagents`.
These units have not been installed on an owner host; TLS and live funds
remain O3/O4 qualification.

1. Confirm the contract, custody addendum, prices, restricted retail Boat
   account, receiver network, and exact ready `oa-coder-main-YYYYMMDD`
   template. Drain original executions with their original configuration
   before changing provider bindings or templates; unknown financial holds
   remain on the same ledger until reconciled. Run the
   [funded qualification](retail-qualification.md) against this receiver,
   template, Boat account, source plan, and price book.
2. Build with `deploy/retail/build.sh /absolute/new/release-directory`.
   Install its executables as root-owned mode `0755` files under a mode
   `0755` version directory in `/opt/openagents-retail/`, provision the
   dedicated `openagents-retail` user and private state, and select it as
   `current`. Initialize the dedicated receiver through its wallet's
   existing operator path; never copy the owner's wallet home.
3. Copy `host.example.json` into `/etc/openagents-retail/` and the reviewed
   Caddy configuration to `/etc/caddy/Caddyfile` as root-owned mode `0644`.
   Point `operations.ingress_file` at those exact deployed ingress bytes;
   the runtime refuses writable, shared, or substituted ingress input.
   Keep the host config, dedicated Boat key, separate operator bearer,
   plan, and funded receipt owned by the service user with mode `0600`. Use
   its `0700` state and qualification directories. The example has no grants,
   confirmed terms, approved identity, or paid availability.
4. Configure the exact private native qualification journal and ledger,
   receipt byte digest, selected plan digest, start allowance, retail
   grants, and external HTTPS origin. Set `mode` to `production`; inspect
   the selected binary with `retail-service inspect --config HOST.json`,
   review its complete `identity`, and write its `identity_digest` to
   `operations.approved_identity`. Approval alone cannot qualify a fake
   payment or delivery.
5. Validate the ingress with `caddy validate --config Caddyfile`. Install
   the dedicated receiver and service units, start the receiver and then
   the service, and validate the actual TLS origin with the selected
   client. The ingress proxies only `/v1/retail`, limits bodies to 32 KiB,
   and excludes operator metadata and liveness. No TLS grant creates
   customer spend, execute, or disclosure rights.

The production runtime pins the executable, build commit and tree, canonical
configuration, receiver, Boat key digest and organization, template, price
book, plan, and ingress bytes. It rereads the private sources before
admission and after receiver lookup. It reconstructs native signed invoice,
full inbound receiver amount, hold, meter, checked delivery, cleanup, and
central debit evidence. Missing, changed, fake, or mismatched evidence
closes paid capacity. A normal LDK record can omit its returned invoice and
inbound fee; the retained signed invoice and full claimed amount supply
payment evidence, and absent provider expense remains unknown.

`GET /operator/status` on loopback requires the separate operator bearer,
refuses browser origins, and returns the running commit, executable/config
identity, versions, qualification closure reason, health alerts, and worker
status. It returns no credentials. A gateway or customer bearer cannot
read it. Production closes new paid admission when native health alerts
exist or the worker fails or becomes stale; recovery of original funded
work continues independently.

## Checkpoint, restore, and rollback

Stop the service first and finish removing transient customer credentials.
The offline command takes its exclusive custody lock and SQLite locks;
a running writer, shared file, stale descriptor, or credential-bearing
vault refuses a checkpoint.

```sh
retail-service snapshot --config HOST.json --out /absolute/new/checkpoint
retail-service rollback-check --config HOST.json --snapshot /absolute/checkpoint --candidate /absolute/version/retail-service --digest sha256:SELECTED_BINARY_DIGEST
retail-service restore --snapshot /absolute/checkpoint --state /absolute/new/state --ledger /absolute/new/ledger.sqlite
```

Checkpoints retain at most 512 files and 64 MiB, with private byte-digested
copies of the original ledger, transport, lifecycle, and Boat records.
Expiry is at most 30 days and never later than a copied artifact's original
retention deadline. Delete the checkpoint at that deadline. An expired,
modified, incomplete, or oversize checkpoint cannot restore; an interrupted
copy without `checkpoint.json` is unsealed and must be discarded.

Restore requires separate absent paths and never overwrites newer or active
accounting. Fence the original service and receiver before using restored
records, reconcile any effects after the checkpoint, and keep capacity
closed until the resulting configuration is reviewed. The command does not
clone a wallet seed, recreate funding, or grant admission authority.

For binary rollback, take a fresh stopped checkpoint and select the exact
candidate binary digest. `rollback-check` runs the candidate's read-only
inspection under a 15-second supervised bound, checks storage/customer
compatibility, and rechecks that current accounting bytes remain unchanged.
After that check passes, replace only the binary selection and restart on
those same current files. Review the new running identity before reopening
paid capacity. The isolated acceptance tests exercise this executable path
with the selected schema; a distinct historical binary or schema migration
needs its own compatibility check.

## Monitoring

```sh
cargo run -p retail-qualify -- health --journal JOURNAL.sqlite --ledger LEDGER.sqlite
```

`health` prints the alerts as JSON and exits with status 4 when there are
any:

| Alert | Raised when | What to do |
| --- | --- | --- |
| `stuck_reservation` | A live hold has no provisioning record after 15 minutes | Run recovery for the execution; it releases the hold when nothing started |
| `uncertain_dispatch` | A dispatch is unacknowledged after 5 minutes | Run recovery; it finds the original task by identity and never dispatches another |
| `slow_readiness` | A sandbox is creating or starting past twice the readiness deadline | Check Boat; provisioning replaces once, then reports `unavailable` |
| `cleanup_pending` | Teardown was requested and a sandbox is not acknowledged deleted | Run the cleanup worker (`retail_cloud::retain::service_step`); it deletes the exact recorded resources |
| `meter_gap` | A dispatched, unsettled execution has no usage reading after 5 minutes | Poll usage; an unknown cost stays held |
| `ledger_drift` | An account does not conserve, or a settled hold has no `debit:<hold>` settlement | Stop new offers and reconcile the ledger before anything else |
| `payout_age` | Unpaid OpenAgents shares are older than 7 days | Run the payout path (`openagents pay payouts`) |

## Reconciling a stuck funded request

Recovery is [`retail_cloud::recover::step`](../../crates/retail-cloud/src/recover.rs)
for the funded execution. It reads the payment ledger, the provider's
resource by its provisioning identity, the task owner by the task identity,
the meter, and the retained artifacts, and moves the execution forward
without creating another sandbox or task. A transport timeout or a failed
listing never authorizes a replacement. A lost sandbox with a checkpoint
needs a new offer. Unknown costs stay held, and every liability stays on
the ledger until reconciled.

## Incidents

Record an incident when paid capacity closes unexpectedly, when an alert
stays raised after recovery, or when a customer receipt is wrong. An
incident note names the deployed commit, the affected executions, the
alerts, what recovery did, and every hold, charge, or refund it changed.
While an incident is open, set `mode` to `closed` or withdraw the funded
receipt. Configuration changes close the running gate immediately; restart
with the reviewed configuration. Original cleanup and unknown holds remain
durable until their exact outcome is reconciled.
