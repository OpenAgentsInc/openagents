use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ─── Reverse Swap Pairs ───────────────────────────────────────────────────

/// Response from `GET /v2/swap/reverse`.
/// Keyed by `from` currency (e.g. "BTC"), then `to` currency (e.g. "TBTC").
#[derive(Debug, Clone, Deserialize)]
pub struct ReversePairsResponse(pub HashMap<String, HashMap<String, ReversePairInfo>>);

/// Fee/rate/limit info for a single reverse swap pair.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReversePairInfo {
    pub hash: String,
    pub rate: f64,
    pub limits: PairLimits,
    pub fees: ReversePairFees,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairLimits {
    pub minimal: u64,
    pub maximal: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReversePairFees {
    pub percentage: f64,
    pub miner_fees: MinerFees,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MinerFees {
    pub claim: u64,
    pub lockup: u64,
}

// ─── Reverse Swap Creation ────────────────────────────────────────────────

/// Request body for `POST /v2/swap/reverse`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReverseSwapRequest {
    pub from: String,
    pub to: String,
    pub preimage_hash: String,
    pub claim_address: String,
    pub invoice_amount: u64,
    pub pair_hash: String,
    pub referral_id: String,
    /// Compressed secp256k1 public key (hex). Sent for all assets including EVM.
    pub claim_public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_expiry: Option<u64>,
}

/// Response from `POST /v2/swap/reverse`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReverseSwapResponse {
    pub id: String,
    pub invoice: String,
    #[serde(default)]
    pub swap_tree: Option<serde_json::Value>,
    pub lockup_address: String,
    pub timeout_block_height: u64,
    pub onchain_amount: u64,
    /// Boltz's refund public key (UTXO swaps).
    #[serde(default)]
    pub refund_public_key: Option<String>,
    /// Boltz's EVM refund address (EVM swaps).
    #[serde(default)]
    pub refund_address: Option<String>,
}

// ─── Swap Status ──────────────────────────────────────────────────────────

/// Response from `GET /v2/swap/{id}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapStatusResponse {
    pub status: String,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub transaction: Option<SwapTransaction>,
}

/// Response from `GET /v2/swap/reverse/{id}/transaction`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapTransactionResponse {
    pub id: String,
    #[serde(default)]
    pub hex: Option<String>,
    #[serde(default)]
    pub timeout_block_height: Option<u64>,
    #[serde(default)]
    pub timeout_eta: Option<u64>,
}

/// Transaction info included in status updates.
/// EVM transactions only have `id` (tx hash). UTXO transactions also have `hex`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapTransaction {
    pub id: String,
    #[serde(default)]
    pub hex: Option<String>,
}

// ─── DEX Quotes ───────────────────────────────────────────────────────────

/// Single quote from `GET /v2/quote/ARB/in` or `GET /v2/quote/ARB/out`.
/// The endpoint returns an array of these.
#[derive(Debug, Clone, Deserialize)]
pub struct QuoteResponse {
    /// Quoted amount as a string-encoded number.
    pub quote: String,
    /// Opaque quote data — passed through to the encode endpoint.
    pub data: serde_json::Value,
}

/// Request body for `POST /v2/quote/ARB/encode`.
/// Critical: `amount_in` and `amount_out_min` must be serialized as strings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeRequest {
    pub recipient: String,
    #[serde(serialize_with = "serialize_as_string")]
    pub amount_in: u128,
    #[serde(serialize_with = "serialize_as_string")]
    pub amount_out_min: u128,
    pub data: serde_json::Value,
}

/// Response from `POST /v2/quote/ARB/encode`.
#[derive(Debug, Clone, Deserialize)]
pub struct EncodeResponse {
    pub calls: Vec<QuoteCalldata>,
}

/// A single call from the encode response.
/// Field names match the Boltz API (`to`, `data`), NOT the Router contract (`target`, `callData`).
#[derive(Debug, Clone, Deserialize)]
pub struct QuoteCalldata {
    pub to: String,
    pub value: String,
    pub data: String,
}

// ─── Chain Contracts ──────────────────────────────────────────────────────

