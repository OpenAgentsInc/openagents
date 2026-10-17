# Spark wallet: what the SDK offers, what we use, and what to add

Date: 2026-10-17. Scope: the Breez SDK (Spark implementation) as the base of
a full-featured Bitcoin and Lightning wallet on the phone, the desktop, the
`openagents` command, and agents, including x402. Owner request: "look
through https://github.com/breez/spark-sdk, see how we're using this now
and what we may want to add, for a full-featured bitcoin/lightning wallet
that will work also with x402 etc."

Reference read: `breez/spark-sdk` `main` at
`fc7ae22b90bd4a935990c227e5941774747e6b5d` (2026-10-16), in a read-only
clone at `~/work/ref/spark-sdk`. Tags compared: `0.26.0` (`2c140a5`,
2026-09-23, the tag we pin), `0.26.1` (`eab30ba`, 2026-10-02), and
`0.27.0-dev2` (`54ab3bb`, 2026-10-01). SDK paths below are relative to that
clone at that commit. Nothing in the SDK was built or run, and nothing here
was run against regtest or mainnet. Claims marked **(inferred)** are
readings of the code that were not confirmed by running it.

This audit builds on the earlier studies in [`docs/breez/`](../breez/README.md),
especially the [SDK review](../breez/sdk-review.md) (at `754ae7959`,
2026-09-28) and the [wallet design](../breez/wallet-design.md). It does not
repeat their trust-model and fee tables; it records what changed since,
what the current code uses, and what is missing.

## Verdict

- **The SDK already covers almost everything a full wallet needs.** Spark
  transfers, BOLT11 send and receive, HODL invoices, Spark HTLCs, LNURL-pay,
  -withdraw, and -auth, Lightning addresses, on-chain deposits with claims
  and refunds, cooperative withdrawals, unilateral exit, tokens and a USD
  stable balance, contacts, fiat rates, message signing, webhooks, batch
  token sends, client signing, and a pre-send policy hook. BOLT12 is parsed
  but can't be paid or received.
