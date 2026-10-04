//! On-chain liveness checks for `ERC20Swap` lockups.
//!
//! These are used by the live claim path to verify funds are actually locked
//! before revealing the preimage (anti-fraud guard), and by the resume/retry
//! logic to decide whether a claim is still worth attempting.

use alloy_primitives::U256;

use crate::config::{ARBITRUM_TBTC_ADDRESS, SATS_TO_TBTC_FACTOR};
use crate::error::BoltzError;
use crate::evm::contracts::{
    DecodedLockupEvent, bytes32_to_topic, claim_event_topic0, decode_hash_values_return,
    decode_swaps_check_return, encode_hash_values, encode_swaps_check, parse_address,
    refund_event_topic0,
};
use crate::evm::provider::EvmProvider;
use crate::models::BoltzSwap;
use crate::secrets::SecretProvider;

/// Check whether a swap is still locked on-chain (not yet claimed/refunded).
pub async fn is_swap_still_locked(
    evm_provider: &EvmProvider,
    erc20swap_address: &str,
    event: &DecodedLockupEvent,
) -> Result<bool, BoltzError> {
    let hash_calldata = encode_hash_values(
        event.preimage_hash,
        event.amount,
        event.token_address,
        event.claim_address,
        event.refund_address,
        event.timelock,
    );
    let hash_result = evm_provider
        .eth_call(erc20swap_address, &hash_calldata)
        .await?;
    let swap_hash = decode_hash_values_return(&hash_result)?;

    let check_calldata = encode_swaps_check(swap_hash);
    let check_result = evm_provider
        .eth_call(erc20swap_address, &check_calldata)
        .await?;
    decode_swaps_check_return(&check_result)
}

/// Convenience wrapper: check whether a persisted swap's funds are still
/// locked on the `ERC20Swap` contract. Returns `true` if claimable, `false`
/// if already claimed or refunded.
pub(crate) async fn is_swap_still_locked_by_swap(
    evm_provider: &EvmProvider,
    swap: &BoltzSwap,
    secrets: &dyn SecretProvider,
) -> Result<bool, BoltzError> {
    let preimage_hash = secrets.preimage_hash(swap)?;
    let tbtc_evm_amount = U256::from(swap.onchain_amount)
        .checked_mul(U256::from(SATS_TO_TBTC_FACTOR))
        .ok_or_else(|| BoltzError::Generic("tBTC EVM amount overflow".into()))?;

    let event = DecodedLockupEvent {
        preimage_hash,
        amount: tbtc_evm_amount,
        token_address: parse_address(ARBITRUM_TBTC_ADDRESS)?,
        claim_address: parse_address(&swap.claim_address)?,
        refund_address: parse_address(&swap.refund_address)?,
        timelock: U256::from(swap.timeout_block_height),
    };

    is_swap_still_locked(evm_provider, &swap.erc20swap_address, &event).await
}

/// How a spent `ERC20Swap` lockup was spent.
///
/// `swaps(hash)` returns `false` for **both** a claimed and a refunded lockup,
/// so a "spent" reading alone cannot tell success from failure. The contract
/// emits distinct, `preimageHash`-indexed `Claim`/`Refund` events; this is what
/// lets us avoid misclassifying a successful claim — possibly made by *another
/// instance* sharing the same keys — as a failure. See [`classify_spent_lockup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpentClassification {
    /// A `Claim` event for our `preimageHash` exists: the lockup was claimed
    /// (the preimage was revealed). Carries the winning claim tx hash so the
    /// caller can recover the delivered amount from its receipt.
    Claimed { claim_tx_hash: String },
    /// A `Refund` event for our `preimageHash` exists: the lockup refunded.
    Refunded,
    /// Neither event was resolvable (no block anchor, not yet indexed, or RPC
    /// error). Callers MUST NOT finalize on this — it is *not* evidence of
    /// failure; leave the swap for a later retry.
    Unknown,
}

