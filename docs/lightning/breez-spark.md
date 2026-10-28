# Breez Spark

*Reviewed against `breez/spark-sdk` `main` at `83c8122` (2026-10-27),
latest tag `0.27.2`, in a read-only clone. Swift paths are under
`crates/breez-sdk/bindings/examples/cli/langs/swift/`. Nothing was built or
run for this note.*

## What Spark is

Spark is a Bitcoin layer 2 run by Lightspark. A balance is a set of
**leaves**: off-chain outputs whose spending key is split between the user
and a set of **operators**, using FROST threshold Schnorr signatures. A
transfer re-keys a leaf to the receiver. The operators sign their share and
are meant to delete the old one. Each leaf also carries pre-signed
transactions down a tree to the chain, so the user can leave without the
operators.

- **Operators.** The default pool has three: Lightspark, Breez and
  Flashnet. Two of them must sign.
- **Lightning** goes through Lightspark's **service provider (SSP)**. To
  pay, the SSP routes the payment and the leaf moves only against the
  preimage. To receive, the SSP's node issues the invoice and hands over a
  leaf when it is paid. This is why a Spark wallet receives while the app
  is closed.
- **On-chain.** Deposits go to a static address and are claimed into
  leaves. Withdrawals are cooperative exits.

## Trust model

Say these points plainly in the app's trust note:

- **The main assumption.** A transfer is final once at least one operator
  has deleted its old key share. Nobody can verify that deletion.
  - If two operators kept their old shares, they could double-spend a
    leaf's history together with a previous owner of that leaf.
  - Lightspark and Flashnet are enough for the threshold without Breez.
- **Operators can stop serving.** Payments then stop.
- **Leaving without the operators** (unilateral exit):
  - It needs exit data synced to the device while the operators were
    reachable.
  - It needs a confirmed native SegWit UTXO of the user's own to pay CPFP
    fees.
  - It takes days of timelocks.
  - The SDK builds and signs the transactions; the app broadcasts them.
  - Leaves too small to pay their own fees are lost.
- **Old exits.** If a previous owner broadcasts an old exit, the current
  owner must answer within the timelock. The SDK's watchtower fee ladder
  pre-signs refunds at several fee rates for this.
- **Metadata.** The operators and the SSP see payment metadata.
- **Breez's own services:**
  - checking the API key on mainnet;
  - a configuration server;
  - an LNURL / Lightning address server, which can be self-hosted;
  - an encrypted multi-device sync server;
  - a 5 bps integrator fee on Flashnet conversions. We keep conversions
    off.

**Our position** (owner decision, 2026-09-28): acceptable for a person's
spending wallet with small balances, with a trust note in the app. Not
acceptable for a treasury.

## The Swift SDK

Add `https://github.com/breez/breez-sdk-spark-swift.git` with Swift Package
Manager. Its product, `BreezSdkSpark`, is a prebuilt xcframework built with
UniFFI from Rust. The Swift CLI is Breez's reference integration. It is an
automated port of their Rust CLI, which is the source of truth.

```swift
var config = defaultConfig(network: .mainnet)
config.apiKey = breezApiKey                        // required on mainnet
let seed = Seed.mnemonic(mnemonic: words, passphrase: nil)
let builder = SdkBuilder(config: config, seed: seed)
// optional: builder.withRestChainService(...), withStorageBackend(...)
let sdk = try await builder.build()
```

The CLI opens the SDK this way (`main.swift` around line 350). The default
store is SQLite in the data directory. The calls a wallet needs:

| Need | Call |
| --- | --- |
| Balance | `getInfo` (balance, and `recoverableFundsSats` on new versions) |
| Sync | `syncWallet`; events through `addEventListener` (`Synced`, `PaymentSucceeded`, `PaymentPending`, `PaymentFailed`, deposit events) |
| Receive | `receivePayment` with `.bolt11Invoice(description, amountSats, expiry, ...)`, `.sparkAddress`, `.sparkInvoice`, or `.bitcoinAddress` |
| Understand pasted or scanned input | `parse(input:)`: BOLT11, Lightning address, LNURL, Spark address, Bitcoin address |
| Send | `prepareSendPayment` (returns the fee quote), then `sendPayment` with the prepared response and an idempotency key |
| LNURL and Lightning addresses | `prepareLnurlPay`, `lnurlPay`, `lnurlWithdraw`, `lnurlAuth`, and registering, checking or deleting a Lightning address |
| History | `listPayments` with filters, `getPayment` |
| On-chain deposits | `listUnclaimedDeposits`, `fetchClaimDepositQuote`, `claimDeposit`, `refundDeposit`, `recommendedFees`; automatic claims under `maxDepositClaimFee` |
| Exit | `exportUnilateralExitState` and `importUnilateralExitState`; `prepareRecoverFunds`, `recoverFunds` and `checkRecoverFunds` on `main` (check which tag first carries them) |
| Policy hook | `PaymentObserver.beforeSend` sees every payment about to go out and can cancel it |
| Stop | `disconnect` |

