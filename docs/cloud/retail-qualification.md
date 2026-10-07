# Retail cloud qualification

Status: the fake-payment acceptance run passes, and the live wallet and Boat
bindings qualify against simulated backends. Paid availability stays off
until the owner confirms the [retail contract](retail-contract.md) and the
funded qualification below passes on real sats and a real Boat sandbox
(`NEEDS_OWNER.md`).

[`retail-qualify`](../../crates/retail-qualify) holds the integrated
acceptance run, the funded-qualification plan and runner, and the launch
gate. It composes the [retail service](retail-service.md), the [compute
balance](compute-balance.md), and the workbench's compute projection
(`crates/compute-workbench`).

## Fake-payment acceptance

```sh
cargo run -p retail-qualify -- accept --out docs/cloud/evidence/2026-10-06-retail-fake-acceptance.json
```

Each case runs the whole chain in its own temporary directory, on a fake
wallet, a fake Boat, a fake sandbox, and a fake task owner: account,
top-up, offer, the four authorities, reservation, provisioning, material,
dispatch, checks, metering, cancellation, settlement, retention, and
teardown. No real payment, credential, machine, or owner chat is used, and
the receipt says so in its `label`.

| Case | Faults injected | What must hold |
| --- | --- | --- |
| `checked_patch` | None | The patch is `verified`; 61 seconds settle at 103 sats; the standalone window and the Grid workshop show the same account, balance, funded execution, and settled receipt |
| `duplicate_confirmation_and_crash_before_credit` | The service crashes after the wallet is paid and before any callback; the offer is confirmed twice; the hold is reserved twice | One invoice, one credit, one funded request, one hold |
| `lost_create_and_dispatch_acknowledgments_across_restarts` | The provider's create reply and the task owner's dispatch reply are lost, with a restart after each | One sandbox, one executor, one debit; a settlement replay changes nothing |
| `cancel_after_start_with_a_lost_stop_reply` | The stop reply is lost and the service restarts | The cost stays unknown until the exact stop receipt is observed; the stop is not resent; the sandbox is deleted |
| `provider_lost_after_start_keeps_the_hold_until_usage_reads` | The provider loses the sandbox and its usage is unreadable | The whole hold stays held, no replacement runs, and the charge settles once usage reads; the lost sandbox is deleted and delivery reported incomplete |
| `client_disappears_and_the_service_cleans_up` | The client vanishes after dispatch and the service restarts | The service worker deletes the sandbox; the hold stays held until reconciliation |

Every case checks balance conservation (credited = available + held +
settled), that at most one executor started, and that no sandbox is left
running. The run fails if any case fails or the two views disagree.

### Retained receipt

[`evidence/2026-10-06-retail-fake-acceptance.json`](evidence/2026-10-06-retail-fake-acceptance.json)
is the run's receipt. Its simulated-clock measurements are 10 seconds to
readiness, 23 seconds to the first event, 86 seconds to checked completion,
2 seconds to the cancel acknowledgment, and 31 seconds to orphan cleanup.
These are fake-clock values that prove ordering, not real latencies.

The frozen supported limits are the v1 contract's: the
`retail-boat-large-v1` computer, the `retail-repo-change-v1` task, at most
3,600 seconds a task, 4 retail sandboxes at once, 8 checks, and 30 days of
retention.

## Funded qualification

[`retail_qualify::qualify`](../../crates/retail-qualify/src/qualify.rs)

