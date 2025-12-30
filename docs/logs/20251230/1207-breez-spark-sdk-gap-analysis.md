# Breez Spark SDK Gap Analysis for Autopilot IDE

Date: 2025-12-30

## Scope reviewed
- crates/spark/README.md
- crates/spark/src/lib.rs
- crates/spark/src/wallet.rs
- crates/spark/src/signer.rs
- crates/spark/src/error.rs
- crates/spark/tests/integration.rs
- crates/wallet/src/cli/bitcoin.rs
- crates/wallet/src/gui/backend.rs
- crates/wallet/src/gui/view.rs
- crates/wallet/src/core/nwc.rs
- crates/wallet/src/main.rs
- crates/agent-orchestrator/src/integrations/spark_bridge.rs
- crates/nostr/core/src/nip_sa/wallet_integration.rs
- src/agents/runner/compute.rs
- src/cli/wallet.rs
- crates/wgpui/src/components/molecules/balance_card.rs
- crates/wgpui/src/components/molecules/invoice_display.rs
- /Users/christopherdavid/code/spark-sdk/README.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/getting_started.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/initializing.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/customizing.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/config.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/payments.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/receive_payment.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/send_payment.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/list_payments.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/parse.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/lnurl.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/lnurl_pay.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/lnurl_withdraw.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/receive_lnurl_pay.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/token_payments.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/issuing_tokens.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/messages.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/user_settings.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/events.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/optimize.md
- /Users/christopherdavid/code/spark-sdk/docs/breez-sdk/src/guide/fiat_currencies.md
- /Users/christopherdavid/code/spark-sdk/crates/breez-sdk/core/src/lib.rs
- /Users/christopherdavid/code/spark-sdk/crates/breez-sdk/core/src/sdk.rs
- /Users/christopherdavid/code/spark-sdk/crates/breez-sdk/core/src/sdk_builder.rs
- /Users/christopherdavid/code/spark-sdk/crates/breez-sdk/core/src/models/mod.rs

## Current integration summary (what works today)
- openagents-spark wraps breez-sdk-spark with connect, get_info (balance), Spark/Bitcoin receive, prepare/send payments, list_payments, HTLC claim, and event listeners.
- SparkSigner derives BIP44 keys for unified identity; SparkWallet uses the mnemonic to initialize the Breez SDK client.
- The wallet CLI and GUI in crates/wallet can show balances, generate receive payloads, send payments, and list history via SparkWallet.
- Agent compute flows can pay Lightning invoices using SparkWallet (send_payment_simple).
- UI building blocks exist for wallet balance and invoice display in wgpui, and NWC flows use SparkWallet in the wallet core.

## Gaps blocking 100 percent SDK implementation in the IDE

### 1) SDK API surface not exposed in openagents-spark
- No wrappers for parse_input, lnurl pay/withdraw, lightning address management, on-chain claim/refund, get_payment, recommended_fees, fiat rates, sign_message/check_message, user settings, or optimization APIs.
- No access to token metadata APIs or token issuer functionality (create/mint/burn/freeze).
- No exposure of SdkBuilder customization for storage, chain service, LNURL client, fiat service, payment observer, key set, or real-time sync storage.

### 2) Config and key management coverage gaps
- WalletConfig only supports network, api_key, and storage_dir; missing sync_interval_secs, max_deposit_claim_fee, lnurl_domain, prefer_spark_over_lightning, external_input_parsers, real_time_sync_server_url, private_enabled_default, and optimization_config.
- SparkWallet::new ignores the SparkSigner passphrase (Seed::Mnemonic uses passphrase None).
- Key set and account number selection are not configurable; SparkSigner uses BIP44 while Breez SDK defaults to its own key set.
- Network::Testnet/Signet are mapped to Regtest, but the UI/CLI expose them as distinct networks.