### Configuration we set

| Setting | Value | Why |
| --- | --- | --- |
| `network` | `.mainnet` in the app; `.regtest` in tests | Spark has no hosted Lightning test network |
| `apiKey` | The committed Breez key | Owner decision 2026-09-28: it is a basic validation key the Breez team confirmed can't be hidden |
| `crossChainConfig` | unset | USDC/USDT on other chains break the money rule. The CLI sets it on mainnet; don't copy that line. |
| `stableBalanceConfig` | unset | USDB is a Spark token, not a Taproot Asset |
| `realTimeSyncServerUrl` | unset | Sends encrypted wallet data to Breez's server; needs its own decision |
| `maxDepositClaimFee` | recommended rate + 1 sat/vB | What the earlier phone wallet used |
| `lnurlDomain` | ours when we host addresses | `name@openagents.com` (#12307 had a self-hosted LNURL server with NIP-57 zaps) |
| Buying (MoonPay, Cash App) | off | The Cash App bridges are USDC/USDT. MoonPay was on before (#9865); turn it back on only by owner decision. |

### What the CLI does that we must not

- **It writes the words to a plain `phrase` file** (`Persistence.swift`).
  We keep the entropy in the Keychain and in memory only.
- **It derives a seed from a passkey** (`Passkey.swift`,
  `resolvePasskeySeed`), and its file-based PRF secret also sits on disk. A
  passkey seed replaces the 12 words, which would break the one-backup rule
  for Nostr and money. Leave it out.
- **It turns on cross-chain on mainnet.** See the table.

## Keys and the shared words

- 12 BIP-39 words, English list, empty passphrase, give a 64-byte seed.
- **Nostr (NIP-06):** `m/44'/1237'/0'/0/0`.
- **Spark:** account master `m/8797555'/{account}'`. Mainnet's default
  account is 1, other networks use 0. The identity key is at `0'`, leaves
  under `1'`, static deposits under `3'`
  (`crates/spark/src/signer/default_signer.rs`).
- The keys are separate, but **whoever holds the words holds both the
  identity and the money.** The backup screen says so once.
- **A computer must never get the words.** It gets the hardened account key
  `m/8797555'/1'`, which can't be turned back into the seed or the Nostr
  key. The SDK opens from it with `connect_with_signer` and
  `DefaultSigner::from_master`. Or the computer gets another account number,
  with a balance of its own.
- secp256k1 can't live in the Secure Enclave, which supports P-256 only. The
  pattern stays: random entropy in a this-device-only Keychain item, handed
  to the SDK at open.

## Fees

The SDK is free for developers. Users pay Spark transfer fees and Lightning
routing fees, which `prepareSendPayment` quotes. On-chain claims and exits
pay chain fees. We charge nothing on top.

## x402

- **Paying works.** The rules for an x402 payer:
  - refuse non-mainnet invoices;
  - turn `preferSpark` off;
  - cap the fee at the quote;
  - derive the idempotency key from the invoice;
  - accept proof only as a preimage that hashes to the invoice's payment
    hash.
- **Receiving doesn't.** The SSP's shared node signs every Spark BOLT11
  invoice, for every Spark user. NIP-X402 requires a payee that only the
  receiver can issue invoices for. The public `receivePayment` also takes
  only a memo, not a description hash.
- An x402 receiver needs its own node (`ldk-node`), or a new NIP-X402
  profile that proves settlement by the receiver's own record of the
  payment. That profile needs an owner decision.

## Known gaps in the SDK

- No BOLT12 send or receive; offers are parsed only.
- No spending limit for bitcoin below our code. There are only the
  `PaymentObserver` hook and token allowances, which cover tokens only.
- No lookup by payment hash; scan `listPayments`.
- No public description-hash invoices.
