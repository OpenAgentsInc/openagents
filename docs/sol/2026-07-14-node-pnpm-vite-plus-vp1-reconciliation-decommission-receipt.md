# Node, pnpm, and Vite Plus VP-1 reconciliation and decommission receipt

- Class: receipt
- Date: 2026-07-14
- Snapshot: 2026-07-14
- Status: VP-1 implementation complete; owner wallet recovery remains an
  optional, separately authorized custody procedure
- Dispatch: no; use [#8795](https://github.com/OpenAgentsInc/openagents/issues/8795)
- Owner: Sol runtime and toolchain conversion
- Parent: [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777)
- Proof rung: production read-only reconciliation, quiet-window comparison,
  and private immutable archive

## Result

VP-1 captured the production money-bearing Postgres surface before executable
payment authority is deleted. Two repeatable, aggregate-only snapshots at
`2026-07-14T07:50:28Z` and `2026-07-14T07:54:32Z` were quiet: table counts,
state groups, and exact integer amount totals did not change between them.
The four-minute observation is a cutover receipt, not a claim that every
historical nonterminal row is payable.

No private row, mnemonic, access token, invoice, preimage, payment hash, raw
destination, wallet path, provider payload, or customer identifier is in this
document. The row-level export is private and retention-locked.

## Production aggregates

The selected retirement set contains 72 tables, 26 non-empty tables, and 3,799
rows. Amounts remain in their stored integer denominations.

| Surface | Aggregate at both quiet snapshots | Disposition |
| --- | --- | --- |
| Agent balance ledger | 9 rows; `20,260,956 msat` balance, `0 msat` held, `21,000,000 msat` USD-origin marker | Freeze row history. The USD-origin marker is provenance, not a claim of withdrawable bitcoin. |
| Pay-ins | 321 rows; two adjustment rows pending (`2,986 msat`), one sweep forwarding (`240,000 msat`), 318 terminal rows | Preserve and classify nonterminal rows; never replay or pay from state name alone. |
| Pay-in legs | 343 rows; balance-in `24,155,044 msat`, balance-out `2,415,000 msat`, Lightning-in `1,421,000 msat`, Lightning-out `1,625,000 msat` | Immutable audit history. |
| Billing | 44 accounts; 2,264 ledger rows; trial grants `44,000 cents`, manual adjustments `240,000 cents`, container usage `-1,105 cents`, Codex usage `-159,997 cents` | No Stripe checkout or auto-top-up ledger source was present. Preserve balances as retired service-credit history; do not reinterpret them as cash or free capacity. |
| Forum direct tips | 74 attempts: 31 confirmed/settled (`3,091 sats`), 34 observed/recovery-pending (`1,938 sats`), eight failed (`788 sats`), one refunded (`21 sats`) | Historical `recovery_pending` is not payment authority. Freeze pending rows for owner review; retain terminal receipts. |
| Forum webhook reconciliation | two events: one confirmed/settled and one refunded/failed, each `21 sats` | Retain replay/idempotency evidence. |
| Labor escrow | three rows, all released to provider; `3,000 msat`; zero held balance | Terminal evidence only. |
| Nexus payout intents | 67 rows: 45 settled MDK-agent-wallet (`844,000` bitcoin minor units); 22 approved across MDK-agent-wallet, hosted MDK, and Spark (`3,292,000` minor units) | Approved rows are historical proposals until corroborated by attempt/reconciliation/provider evidence. Freeze; do not auto-dispatch. |
| Nexus payout attempts | 64 rows: 46 MDK confirmations (`849,000` minor units), two MDK dispatched (`2,000,000`), 16 Spark dispatched (`1,145,000`) | All 64 have matched reconciliation rows, but state-label disagreement remains historical evidence, not permission to resend. |
| Treasury transaction projection | 127 rows: eight settled inbound (`259,970 sats`), 42 settled outbound (`504,544 sats`), 46 zero-amount pending inbound, and 31 expired/failed rows | Preserve public-safe receipt history. Zero-amount pending invoices are not balances owed. |
| Wallet custody | last observed approximately `8,622 sats` MDK plus `652 sats` Spark (`9,274 sats` total before fees) | Recovery remains owner-controlled under the dedicated runbook; no agent sweep was performed for this receipt. |

The archived Stripe checkout/session/webhook tables, buyer-payment tables, and
partner and site-referral payout ledgers contained no rows. Absence from this
production snapshot does not authorize deleting applied migrations or their
schema history.

## Private archive

The private bucket is `gs://openagentsgemini-vp1-money-retirement`. Public
access prevention is enforced, uniform bucket-level IAM is enabled, object
retention is seven years, and soft delete is 90 days.

The valid archive object is:

`production/openagents-vp1-money-retirement-csv-20260714T0757Z.tar.gz`

- format: schema plus 72 table CSV exports
- size: `311,323 bytes`
- SHA-256:
  `1e3edd152b9fca5789540e16f7c146c1a374ed8524afd19b230427b5aff5e29e`

Local validation reproduced the object size, digest, and 72-table member count.
The bucket also retains two zero-byte failed-attempt objects from `T0751Z` and
`T0753Z`. They are not valid backups and must never be selected for restore.
Bucket retention prevents deleting them; their continued presence is expected
and documented rather than hidden.

The retired L402 Cloud SQL instance was exported separately before shutdown:

| Database archive | Bytes | SHA-256 |
| --- | ---: | --- |
| `production/l402-aperture-db-aperture-20260714T0800Z.sql.gz` | 9,201 | `8d212c7374ad0d3d59517f44acd275bad67be835741eaec628aa29f04f4ba314` |
| `production/l402-aperture-db-openagents_web-20260714T0800Z.sql.gz` | 1,319,620 | `1a4a0de971941c72c937fdc3a0e77fc46860816764688e2647f347432ded2b7d` |
| `production/l402-aperture-db-openagents_web_staging-20260714T0800Z.sql.gz` | 193,369 | `7c5e5e5250b2b803fdbf82d573686b03de1dbaa6da32130fe0c238456a06bc24` |

The Cloud SQL exporter received bucket object-creator access only for those
exports; the grant was removed immediately after hash verification. Instance
`l402-aperture-db` is retained cold with `activationPolicy=NEVER` and observed
state `STOPPED`, so it is recovery evidence rather than an active rail.

## Runtime decommission

The following Cloud Run services were deleted after the quiet snapshots and
archive validation: `l402-aperture`, `l402-upstream-proxy`,
`l402-wallet-executor`, `oa-mdk-sidecar-staging`, `oa-mdk-tips-buffer`,
`oa-mdk-tips-buffer-staging`, `oa-mdk-treasury`, and
`oa-mdk-treasury-staging`. A later attempted treasury redeploy was interrupted
while Cloud Build was still building; Cloud Run confirmed that no
`oa-mdk-treasury` service was recreated. No wallet send or fund movement was
attempted.

The matching source cutover deletes the MDK sidecar, Treasury, and tips-buffer
service trees; removes their Cloud Run secret mounts, Worker bindings,
Durable Object implementations, runtime configuration, and dependencies; and
retains only the applied Wrangler creation/deletion migrations. Treasury,
tips-buffer, and Artanis spend routes, callback ingress, scheduled money
reconciliation, scheduled paid-work dispatch, run-credit metering, and
out-of-credit mutation were removed. Former public/API ingress is intercepted
before authentication or handler construction by the typed
`openagents.money_surface_retired.v1` response.

Pylon no longer contains wallet, Spark, tips, labor-market payment, NIP-90
provider-market, or multi-earning authority. Its retained operator projection
is inventory-only and explicitly reports payment retired with paid-capacity
fallback denied. Billing and Sites UI/discovery were withdrawn, and the
ProductSpec/OpenAPI/capability/promise surfaces now agree with the runtime.

The repository fast policy includes a static VP-1 guard. It rejects restored
service trees, runtime classes/bindings, payment secret mounts, production
billing/Treasury routing vars, missing discovery filters, restored money
schedulers, restored scheduled paid dispatch, run metering, or removal of the
paid-capacity retirement reason. Applied migrations, typed 410 compatibility,
redacted receipts, and owner recovery documentation are explicit exceptions.

## Reconciliation interpretation

The archive preserves facts, not execution authority. In particular:

- a pending, approved, forwarding, dispatched, or recovery-pending label does
  not by itself prove a current external obligation;
- a matched reconciliation row with a stale parent state must not be resent;
- promotional/manual credits and USD-origin service credits are not
  automatically bitcoin-withdrawable or cash-refundable;
- paid capacity is retired, not made free by removing its debit; and
- applied migrations, idempotency keys, terminal receipts, and redacted public
  receipt references remain immutable evidence.

The owner wallet-recovery procedure is
[`VP-1 treasury wallet recovery runbook`](../ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md).
It is deliberately separate from code deletion and requires explicit owner
approval for any real send.

## Closeout gates

The typed retirement contract, no-paid-to-free rule, money cron/checkout/
payout/wallet/settlement deletion, zero live service authority, UI and
discovery withdrawal, dependency/deployment deletion, immutable archive, and
read-only historical evidence gates are complete. Focused Worker contract
tests, Pylon operator/catalog tests, Worker dry-run build, web/Start
typechecks, infrastructure formatting, and the static retirement guard pass.
The Worker typecheck has only the pre-existing VP-0 baseline diagnostics for
Cloudflare `ExecutionContext.tracing`, one Artanis nullable-string fixture, and
sqlite-runtime's Bun `readonly` option; VP-1 introduces no new diagnostic.

The preserved mnemonic and access token are not bound to a live service or
supported route. They remain recovery-only until the owner either performs the
separate sweep runbook or explicitly abandons the dust. Recovery therefore
does not reopen or block the retired product/runtime graph. Secret versions
must be disabled only after terminal destination receipts are verified.

Rollback before deletion is the read-only freeze, never renewed payment
ingress. After deletion, any future payment work begins as a new owner-approved
design rather than reactivating this archived graph.