/// Response from `GET /v2/chain/contracts`.
/// Keyed by lowercase chain name (e.g. "arbitrum", "rsk").
#[derive(Debug, Clone, Deserialize)]
pub struct ContractsResponse(pub HashMap<String, ChainContracts>);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainContracts {
    pub network: ChainNetwork,
    pub swap_contracts: SwapContracts,
    #[serde(default)]
    pub tokens: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainNetwork {
    pub chain_id: u64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SwapContracts {
    pub ether_swap: String,
    #[serde(rename = "ERC20Swap")]
    pub erc20_swap: String,
}

// ─── WebSocket Messages ───────────────────────────────────────────────────

/// Subscribe message sent to Boltz WS.
#[derive(Debug, Clone, Serialize)]
pub struct WsSubscribeMessage {
    pub op: String,
    pub channel: String,
    pub args: Vec<String>,
}

impl WsSubscribeMessage {
    pub fn subscribe(swap_ids: Vec<String>) -> Self {
        Self {
            op: "subscribe".to_string(),
            channel: "swap.update".to_string(),
            args: swap_ids,
        }
    }

    pub fn unsubscribe(swap_ids: Vec<String>) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            channel: "swap.update".to_string(),
            args: swap_ids,
        }
    }
}

/// Incoming WS message from Boltz (generic envelope).
#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<WsSwapUpdate>>,
}

