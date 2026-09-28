# Breez SDK (Spark) review

Date: 2026-09-28

Scope: a source and documentation review of the Breez SDK's Spark
implementation for use as the OpenAgents app's wallet. Reviewed revision:
`breez/spark-sdk` `main` at `754ae7959` (2026-09-28), latest tag `0.26.0`,
in the read-only reference clone at
`/Users/christopherdavid/work/projects/repos/spark-sdk` (listed in the
workspace `projects/manifest.txt`). Documentation: the SDK guide at
<https://sdk-doc-spark.breez.technology/> and its source under
`docs/breez-sdk/src/guide/`, plus the Spark protocol docs at
<https://docs.spark.money/>.

Claims marked **(inferred)** are readings of the code or docs that this review
did not confirm by running anything. Nothing here was run against regtest or
mainnet.

## Verdict

The Breez SDK is a good technical fit for a self-directed phone wallet in a
Rust-core app. It is a Rust crate first, with UniFFI bindings as a byproduct,
so `crates/openagents-mobile` can link it directly with no Swift or Kotlin SDK
layer. It covers Lightning, Spark, on-chain, LNURL, Lightning addresses,
tokens, a USD stable balance, and the new USDC and USDT receive. It supports
several independent wallets from one seed, a pre-send hook for policy, and a
client-signing flow in which a keyless process prepares a payment and a key
holder signs it. Those last three are the primitives agent spending needs.

Its costs are trust and operations, not code:

- **Trust:** a Spark balance is protected by a 2-of-3 operator set
  (Lightspark, Breez, Flashnet) that must delete old key shares, which no one
  can verify. Lightning goes through Lightspark's service provider. Unilateral
  exit is possible but slow and needs a separate on-chain UTXO.
- **Operations:** our own history shows slow sync, stale history, and
  unspendable leaves on a busy server wallet. See [history](history.md).