### 3) Wallet data mapping gaps
- Balance maps only GetInfoResponse.balance_sats and sets lightning_sats/onchain_sats to 0; token balances are dropped.
- WalletInfo exists but is never populated by SparkWallet.
- create_lightning_invoice uses SparkInvoice under the hood, not a true BOLT11 invoice.

### 4) Autopilot IDE integration gaps
- No wallet panel or live wallet data stream in the Autopilot IDE; wgpui components are not wired to SparkWallet state.
- agent-orchestrator SparkPaymentProvider remains stubbed, so cost tracking cannot pay or invoice via Spark.
- crates/wallet/src/main.rs still bails out for Spark commands while src/cli/wallet.rs uses the newer wallet CLI, creating two divergent entry points.

### 5) Eventing, sync, and logging gaps
- SdkEvent streams are not surfaced in the IDE runtime; no live payment status updates, sync state, or incoming payment notifications.
- No background sync scheduler or real-time sync visibility in the UI.
- init_logging is not wired to IDE or CLI log pipelines.

### 6) Receive and send feature gaps vs Breez SDK
- Missing LNURL pay/withdraw flows, Lightning address registration/lookup, and LNURL metadata handling.
- No on-chain deposit claim/refund flows or unclaimed deposit listing.
- No fee quote UX for on-chain withdrawals or claims.
- No single-payment lookup (get_payment) or structured parse_input flow in the wrapper; parse_input is used ad-hoc in wallet CLI core.

### 7) Token feature gaps
- No UI or CLI support for token balances, token send/receive, or token issuer operations.
- No exposure of token metadata in wallet balance or history views.

### 8) Testing and verification gaps
- Many integration tests are ignored or require manual env setup; no automated regtest harness for wallet flows.
- No tests for LNURL, Lightning address, on-chain claim/refund, tokens, or optimization flows.
- No UI tests for wallet panels, balance refresh, or payment history in the IDE.

## SDK roadmap items not yet available to implement
- spark-sdk README roadmap (may be stale) lists pending features: NWC, WebLN, LNURL-Auth, fiat on-ramp, BTC<->USDX swaps, hodl invoices, Bolt12, and fee portal additions.

## Suggested priorities to reach 100 percent in the IDE
1) Expand openagents-spark to expose the full Breez SDK surface (payments, LNURL, lightning address, on-chain claim/refund, tokens, user settings, optimization, fiat rates, logging, parse_input).
2) Fix wallet seed handling (passphrase), key set configuration, and network mapping, then surface the additional Config fields in WalletConfig.
3) Wire SparkPaymentProvider into agent-orchestrator and surface wallet panels in the IDE using wgpui components and SdkEvent streams.
4) Implement receive/send UX for LNURL, lightning addresses, and on-chain deposit/withdraw with fee previews and unclaimed deposit handling.
5) Add token balances, token payments, and issuer tooling to CLI/UI.
6) Build a regtest-backed test harness and add end-to-end coverage for wallet flows, including event-driven updates.

## Addendum (2025-12-30)
Phase 1 implementation updates in openagents-spark:
- Added SparkWalletBuilder to allow advanced Breez SDK configuration and key set selection without breaking existing WalletConfig call sites.
- Re-exported Config/KeySetType/InputType/ExternalInputParser at the crate root and surfaced parse_input for easier access to the parsing API.
- Updated spark README to reflect the builder path and the remaining integration gaps.

Remaining gaps still blocked in Phase 2:
- WalletConfig does not expose advanced SDK config (sync interval, max claim fee, lnurl domain, real-time sync, private mode, key set/account selection).
- WalletInfo population and token-aware balance shaping are still missing.
- CLI/GUI/IDE integration for LNURL, on-chain claims, tokens, user settings, and optimization is not wired.

## Addendum (2025-12-30, docs)
Documentation added under `crates/spark/docs/`:
- README.md: overview + quickstart + builder guidance.
- CONFIGURATION.md: WalletConfig vs Breez Config + builder/key set/passphrase details.
- API.md: wrapper API coverage + workflows (LNURL, on-chain claims, tokens, events).
