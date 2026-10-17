# Design principles

Status: OpenAgents design recommendations, reviewed 2026-10-17.
Start with the [design index](README.md) for scope and source ownership.

## Make the next action clear

Give each step one primary action. Use secondary actions for alternatives such
as restoring an existing wallet. Explain a choice where the person makes it;
keep longer explanations available on request. Use a short sequence of screens
for a complex task rather than a long instructional card.

Keep everyday actions visible and advanced operations discoverable. A full
wallet needs room for recovery and deposit problems as well as its balance.
Do not hide an urgent action under a closed Advanced section. Hide controls
whose underlying behavior does not work yet.

These are our application rules. [WalletUI's button styles](sources.md#walletui)
provide a useful visual reference for action hierarchy, not a complete flow.

## Describe the person's situation

Write labels and errors in the words a person uses: what happened, what is
known about their money, and what they can do. Use **Send**, **Receive**, and
**Back up your wallet**. Explain a network or payment method when it changes
compatibility, cost, waiting time, or recovery.

A timeout does not establish that a payment failed. Suggested copy while its
outcome is unknown: “We're checking your payment. Check its status before
sending again.” Use “We couldn't send this payment” only after the wallet
confirms it was not sent. Keep diagnostic identifiers in expandable details.
OpenAgents speaks as “we”; an individual agent speaks as “I”.

## Make amounts unambiguous

Our review rule: every amount has a unit. Preserve the exact Bitcoin amount
through entry, review, and receipt; distinguish any approximate fiat value.
Show the recipient amount, fee or maximum fee, and total debit together.
Never show a missing balance or rate as zero. Distinguish spendable money from
incoming money that is still waiting. Check long amounts, decimal separators,
and larger text before approving a layout.

## Design for more than the default screen

[Bitcoin Design's accessibility guidance](https://bitcoin.design/guide/designing-products/accessibility/)
emphasizes meaningful labels, logical reading order, alternative input methods,
and respect for text size, contrast, and motion preferences.

For OpenAgents reviews:

- Provide scan, paste, copy, and share alternatives where applicable.
- Label icon buttons by their action. Give a QR code a useful accessible name.
- Keep errors and payment direction understandable without color.
- Keep amount, fee, total, and confirmation usable with large text.
- Support keyboard focus on desktop and screen readers on each native shell.
- Keep payment QR codes steady and readable; respect reduced motion.

## Share behavior across surfaces

Follow the [app contract](../../apps/openagents/OPENAGENTS.md): Rust owns
application state and wallet behavior; platform shells provide presentation.
Wallet cards, sheets, and forms use Rust Native and each shell's adapter.
WalletUI is a reference, not a new Swift application layer to import.

Our proposed component inventory is an amount field, balance summary, payment
request, address display, fee review, payment status, activity row, backup
reminder, and spend approval. Each needs loading, empty, error, and disabled
behavior where relevant. Product styling belongs in product modules; generic
semantics belong in Rust Native. This list proposes no new framework contract.

## Review a flow

Use fixture data to inspect narrow and wide layouts, light and dark themes,
large text, long addresses, a denied camera permission, and an unavailable
network. Walk through success, a delayed result, failure, and relaunch during a
payment. Ask whether the person can tell what happened, what they can safely
do next, and how to return to the result later. Use disposable test wallets
for any later payment verification; never the owner's funds.