A plan (`openagents.cloud.retail-qualification-plan.v1`) pins everything one
qualification may touch: a test account (never the owner's personal one), a
top-up of at most 1,000 sats, the v1 computer and task class, the price
book version, a public source at an exact commit, the task text, one
independent check, the wall time, a spending ceiling no higher than the
top-up, the provider disclosure (exactly the contract's recipients), and a
cleanup deadline that covers the task and one replacement. The checked-in
plan is [`fixtures/qualification-plan-v1.json`](../../crates/retail-qualify/fixtures/qualification-plan-v1.json).

```sh
cargo run -p retail-qualify -- plan                 # check the plan; print its digest
cargo run -p retail-qualify -- qualify --fake --out docs/cloud/evidence/2026-10-06-retail-fake-qualification.json
cargo run -p retail-qualify -- qualify --funded --confirm PLAN_DIGEST --out /absolute/new/funded.json
```

- `qualify --fake` runs the plan end to end on fakes and writes a receipt
  labeled `FAKE QUALIFICATION`. It retains the invoice payment hash, the
  execution, hold, sandbox, and task identities, the check verdict, the
  settlement source, the charge and the released amount, any unknown held
  amount, the provider's seconds, the teardown acknowledgment, and ledger
  conservation. It qualifies only when the check is `verified`, the sandbox
  is deleted within the deadline, nothing stays held, and the ledger
  conserves. The retained fake receipt is
  [`evidence/2026-10-06-retail-fake-qualification.json`](evidence/2026-10-06-retail-fake-qualification.json):
  104 sats charged and 20 sats released for 90 metered seconds.
- `qualify --funded` checks the plan, requires `--confirm` to name the
  plan's exact digest, a new absolute `--out` receipt path, and the fake
  run to pass. It creates the receipt with mode `0600` and never overwrites
  an existing file. Without
  `--bindings` it then refuses (`no_live_binding`, exit status 3). With
  them, it runs the plan on the live bindings below and writes a receipt
  labeled `FUNDED QUALIFICATION`. It never pays anything itself: it prints
  the top-up invoice and waits for the owner to pay it. The receipt retains
  the exact receiver, Boat organization/key digest, daily template, model
  provider, and price book bindings. The production service rechecks the
  retained native ledger/journal and authenticated receiver evidence; the
  standalone `advertise` command only previews its supplied record.
- `qualify --simulated` runs the same live adapters against simulated
  backends and writes a receipt labeled `SIMULATION`. The launch gate never
  accepts it.

### Live bindings

[`retail_qualify::bindings`](../../crates/retail-qualify/src/bindings.rs)
reads a bindings file (`openagents.cloud.retail-bindings.v1`):

```json
{
  "schema": "openagents.cloud.retail-bindings.v1",
  "simulation": false,
  "wallet_home": "/path/to/retail-wallet",
  "boat_api_base": "https://boat.dev/api/v1",
  "boat_org": "RETAIL_ORG",
  "template": "oa-coder-main-20261006",
  "state_dir": "/path/to/new-empty-directory",
  "model_provider": "openai",
  "payment_wait_seconds": 900,
  "poll_millis": 5000
}
```

- **Receiver wallet.** The resident `openagents-wallet` node answering at
  `wallet_home/control.sock`, reached through its socket client
  (`openagents_wallet::resident::RemoteWallet`), the same
  `LightningWallet` contract the top-up path uses. The run checks that the
  paid invoice's recorded preimage hashes to its payment hash.
- **Boat.** [`retail_cloud::boat::BoatAdapter`](../../crates/retail-cloud/src/boat.rs)
  implements the provider, sandbox, task-owner, stop, and artifact seams
  over the Boat API. Creates carry an idempotency key derived from the
  provisioning identity, so a lost reply finds the same sandbox. Deletion
  counts only when Boat's deletion operation completes. The adapter writes
  a fixed `sh` program, [`owner-v1.sh`](../../crates/retail-cloud/src/owner-v1.sh),
  to the sandbox and runs one verb at a time through the commands API: it
  clones the admitted commit, keeps the key file at mode 0600, runs the
  engine and the frozen checks, scrubs the key from the patch and logs, and
  writes the status, events, manifest, and stop receipts the adapter reads
  back. The key travels only in a files API body, never on a command line.
- **Secrets.** The retail Boat key comes from
  `OPENAGENTS_RETAIL_BOAT_API_KEY` and the test customer's own model key
  from `OPENAGENTS_RETAIL_CUSTOMER_MODEL_KEY`. The run never reads
  `BOAT_API_KEY` or Secret Manager for its own use, and refuses a retail
  key equal to the operator's `BOAT_API_KEY`.
- **Refusals.** Simulated bindings on a funded run, any Boat base other
  than `https://boat.dev/api/v1`, a template that is not a daily
  `oa-coder-main-YYYYMMDD`, a missing key, a state directory with files in
  it, or no resident wallet answering. Each refuses before any wallet or
  Boat call.

### Simulated funded qualification

```sh
cargo run -p retail-qualify -- qualify --simulated --out docs/cloud/evidence/2026-10-06-retail-simulated-qualification.json
```

The run starts the real resident wallet server on a private socket over a
simulated Lightning network (`lnsim` invoices with real preimages), and a
fake Boat API on loopback that speaks the HTTP the `boat` SDK sends and
simulates the owner program's verbs. The first create reply is lost on
purpose. The receipt's `simulation` block retains what the backends saw.
[`retail-cloud/tests/owner_script.rs`](../../crates/retail-cloud/tests/owner_script.rs)
runs `owner-v1.sh` itself with `sh` against a local Git repository and a
stub engine.

[`evidence/2026-10-06-retail-simulated-qualification.json`](evidence/2026-10-06-retail-simulated-qualification.json)
is the retained receipt: qualified, the preimage verified, the check
`verified`, 90 metered seconds settling at 100 sats with 24 sats released,
nothing held, and the ledger conserved. Boat saw two create requests and
created one sandbox, one executor started, the sandbox was deleted, and the
customer's key never appeared on a command line and was removed. It proves
the bindings, not a funded outcome.

### Owner runbook

Each step is the owner's; none runs without them (`NEEDS_OWNER.md`).

1. Confirm the retail contract and the price book ("Review the first retail
   cloud contract").
2. Set up the live bindings: run the resident receiver wallet on mainnet
   under its own home (`OPENAGENTS_WALLET_HOME=DIR openagents x402 node
   serve`), create a separate retail Boat account or organization with its
   own API key (never the operator allowance used by `chat work --on
   boat`), and give the test customer an OpenAI key of its own. Write a
   bindings file as above with a new, empty `state_dir`, and export
   `OPENAGENTS_RETAIL_BOAT_API_KEY` and
   `OPENAGENTS_RETAIL_CUSTOMER_MODEL_KEY`.
3. Review the plan: run `retail-qualify plan` and keep the digest.
4. Run `retail-qualify qualify --funded --confirm DIGEST --bindings PATH
   --out RECEIPT.json` once. It prints the top-up invoice (at most 1,000
   sats); pay it from a wallet you control within the wait. Keep the
   receipt, the ledger and journal in `state_dir`, the Boat usage for the
   sandbox, and the retained artifacts.
5. The first real run also checks what the simulation cannot: that the
   daily template has `git`, `setsid`, and a `codex` that accepts
   `login --with-api-key` and `exec --full-auto`, and that Boat's files and
   commands APIs behave as the fake Boat server assumes.
6. Check that the Boat sandbox is deleted, the charge is at most the
   ceiling, and nothing is held. Any unknown charge stays held until
   reconciled.

A failed funded run opens a new defect issue. Until a funded receipt
exists, no funded outcome is claimed.
