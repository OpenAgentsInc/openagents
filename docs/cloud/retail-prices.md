# Retail cloud prices and charge terms

Status: price book `retail-2026-10-05.1`, published for implementation
([#10706](https://github.com/OpenAgentsInc/openagents/issues/10706)). Paid
availability stays off until the funded qualification passes, and the owner
confirms the rates and the refund policy (`NEEDS_OWNER.md`). A change is a
new book version; a quote names the exact book it came from.

These terms price the [retail contract v1](retail-contract.md): the
`retail-boat-large-v1` computer running a `retail-repo-change-v1` task. The
book is [`crates/route-contract/fixtures/price-book-v1.json`](../../crates/route-contract/fixtures/price-book-v1.json),
and [`route_contract::price_book`](../../crates/route-contract/src/price_book.rs)
quotes and settles it.

## Units

- **One credit is one sat.** Credits are how surfaces show the purchased
  compute balance; the book refuses any other conversion rather than round
  it. Credits are not XP, game gold, a wallet balance, provider credits,
  Boat's allowance, or the GCE bill.
- **Integers only.** Prices are whole sats; the compute rate is whole
  millisatoshis per second. A task's compute charge is rounded up to a whole
  sat once, at settlement, never per second or per line.

## Prices in `retail-2026-10-05.1`

| Line | Payer | Basis | Price |
| --- | --- | --- | --- |
| Compute | Your purchased balance | Metered seconds of the task's sandbox | 40 millisatoshis a second (144 sats an hour) |
| Coordination | Your purchased balance | Once per task whose executor started | 100 sats |
| Model | Your own OpenAI key | Billed by OpenAI to you | Nothing from OpenAgents |

Both balance lines go to OpenAgents. A task is quoted for at most 3,600
seconds; you can set a lower limit and a ceiling in sats.

## Quotes

Every offer shows a quote: the book's version and digest, the computer and
task class, the quoted seconds, one line per resource with its payer and
its maximum, and the total maximum in sats and credits. Confirming the
offer reserves that maximum from your balance before anything starts. A
quote above your ceiling is not offered. The longest task's maximum is 244
sats: 144 for compute and 100 for coordination.

Work on your own computer, or one you paired, needs no purchase and is never
quoted.

## What a task is charged

| How the task ended | Charge |
| --- | --- |
| No sandbox started (no capacity, or refused before provisioning) | Nothing |
| The sandbox never became reachable | Nothing |
| The provider lost the sandbox before the executor started | Nothing |
| The executor ran: completed, failed its checks, failed, hit its provider's limit, or timed out | Metered compute seconds and the coordination charge |
| You cancelled, or a right was revoked, after the executor started | Seconds until the stop was acknowledged, and the coordination charge |
| The provider lost the sandbox after the executor started | Seconds until the loss, and the coordination charge, once the usage is read |
| The ending, or the usage, is not known yet | Nothing yet: the whole reservation stays held until reconciliation |

So a failed remote attempt is charged only when the executor started on the
machine; a failure of the provider before that costs nothing. No charge ever
exceeds the quote: metered seconds past the quoted ones are capped.

## Holds, releases, and refunds

- **A release is not a refund.** The part of a reservation a task did not
  use returns to your balance when the task settles. Nothing was paid, so
  nothing is refunded.
- **Settled charges are final.** As for every OpenAgents API call, a
  settled charge is not refunded automatically. A retry of a task that
  failed is a new task with a new quote and reservation.
- **Unknown stays held.** When a crash or a provider loss leaves the
  outcome or the usage unknown, the reservation stays held; it is never
  freed because a process restarted. Reconciliation then charges what was
  measured and releases the rest.
- **Purchases.** A top-up credits your balance in sats over Lightning. In
  v1, a purchased balance is not paid back out to Lightning.

## Estimates, metering, and bills

Three amounts are kept apart and never stand in for one another:

- **The quote's maximum**, from the book: what a task can at most cost you.
- **The metered charge**: the sandbox's measured seconds at the book's
  rate, plus coordination. This is what you pay.
- **The provider's bill**: what Boat bills OpenAgents for the sandbox,
  including replacements and failed starts. OpenAgents pays it; it never
  becomes your charge.

An amount that is not known is recorded as unknown, never as zero.
