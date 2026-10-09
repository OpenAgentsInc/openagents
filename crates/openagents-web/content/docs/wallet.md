# Wallet

The iPhone app's Wallet tab is a bitcoin wallet on mainnet, with its keys
on your phone. It runs on Breez's Spark SDK. It's real bitcoin, so keep
amounts you'd be comfortable carrying in a phone wallet.

The wallet is in the phone app only (iPhone, and Android in testing). The
Mac app, the Terminal, and the website have none; asking a computer's chat
to open your wallet says where to find it.

## Receive

Tap **Receive** on the [Wallet tab](/docs/iphone). You get a payment request with a QR code, **Copy**, and
**Share**, and you can set an amount. Under **Advanced → Other ways to
receive** are a Lightning invoice, your Spark address, a Bitcoin deposit
address, and your npub, with a switch that publishes your Spark address in
your Nostr profile so people can pay your npub.

On-chain deposits are added to your balance automatically once they
confirm. A receiving Lightning address of your own isn't available yet.

## Send

Tap **Send** on the [Wallet tab](/docs/iphone), then paste or scan what you're paying: a Lightning invoice, a
Lightning address, an LNURL code, an npub, or a Spark or Bitcoin address.
Enter an amount if it needs one. A confirm screen shows the amount and the
fee, and nothing is sent until you confirm. On-chain withdrawals offer
three speeds.

## Buy bitcoin

**Advanced → Buy bitcoin** pays with dollars through MoonPay or Cash App,
which open their own page.

## Back up and restore

Your wallet's backup is its recovery words.

1. Until you've written them down, the Wallet shows **Back up your wallet**.
2. **Show recovery words** reveals them behind a warning. Write them down
   and keep them offline.
3. To restore, enter 12 or 24 words.

No one from OpenAgents will ever ask for your recovery words. Anyone who
has them has your bitcoin.

The **Exit backup** under Advanced saves the state you'd need to withdraw
on-chain if Spark's operators were down, and exports it to Files. Running
an exit from that file is a later recovery feature.

## Agent payments

An agent on one of your computers can ask your phone to pay an invoice.
The request shows as a **Payment request** sheet: the computer, the task,
the purpose, who's paid, the amount, the fee, and what that computer's
grant has left. Nothing pays until you tap **Approve**; above ₿1,000,
Approve asks for Face ID or your passcode. **Deny** refuses it, and **Stop
payment requests** stops that computer asking. **Advanced → Agent
payments** lists the requests and which computers may ask.

## Amounts

Amounts show as whole numbers of the smallest unit: ₿1 is what was called
a satoshi, so 0.00010000 BTC shows as ₿10,000. **Show amounts as** under
Advanced switches to the BTC form.

## Who you rely on

Spark's operators are run by Lightspark, Breez, and Flashnet. Two must
cooperate for payments off the chain, and your safety depends on at least
one of them having deleted old keys, which no one can check. If the
operators stop, you can still withdraw on-chain yourself, though it can
take days and needs a separate on-chain fee. The **i** button in the
Wallet shows this note.

Next: [Decks and the Map](/docs/decks).
