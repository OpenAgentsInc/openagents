# Purchase terms for billed plans

This document publishes what buying a plan on an OpenAgents deployment
means today. It exists because
[#9492](https://github.com/OpenAgentsInc/openagents/issues/9492)
requires published terms before checkout is enabled, and it is honest
about the line that remains: the only provider this build serves is
`sandbox`, which moves no real money. These terms govern the billing
surface as implemented; the commercial decisions — launch prices,
processor, jurisdiction — stay open in
[#9498](https://github.com/OpenAgentsInc/openagents/issues/9498) and
block any live checkout.

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

## Cancellation and expiry

`POST .../billing/cancel` ends renewal: the paid period's entitlement
and allowance run to their end, and the subscription expires when the
period does. Grant credit with `credit_expiry_secs` debits at expiry
under the same bounded-clawback rule as a refund. A cancellation does
not refund the current period — that is a refund event's job.

## Authorization

Billing management is owner-only: `Permission::ManageBilling` sits on
the owner role, and every mutation route — subscribe, checkout, plan
change, cancel, reconcile — requires it. A paid change or a top-up is
always an explicit owner act; nothing in the surface moves money on a
browser redirect, a shared link, or a member credential.
