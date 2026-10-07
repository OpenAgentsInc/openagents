# Purchase terms for billed plans

This document publishes what buying a plan on an OpenAgents deployment
means today. It exists because
[#9492](https://github.com/OpenAgentsInc/openagents/issues/9492)
requires published terms before checkout is enabled, and it is honest
about commercial activation. The sandbox plan provider moves no real money;
the optional native Stripe prepaid profile requires explicit deployment terms,
restricted credentials, and owner qualification. These terms describe both
code paths. Launch prices, processor selection, and jurisdiction still require
owner decisions in `NEEDS_OWNER.md` before enabling live checkout.

## Price configuration

`GET /v1/plans` publishes the deployment's actual configured catalog —
each plan's id, version, price in millionths of the named currency,
period, allowance, seats, and entitled doors. The catalog is the price
list; there is no hidden schedule. A subscription pins the plan
version it bought, so an operator changing the catalog reprices only
new checkouts, never a subscription mid-period.

## Currencies

Amounts are fixed-point millionths of an explicitly named currency —
`9000000` under `USD` is nine dollars. A plan declares its currency;
a checkout in any other currency is refused. Multi-currency accounts
are not supported: a workspace's ledger position exists in the
currency its grants and charges share.

## What a payment is

Under `sandbox`, a payment is an operator-emitted provider event —
`billing-sandbox emit` writes the event a live processor's webhook
would carry. It exercises the full checkout, renewal, failure,
cancellation, refund, dispute, and recovery path, and it creates no
obligation to pay and receives no funds. A deployment selling real
plans requires a live provider adapter and the resolved commercial
terms in [#9498](https://github.com/OpenAgentsInc/openagents/issues/9498);
until then, treat every invoice and charge on this surface as fixture
data.

## Refunds, disputes, and support

A refund or a dispute arrives as a signed provider event
(`charge-refunded`, `charge-disputed`) and claws back the granted
credit — the lesser of the named amount and the workspace's available
balance, so a clawback never removes credit already committed to work
and never drives an account negative. There is no self-serve refund
route: a workspace's owner requests one through the operator, and the
operator issues the provider event. Support for billing state is the
owner's `GET /v1/workspaces/{id}/billing` view — subscription,
invoices, checkouts, and balance — and the operator's
`POST .../billing/reconcile` sweep for lost or replayed deliveries.

The separate native prepaid ledger implemented in
[REV-22](https://github.com/OpenAgentsInc/openagents/issues/10829) separately
records processor adjustment expense in ledger units. Under its initial
zero-risk policy, known positive expense or uncovered spent/held obligations
restricts new reservations across all workspace lots; existing holds survive.
A pending fee return cannot release that restriction, and an available fee
return creates no customer credit. Its native controller and routes remain off
unless explicitly configured. Owners and admins with current account,
workspace, and resource authority may quote, approve checkout, read status, or
request native reconciliation. The quote pins USD terms and customer-paid
verified fees; only independently verified final collection makes purchased
credit available. Missing native evidence or merchant access quarantines
uncommitted credit while preserving holds and unknown obligations. A browser
return never funds an account. Old sandbox balances cannot fund dispatch in
this profile, and card credit does not establish outbound wallet liquidity.
See [native prepaid configuration](billing.md#native-prepaid-profile).

## Cancellation and expiry

`POST .../billing/cancel` ends renewal: the paid period's entitlement
and allowance run to their end, and the subscription expires when the
period does. Grant credit with `credit_expiry_secs` debits at expiry
under the same bounded-clawback rule as a refund. A cancellation does
not refund the current period — that is a refund event's job.

## Authorization

Sandbox subscription management is owner-only: `Permission::ManageBilling` sits on
the owner role, and every mutation route — subscribe, checkout, plan
change, cancel, reconcile — requires it. A paid change or a top-up is
always an explicit owner act; nothing in the surface moves money on a
browser redirect, a shared link, or a member credential.
