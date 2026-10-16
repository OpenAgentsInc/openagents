# Sovereign treasury

`sov-treasury` implements the NIP-SOV treasury binding. It is one durable
accounting authority that ties an agent to its verified API account, its
earnings beneficiary, its wallet, an authorizer, and the X402 top-up resources
it may buy from. The money stays in the existing owners:

- `pay-ledger` records LAB earnings and the payouts that actually went out
  (`Earnings` is implemented for `pay_ledger::Ledger`).
- The agent's wallet pays invoices and reports inbound payments (`Wallet`).
- The X402 router (`openagents-x402`) sells the top-up, and the account
  service credits the API account once per settled payment (`RouterTopUp`,
  `Accounts`).
- The X402 buyer ledger (`openagents_x402::policy::Ledger`) holds and settles
  each purchase under its ceilings and provider allowlist.

The treasury keeps no spendable balance of its own.

```sh
sov-treasury init --state DIR --key-file KEY --config CONFIG.json
```

CONFIG maps each agent to its governing authority. The hosting process mounts
the wallet, the earnings ledger, and the top-up resource (`Owners`) and submits
signed private `3188` requests (`Treasury::submit`). Wiring this into
`openagents sov` waits for the next Coder release.

## Rules the treasury enforces

- **Policy.** The agent's authority signs the policy. The policy binds the
  agent, the API account and its retained control evidence, the beneficiary,
  the wallet adapter and account, the authorizer, approved destinations, the
  payment profile, the network, exact `msat` units, the per-operation, fee,
  and aggregate ceilings, the accounting period, spender caps, the income rule,
  the reconciliation authority, and validity. Regtest is refused rather than
  aliased, because X402 covers only mainnet and testnet. A wallet without a
  hard fee ceiling and a payment lookup refuses with `cannot_enforce`. Without
  a policy (a null treasury), nothing is spent.
- **Income.** Income counts only when the ledger shows a `sent` payout to this
  beneficiary for that exact amount and wallet reference, and the agent's
  wallet has that inbound payment. It must be reported by the agent or its
  reconciliation authority. A payout is credited once. Income raises the
  allowance only under `income.mode = "replenish"`, and never by more than
  `max_msat` in a period.
- **Purchases.** A spender requests an exact purchase, and the authorizer's
  signed approval for those exact bytes must accompany it. The destination
  must be approved, and the amount must fit the per-operation ceiling and the
  shared allowance. The allowance counts settled spend, open holds, and unknown
  liabilities across every child and host, and a child's cap draws on the same
  allowance. The seller's X402 challenge is validated (binding, invoice terms,
  amount, network, payee) before the wallet is touched. The hold is persisted
  before paying.
- **Recovery.** A purchase ID with changed bytes refuses; an exact retry
  returns the retained receipt and never pays again. A lost wallet reply or a
  lost credit acknowledgment stays `unknown`, keeps its hold across period
  rollover, and is reconciled only by the exact payment hash: a credit receipt
  completes it, a proven wallet failure releases it, and anything else stays
  unknown. Each payment is credited once.
- **Statement.** `statement` signs a private view of the period: allowance,
  replenishment, settled, held, and unknown amounts, with their source
  references. It is not another ledger.

The fixture runs the full loop with fake money: real BOLT11 test invoices, the
real X402 router and replay store, and an in-memory `pay-ledger`. Qualifying
it with real money is an owner step.
