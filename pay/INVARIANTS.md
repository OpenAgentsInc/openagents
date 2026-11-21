# Invariants — pay/INVARIANTS.md

> Scope: Payments, Spark wallet, Lightning, LNURL, tips, voice-meter (2 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## The iOS wallet seed is read, never written; sends go through Coax

The Coax Native replacement (build 1069) exposes only `wallet.balance`, in
the OpenAgents-only `apps/openagents-desktop/extras/` side module. The same
`WalletKeychain.readSeed`, BIP-39 word list, default mainnet account and
`Application Support/spark-mainnet` directory are reused. `SparkHost.swift`
still owns every SDK import; `SparkConnection` shares the connection setup
with the previous wallet. `COAX_NATIVE_BALANCE` excludes all previous payment
methods and the seed-add method from the starter target. The SDK server
preset has background services and optimizers disabled, real-time sync off
and no automatic deposit claim fee. The adapter explicitly syncs, reads
`balanceSats` and disconnects. It neither creates a seed nor exposes keys,
words, wallet identity or SDK error text to Coax/JS. Missing, locked,
unreadable and unavailable never become a zero balance.

`pay/wallet-balance-policy.coax` admits only a successful supply-bounded
Bitcoin observation; its 27 root mutants are killed. The one bridge codec
is `pay/wallet-balance-wire.coax` (`openagents.wallet.balance.v1`). Native
unit/UI checks cover the side menu, missing-vs-zero and SDK configuration;
an opt-in mainnet check uses fresh in-memory test entropy and a separate
temporary SDK directory, never the owner's seed or money. This is not book
functionality. The following payment notes describe the excluded earlier
wallet implementation.

`apps/openagents-ios/OpenAgents/Wallet/` opens the person's existing Spark
wallet from the Keychain item the earlier apps wrote (generic password,
service `com.openagents.app.spark`, account `seed-v1`, 16 or 32 bytes of
BIP-39 entropy, default access group). The app has no code that adds,
updates or deletes that item, and never makes a seed. A missing, locked or
wrong-length item is reported on screen and left as it is. The entropy
becomes English BIP-39 words only to open Breez Spark on mainnet with no
passphrase; the words are never logged, stored elsewhere or passed to Coax.
The word list is checked against its published SHA-256 before use.

The wallet sends and receives (Lightning invoices and addresses, LNURL,
on-chain, Spark). `Wallet/SparkHost.swift` is the only file that imports the
SDK and hands the screens plain values. `pay/wallet.coax` is
`#authority` and compiled into the app, so its checks run on every build. It
decides each send: `metaverse_wallet_may_send` passes a quote only while its
payment is quoted, before it expires, with the fee no more than the review
screen showed, and with amount plus fee within what is spendable. The payment
moves only through `metaverse_wallet_step`:
- one `sendPayment` per quote, under one idempotency key;
- an unknown outcome waits for the SDK and is never shown as a failure;
- a finished payment stays finished until the person closes it.

The phone's owner check (Face ID, Touch ID or passcode) comes last, before
the send. SDK errors are shown in words, never as the SDK's own text. Backup
of the words and exit data are later work (`docs/lightning/`).

Buying bitcoin (Wallet → Buy bitcoin, #12613) goes through the SDK's
`buyBitcoin` with MoonPay's Bitcoin delivery or Cash App's Lightning invoice,
never an asset other than bitcoin. `pay/wallet.coax` decides it:

- `metaverse_wallet_buy_amount` passes a MoonPay amount only when it is more than
  the most the SDK may pay to claim the deposit
  (`metaverse_wallet_claim_fee_max`: the fastest recommended rate plus 1 sat/vB,
  times its 99-vbyte claim). When the SDK can't give that rate, a MoonPay
  purchase is refused.
- `metaverse_wallet_moonpay_page` passes only `https://buy.moonpay.io` with
  exactly one `currencyCode=btc` and exactly one `walletAddress` equal to the
  deposit address the wallet itself reports after the call. Every query
  parameter name is letters and digits, and the page has no `#` or `;`, so
  no other currency or address can hide behind an encoded name or a
  fragment.
- `metaverse_wallet_cashapp_page` passes only a `https://cash.app/launch/lightning/`
  link to a mainnet invoice (`lnbc` followed by the amount, not `lnbcrt`)
  whose own stated amount is exactly the sats asked, to the millisatoshi,
  and whose amount as the SDK parsed it is the same.
- `metaverse_wallet_buy_step` moves the purchase, and the host acts only on what
  `metaverse_wallet_buy_starts` and `metaverse_wallet_buy_opens` say from the state
  before each event: it asks the SDK for a page only when no request is
  already out, and opens a page only on the step from asking to open. One
  SDK answer opens at most one page, and a second tap or a late answer opens
  none.

A refused page is not opened.

Model-boundary exception: Debug builds launched with
`OPENAGENTS_THROWAWAY_WALLET=1` (the wallet UI test only) open a fresh wallet
from random words held in memory for that launch, in its own folder, never
the Keychain. Release builds compile this path out. Debug builds launched with
`OPENAGENTS_BUY_KEEP_PAGE=1` (the buy UI test only) keep the passed
provider page on screen instead of opening it, count the pages kept, and hold
each SDK answer for two seconds so a second tap lands while a page is being
asked for; Release builds compile it out.

Check: `apps/openagents-ios/tools/test-bip39.sh` (published BIP-39 vectors);
`coax test` and `coax mutate pay/wallet.coax` (0 survivors); the iOS UI
tests (a phone with no wallet shows that and changes nothing, and
`WalletUITests` against mainnet with a throwaway wallet: every kind of
request is made, sends are parsed and quoted, and an empty wallet's send is
refused in words; `testBuyMakesTheProviderPagesForThisWallet`: MoonPay's page
is for bitcoin to the address Receive then shows, Cash App's for a mainnet
invoice of the amount, too-small amounts are refused, and Cash App tapped
while MoonPay's page is being asked for starts nothing, so one page is kept);
and the owner's
device showing the existing balance.

## Inference credit is bought only with payments our own Spark wallet received

`POST /api/v1/inference/credit` (`coax/web/api/inference.c`) sells prepaid
inference credit under NIP-X402's `openagents:spark-receiver:1` profile.
Credit is added only when all of these hold, checked by the server, never
taken from the client:

- the invoice was issued by this server for this exact request: its row in
  `inference.topups` binds the payment hash, the invoice, the pubkey, the
  msat amount and the request digest, and the retry recomputes the digest
  from the actual request;
- the server's own Spark wallet reports that exact invoice received and
  complete, for at least its amount, with a preimage whose SHA-256 is the
  payment hash; a payer's preimage, when sent, must equal it, and alone
  never credits;
- the replay key `lnbtc:000000000019d6689c085ae165831e93:<payment_hash>` is
  inserted into `inference.used_payments` in the same transaction that adds
  the credit, so one payment credits once (`duplicate_settlement` after);
- the amount is within the top-up bounds; mainnet only, Bitcoin only.

`/api/v1/inference` admits admins, the allowlist, and any signed key whose
credit is above zero. A paying key's call is debited in msat at the cached
BTC/USD rate when it is metered; with no rate younger than an hour, paid use
is refused (503), never priced at a guess. The daily caps still apply to
every key. The wallet process (`coax/web/api/spark/`) is the only code that
imports the Breez SDK; it issues invoices, reports receives and reads the
rate, and sends nothing. Its seed exists only in Secret Manager. The wallet
is hot and holds small balances; the owner sweeps it.

Check: `coax/web/api/test.sh` (the `credit:` and `drawdown:` checks in
`test_inference.py`, against a fake wallet: unbound, unpaid, wrong amount,
wrong preimage, replayed and out-of-bounds top-ups refuse; one payment
credits once; no rate refuses paid use); `coax test coax/lib/x402/x402.coax`.