- **x402:** a Spark wallet can pay an x402 seller but cannot be an x402
  receiver. See [the wallet design](wallet-design.md#x402).

## Repository and packaging

- **Core crate:** `crates/breez-sdk/core`, package `breez-sdk-spark`, library
  `breez_sdk_spark`, edition 2024, crate types `staticlib`, `cdylib`, and `lib`.
- **Protocol crates:** `crates/spark` (operator gRPC, service-provider
  GraphQL, FROST signing, transfer, Lightning, cooperative exit, deposit, and
  unilateral-exit services) and `crates/spark-wallet` (the wallet layer and
  `SparkWalletConfig`). These are Breez's own Rust client for the Spark
  protocol. Lightspark's official SDK is TypeScript; see
  [Breez and Spark](breez-vs-spark.md).
- **Other crates:** `crates/flashnet` (the Flashnet AMM and Orchestra clients),
  `crates/breez-sdk/lnurl` (Breez's open-source LNURL and Lightning-address
  server), `crates/spark-service-provider/sspd` (Breez's own Spark service
  provider, first landed 2026-09-14, used only in integration tests so far),
  and storage backends for SQLite, Postgres, and MySQL.
- **Bindings:** UniFFI for Swift, Kotlin, Kotlin Multiplatform, Python, C#,
  and Go; flutter_rust_bridge for Flutter; ubrn for React Native; and a WASM
  package. The bindings makefile builds `aarch64-apple-ios`, the iOS
  simulators, and four Android ABIs.
- **Rust dependency:** the documented form is a Git tag,
  `breez-sdk-spark = { git = "https://github.com/breez/spark-sdk", tag = "0.26.0" }`
  (`docs/breez-sdk/src/guide/install_rust.md`). **(inferred)** It is not on
  crates.io and cannot easily be, because the workspace pulls Git forks of
  `frost-core`/`frost-secp256k1-tr` (Lightspark), `boltz-client` (Breez), and
  `uniffi`.
- **Features:** none are default. `connect()` requires `sqlite`. Others are
  `postgres`, `mysql`, `passkey` (pulls `nostr-sdk`), `turnkey`,
  `turnkey-p256`, and `uniffi`.
- **Toolchain:** MSRV 1.88; the repository pins 1.95.0.
- **Heavy dependencies:** multi-threaded Tokio, tonic 0.12 gRPC, rustls 0.23
  on ring, reqwest, bundled rusqlite 0.32, `bitcoin` 0.32, `secp256k1` 0.29,
  `lightning` 0.1.3, and `lightning-invoice`. The bundled SQLite and ring
  need a C toolchain for each target; an earlier OpenAgents build stubbed the
  crate out for that reason (`7cf7da4c64`).

**Fit with our build (inferred):** `crates/openagents-mobile` already builds a
`staticlib` for `aarch64-apple-ios` and uses `cargo ndk` for Android, and
`crates/wallet` already brings Tokio, rustls, SQLite, and `ldk-node`. Adding
`breez-sdk-spark` adds `tonic`, a second `lightning` version, and the Spark
crates. Expect duplicate-version churn with `ldk-node`'s `lightning` and
`bitcoin` versions, and a larger binary. Check `cargo tree -d` and the iOS
archive size before committing to a pin.

## API surface

Verified in `crates/breez-sdk/core/src/sdk/` and `models/`.

**Connect.** `connect(ConnectRequest { config, seed, storage_dir })`,
`connect_with_signer`, `connect_with_signing_only_signer`,
`default_config(Network)`, and `SdkBuilder` (`with_account_number`,
`with_default_storage`, `with_storage`, `with_chain_service`,
`with_rest_chain_service`, `with_lnurl_client`, `with_payment_observer`,
`with_shared_context`, `build`).

- `Network` is `Mainnet`, `Regtest`, or `Signet`. There is no testnet. Signet
  has no default operators.
- `Config` includes `api_key: Option<String>`, `sync_interval_secs` (60),
  `max_deposit_claim_fee` (1 sat/vB), `lnurl_domain` (`breez.tips` on
  mainnet), `prefer_spark_over_lightning`, `real_time_sync_server_url`
  (`https://datasync.breez.technology`), `stable_balance_config`,
  `background_tasks_enabled`, `proxy` (SOCKS5), and `cross_chain_config`
  (off unless set).
- **API key:** required on mainnet only (`sdk/init.rs`). It is a
  Base64-encoded X.509 certificate whose issuer common name starts with
  "Breez", checked offline and sent to Breez services. Keys are free on
  request by email. No written terms, quota, or service level were found.
- `Seed` is a BIP39 mnemonic with optional passphrase, or raw entropy. A
  passkey-derived seed is also supported.

**Payments.** `parse`, `prepare_send_payment`/`send_payment` (with an
`idempotency_key`), `prepare_send_batch`/`send_batch`, `receive_payment`,
`list_payments`, `get_payment`, and `claim_htlc_payment`.

**Client signing.** `build_unsigned_transfer_package`,
`publish_signed_transfer_package`, `build_unsigned_lnurl_pay_package`, and
`publish_signed_lnurl_pay_package`. See [client signing](#client-signing).

**LNURL and Lightning address.** `prepare_lnurl_pay`/`lnurl_pay`,
`lnurl_withdraw`, `lnurl_auth`, `register_lightning_address`,
`check_lightning_address_available`, `get_lightning_address`,
`delete_lightning_address`, and address-transfer methods.

**On-chain.** `list_unclaimed_deposits`, `claim_deposit`,
`fetch_claim_deposit_quote`, `refund_deposit`, and the unilateral-exit set
(`prepare_unilateral_exit`, `unilateral_exit`, `check_unilateral_exit`,
`export_unilateral_exit_state`, `import_unilateral_exit_state`).

**Tokens and dollars.** `get_tokens_metadata`, `fetch_conversion_limits`,
`refund_pending_conversions`, `get_token_issuer()` (create, mint, burn,
freeze), `get_cross_chain_routes`, `prepare_payment_link` (Cash App), and
`buy_bitcoin`.

**Other.** `get_info` (identity key, sats balance, token balances),
`sync_wallet`, `sign_message`/`check_message`, `optimize_leaves`, webhooks,
contacts, user settings, fiat rates, and `add_event_listener` with events
`Synced`, `PaymentSucceeded`, `PaymentPending`, `PaymentFailed`,
`NewDeposits`, `ClaimedDeposits`, `UnclaimedDeposits`, `AutoOptimization`,
`LightningAddressChanged`, and `UnilateralExitStateChanged`.

## Payment methods

| Method | Receive | Send | Notes |
| --- | --- | --- | --- |
| Spark address | `SparkAddress` (static) | Yes | Spark-to-Spark transfer between operators' records. Free today; Spark says a small flat fee is coming. |
| Spark invoice | `SparkInvoice` (amount, token, expiry, optional sender key) | Yes | Single-use request, BTC or token. |
| BOLT11 | `Bolt11Invoice` (description, amount, expiry, optional HODL hash) | Yes, with `prefer_spark` | Receive works offline. The service provider creates and signs the invoice. |
| LNURL-pay, Lightning address | Through Breez's LNURL server | `lnurl_pay` | The server issues invoices while the phone is offline. |
| LNURL-withdraw, LNURL-auth | `lnurl_withdraw` | n/a | LNURL-auth needs a signer that can do local ECIES and HMAC. |
| On-chain | `BitcoinAddress` (static deposit address) | Cooperative exit | Deposits credit after 3 confirmations (1 on regtest) if the claim fee fits `max_deposit_claim_fee`. Send is priced by `SendOnchainFeeQuote`. |
| Tokens (BTKN) | `SparkInvoice` with a token | `prepare_send_payment` with `token_identifier` | USDB is the USD token. |
| USDC, USDT on other chains | `CrossChain` | Cross-chain send | Mainnet only. See [stablecoin receive](stablecoin-receive.md). |

**Lightning receive mechanics.** The wallet splits the payment preimage into
shares and gives them to the operators (`crates/spark/src/services/lightning.rs`,
`store_preimage_shares`). The service provider, Lightspark
(`https://api.lightspark.com`) by default, creates the invoice from its own
Lightning node and routes the payment; the operators release the preimage when
the provider transfers leaves to the user. The SDK checks the returned
invoice's hash, network, amount, and, when requested, description hash
(`validate_received_invoice`, whose errors read "SSP invoice ... does not
match"). The invoice's payee is the provider's node, shared by every Spark
user on that provider.

**Description hash.** The protocol crate supports a description hash
(`InvoiceDescription::DescriptionHash`), and the LNURL server uses it. The
public `ReceivePaymentMethod::Bolt11Invoice` exposes only a `description`
string. Issue #9777 said the same of an earlier version; it still holds in
0.26.

**Preimage on send.** A Lightning payment's `PaymentDetails::Lightning`
carries `htlc_details: SparkHtlcDetails` with `preimage: Option<String>`.
**(inferred)** That is populated after a successful send, which an x402
payer needs. Verify on mainnet before relying on it.

## Trust model

- **Operators.** The SDK's default pool (`crates/spark-wallet/src/config.rs`)
  is `0.spark.lightspark.com`, `spark-operator.breez.technology`, and
  `2.spark.flashnet.xyz`, with `split_secret_threshold: 2`. Spark signs with
  FROST: the user's key plus a threshold of operator shares. Spark's docs say
  "SOs cannot move funds without the users" and call the model "moment-in-time"
  trust: a transfer is safe if at least one operator deleted its old key share
  (<https://docs.spark.money/learn/trust-model>). Deletion cannot be proven.
- **What an operator coalition can do.** It cannot move funds alone. Two
  colluding operators that kept old shares could, together with a previous
  owner of a leaf, double-spend that leaf's history (**inferred** from the
  statechain model). Operators can also stop serving, which halts off-chain
  payments.
- **Unilateral exit.** Implemented in the SDK. It needs exit data collected
  in advance, a separate native SegWit UTXO for CPFP fees, and can take days;
  leaves too small to pay their own fee are abandoned
  (`unilateral_exit.md`). Spark notes that if a previous owner broadcasts an
  old exit, the current owner must respond within the timelock window
  (<https://docs.spark.money/learn/limitations>).
- **Service provider.** Lightspark fronts Lightning send and receive,
  cooperative exits, and instant deposit claims. The transfer is atomic
  against proof of payment (<https://docs.spark.money/learn/lightning>).
- **Breez's services.** API-key gating on mainnet, a configuration server
  (`bs1.breez.technology`, which also hands out the Flashnet Orchestra key),
  the LNURL server, the encrypted sync server, one of the three operators,
  and a 5 bps integrator fee on Flashnet conversions
  (`DEFAULT_INTEGRATOR_FEE_BPS`).
- **LNURL trust.** The LNURL server's README says users must trust that the
  LNURL server and the service provider do not collude by sharing the
  preimage, and that the LNURL server returns invoices that pay the user.
  The server is open source and can be self-hosted.
- **Criticism.** spark.exposed (September 2026, unnamed author) argues that
  Lightspark and Flashnet form the threshold without Breez, that forward
  security rests on unverifiable key deletion, that a seed alone is not an
  outage backup, and that operators see extensive metadata. bitcoinlayers.org
  rates Spark's operator and finality risk "Very High."

**Our position.** Spark is **not self-custody in the sense `ldk-node` is**.
It is a trust-minimized, federated off-chain system with an on-chain escape
hatch. That is acceptable for a spending wallet with small balances on a
phone, the use Breez markets it for. It is not acceptable as the only home of
a treasury, and the app must say so in plain words. See
[the wallet design](wallet-design.md#disclosures).

## Keys and signers

- Everything is secp256k1: ECDSA, Schnorr, and FROST. The iOS Secure Enclave
  and most Android Keystore implementations support only P-256, so **the
  Spark key cannot live inside secure hardware**. The workable pattern is the
  one the phone already uses for the `ldk-node` wallet: random entropy in a
  this-device-only Keychain item or a Keystore-wrapped file, handed to Rust at
  open, held in memory only.
- Derivation is `m/8797555'/<account>'`: identity at `0'`, leaves under `1'`,
  static deposits under `3'` (`crates/spark/src/signer/default_signer.rs`).
  Mainnet's default account is 1; the others default to 0.
- `SdkBuilder::with_account_number(n)` gives an independent wallet from the
  same seed. This is how one phone seed can hold separate agent sub-wallets.
- `ExternalBreezSigner` and `ExternalSparkSigner` let the key live elsewhere,
  including Turnkey. A signing-only signer disables LNURL-auth, real-time
  sync, and cross-chain receive.

## Client signing

`docs/breez-sdk/src/guide/client_signing.md` describes a flow in which a
process that cannot sign prepares a payment, builds a small package stating
the amount, fee, and destination, sends it to the key holder, and publishes
the signed result. The server keeps no state between steps; publishing the
same signed package twice returns the same result; any change to amount, fee,
or destination needs a new signature. A denomination swap arrives as its own
package first. Conversion payments and cross-chain sends are excluded.

This maps directly onto "an agent asks, the phone signs." See
[the wallet design](wallet-design.md#approval-flows).

## Policy hooks

There is no built-in spending limit or allowance. The hook is
`PaymentObserver` (`models/payment_observer.rs`): `before_send` receives every
provisional Lightning, Spark, on-chain, and token payment and can return an
error to cancel it; `after_send` maps provisional to final IDs for token
payments. Our grant enforcement would live in that observer, backed by our
own ledger.

## Storage and sync

- SQLite under `<storage_dir>/<network>/<first 8 hex of sha256(identity key)>`.
  Different account numbers therefore get different directories under one
  root **(inferred)**.
- Real-time sync to Breez's data-sync server, with records ECIES-encrypted by
  the signer before upload. It gives multi-device and multi-app consistency.
  Breez recommends turning it off for a single treasury wallet.
- `new_shared_sdk_context` lets several instances in one process share HTTP,
  gRPC, and database pools.

## Fees

| Flow | Fee | Source |
| --- | --- | --- |
| Spark to Spark | Free; "small flat fee coming in 6-12 months" | docs.spark.money, estimate-fees |
| Spark to Lightning | 0.25% plus routing | same |
| Lightning to Spark | Free to the receiver; 0.15% for the sender | same |
| Spark to on-chain | `250 × sat/vB + 750` sats | same |
| Deposit claim | On-chain claim fee, capped by `max_deposit_claim_fee`; instant claims add a provider fee | `onchain_claims.md` |
| BTC and token conversion | AMM spread plus 5 bps Breez integrator fee, slippage default 10 bps | `token_conversion/models.rs` |
| USDC and USDT receive | Paid by the sender through the quote; receiver pays none | `cross_chain.md` |

The SDK itself is free for developers.

## Testing

- **Regtest** is a hosted network run by Lightspark, with a faucet at
  <https://app.lightspark.com/regtest-faucet> and no API key. It covers Spark
  transfers, deposits, withdrawals, and token issuance.
- **Regtest does not cover Lightning, USDB, or USDC and USDT.** The docs say
  to test those on mainnet with small amounts (`testing.md`).
- **Local cluster:** `spark-itest` runs Docker bitcoind, Postgres, several
  operators built from `github.com/breez/spark`, `ldk-server`, and `sspd`
  (`make itest`). That is the only fully self-contained Lightning test path.
- There is no public testnet or Mutinynet support.

## Platform fit summary

| Question | Answer |
| --- | --- |
| Can the Rust core link it directly? | Yes, as a Git-tag dependency. No UniFFI layer needed. |
| iOS and Android targets? | Built by Breez for both. |
| Can Swift or Kotlin stay thin? | Yes. They keep only the Keychain or Keystore item. |
| Secure hardware for the key? | No, secp256k1. Wrap with Keychain or Keystore. |
| Background receive? | Lightning and LNURL receive work offline through the provider and LNURL server; the app claims on next open. |
| Multiple wallets? | Yes, by account number from one seed. |
| Spending limits? | Build on `PaymentObserver` and client signing. |