/// Classify an already-spent lockup as claimed vs refunded via the
/// `ERC20Swap` `Claim`/`Refund` events for this swap's `preimageHash`.
///
/// Call only once the lockup is known spent (`is_swap_still_locked* == false`):
/// this issues `eth_getLogs` and does not re-check the `swaps()` mapping. The
/// block range is anchored at the lockup tx's block (resolved from the persisted
/// `lockup_tx_id`); without that anchor the range can't be bounded and we return
/// [`SpentClassification::Unknown`] rather than guess. A `Claim` log for our
/// indexed `preimageHash` on our own `ERC20Swap` is conclusive proof of a claim,
/// so no preimage re-check is needed.
pub(crate) async fn classify_spent_lockup(
    evm_provider: &EvmProvider,
    swap: &BoltzSwap,
    secrets: &dyn SecretProvider,
) -> Result<SpentClassification, BoltzError> {
    let preimage_hash = secrets.preimage_hash(swap)?;
    let preimage_topic = bytes32_to_topic(&preimage_hash);

    let Some((from_block, to_block)) = resolve_lockup_block_range(evm_provider, swap).await? else {
        tracing::warn!(
            swap_id = swap.id,
            "No lockup block anchor (missing/unresolvable lockup_tx_id); cannot classify spent lockup"
        );
        return Ok(SpentClassification::Unknown);
    };

    let claim_topic0 = claim_event_topic0();
    let claim_logs = evm_provider
        .eth_get_logs(
            &swap.erc20swap_address,
            &[Some(&claim_topic0), Some(&preimage_topic)],
            from_block,
            to_block,
        )
        .await?;
    if let Some(log) = claim_logs.into_iter().next() {
        return Ok(SpentClassification::Claimed {
            claim_tx_hash: log.transaction_hash,
        });
    }

    let refund_topic0 = refund_event_topic0();
    let refund_logs = evm_provider
        .eth_get_logs(
            &swap.erc20swap_address,
            &[Some(&refund_topic0), Some(&preimage_topic)],
            from_block,
            to_block,
        )
        .await?;
    if !refund_logs.is_empty() {
        return Ok(SpentClassification::Refunded);
    }

    Ok(SpentClassification::Unknown)
}

/// Resolve `(from_block, to_block)` for a spent-lockup log query. `from_block`
/// is the block of the lockup tx (both the `Claim` and `Refund` occur at or
/// after it); `to_block` is the latest L2 block. Returns `None` if the lockup
/// tx hash is missing or its receipt can't be resolved — the caller then treats
/// the outcome as [`SpentClassification::Unknown`].
///
/// The range can span the swap's whole lifetime (lockup → now), so the query
/// relies on the RPC serving a wide-range `eth_getLogs` when the filter is
/// narrow (address + topic0 + indexed `preimageHash`, ≤1 result). Both the
/// default `arb1.arbitrum.io/rpc` and Alchemy do — they cap by result count,
/// not block range — so no chunking is needed; a result-count-capped endpoint
/// would error, classify as `Unknown`, and leave the swap for retry (the safe
/// direction — never a wrongful finalize).
async fn resolve_lockup_block_range(
    evm_provider: &EvmProvider,
    swap: &BoltzSwap,
) -> Result<Option<(u64, u64)>, BoltzError> {
    let Some(lockup_tx_id) = swap.lockup_tx_id.as_deref() else {
        return Ok(None);
    };
    let Some(receipt) = evm_provider
        .eth_get_transaction_receipt(lockup_tx_id)
        .await?
    else {
        return Ok(None);
    };
    let Some(from_block) = parse_block_hex(&receipt.block_number) else {
        return Ok(None);
    };
    let to_block = evm_provider.eth_block_number().await?;
    Ok(Some((from_block, to_block.max(from_block))))
}