- **We use about a third of it.** `crates/spark-wallet` wraps balance,
  receive (Spark address, BOLT11, Bitcoin address), quoted send to any of
  four destinations, history, deposits, claims, refunds, contacts, buying,
  exit-state export, and an agent pay path that returns the preimage. The
  phone app that landed today (#11999) shows only the main surface; the
  computer CLI exposes a little more.
- **x402 paying works today; x402 receiving into Spark doesn't, and the
  blocker isn't code.** `openagents x402 fetch` already pays from the
  person's Spark wallet with a fee cap, an invoice-derived idempotency key,
  and a checked preimage (`crates/openagents-cli/src/x402_spark.rs`). A
  Spark BOLT11 invoice's payee is the service provider's shared node, which
  NIP-X402's exclusive-receiver rule forbids. Receiving stays on
  `crates/wallet` (`ldk-node`).
- **The biggest risk is custody, not features.** After the Nostr identity
  work (#11995), the same 12 words derive the person's Nostr key and open
  the wallet. `openagents wallet link` hands those words' entropy to each
  linked computer, so every linked computer would also hold the person's
  Nostr identity. The SDK makes a narrower hand-off possible (an account
  key instead of the seed); see [Custody](#security-and-custody).
- **Upgrade soon, but to a release.** Recover funds (the replacement for
  the unilateral-exit calls), token allowances, the watchtower fee ladder,
  and recoverable-balance reporting exist only on `main`. No tag has them
  yet. Upgrading needs two new storage methods in our JSON store and one
  changed config type.

## What the SDK offers

The public surface is `BreezSdk` in `crates/breez-sdk/core/src/sdk/` and
`SdkBuilder` in `crates/breez-sdk/core/src/sdk_builder.rs`; models are in
`crates/breez-sdk/core/src/models/mod.rs`. "New" means not in `0.26.0`.

| Area | What it does | SDK path |
| --- | --- | --- |
| Connect and build | `connect`, `connect_with_signer`, `connect_with_signing_only_signer`, `default_config`, `default_server_config`; builder options for account number, storage backend (SQLite, Postgres, MySQL, custom `Storage`), chain service, fiat service, LNURL client, payment observer, shared context | `sdk/mod.rs`, `sdk_builder.rs` |
| Network and key | Mainnet, Regtest, Signet. Mainnet needs the Breez API key (a certificate checked offline). Optional SOCKS5 proxy, custom Spark operator set (`spark_config`) | `models/mod.rs` (`Config`), `sdk/init.rs` |
| Receive | `ReceivePaymentMethod::SparkAddress`, `SparkInvoice` (amount, token, expiry, payer key), `BitcoinAddress` (with rotation), `Bolt11Invoice` (description, amount, expiry, optional HODL `payment_hash`, optional receiver key), `CrossChain` (USDC and USDT) | `models/mod.rs:1929`, `sdk/payments/receive.rs` |
| Send | `prepare_send_payment` and `send_payment` for BOLT11, Spark address, Spark invoice, Bitcoin address, and cross-chain address; `FeePolicy`; UUID `idempotency_key` for Spark-based sends; `completion_timeout_secs`; Spark HTLC send (`SparkHtlcOptions`) | `models/mod.rs:1999`, `:2488`, `:2517`, `sdk/payments/` |
| Conditional payments | HODL invoices, `claim_htlc_payment`, Spark HTLCs with a caller-held preimage | `docs/breez-sdk/src/guide/htlcs.md` |
| Lightning fee cap | The quote's fee is passed as the maximum; the send re-asks the provider and refuses a higher fee before any leaves move | `crates/spark/src/services/lightning.rs:825-860`, `sdk/lightning_sender.rs:131` |
| Preimage on send | `PaymentDetails::Lightning.htlc_details.preimage`; a payment with a preimage is marked completed | `models/mod.rs:491`, `models/adaptors.rs:407` |
| Interrupted sends | A Lightning send is recorded before leaves move and resumed after a restart under the same transfer ID | `sdk/lightning_sender.rs` (`resume_pending_sends`) |
| LNURL | `prepare_lnurl_pay`, `lnurl_pay`, `lnurl_withdraw`, `lnurl_auth`, signed-package LNURL-pay | `sdk/lnurl/` |
| Lightning address | Register, check, get, delete, and transfer an address on Breez's LNURL server or a custom domain; an open-source LNURL server with NIP-57 zaps | `sdk/lightning_address.rs`, `crates/breez-sdk/lnurl/`, `docs/breez-sdk/src/guide/custom_domain.md` |
| On-chain | `list_unclaimed_deposits`, `fetch_claim_deposit_quote`, `claim_deposit`, `refund_deposit`, `recommended_fees`; automatic claims under `max_deposit_claim_fee` | `sdk/deposits.rs` |
| Recovery (new) | `prepare_recover_funds`, `recover_funds`, `check_recover_funds` replace `prepare_unilateral_exit`, `unilateral_exit`, and `check_unilateral_exit`; cooperative recovery of leaves the operators moved on-chain; `GetInfoResponse.recoverable_funds_sats`; `SdkEvent::RecoverableFunds` | `sdk/recover_funds.rs`, `docs/breez-sdk/src/guide/recover_funds.md` |
| Exit data | `export_unilateral_exit_state`, `import_unilateral_exit_state`; `exit_chain_auto_fetch_enabled`; watchtower fee ladder (new, on by default) | `sdk/unilateral_exit_backup.rs`, `sdk/watchtower_exit.rs` |
| Tokens and dollars | Token balances, `get_tokens_metadata`, stable balance (USDB), conversions and refunds, token issuer API, batch token send | `stable_balance/`, `token_conversion/`, `issuer/sdk.rs` |
| Token allowances (new) | The owner grants a spender a per-payment limit, a total limit, an expiry, and an optional recipient list; the operators enforce it; the spender pulls with `prepare_pull_payment` and `pull_payment`. Tokens only, not bitcoin | `sdk/token_allowances.rs`, `docs/breez-sdk/src/guide/token_allowances.md` |
| Client signing | Build an unsigned transfer, batch, pull, or LNURL-pay package; sign it elsewhere; publish it. Stateless and replay-safe | `sdk/payments/mod.rs:139-200`, `docs/breez-sdk/src/guide/client_signing.md` |
| Policy hook | `PaymentObserver::before_send` sees every provisional payment and can cancel it | `models/payment_observer.rs:76` |
| History | `list_payments` with type, status, asset, details, and time filters; `get_payment`; metadata for LNURL and conversions | `sdk/payments/mod.rs:248`, `models/mod.rs:2670` |
| Events | `Synced`, `PaymentSucceeded`, `PaymentPending`, `PaymentFailed`, `PaymentMetadataUpdated`, deposit events, `LightningAddressChanged`, `UnilateralExitStateChanged`, `RecoverableFunds`, optimization events | `events.rs:19` |
| Multi-device | Encrypted real-time sync to Breez's server; private mode setting | `realtime_sync/`, `sdk/api.rs:267-320` |
| Other | `sign_message`, `check_message`, contacts, fiat currencies and rates, webhooks from the service provider (HMAC-signed), `optimize_leaves`, `buy_bitcoin` (MoonPay, Cash App), Cash App bridges (new), passkey-derived seeds, Turnkey signers | `sdk/api.rs`, `sdk/contacts.rs`, `sdk/buy.rs`, `passkey/`, `turnkey/` |
| Bindings | UniFFI (Swift, Kotlin, Python, C#, Go), Flutter, React Native, WASM | `crates/breez-sdk/bindings`, `crates/breez-sdk/wasm` |

Not present: BOLT12 send or receive (offers parse only,
`crates/breez-sdk/common/src/input/models.rs:81-133`), a spending limit for
bitcoin (only the observer hook and token allowances), lookup of a payment
by payment hash (scan `list_payments`), and a public way to make an invoice
with a description hash (next section).

### Description hash

The protocol layer supports it: `InvoiceDescription::DescriptionHash`
(`crates/spark/src/services/lightning.rs:102`), checked against the
provider's returned invoice. The public `receive_payment` passes only
`InvoiceDescription::Memo` (`crates/breez-sdk/core/src/sdk/payments/receive.rs:402`
and `:412`). This is unchanged since issue #9777 and the 2026-09-28 review.
An upstream change that adds `description_hash: Option<String>` to
`ReceivePaymentMethod::Bolt11Invoice` is small, because the lower layer
already validates it.

## What we use today

### Shared wallet crate

`crates/spark-wallet` (package `openagents-spark`) pins
`breez-sdk-spark` at tag `0.26.0` with default features off. It's the only
place the SDK is called.

- `src/spark.rs`: `SparkNode` over any Breez storage. Config
  (`sdk_config`): committed Breez API key (the exception recorded in
  `AGENTS.md` and `INVARIANTS.md`), real-time sync off, deposit claims up to
  the network's recommended rate plus 1 sat/vB. Calls used: `get_info`,
  `sync_wallet`, `receive_payment` (Spark address, BOLT11, Bitcoin
  address), `parse`, `prepare_send_payment`, `send_payment` (with an
  idempotency key and a completion wait), `prepare_lnurl_pay`, `lnurl_pay`,
  `list_payments`, `get_payment`, `list_unclaimed_deposits`,
  `claim_deposit`, `refund_deposit`, `recommended_fees`, `add_contact`,
  `list_contacts`, `buy_bitcoin`, `export_unilateral_exit_state`,
  `add_event_listener`, `disconnect`.
- `src/model.rs`: the `Node` trait the phone and CLI share, including
  `pay_invoice`, which returns the preimage and separates "not sent" from
  "outcome unknown" (`AgentPayFailure`).
- `src/seed.rs`: BIP-39 entropy, 12 or 24 words, no passphrase.
- `src/store.rs`: Breez's `Storage` trait as one JSON file, because the
  root workspace can't link Breez's SQLite beside `ldk-node`'s.
- `src/computer.rs`: `~/.openagents/spark`, the seed as a 0600 file.
- `src/link.rs`: the NIP-44 envelope that carries the seed from the phone to
  a computer.

Never called: `claim_htlc_payment`, Spark HTLC send, `lnurl_withdraw`,
`lnurl_auth`, any Lightning address registration, `sign_message`, fiat
rates, webhooks, tokens, stable balance, batch send, client signing,
`PaymentObserver`, account numbers other than the default, and
`import_unilateral_exit_state`.

### Phone (current app)

`apps/openagents/crates/mobile/src/wallet/` (#11999, `67c75999ee`) links
`openagents-spark` with Breez's `sqlite` feature. It shows the balance,
Receive (Spark address, Lightning invoice, Bitcoin address, with QR), Send
with a confirmed quote, recent activity, the trust note, and Back up.
`seed.rs` reads the earlier app's Keychain item (`com.openagents.app.spark`
/ `seed-v1`) first and never deletes, overwrites, or regenerates it. iOS
glue is in `apps/openagents/apps/ios/OpenAgents/Wallet/`. Android has no
wallet yet; [the Android audit](2026-10-17-android-app.md) lists the
Keystore work.

### Computers and the `openagents` command

`crates/openagents-cli/src/wallet.rs`: `create`, `balance`, `address`,
`receive`, `send`, `history`, `link`, `restore`, all on mainnet in plain
words. The host half of `link` is `crates/coder-host/src/wallet_link.rs` and
`crates/coder-access/src/wallet_link.rs`.

### x402 and agent payments

- **Paying:** `crates/openagents-cli/src/x402_spark.rs` is the default payer
  for `x402 fetch`, `call`, and `buy` (#10324). It refuses non-mainnet
  invoices, syncs, derives a UUID idempotency key from the invoice, pays
  under the fee cap, and builds a proof only from a preimage. `--pay-with
  node`, `lnd`, and `phone` choose other payers
  (`crates/openagents-cli/src/x402.rs:693`).
- **Receiving:** `crates/wallet` (`LightningWallet`, `ldk-node` with
  MoneyDevKit's LSPS4) behind `openagents x402 node` and the pay host
  (`docs/deployment/pay-host.md`). `receive_exact` issues an invoice whose
  `h` field is the request hash.
- **Credit top-ups:** `crates/gateway/src/funding.rs` and
  `crates/tenancy/src/money/funding.rs` issue exact invoices through
  `LightningWallet` and credit only on the wallet's own lookup. A person
  pays these from Spark like any invoice.
- **Payouts:** `crates/openagents-cli/src/pay_payout.rs` runs a separate
  payout Spark wallet with its own seed, paying Spark addresses and
  Lightning addresses resolved from NIP-A3 and kind-0
  (`crates/nostr/src/payto.rs`).
- **Agent spend requests:** the formats in
  [the spend protocol](../breez/spend-protocol.md), the host book in
  `crates/coder-host/src/spend.rs`, and the checks in
  `crates/coder-access/src/spend.rs`.
- **NIP-47:** `crates/nostr/src/domain/wallet_connect.rs` has the message
  types. Nothing serves them over the Spark wallet.

### Retired with the earlier app

The earlier app (`c63b8bbe5c^:crates/openagents-mobile/src/wallet.rs`,
`spend.rs`, `wallet_link.rs`, and the iOS and Android wallet screens) had
features #11999 left out:

- deposits, claims with a quote, and refunds to a reviewed address
- on-chain withdrawal speed choice
- buying through MoonPay and Cash App
- contacts, paying an npub (NIP-A3 Spark address, then `lud16`), and
  publishing the phone's Spark address (kind 10133, off by default)
- the encrypted exit-state backup and its export to Files
- **the phone's answers to `openagents wallet link`** and **to agent spend
  requests**, including standing grants

The shared crate still has the methods for the first five. The last one
matters most: the host side of both flows still exists, but no current app
answers them, so `openagents wallet link` and `x402 --pay-with phone` wait
until they time out.

## Findings

1. **Two backups instead of one (blocks #11995's promise).**
   `apps/openagents/crates/mobile/src/wallet/seed.rs:130` makes a new
   wallet with `Seed::generate()`. The #11995 branch makes its own 12 words
   in a separate Keychain item when no wallet seed exists. A person who
   makes the identity first and the wallet later gets two sets of words.
   Fix: when #11995 lands, `seed::create` takes the identity's entropy.
2. **Stale invariants.** The [Phone wallet](../../INVARIANTS.md#phone-wallet)
   rows still name `crates/openagents-mobile/src/wallet.rs`, which #11901
   removed, and cite tests that no longer exist (spend approval, standing
   grants, exit-state backup, deposit refunds). They describe the earlier
   app, not the current one.
3. **Link and phone payments have no answerer.** See
   [Retired with the earlier app](#retired-with-the-earlier-app).
4. **A pending agent payment has no follow-up.** `x402_spark::pay` returns
   "its proof hasn't arrived yet" when `send_payment` returns before the
   preimage. The SDK keeps polling and emits `PaymentSucceeded`, and the
   idempotency key returns the same payment on a repeat, so the payer could
   wait on that event or poll `get_payment` until the invoice's paid-retry
   grace ends instead of failing the call.
5. **The pin is behind on recovery.** `0.26.0` has the old unilateral-exit
   calls and no recoverable-balance report. A person whose leaves the
   operators moved on-chain can't see or recover them today.
6. **No hard spending limit below the wallet.** Agent limits live in our
   ledgers (`coder-access`, `coder-host`), and computers run the full seed.
   The SDK's `PaymentObserver` is unused, so a bug in a caller of
   `SparkNode` isn't caught by a second check.

## Gap list, ranked

Ranked for a full-featured wallet across the phone, desktop, CLI, and
agents. Size: S is a day or less, M is two to four days, L is a week or more.

| Rank | Gap | Surfaces | Size |
| --- | --- | --- | --- |
| 1 | One set of words: a new wallet uses the identity's entropy (finding 1) | Phone | S |
| 2 | Phone answers `wallet link` again, ported from `c63b8bbe5c^:crates/openagents-mobile/src/wallet_link.rs` | Phone, CLI | M |
| 3 | Narrow what a computer holds: an account key instead of the seed ([Custody](#security-and-custody)) | CLI, desktop, agents | M |
| 4 | Upgrade to the first tag with recover funds, show recoverable funds, and keep exit data current | All | M |
| 5 | Phone answers agent spend requests and standing grants again, from `c63b8bbe5c^:crates/openagents-mobile/src/spend.rs` | Phone, agents | L |
| 6 | Restore the retired phone controls under Advanced: deposits, claims, refunds, withdrawal speed, buying, exit backup | Phone | M |
| 7 | Pay an npub, contacts, and the Spark address switch on the phone | Phone | M |
| 8 | Android wallet: Keystore `KeyStore`, the retired app's seed file read first | Android | M |
| 9 | Desktop wallet in the GPUI app, as Rust Native wallet cards through the GPUI adapter | Desktop | L |
| 10 | Lightning address `name@openagents.com`, hosted (CNAME to Breez) or self-hosted LNURL server with zaps, published as `lud16` | Phone, desktop | M |
| 11 | `PaymentObserver` that re-checks each send against the caller's grant | CLI, agents | M |
| 12 | Agent payer waits for the preimage (finding 4) | CLI, agents | S |
| 13 | Separate agent wallets by account number (`with_account_number`), funded by the person | Agents | M |
| 14 | NIP-47 wallet service over Spark for outside agents, with fee caps and idempotency enforced by us | Agents | L |
| 15 | Dollar balance (USDB) and token allowances for dollar-denominated agent grants | Phone, agents | L |
| 16 | Upstream change: description hash on `Bolt11Invoice` | x402 | S upstream, then wait for a tag |
| 17 | LNURL-withdraw and LNURL-auth on the phone scanner | Phone | S |
| 18 | Multi-device sync (Breez real-time sync) so history and contacts match across devices | All | M, plus a decision |

## x402 fit

[NIP-X402](../../nips/openagents/NIP-X402.md) section "Standard network and
shared payment rules" sets the requirements. Each row says whether a Spark
wallet meets it.

### As payer

| Requirement | Spark | Evidence |
| --- | --- | --- |
| Exact mainnet BOLT11 (`bc`) | Yes. The SDK checks the network; we refuse other networks before paying | `crates/spark/src/services/lightning.rs:833`, `x402_spark.rs` |
| Hard fee cap | Yes **(inferred)**. The quoted fee is passed as the maximum and re-checked before leaves move; the amount committed to the provider is amount plus that fee | `crates/spark/src/services/lightning.rs:853`, `crates/breez-sdk/core/src/sdk/lightning_sender.rs:158` |
| Preimage for the exact invoice and hash | Yes, once settled. We check it hashes to the invoice's payment hash | `models/mod.rs:491`, `x402_spark.rs` |
| Pay at most once | Yes. UUID idempotency key derived from the invoice; interrupted sends resume under one transfer ID | `models/mod.rs:2517`, `lightning_sender.rs` |
| Testable off mainnet | No. Spark's hosted regtest has no Lightning, and x402 admits only `bc` and `tb`. Only mainnet with small amounts, or Breez's local `spark-itest` cluster | `docs/breez-sdk/src/guide/testing.md` |

One caveat: when an invoice embeds a Spark address and `prefer_spark` is
on, the SDK pays by Spark transfer instead of Lightning. x402 invoices come
from `ldk-node` receivers and carry no Spark address, so this doesn't arise
today. A payer should still turn `prefer_spark` off for x402.

### As receiver

Not compatible, for one blocker and one gap:

1. **Blocker: the payee isn't ours.** The service provider's node creates
   and signs every Spark BOLT11 invoice, for every Spark user. NIP-X402
   requires `payTo` to have exclusive invoice-issuance authority and names
   a shared custodial key as incompatible; `crates/nostr/src/x402.rs`
   checks the payee. Anyone with a Spark wallet can get an invoice from that
   same payee with any description hash they like (once the hash is
   exposed), pay themselves, and present a valid proof.
2. **Gap: no description hash.** See [Description hash](#description-hash).

The earlier review gave a third reason, that the receiver doesn't hold the
preimage. That's weaker than it read: the local signer creates the preimage
for a normal invoice, and a HODL invoice lets the receiver choose the hash
and release the preimage itself (`claim_htlc_payment`). It doesn't change
the verdict, because the blocker is the payee.

What could make Spark a receiver is a separate profile, not a wallet fix:
for example `payTo` as a Spark identity key, with settlement proven by the
receiver's own record of a completed receive for that payment hash (which
`crates/gateway/src/funding.rs` already does for top-ups through
`LightningWallet::lookup`), not by the preimage alone. That needs a
NIP-X402 change and an owner decision. Until then, receiving stays on
`crates/wallet`.

### A Spark `LightningWallet`

`crates/wallet/src/lib.rs:33` documents the trait as "a Lightning wallet on
an exclusively held node key". A Spark adapter can implement `pay`,
`pay_from_node`, `balance`, and `lookup` (scanning `list_payments` for the
hash). It can't honestly implement `node_id` or `receive_exact`, so it
should be a payer-only type, as `x402_phone.rs` already does for the phone.
Gateway top-ups could take a Spark receiver, because they credit on lookup
rather than on a preimage, but that moves the pay host's money into Spark's
trust model, which the 2026-09-28 decision ruled out for a treasury.

### Agents paying on their own

The pieces exist: the spend grant and receipt formats, the host book, the
payer's fee cap and idempotency, and standing grants (retired from the
phone, still in `coder-access`). What's missing is enforcement below the
caller and a smaller key on the computer:

- A `PaymentObserver` in `SparkNode` that checks each provisional payment
  against the grant and reservation before any send.
- An agent wallet per computer or per agent (`with_account_number`),
  funded by the person, so a mistake costs at most its balance.
- Later, USDB with token allowances: the operators enforce per-payment
  and total limits, expiry, and recipients. It's the only limit in Spark
  that doesn't depend on our code, but it covers tokens only.

## Security and custody

- **One seed for identity and money.** The words derive the NIP-06 key
  (`m/44'/1237'/…`) and the Spark keys (`m/8797555'/<account>'`, mainnet
  account 1, `crates/spark/src/signer/default_signer.rs:61`). The keys
  differ, but whoever holds the words holds both. The phone keeps them in a
  this-device-only Keychain item; the backup screen must say once that the
  words are the account and the money.
- **Computers hold the whole seed today.** `crates/spark-wallet/src/computer.rs`
  writes the entropy to `~/.openagents/spark/seed`. After #11995, a linked
  computer can derive the person's Nostr key from that file. The SDK offers
  a narrower hand-off: the Spark signer runs from the account master key
  alone (`DefaultSigner::from_master`,
  `crates/spark/src/signer/default_signer.rs:145`), and `connect_with_signer`
  takes an external signer. A hardened account key can't be turned back
  into the seed or the Nostr key. Sending a computer only
  `m/8797555'/1'` keeps "one balance everywhere" and stops a stolen laptop
  from becoming the person's identity **(inferred; check that every
  feature we use works with an external signer, since a signing-only signer
  disables LNURL-auth, real-time sync, and cross-chain receive)**. Sending
  another account number gives the computer its own, separate balance.
- **Exit.** The 2026-09-28 trust note stands. A unilateral exit needs exit
  data collected while the operators were reachable, a confirmed native
  SegWit UTXO for fees, and days of timelocks. Recover funds on `main` also
  recovers leaves the operators moved on-chain, at no extra funding. The
  watchtower fee ladder pre-signs refunds at a range of fee rates, so an exit
  holds when fees spike.
- **Backup.** Words alone restore the balance while Spark works. They don't
  restore exit data during an outage; the earlier app saved
  `export_unilateral_exit_state` encrypted after every sync and exported it
  to Files, and the current app doesn't yet. Restoring the backup is gap 6.
- **API key.** The committed Breez key stays as recorded in `INVARIANTS.md`;
  this audit doesn't copy it.
- **Tests.** Tests never move the owner's funds: spend tests use fakes, and
  live checks use regtest or a throwaway mainnet seed with a few sats, never
  the owner's wallet.

## Phased plan

Each step is sized as one issue. Estimates are working days for one agent.

### Phase 0: correctness (2 days)

1. One set of words: `seed::create` takes the identity's entropy when
   #11995 lands; test that identity and wallet words match. (0.5)
2. Rewrite the Phone wallet rows in `INVARIANTS.md` for the current app, and
   mark the retired flows as not shipped. (0.5)
3. Agent payer waits for the preimage until the invoice's paid-retry grace,
   on `PaymentSucceeded` or `get_payment`, and turns `prefer_spark` off for
   x402. (1)

### Phase 1: computers and links (1 to 1.5 weeks)

4. Phone answers `wallet.link.list` and `wallet.link.answer` with the
   six-digit code and Face ID, ported to a Rust Native sheet. (3)
5. Account-key hand-off: the link carries `m/8797555'/1'` instead of the
   entropy; computers run an external signer; existing seed files keep
   working and can be replaced with `--replace`. Needs decision 1. (3)
6. `PaymentObserver` in `SparkNode` that re-checks a caller-supplied
   limit; the CLI and x402 payer pass theirs. (2)

### Phase 2: SDK upgrade (3 to 4 days, after Breez tags it)

7. Bump the pin in `crates/spark-wallet/Cargo.toml` and
   `apps/openagents/crates/mobile/Cargo.toml`; implement
   `list_leaf_recoveries` and `update_leaf_recovery` in `store.rs`; add
   `proportional_ppm` to the `MaxFee::NetworkRecommended` config; run
   Breez's storage conformance suite (`crates/spark-wallet/tests/store.rs`). (2)
8. Show recoverable funds on the phone and in `openagents wallet balance`;
   `openagents wallet recover` prepares a cooperative recovery to an
   address the person reviews. (2)

### Phase 3: phone parity (2 weeks)

9. Advanced: deposits, quoted claims, refunds, withdrawal speed. (3)
10. Exit-data backup, encrypted, with export to Files. (2)
11. Pay an npub, contacts, and the Spark address switch. (3)
12. Android wallet over the Keystore. (3)
13. Phone answers spend requests and standing grants. (5)

### Phase 4: receive and desktop (2 to 3 weeks)

14. Lightning address on our domain, published as `lud16`. Needs
    decision 3. (3 hosted, 6 self-hosted)
15. Desktop wallet: the GPUI adapter for the Rust Native wallet cards,
    seed or account key from the phone link. (8)
16. LNURL-withdraw and LNURL-auth from the scanner. (1)

### Phase 5: agents (3 weeks or more)

17. Agent wallets by account number, funded from the main balance, with a
    per-agent ceiling. (4)
18. NIP-47 service over Spark for outside agents, with our fee cap and
    idempotency. (6)
19. USDB balance and token allowances for dollar grants. Needs decision 4. (6)
20. Upstream: propose `description_hash` on `Bolt11Invoice` to Breez. (1,
    then wait)
21. If decision 2 says yes: a draft Spark receiver profile for NIP-X402. (3)

## Decisions for the owner

1. **What a linked computer holds.** Today: the whole seed, which after
   #11995 includes the person's Nostr identity. Recommended: the Spark
   account key only, keeping one balance. Alternative: a separate agent
   account per computer with its own balance.
2. **x402 receive on Spark.** Keep receiving on `ldk-node` (recommended),
   or write a Spark receiver profile that proves payment by the receiver's
   own records instead of an exclusive payee key.
3. **Lightning address.** Breez-hosted `name@openagents.com` through a CNAME
   or path forward (fast; Breez's server issues our users' invoices), or a
   self-hosted LNURL server on our infrastructure (more work; we hold that
   trust instead).
4. **Dollars.** Turn on USDB and stable balance for people and agent
   grants, or keep the wallet bitcoin-only. The 2026-09-28 position kept
   USDC and USDT receive off.
5. **Multi-device sync.** Keep Breez's real-time sync off (each device has
   its own history; the balance still matches) or turn it on so history,
   contacts, and settings match across the phone and computers.
6. **When to upgrade.** Wait for the first tag with recover funds
   (recommended), or pin a `main` revision now to show recoverable funds
   sooner.
