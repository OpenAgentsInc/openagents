# Operating the retail cloud service

Status: launch tooling on main; paid capacity is closed. The service
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

The published customer terms are the [retail contract](retail-contract.md)
(supported class and task, credentials, retention) and the [retail
prices](retail-prices.md) (charges, holds versus refunds, unknown costs).
The [customer transport and worker](retail-service.md#customer-transport-and-resident-worker)
mount these records over loopback HTTP. Native customer controls (REV-14)
and production TLS/deployment packaging (REV-15) remain separate; the
compute-workbench host is a read-only observer.

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
same records. REV-15 owns the exact production configuration, external TLS
origin, supervision, revision health, and deployment qualification.

## Deploying

- Deploy only from a commit rebased on current `origin/main`, with its own
  Cargo target directory, so an older checkout cannot roll the service back.
- Record the deployed commit, the service version, the price book version,
  and the account and journal schemas (`openagents.compute-workbench.v1`,
  `openagents.cloud.retail-admission.v1`).
- State compatibility: the ledger and journal tables are created with
  `CREATE TABLE IF NOT EXISTS` and only ever gain tables, so a rollback to
  the previous commit keeps reading the same files. A schema change that
  is not additive is a new version and needs its own migration test.
- After a deploy, run `retail-qualify health` against the service's files
  and check the running commit.

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
While an incident is open, close paid capacity by withdrawing the funded
receipt from the gate's input.
