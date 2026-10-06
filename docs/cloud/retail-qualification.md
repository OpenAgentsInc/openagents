# Retail cloud qualification

Status: the fake-payment acceptance run passes. Paid availability stays off
until the owner confirms the [retail contract](retail-contract.md) and the
funded qualification below passes (`NEEDS_OWNER.md`).

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