/// A single swap status update from the WS `args` array.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsSwapUpdate {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub transaction: Option<SwapTransaction>,
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn serialize_as_string<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn test_deserialize_reverse_pairs() {
        let json = r#"{
            "BTC": {
                "TBTC": {
                    "hash": "abc123",
                    "rate": 1.0,
                    "limits": { "minimal": 10000, "maximal": 25000000 },
                    "fees": {
                        "percentage": 0.25,
                        "minerFees": { "claim": 170, "lockup": 171 }
                    }
                }
            }
        }"#;

        let parsed: ReversePairsResponse = serde_json::from_str(json).unwrap();
        let tbtc = &parsed.0["BTC"]["TBTC"];
        assert_eq!(tbtc.hash, "abc123");
        assert!((tbtc.rate - 1.0).abs() < f64::EPSILON);
        assert_eq!(tbtc.limits.minimal, 10000);
        assert_eq!(tbtc.limits.maximal, 25_000_000);
        assert!((tbtc.fees.percentage - 0.25).abs() < f64::EPSILON);
        assert_eq!(tbtc.fees.miner_fees.claim, 170);
        assert_eq!(tbtc.fees.miner_fees.lockup, 171);
    }

    #[macros::test_all]
    fn test_serialize_create_reverse_swap_request() {
        let req = CreateReverseSwapRequest {
            from: "BTC".to_string(),
            to: "TBTC".to_string(),
            preimage_hash: "abcd1234".to_string(),
            claim_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            invoice_amount: 100_000,
            pair_hash: "hash123".to_string(),
            referral_id: "test_ref".to_string(),
            claim_public_key: "02abcdef".to_string(),
            description: None,
            invoice_expiry: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["from"], "BTC");
        assert_eq!(json["to"], "TBTC");
        assert_eq!(json["preimageHash"], "abcd1234");
        assert_eq!(
            json["claimAddress"],
            "0x1234567890abcdef1234567890abcdef12345678"
        );
        assert_eq!(json["invoiceAmount"], 100_000);
        assert_eq!(json["pairHash"], "hash123");
        assert_eq!(json["referralId"], "test_ref");
        assert_eq!(json["claimPublicKey"], "02abcdef");
        // Optional fields should be absent when None
        assert!(json.get("description").is_none());
        assert!(json.get("invoiceExpiry").is_none());
    }

    #[macros::test_all]
    fn test_deserialize_create_reverse_swap_response() {
        let json = r#"{
            "id": "swap123",
            "invoice": "lnbc1000n1...",
            "swapTree": { "claimLeaf": {}, "refundLeaf": {} },
            "lockupAddress": "0xabc",
            "timeoutBlockHeight": 123456,
            "onchainAmount": 99500,
            "refundAddress": "0xdef"
        }"#;

        let resp: CreateReverseSwapResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "swap123");
        assert_eq!(resp.invoice, "lnbc1000n1...");
        assert_eq!(resp.timeout_block_height, 123_456);
        assert_eq!(resp.onchain_amount, 99_500);
        assert_eq!(resp.refund_address.as_deref(), Some("0xdef"));
        assert!(resp.refund_public_key.is_none());
    }

    #[macros::test_all]
    fn test_deserialize_swap_status() {
        let json = r#"{
            "status": "transaction.confirmed",
            "transaction": { "id": "0xabc", "hex": "0xdef" }
        }"#;

        let resp: SwapStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "transaction.confirmed");
        assert!(resp.failure_reason.is_none());
        let tx = resp.transaction.unwrap();
        assert_eq!(tx.id, "0xabc");
    }

    #[macros::test_all]
    fn test_deserialize_quote_response() {
        let json = r#"[{
            "quote": "71044592",
            "data": {
                "type": "uniswapV3",
                "tokenIn": "0x6c84a8f1c29108f47a79964b5fe888d4f4d0de40",
                "hops": [
                    { "fee": 100, "token": "0x2f2a2543b76a4166549f7aab2e75bef0aefc5b0f" },
                    { "fee": 500, "token": "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9" }
                ]
            }
        }]"#;

        let quotes: Vec<QuoteResponse> = serde_json::from_str(json).unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].quote, "71044592");
        assert!(quotes[0].data.is_object());
    }

    #[macros::test_all]
    fn test_serialize_encode_request() {
        let req = EncodeRequest {
            recipient: "0xRouterAddress".to_string(),
            amount_in: 1_000_000_000_000_000_000,
            amount_out_min: 71_000_000,
            data: serde_json::json!({"type": "uniswapV3"}),
        };

        let json = serde_json::to_value(&req).unwrap();
        // Amounts must be serialized as decimal strings — the Boltz API
        // rejects integer-typed amounts because JS clients send `BigInt`s
        // stringified.
        assert_eq!(json["amountIn"], "1000000000000000000");
        assert_eq!(json["amountOutMin"], "71000000");
        assert_eq!(json["recipient"], "0xRouterAddress");
    }

    #[macros::test_all]
    fn test_deserialize_encode_response() {
        let json = r#"{
            "calls": [{
                "to": "0xDexRouter",
                "value": "0",
                "data": "0xabcdef"
            }]
        }"#;

        let resp: EncodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.calls.len(), 1);
        assert_eq!(resp.calls[0].to, "0xDexRouter");
        assert_eq!(resp.calls[0].data, "0xabcdef");
    }

    #[macros::test_all]
    fn test_deserialize_contracts_response() {
        let json = r#"{
            "arbitrum": {
                "network": { "chainId": 42161, "name": "Arbitrum One" },
                "swapContracts": {
                    "EtherSwap": "0xEtherSwap",
                    "ERC20Swap": "0x6398B76DF91C5eBe9f488e3656658E79284dDc0F"
                },
                "tokens": {
                    "TBTC": "0x6c84a8f1c29108F47a79964b5Fe888D4f4D0dE40"
                }
            }
        }"#;

        let resp: ContractsResponse = serde_json::from_str(json).unwrap();
        let arb = &resp.0["arbitrum"];
        assert_eq!(arb.network.chain_id, 42161);
        assert_eq!(
            arb.swap_contracts.erc20_swap,
            "0x6398B76DF91C5eBe9f488e3656658E79284dDc0F"
        );
        assert_eq!(
            arb.tokens["TBTC"],
            "0x6c84a8f1c29108F47a79964b5Fe888D4f4D0dE40"
        );
    }

    #[macros::test_all]
    fn test_ws_subscribe_message() {
        let msg = WsSubscribeMessage::subscribe(vec!["swap1".to_string(), "swap2".to_string()]);
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["op"], "subscribe");
        assert_eq!(json["channel"], "swap.update");
        assert_eq!(json["args"], serde_json::json!(["swap1", "swap2"]));
    }

    #[macros::test_all]
    fn test_deserialize_ws_update() {
        let json = r#"{
            "event": "update",
            "channel": "swap.update",
            "args": [{
                "id": "swap123",
                "status": "transaction.mempool",
                "transaction": { "id": "0xtx", "hex": "0xraw" }
            }]
        }"#;

        let msg: WsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event.as_deref(), Some("update"));
        let args = msg.args.unwrap();
        assert_eq!(args[0].id, "swap123");
        assert_eq!(args[0].status, "transaction.mempool");
        assert!(args[0].transaction.is_some());
    }

    #[macros::test_all]
    fn test_deserialize_ws_ping_pong() {
        let json = r#"{"event": "pong"}"#;
        let msg: WsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event.as_deref(), Some("pong"));
        assert!(msg.args.is_none());
    }
}
