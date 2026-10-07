# Payments

How OpenAgents receives every payment centrally, splits it between plugin
authors, resource owners, and OpenAgents, pays people out, and shows the money
moving in real time. The pieces (#10185 to #10199) are built and run on
the pay host `oa-pay-1` ([deployment](../deployment/pay-host.md)); the
first real payments are the owner's.

| Document | What it covers |
| --- | --- |
| [Central receive, splits, payouts, and the live flow view (design, 2026-10-02)](2026-10-02-central-receive-and-splits.md) | The owner's direction; why the receiver is our own `crates/wallet` node on MoneyDevKit's LSPS4 liquidity and not the MDK treasury container; the architecture; who gets paid and the launch bonus; the ledger, split rules, and reconciliation; payout destinations and rails; the public flow stream, `/stats`, `/live`, and the `routes-live` deck scene; the end-to-end demo; risks; phases; the issues. |
| [End-to-end demo, the agent run (2026-10-03)](2026-10-03-end-to-end-demo.md) | `scripts/payments-demo.sh`, what was set up for it (the pay front behind `api.openagents.com`, the relay update, the check plugin), what was verified without owner funds, and the table for the owner's paid run. |

Related: [the OpenAgents API](../api/README.md) (x402 payment, plugin fees),
[Breez and Spark](../breez/README.md) (the user Wallet),
[Bitcoin](../bitcoin/README.md) (node history), and
[NIP-X402](../../nips/openagents/NIP-X402.md).

## Private earnings and payout management

The gateway's optional `earnings` block connects account authorization to the
receiver and payout worker's existing SQLite ledger. Set `ledger` to that
ledger's path and declare `grants` as `{party, account, workspace}` records
after checking the payee's ownership. Each request requires that exact account
and current active membership in the named workspace. Public payment events
grant no private access. Removing membership, closing a session, or revoking
an API key denies subsequent reads and destination changes.

`GET /v1/earnings` lists authorized payees. `GET /v1/earnings/{party}` returns
exact `msat` obligations, release and rule references, available claims,
reserved claims, claims consumed by sent payouts, actual rail amounts, and
sub-sat rounding. `limit` accepts 1–200 settlements and payouts per page;
`after_earning` and `after_payout` are exclusive cursors. Totals cover the
whole payee ledger. JSON output is capped at 1 MiB; an oversized page asks you
to lower `limit`. `/export` downloads the same bounded JSON page without
payer aliases, settlement payment hashes, invoices, raw wallet errors, or
resource queries. Wallet references identify that payee's exact payout
attempt. `/payouts/{payout}` supplies private reconciliation details; it never
initiates or retries a payment. These responses use `Cache-Control: no-store`.

`GET` and `PUT /v1/earnings/{party}/destination` read and update the account
fallback. A write requires `{expected_version, value}`; use version `0` to
create it and the current version to change it. The existing resolver checks
mainnet Spark or Lightning address format. Signed releases, registrations,
and published profiles retain priority. Every existing reservation retains
its original destination and wallet reference. The payout worker reconciles
`sending` and `unknown` attempts by looking up that same reference; an absent
wallet record does not release the reservation or justify another send.

Declare supported rails in `earnings.rails` as `spark` or `lightning` mapped
to an owner qualification evidence reference. An undeclared rail is
unavailable for destination changes. Source validation and this declaration
do not prove that a particular payment succeeded. The owner still qualifies
real destinations and funds the existing worker. The surface does not add
a payment engine, withdraw purchased credit, infer commissions, or invent
reversals. Commission and reversal capabilities are explicitly unavailable
until their authoritative obligation owners are integrated.

The authenticated dashboard links to `/dashboard/earnings`; it renders the
same scoped statements, private downloads, reconciliation details, and a
versioned destination form with a session-bound forgery token. Isolated ledger,
fake-rail, and real HTTP tests cover conservation, scoped access, revocation,
conflicting destination changes, and restart recovery without owner funds.
