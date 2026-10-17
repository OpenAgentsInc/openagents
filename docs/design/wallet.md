# Wallet experience

Status: proposed experience guidance for the full OpenAgents Bitcoin wallet,
reviewed 2026-10-17. Current capability statements below come from the
[Spark wallet audit](../audits/2026-10-17-spark-wallet.md), not a new runtime test.

## First use

[Bitcoin Design's first-use reference](https://bitcoin.design/guide/daily-spending-wallet/first-use/)
favors a flexible setup, a clear create action, an existing-wallet recovery
path, timely explanations of responsibility, and reminders for important
skipped steps. It introduces funding costs before the first deposit and
shows incoming funds as soon as they are detected.

Our recommended flow:

1. Offer **Create wallet** and **Restore wallet**. Returning users must not
   accidentally replace existing money or keys.
2. Explain who controls the wallet and what losing its backup means.
3. Offer the backup and supported security settings. Record what actually
   completed so reminders survive reopening the app.
4. Present an empty wallet with **Receive** and **Send**, a backup reminder,
   and access to help. Do not require a deposit to explore it.
5. For the first receive, explain the amount expected, possible fees, and
   waiting conditions before the person shares the request.
6. Show progress through receipt and spendability. Revisit an unfinished
   backup after funding without interrupting every visit.

Do not copy the reference's channel-opening, channel-backup, or automatic
on-chain-to-Lightning swap explanations into Spark. Those describe its chosen
Lightning architecture. Spark needs its own deposit and recovery explanations.

## Everyday wallet

Keep a clear balance, **Receive**, **Send**, and recent activity at the top.
Provide details for each payment and a persistent route to payments that need
attention. Advanced features can sit below this main surface.

The audit reports a current phone surface with balance, three receive methods,
quoted sending, recent activity, a trust note, and backup. Deposits, refunds,
exit-data export, and phone spend approvals from the earlier app need porting.
A screen from the earlier app is not evidence that its action works today.

## Receive and fund

[Bitcoin Design's receiving reference](https://bitcoin.design/guide/daily-spending-wallet/requesting/receiving/)
shows incoming status, explicit fees, and persistent actions when receipt
needs help. Its offline and channel-management mechanisms are specific to
that reference wallet.

Our Spark adaptation:

| Situation | Experience to design |
| --- | --- |
| Choose a request | Make Spark, Lightning, and Bitcoin compatibility understandable at the point it matters. A QR, copy, and share action must use the same request. |
| Amount or expiry changes | Regenerate the request and show its current amount and expiry. Make an expired request visibly unusable. |
| On-chain deposit detected | Show the amount waiting separately from spendable funds, with the SDK's actual confirmation requirement. |
| Deposit needs a claim | Explain the required action and show a current claim fee and net credit before approval. |
| Refund available | Review the destination, fee, and expected return before a separate confirmation. |
| Received | Show the credited amount, any fee, and a payment detail entry. |

Do not invent a fixed confirmation count or promise a free first deposit.
Do not add a buy button until the provider handoff and return path work.

## Send

[Bitcoin Design's sending reference](https://bitcoin.design/guide/daily-spending-wallet/sending/)
places review before payment, explains delays and errors, and keeps the wallet
usable while a transaction completes.

Our proposed sequence is **Scan or paste → Review → Confirm → Status**.
Parse and validate in Rust. Review the destination, amount, unit, fee ceiling,
and total debit. If the quote changes, require another review. A scan, paste,
or deep link must never itself authorize payment.

| Result | Suggested presentation and next action |
| --- | --- |
| Invalid destination, wrong network, expired request, or unsupported format | Explain the specific problem before confirmation; offer replacement input. |
| Confirmed not sent | Say it was not sent and offer a safe correction or retry. |
| Still processing or outcome unknown | Keep one payment entry, check its status, and avoid encouraging another payment. |
| Completed | Show a receipt with amount, fee, destination, and time. |

The audit reports invoice-derived idempotency for the x402 Spark payer and
SDK resumption of interrupted sends. UI retries must continue that payment,
not create a new attempt with fresh identity. A missing preimage is not proof
of failure. Keep technical payment proofs in details rather than the success
headline. Do not offer cancellation, fee bumping, or routing controls unless
our wallet actually supports them.

## Trust and service limits

Explain Spark's trust model during setup and keep it available in settings.
The audit carries forward the warning that off-chain payments depend on
operators, Lightning payments use a provider, and an outage exit can take days
and require separate fee funding. Describe those consequences in plain words;
a self-custody label alone does not explain them. Recheck the exact operator
and recovery details before publishing product copy.

## Backup, recovery, and computers

The audit distinguishes ordinary restoration from an operator outage. Words
can restore the wallet while Spark works; an outage exit also needs previously
collected exit data, fee funding, and time. The current phone has not restored
its earlier exit-data backup flow. Do not claim that writing down words alone
makes every recovery possible.

The identity work aims to use the same words for identity and money, while the
audit found a creation-order bug that can still produce two sets. Before
shipping unified-backup copy, verify which words the actual wallet uses.
Explain the shared consequences when the identity and wallet do share them.
Never silently replace existing wallet words to make the design fit.

Computer linking currently transfers the seed, and the audit says the phone
answerer is missing. A narrower account-key handoff is a proposal. Design
linking disclosures for the authority actually transferred; do not call a
computer read-only or budget-limited while it holds full spending keys.

## Agents and x402

Proposed spend approvals show the agent, purpose, recipient, amount, maximum
fee, and total before approval. A standing permission also needs a visible
budget, expiry, remaining amount, and revocation control. A chat request or
computer pairing is not spending consent. Keep approval controls hidden until
the current phone can answer them.

According to the audit, Spark pays x402 today, but x402 receiving stays on
`ldk-node`: the Spark provider's shared invoice key does not satisfy the
current exclusive-receiver requirement. Do not market a generic Spark receive
screen as an x402 merchant receiver. Token allowances are not Bitcoin spending
limits. Dollar balances, Lightning address hosting, history sync, and narrower
computer keys remain owner decisions in the audit.

## Next design work

Prioritize backup correctness, honest payment status, and actionable deposit
states before new payment formats. Design restored phone approvals and desktop
wallet cards against working shared Rust operations. Treat BOLT12, dollar
balances, and additional LNURL flows as separate feature work; parsing or SDK
availability alone does not make them usable in the app.
