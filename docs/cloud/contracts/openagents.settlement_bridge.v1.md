# `openagents.settlement_bridge.v1`

Status: design contract scaffold

This contract specifies how private Cloud compute metering and usage receipts
bridge into the customer-facing credit and invoice ledger in the public
`openagents.com` monorepo. It is a bridge contract, not a wallet contract and
not a public invoice schema.

Cloud remains the authority for private metering facts. The public
`openagents.com` D1 ledger remains the authority for customer credits, invoice
line items, and customer-visible billing state. Nexus/MDK remains the bridge for
any outbound payout path. The dormant `treasury` repo is not used.

## Purpose

Managed Cloud runs produce resource, model-usage, and internal-accounting
evidence that must be priced for customer billing without leaking private fleet
topology, raw provider cost, customer payment material, or private settlement
policy into public refs.

This bridge defines:

- the private Cloud inputs that are eligible for pricing;
- the customer-facing ledger outputs written by `openagents.com`;
- the separation between metering, priced line items, and settlement receipts;
- the required markup rules for compute and Model-2 inference;
- the retained refs-only projections shared across the private/public boundary.

## Inputs From Cloud

Cloud emits two receipt families into the bridge.

### Resource Usage Receipt

The bridge consumes refs to `openagents.resource_usage_receipt.v1` records. The
resource-usage receipt remains the authoritative proof of measured run facts:
host class, run duration, storage and artifact dimensions, model usage, token
count availability, and receipt digest.

Public projections must cite only bounded refs and digests from the receipt.
They must not copy raw host topology, raw provider account identifiers, raw cost
figures, raw customer identifiers, bearer tokens, wallet material, or payment
material into public-facing fields.

### Internal-Accounting Record

When `oa-node` runs in `internal-accounting` mode, Cloud emits an
internal-accounting record for the run or batch. The record cites the relevant
resource receipt digest, node/workroom/run refs, settlement-mode metadata refs,
result, and its own receipt digest.

The record is private Cloud evidence. It is not an invoice and not a public
ledger debit. `contributor-wallet` is out of scope for this repo, and
`no-wallet` runs may still emit metering receipts that later price into public
credits or invoice lines.

## Bridge Output

The settlement bridge maps eligible Cloud inputs to public `openagents.com` D1
ledger entries.

| Output | Purpose |
| --- | --- |
| Credit-ledger debit | Immediate customer credit consumption for prepaid or credit-backed accounts. |
| Invoice line item | Postpaid customer billing row for the same priced usage. |
| Public receipt projection | Refs-only pointer that lets public surfaces reconcile the debit or invoice line to private Cloud evidence. |

The public monorepo owns customer account state, credit balances, invoice
status, tax/payment integration, and customer-facing presentation. Cloud does
not write customer payment state directly.

## Pricing Rules

Pricing is deterministic from the accepted Cloud metering input and the active
public ledger price policy for the account at pricing time.

### Compute

Compute usage is priced from Cloud's metered infrastructure input plus the
customer-facing compute markup configured by `openagents.com`.

The priced line item must retain:

- `usage_ref`, pointing to the private resource-usage receipt digest or bridge
  usage ref;
- compute class and metered dimensions only when those dimensions are
  public-safe for the account surface;
- `pricing_policy_ref`, pointing to the public ledger pricing policy version;
- `amount_microusd`, the customer-facing debit or invoice amount.

Raw provider cost and private fleet placement remain private Cloud material.
If the Cloud receipt has nullable cost input because provider billing data was
unavailable, the bridge must either defer pricing or price only from an
explicit public policy that does not require raw provider cost. It must not
invent provider cost.

### Model-2 Inference

Model-2 inference is priced as provider inference cost plus a 10% fee.

```text
model_2_customer_amount = provider_inference_cost + 10%
```

The line item must cite the model usage record ref, usage receipt digest, and
pricing policy ref. Provider cost may be used inside private Cloud or the
bridge pricing worker, but public retained projections must not expose raw
provider invoices, provider account refs, or raw account credentials.

If the model provider does not expose trustworthy token counts or cost for a
run, the bridge must preserve the explicit `usage.unavailable` or
`count_source = unavailable` evidence and defer Model-2 pricing for that
record. It must not estimate billable Model-2 cost unless a later contract
ratifies an estimation policy.

## State Separation

The bridge keeps three states separate.

### Metering State

Metering state is private Cloud evidence. It includes resource usage receipts,
model usage records, internal-accounting records, node/workroom refs, and
receipt digests.

Metering proves what happened. It does not debit credits, create an invoice
line, or prove customer settlement.

### Priced Line Item State

Priced line item state lives in the public `openagents.com` D1 ledger. It
contains customer-facing debits and invoice rows derived from accepted metering
refs and a pricing policy ref.

Pricing decides what the customer owes or what credit balance is consumed. It
does not prove that a payment settled or that an outbound payout occurred.

### Settlement Receipt State

Settlement receipt state proves the result of a payment, credit consumption,
invoice collection, or outbound payout bridge action. It cites the priced line
item refs and the appropriate payment or payout refs.

Settlement receipts must not be collapsed into metering receipts. A run can be
metered before it is priced, priced before it is collected, and collected
without granting Cloud wallet authority.

## Treasury Exclusion

The dormant `treasury` repo is not part of this contract and must not be used
as an implementation, dispatch, reconciliation, billing, custody, or payout
authority.

Current ownership is:

| Surface | Authority |
| --- | --- |
| Private metering and internal-accounting evidence | Cloud private repo |
| Customer credits, invoice ledger, and payment state | Public `openagents.com` D1 ledger |
| Outbound payout bridge, if required | Nexus/MDK |
| Contributor wallet UX | Public Pylon, not this private Cloud repo |

Historical `treasury://...` refs may appear only as legacy metadata refs when
already emitted by older internal-accounting receipts. New bridge work must use
current Cloud refs, public ledger refs, and Nexus/MDK payout refs as
applicable.

## Refs-Only Retained Projections

Public retained projections are refs-only. They may contain:

- `usage_ref`;
- `resource_usage_receipt_digest`;
- `internal_accounting_receipt_digest`;
- `priced_line_item_ref`;
- `credit_ledger_entry_ref` or `invoice_line_item_ref`;
- `pricing_policy_ref`;
- `settlement_receipt_ref`, when settlement has occurred;
- coarse product labels such as `compute` or `model_2_inference`;
- customer-visible amount fields that belong to the public ledger.

Public retained projections must not contain:

- raw provider cost or raw provider invoices;
- raw customer identity beyond the public ledger account ref;
- payment method material, processor secrets, bearer tokens, wallet seeds,
  private keys, or raw capability credentials;
- private fleet topology, raw node placement, raw provider account refs, or
  internal settlement policy;
- unredacted logs, prompts, artifacts, or workroom filesystem paths.

## Validation Rules

- Every priced line item must cite at least one accepted Cloud usage ref or
  receipt digest.
- Every public debit or invoice line must cite a public pricing policy ref.
- Compute markup must be applied only in priced line item state, not by
  mutating the source metering receipt.
- Model-2 inference line items must apply provider inference cost plus 10% fee
  or remain unpriced until trustworthy cost is available.
- `count_source = unavailable` or `usage.unavailable` must remain visible in
  refs-only projections for unpriced usage.
- Settlement receipts must cite priced line item refs; metering receipts alone
  are not settlement proof.
- Public retained projections must remain refs-only and must reject raw cost,
  customer payment material, secrets, private topology, and raw accounting
  credentials.
- The bridge must not dispatch through, depend on, or revive the dormant
  `treasury` repo.
