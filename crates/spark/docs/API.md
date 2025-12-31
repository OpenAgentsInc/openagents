# API Coverage and Workflows

This file summarizes the openagents-spark wrapper API and common flows.

## Core Types
- SparkSigner: BIP39 to BIP44 key derivation for unified identity.
- SparkWallet: Breez SDK wrapper for payments and wallet operations.
- SparkWalletBuilder: Advanced SDK config and key set selection.

## Seed sources
When you already have raw seed entropy (16-64 bytes), use `SparkSigner::from_entropy`.

```rust
use openagents_spark::SparkSigner;

let signer = SparkSigner::from_entropy(&[0u8; 32])?;
println!("Pubkey: {}", signer.public_key_hex());
```

## Parsing inputs
Use parse_input to classify a payment request or address.

```rust
use openagents_spark::{parse_input, ExternalInputParser};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let input = "lnbc1...";
let parsed = parse_input(input, None).await?;
println!("Parsed input: {:?}", parsed);
# Ok(())
# }
```

## Receive payments
- Spark address: reusable, for Spark-native payments.
- Spark invoice: single-use, supports amount/expiry.
- Bitcoin address: static deposit address for on-chain funding.
- BOLT-11 invoice: Lightning invoice generation (when configured).

```rust
use openagents_spark::{ReceivePaymentRequest, ReceivePaymentMethod};

# async fn example(wallet: openagents_spark::SparkWallet) -> Result<(), Box<dyn std::error::Error>> {
let resp = wallet.receive_payment(ReceivePaymentRequest {
    payment_method: ReceivePaymentMethod::SparkAddress,
}).await?;
println!("Spark address: {}", resp.payment_request);
# Ok(())
# }
```

## Send payments
Use prepare_send_payment for validation + fees, then send_payment.

```rust
# async fn example(wallet: openagents_spark::SparkWallet) -> Result<(), Box<dyn std::error::Error>> {
let prepared = wallet.prepare_send_payment("lnbc1...", None).await?;
let sent = wallet.send_payment(prepared, None).await?;
println!("Payment id: {}", sent.payment.id);
# Ok(())
# }
```

## LNURL pay and withdraw
- prepare_lnurl_pay + lnurl_pay
- lnurl_withdraw for withdraw requests

```rust
# async fn example(wallet: openagents_spark::SparkWallet, pay_req: openagents_spark::LnurlPayRequest) -> Result<(), Box<dyn std::error::Error>> {
let result = wallet.lnurl_pay(pay_req).await?;
println!("LNURL payment status: {:?}", result.payment.status);
# Ok(())
# }
```

## Lightning address management
- check_lightning_address_available
- register_lightning_address
- get_lightning_address
- delete_lightning_address

## On-chain deposits
- list_unclaimed_deposits
- claim_deposit
- refund_deposit
- recommended_fees

These flows are needed to handle auto-claim failures and refunds.

## Tokens
- get_tokens_metadata
- get_token_issuer (issuer API: create/mint/burn/freeze)

Token balances are returned in GetInfoResponse.token_balances.

## User settings and privacy
- get_user_settings
- update_user_settings

Spark private mode is controlled via user settings and SDK config.

## Optimization controls
- start_leaf_optimization
- cancel_leaf_optimization
- get_leaf_optimization_progress

## Eventing
Use add_event_listener/remove_event_listener on SparkWallet to subscribe to SdkEvent.

## Sync and lifecycle
- sync_wallet: force synchronization
- disconnect: stop background tasks

## Errors
SparkError wraps Breez SDK failures into user-facing errors. For UX, prefer SparkError::user_friendly_message where appropriate.