fn parse_block_hex(s: &str) -> Option<u64> {
    let clean = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(clean, 16).ok()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::models::{Asset, BoltzSwapStatus, BridgeKind, SwapKeySource};
    use crate::secrets::DerivedSecretProvider;
    use crate::store::MemoryBoltzStorage;
    use platform_utils::http::{HttpClient, HttpError, HttpResponse};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// HTTP client that replays canned JSON-RPC responses in order. Panics if a
    /// request arrives with no response left — so a test that supplies exactly
    /// the expected number of responses also asserts no *extra* RPC was made
    /// (e.g. that the refund query is skipped once a `Claim` log is found).
    struct MockHttpClient {
        responses: Arc<Mutex<Vec<HttpResponse>>>,
    }

    impl MockHttpClient {
        fn new(responses: Vec<HttpResponse>) -> Self {
            let mut r = responses;
            r.reverse();
            Self {
                responses: Arc::new(Mutex::new(r)),
            }
        }
    }

    #[macros::async_trait]
    impl HttpClient for MockHttpClient {
        async fn get(
            &self,
            _url: String,
            _headers: Option<HashMap<String, String>>,
        ) -> Result<HttpResponse, HttpError> {
            unimplemented!()
        }

        async fn post(
            &self,
            _url: String,
            _headers: Option<HashMap<String, String>>,
            _body: Option<String>,
        ) -> Result<HttpResponse, HttpError> {
            Ok(self
                .responses
                .lock()
                .unwrap()
                .pop()
                .expect("unexpected extra RPC request"))
        }

        async fn delete(
            &self,
            _url: String,
            _headers: Option<HashMap<String, String>>,
            _body: Option<String>,
        ) -> Result<HttpResponse, HttpError> {
            unimplemented!()
        }
    }

    fn rpc_ok(result: &serde_json::Value) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": result }).to_string(),
        }
    }

    /// A minimal `eth_getTransactionReceipt` reply anchoring the lockup block.
    fn receipt_at_block(block_hex: &str) -> serde_json::Value {
        serde_json::json!({
            "transactionHash": "0xlockup",
            "status": "0x1",
            "blockHash": "0xbh",
            "blockNumber": block_hex,
            "gasUsed": "0x1",
        })
    }

    /// A single log carrying only the field the classifier reads.
    fn log_with_tx(tx_hash: &str) -> serde_json::Value {
        serde_json::json!({
            "address": "0xswap",
            "topics": ["0xtopic0", "0xpreimage"],
            "data": "0x",
            "blockNumber": "0x11",
            "transactionHash": tx_hash,
        })
    }

    fn provider(responses: Vec<HttpResponse>) -> EvmProvider {
        EvmProvider::new(
            "http://mock".to_string(),
            Box::new(MockHttpClient::new(responses)),
        )
    }

    fn test_swap(lockup_tx_id: Option<&str>) -> BoltzSwap {
        BoltzSwap {
            id: "swap-1".to_string(),
            status: BoltzSwapStatus::Claiming,
            bridge_kind: BridgeKind::Oft,
            key_source: SwapKeySource::Derived { claim_key_index: 0 },
            chain_id: 42161,
            claim_address: "0xabc".to_string(),
            destination_address: "0xdef".to_string(),
            destination_chain: "Arbitrum One".to_string(),
            asset: Asset::Usdt,
            refund_address: "0x123".to_string(),
            erc20swap_address: "0xswap".to_string(),
            router_address: "0xrouter".to_string(),
            invoice: "lnbc".to_string(),
            invoice_amount_sats: 100_000,
            onchain_amount: 99_500,
            expected_output_amount: 71_000_000,
            slippage_bps: 100,
            timeout_block_height: 123_456,
            lockup_tx_id: lockup_tx_id.map(str::to_string),
            claim_tx_hash: None,
            pending_call_id: None,
            delivered_amount: None,
            bridge_ref: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        }
    }

    fn secret_provider() -> DerivedSecretProvider {
        DerivedSecretProvider::new(
            crate::keys::EvmKeyManager::from_seed(&[7u8; 32]).unwrap(),
            Arc::new(MemoryBoltzStorage::new()),
        )
    }

    #[macros::async_test_all]
    async fn classify_claimed_when_claim_log_present() {
        // receipt → blockNumber → claim getLogs (one hit). Refund is never
        // queried (only 3 responses supplied; a 4th request would panic).
        let evm = provider(vec![
            rpc_ok(&receipt_at_block("0x10")),
            rpc_ok(&serde_json::json!("0x20")),
            rpc_ok(&serde_json::json!([log_with_tx("0xwinner")])),
        ]);
        let result = classify_spent_lockup(&evm, &test_swap(Some("0xlockup")), &secret_provider())
            .await
            .unwrap();
        assert_eq!(
            result,
            SpentClassification::Claimed {
                claim_tx_hash: "0xwinner".to_string()
            }
        );
    }

    #[macros::async_test_all]
    async fn classify_refunded_when_only_refund_log() {
        // receipt → blockNumber → empty claim getLogs → refund getLogs (one hit).
        let evm = provider(vec![
            rpc_ok(&receipt_at_block("0x10")),
            rpc_ok(&serde_json::json!("0x20")),
            rpc_ok(&serde_json::json!([])),
            rpc_ok(&serde_json::json!([log_with_tx("0xrefund")])),
        ]);
        let result = classify_spent_lockup(&evm, &test_swap(Some("0xlockup")), &secret_provider())
            .await
            .unwrap();
        assert_eq!(result, SpentClassification::Refunded);
    }

    #[macros::async_test_all]
    async fn classify_unknown_when_no_events_found() {
        // Spent on-chain, but neither a Claim nor a Refund log resolves — must
        // be Unknown (leave for retry), never a finalize.
        let evm = provider(vec![
            rpc_ok(&receipt_at_block("0x10")),
            rpc_ok(&serde_json::json!("0x20")),
            rpc_ok(&serde_json::json!([])),
            rpc_ok(&serde_json::json!([])),
        ]);
        let result = classify_spent_lockup(&evm, &test_swap(Some("0xlockup")), &secret_provider())
            .await
            .unwrap();
        assert_eq!(result, SpentClassification::Unknown);
    }

    #[macros::async_test_all]
    async fn classify_unknown_when_lockup_tx_id_missing() {
        // No block anchor → Unknown without issuing any RPC (empty mock).
        let evm = provider(vec![]);
        let result = classify_spent_lockup(&evm, &test_swap(None), &secret_provider())
            .await
            .unwrap();
        assert_eq!(result, SpentClassification::Unknown);
    }

    #[macros::async_test_all]
    async fn classify_unknown_when_receipt_not_found() {
        // Lockup tx hash known but its receipt isn't resolvable (null) → Unknown.
        let evm = provider(vec![rpc_ok(&serde_json::Value::Null)]);
        let result = classify_spent_lockup(&evm, &test_swap(Some("0xlockup")), &secret_provider())
            .await
            .unwrap();
        assert_eq!(result, SpentClassification::Unknown);
    }
}
