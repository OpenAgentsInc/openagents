# Wallet

[OpenAgents for iPhone](/docs/iphone) has a bitcoin wallet on mainnet,
with its keys on your phone. It runs on Spark, through Breez's SDK. It's
real bitcoin, so keep amounts you'd be comfortable carrying in a phone
wallet.

It's under **Settings → Wallet**. The wallet is in the phone app only;
the website and the Terminal have none.

## Receive

**Receive** gives you three ways to be paid, each with a code to scan and
**Copy payment request**:

- **Lightning invoice:** for an amount you set, or any amount.
- **Spark address:** free and instant from other Spark wallets.
- **Bitcoin address:** arrives after 3 confirmations on the Bitcoin
  chain.

## Send

**Send** takes a Lightning invoice, a Bitcoin address, or a Lightning
address. You paste it, and enter an amount if the request has none.
**Review payment** shows the amount and the fee, and nothing is sent until
you confirm. **Edit payment** goes back to change it.

**Activity** lists your recent payments.

## Back up and restore

Your wallet's backup is its recovery words. On a new account, they're the
same 12 words that make your OpenAgents account, so one backup covers
both. A wallet you had before keeps its own words and balance.

1. Until you've saved them, the wallet shows **Back up your wallet**.
2. **Recovery words** shows them after Face ID. Write them down and keep
   them offline.
3. **Restore wallet** brings a wallet back from its 12 or 24 words.

No one from OpenAgents will ever ask for your recovery words. Anyone who
has them has your bitcoin.

## Who you rely on

The wallet runs on Spark, not on a Lightning node of your own, and
Lightning payments go through Lightspark. Spark's operators must
cooperate for payments off the chain. If they stop, you can still take
your bitcoin out on the Bitcoin chain yourself, but it can take days and
needs a separate payment for fees. **About this wallet** shows this note.

Next: [Settings](/docs/settings).
