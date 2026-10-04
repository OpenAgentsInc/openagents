use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use alloy_primitives::{Bytes, FixedBytes, U256};
use lightning_invoice::Bolt11Invoice;

use crate::api::BoltzApiClient;
use crate::api::types::{EncodeRequest, QuoteResponse, ReversePairInfo};
use crate::config::{
    ARBITRUM_ROUTER_ADDRESS, ARBITRUM_TBTC_ADDRESS, ARBITRUM_USDC_ADDRESS, ARBITRUM_USDT_ADDRESS,
    BoltzConfig, CCTP_ARBITRUM_DOMAIN, CCTP_FINALITY_FAST, CCTP_TOKEN_MESSENGER_V2,
    MAX_SLIPPAGE_BPS, MIN_TIMEOUT_L1_MARGIN, POLYGON_EVM_CHAIN_ID, PROBE_INVOICE_EXPIRY_SECS,
    SATS_TO_TBTC_FACTOR, SOLANA_MESSAGE_TRANSMITTER_V2, SOLANA_USDT0_MINT, ZERO_ADDRESS,
};
use crate::error::BoltzError;
use crate::evm::alchemy::{AlchemyGasClient, EvmCall};
use crate::evm::cctp::{self, CctpFeeClient};
use crate::evm::contracts::{
    self, CctpData, ClaimCctpAuthorization, ClaimSendAuthorization, Erc20Claim, SendData,
    encode_claim_erc20_execute, encode_claim_erc20_execute_cctp, encode_claim_erc20_execute_oft,
    hash_cctp_data, parse_address, quote_calldata_to_call,
};
use crate::evm::lockup::is_swap_still_locked_by_swap;
use crate::evm::lz_options::build_extra_options;
use crate::evm::lz_scan::LzScanClient;
use crate::evm::oft::legacy_mesh_source_amount;
use crate::evm::provider::EvmProvider;
use crate::evm::recipient::{
    encode_oft_recipient, is_valid_destination_address, normalize_token_address,
};
use crate::evm::signing::EvmSigner;
use crate::keys::EvmKeyPair;
use crate::models::{
    Asset, BoltzSwap, BoltzSwapStatus, Bridge, Destination, DestinationRegistry, NetworkTransport,
    PreparedSwap, SwapLimits, Usdt0Kind,
};
use crate::secrets::SecretProvider;
use crate::solana::ata::{derive_ata, derive_used_nonce_pda};
use crate::solana::rpc::SolanaRpcClient;
use crate::store::BoltzStorage;

/// Maximum claim retries (quote may go stale between encode and submit).
const MAX_CLAIM_RETRIES: u32 = 5;

/// Maximum attempts to verify lockup on-chain before claiming.
/// The public RPC endpoint may lag behind Boltz's node by a few seconds.
const LOCKUP_CHECK_MAX_ATTEMPTS: u32 = 10;

/// Orchestrates the LN -> stablecoin reverse swap flow.
pub(crate) struct ReverseSwapExecutor {
    api_client: BoltzApiClient,
    pub(crate) secrets: Arc<dyn SecretProvider>,
    alchemy_client: AlchemyGasClient,
    pub(crate) evm_provider: EvmProvider,
    pub(crate) chain_registry: Arc<DestinationRegistry>,
    pub(crate) config: BoltzConfig,
    /// Persistence handle, used to durably record the in-flight gas-sponsor
    /// `call_id` mid-claim (between submission and confirmation) so a crash in
    /// that window is recoverable on resume.
    store: Arc<dyn BoltzStorage>,
    /// Circle Iris fee client, used to quote the CCTP burn fee for USDC
    /// destinations at prepare and claim time, and to confirm CCTP delivery.
    cctp_fee_client: CctpFeeClient,
    /// `LayerZero` Scan client, used to confirm OFT (USDT0) cross-chain
    /// delivery while a swap is `Settling`.
    lz_scan_client: LzScanClient,
    pub(crate) erc20swap_address: String,
    /// Used only when the destination chain is Solana, to query whether the
    /// recipient's Associated Token Account already exists. Always
    /// constructed — `BoltzConfig::solana_rpc_url` has a mainnet default.
    solana_rpc: SolanaRpcClient,
    /// Recipient pubkeys (base58) whose USDT0 Associated Token Account has
    /// been observed to already exist on-chain. Asymmetric cache: only
    /// "exists" answers are memoised — "doesn't exist" is not, because the
    /// user may have created the ATA between calls.
    ata_cache: Mutex<HashSet<String>>,
}

impl ReverseSwapExecutor {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        api_client: BoltzApiClient,
        secrets: Arc<dyn SecretProvider>,
        alchemy_client: AlchemyGasClient,
        evm_provider: EvmProvider,
        chain_registry: Arc<DestinationRegistry>,
        config: BoltzConfig,
        store: Arc<dyn BoltzStorage>,
        cctp_fee_client: CctpFeeClient,
        lz_scan_client: LzScanClient,
        erc20swap_address: String,
        solana_rpc: SolanaRpcClient,
    ) -> Self {
        Self {
            api_client,
            secrets,
            alchemy_client,
            evm_provider,
            chain_registry,
            config,
            store,
            cctp_fee_client,
            lz_scan_client,
            erc20swap_address,
            solana_rpc,
            ata_cache: Mutex::new(HashSet::new()),
        }
    }

    /// Get swap limits from the Boltz pairs endpoint.
    pub async fn get_limits(&self) -> Result<SwapLimits, BoltzError> {
        let tbtc_pair = self.fetch_tbtc_pair().await?;
        Ok(SwapLimits {
            min_sats: tbtc_pair.limits.minimal,
            max_sats: tbtc_pair.limits.maximal,
        })
    }

    /// Look up a [`Destination`] by its `(chain, asset)` identity, raising a
    /// hard error if unsupported. Used by every path that needs per-destination
    /// metadata.
    fn resolve_destination(&self, chain: &str, asset: Asset) -> Result<&Destination, BoltzError> {
        self.chain_registry.find(chain, asset).ok_or_else(|| {
            BoltzError::Generic(format!("Unsupported destination '{chain}' for {asset}"))
        })
    }

    /// Prepare a reverse swap quote. No side effects.
    ///
    /// Works backwards from the caller's target USDT amount:
    ///
    /// 1. Same-chain: one DEX quote (tBTC ← output token, USDT or USDC, for
    ///    the target amount); floor to sats; apply the Boltz reverse fee.
    /// 2. Cross-chain: invert the OFT to get the USDT needed on Arbitrum,
    ///    quote the LZ messaging fee, then convert **each** leg to tBTC
    ///    sats independently (tBTC ← USDT and tBTC ← ETH), floor each to
    ///    sats, sum, and apply the Boltz reverse fee.
    ///
    /// The cross-chain path splits the tBTC conversion into two floored
    /// legs rather than summing in USDT and flooring once: pricing the
    /// messaging fee against tBTC directly keeps both legs on the same
    /// DEX pools and avoids a 1-sat drift from combined-flooring.
    pub async fn prepare(
        &self,
        destination: &str,
        chain: &str,
        asset: Asset,
        output_amount: u64,
        max_slippage_bps: Option<u32>,
    ) -> Result<PreparedSwap, BoltzError> {
        let slippage_bps = resolve_slippage_bps(max_slippage_bps, self.config.slippage_bps)?;

        let dest = self.resolve_destination(chain, asset)?;
        self.validate_destination(dest, destination)?;

        // Compute the total tBTC claim amount (in sats) needed to fund the
        // destination-side delivery. CCTP inverts a burn fee in its own flow
        // and returns directly; Direct and Oft yield the tBTC sats and fall
        // through to the shared fee/limit/construction tail below.
        let total_tbtc_sats = match &dest.bridge {
            // USDC (CCTP): the Router burns Arbitrum USDC and the burn fee is
            // deducted from the burned amount, so invert the fee — the DEX must
            // produce `target + fee` USDC — then quote that in tBTC.
            Bridge::Cctp { domain } => {
                let fee = self
                    .cctp_prepare_fee(dest.transport, destination, *domain)
                    .await?;
                let required_burn = cctp::cctp_required_burn(u128::from(output_amount), &fee);
                let required_burn_u64 = u64::try_from(required_burn)
                    .map_err(|_| BoltzError::Generic("USDC burn amount overflow".into()))?;
                let tbtc_wei = self
                    .fetch_quote_out_tbtc_for_token(required_burn_u64, dest.dex_output_token)
                    .await?;
                tbtc_wei_to_sats_u64(tbtc_wei)?
            }
            // Direct: deliver the DEX output (USDT or USDC) on Arbitrum. One
            // DEX quote for how much tBTC buys `output_amount`, floored to sats.
            Bridge::Direct => {
                let tbtc_wei = self
                    .fetch_quote_out_tbtc_for_token(output_amount, dest.dex_output_token)
                    .await?;
                tbtc_wei_to_sats_u64(tbtc_wei)?
            }
            // Cross-chain OFT: find how much USDT on Arbitrum is needed to
            // deliver `output_amount` on the destination after OFT fees, then
            // convert the USDT leg and the LZ messaging-fee leg to tBTC sats
            // independently.
            Bridge::Oft { .. } => {
                // LayerZero executor options for this (chain, destination)
                // pair. Solana destinations may need an ATA-creation hint,
                // which affects the messaging fee and must feed every quote.
                let extra_options = self.compute_extra_options(dest, destination).await?;
                let required_usdt = self
                    .estimate_oft_required_send_amount(
                        dest,
                        u128::from(output_amount),
                        &extra_options,
                    )
                    .await?;
                let (msg_fee_native, _) = self
                    .quote_oft_messaging_fee(dest, required_usdt, &extra_options)
                    .await?;

                let required_usdt_u64 = u64::try_from(required_usdt)
                    .map_err(|_| BoltzError::Generic("USDT amount overflow".into()))?;
                let usdt_leg_tbtc_wei = self.fetch_quote_out_tbtc(required_usdt_u64).await?;
                let usdt_leg_tbtc_sats = tbtc_wei_to_sats_u64(usdt_leg_tbtc_wei)?;

                let msg_fee_tbtc_sats = if msg_fee_native == 0 {
                    0u64
                } else {
                    let tbtc_wei = self.fetch_quote_out_tbtc_for_eth(msg_fee_native).await?;
                    tbtc_wei_to_sats_u64(tbtc_wei)?
                };

                usdt_leg_tbtc_sats
                    .checked_add(msg_fee_tbtc_sats)
                    .ok_or_else(|| BoltzError::Generic("tBTC sats overflow".into()))?
            }
        };

        let bridge_kind = dest.bridge.kind();
        let tbtc_pair = self.fetch_tbtc_pair().await?;

        // Apply Boltz fee
        let fee_calc = compute_invoice_amount(&tbtc_pair, total_tbtc_sats)?;

        // Validate against Boltz swap limits
        if fee_calc.invoice_sats < tbtc_pair.limits.minimal
            || fee_calc.invoice_sats > tbtc_pair.limits.maximal
        {
            return Err(BoltzError::AmountOutOfRange {
                amount: fee_calc.invoice_sats,
                min: tbtc_pair.limits.minimal,
                max: tbtc_pair.limits.maximal,
            });
        }

        let now = current_unix_timestamp();
        Ok(PreparedSwap {
            destination_address: destination.to_string(),
            destination_chain: chain.to_string(),
            asset,
            bridge_kind,
            output_amount,
            invoice_amount_sats: fee_calc.invoice_sats,
            boltz_fee_sats: fee_calc.boltz_fee_sats,
            estimated_onchain_amount: fee_calc.onchain_sats,
            slippage_bps,
            pair_hash: tbtc_pair.hash.clone(),
            expires_at: now.saturating_add(60),
        })
    }

    /// DEX quote: tBTC (EVM units) needed to buy `amount` of `token_in`
    /// (Arbitrum USDT or USDC), "out" direction (least tBTC for the output).
    async fn fetch_quote_out_tbtc_for_token(
        &self,
        amount: u64,
        token_in: &str,
    ) -> Result<u128, BoltzError> {
        let quotes = self
            .api_client
            .get_quote_out("ARB", ARBITRUM_TBTC_ADDRESS, token_in, u128::from(amount))
            .await?;
        let quote = pick_best_quote(&quotes, QuoteDirection::Out)?;
        if quote == 0 {
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero tBTC".to_string(),
            ));
        }
        Ok(quote)
    }

    /// DEX quote: amount of `token_out` (Arbitrum USDT or USDC) that
    /// `tbtc_evm_units` of tBTC buys, "in" direction.
    async fn fetch_quote_in_for_token(
        &self,
        tbtc_evm_units: u128,
        token_out: &str,
    ) -> Result<u128, BoltzError> {
        let quotes = self
            .api_client
            .get_quote_in("ARB", ARBITRUM_TBTC_ADDRESS, token_out, tbtc_evm_units)
            .await?;
        let amount = pick_best_quote(&quotes, QuoteDirection::In)?;
        if amount == 0 {
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero output".to_string(),
            ));
        }
        Ok(amount)
    }

    /// Prepare a reverse swap quote starting from input sats.
    ///
    /// Walks the route forward:
    /// 1. Apply the Boltz reverse fee to get onchain tBTC sats.
    /// 2. Same-chain: one DEX quote for how much USDT those tBTC sats buy.
    /// 3. Cross-chain: forward-quote tBTC → USDT; quote the OFT messaging
    ///    fee; convert the messaging fee to **tBTC sats** (not USDT) via
    ///    `quote_out(tBTC, ETH)`; subtract in tBTC-sat domain; re-quote
    ///    the DEX with the adjusted tBTC amount; then re-quote the OFT to
    ///    get the final destination receive amount.
    ///
    /// Subtracting the messaging fee in tBTC sats rather than in USDT
    /// keeps both legs on the same tBTC↔USDT / tBTC↔ETH DEX pools, so
    /// cross-chain quotes agree to the sat in both directions.
    pub async fn prepare_from_sats(
        &self,
        destination: &str,
        chain: &str,
        asset: Asset,
        invoice_amount_sats: u64,
        max_slippage_bps: Option<u32>,
    ) -> Result<PreparedSwap, BoltzError> {
        let slippage_bps = resolve_slippage_bps(max_slippage_bps, self.config.slippage_bps)?;

        let dest = self.resolve_destination(chain, asset)?;
        self.validate_destination(dest, destination)?;

        let bridge_kind = dest.bridge.kind();
        let tbtc_pair = self.fetch_tbtc_pair().await?;

        // Validate against Boltz swap limits
        if invoice_amount_sats < tbtc_pair.limits.minimal
            || invoice_amount_sats > tbtc_pair.limits.maximal
        {
            return Err(BoltzError::AmountOutOfRange {
                amount: invoice_amount_sats,
                min: tbtc_pair.limits.minimal,
                max: tbtc_pair.limits.maximal,
            });
        }

        let fee_calc = compute_onchain_amount(&tbtc_pair, invoice_amount_sats)?;

        // Convert onchain sats to tBTC EVM units
        let tbtc_evm_units = u128::from(fee_calc.onchain_sats)
            .checked_mul(u128::from(SATS_TO_TBTC_FACTOR))
            .ok_or_else(|| BoltzError::Generic("tBTC amount overflow".into()))?;

        let output = match &dest.bridge {
            // USDC (CCTP): the DEX produces Arbitrum USDC (the burned amount);
            // what lands on the destination is that minus the CCTP burn fee.
            Bridge::Cctp { domain } => {
                let burn_usdc = self
                    .fetch_quote_in_for_token(tbtc_evm_units, dest.dex_output_token)
                    .await?;
                let fee = self
                    .cctp_prepare_fee(dest.transport, destination, *domain)
                    .await?;
                let total_fee = cctp::compute_total_fee(burn_usdc, fee.bps_units, fee.forward_fee);
                let delivered = burn_usdc.saturating_sub(total_fee);
                if delivered == 0 {
                    return Err(BoltzError::Generic(
                        "Amount too small to cover the CCTP burn fee".into(),
                    ));
                }
                u64::try_from(delivered)
                    .map_err(|_| BoltzError::Generic("USDC amount overflow".into()))?
            }
            // Cross-chain OFT: forward DEX quote tBTC → USDT, quote OFT for the
            // messaging fee, then convert that fee to tBTC sats directly via a
            // second DEX quote and subtract in tBTC-sat domain.
            Bridge::Oft { .. } => {
                // LayerZero executor options — same reasoning as `prepare`: any
                // ATA-creation hint must feed every quote call.
                let extra_options = self.compute_extra_options(dest, destination).await?;
                let initial_usdt = self.fetch_quote_in_usdt(tbtc_evm_units).await?;
                let (msg_fee_native, _) = self
                    .quote_oft_messaging_fee(dest, u128::from(initial_usdt), &extra_options)
                    .await?;

                let msg_fee_tbtc_sats = if msg_fee_native == 0 {
                    0u64
                } else {
                    let tbtc_wei = self.fetch_quote_out_tbtc_for_eth(msg_fee_native).await?;
                    tbtc_wei_to_sats_u64(tbtc_wei)?
                };

                let adjusted_tbtc_sats = fee_calc
                    .onchain_sats
                    .checked_sub(msg_fee_tbtc_sats)
                    .ok_or_else(|| {
                        BoltzError::Generic(
                            "Amount too small to cover OFT cross-chain messaging fee".into(),
                        )
                    })?;
                if adjusted_tbtc_sats == 0 {
                    return Err(BoltzError::Generic(
                        "Amount too small to cover OFT cross-chain messaging fee".into(),
                    ));
                }

                // Re-quote the DEX with the adjusted tBTC claim amount to get
                // the USDT that actually arrives on Arbitrum, then re-quote the
                // OFT to translate that to the destination chain's USDT.
                let adjusted_tbtc_evm_units = u128::from(adjusted_tbtc_sats)
                    .checked_mul(u128::from(SATS_TO_TBTC_FACTOR))
                    .ok_or_else(|| BoltzError::Generic("tBTC amount overflow".into()))?;
                let adjusted_usdt = self.fetch_quote_in_usdt(adjusted_tbtc_evm_units).await?;
                let (_, oft_received) = self
                    .quote_oft_messaging_fee(dest, u128::from(adjusted_usdt), &extra_options)
                    .await?;
                u64::try_from(oft_received)
                    .map_err(|_| BoltzError::Generic("USDT amount overflow".into()))?
            }
            // Direct (same-chain) delivery: single forward DEX quote
            // tBTC → output token (USDT or USDC on Arbitrum).
            Bridge::Direct => {
                let out = self
                    .fetch_quote_in_for_token(tbtc_evm_units, dest.dex_output_token)
                    .await?;
                u64::try_from(out)
                    .map_err(|_| BoltzError::Generic("output amount overflow".into()))?
            }
        };

        let now = current_unix_timestamp();
        Ok(PreparedSwap {
            destination_address: destination.to_string(),
            destination_chain: chain.to_string(),
            asset,
            bridge_kind,
            output_amount: output,
            invoice_amount_sats,
            boltz_fee_sats: fee_calc.boltz_fee_sats,
            estimated_onchain_amount: fee_calc.onchain_sats,

            slippage_bps,
            pair_hash: tbtc_pair.hash.clone(),
            expires_at: now.saturating_add(60),
        })
    }

    /// Call the Boltz API to create a reverse swap. The [`SecretProvider`]
    /// mints this swap's preimage hash and claim key (reserving an HD index in
    /// seeded mode, generating random secrets in seedless mode). Returns the
    /// validated `BoltzSwap`; the caller handles persistence.
    pub async fn create(&self, prepared: &PreparedSwap) -> Result<BoltzSwap, BoltzError> {
        if current_unix_timestamp() >= prepared.expires_at {
            return Err(BoltzError::QuoteExpired);
        }

        // Re-validate the caller-supplied `PreparedSwap` against the registry
        // and bounds. `prepare()` validated these, but `PreparedSwap` is public
        // and its fields are too, so a hand-built or stale value could carry a
        // destination not in the registry, an address the per-transport encoder
        // rejects, or out-of-range slippage. Fail fast here — before the Boltz
        // create call issues an invoice — rather than surfacing only at claim
        // time (after the LN HTLC may already be funded).
        let dest = self.resolve_destination(&prepared.destination_chain, prepared.asset)?;
        self.validate_destination(dest, &prepared.destination_address)?;
        resolve_slippage_bps(Some(prepared.slippage_bps), self.config.slippage_bps)?;

        let chain_id_u32 = to_chain_id_u32(self.config.chain_id)?;
        let issued = self.secrets.issue(chain_id_u32).await?;

        let create_req = crate::api::types::CreateReverseSwapRequest {
            from: "BTC".to_string(),
            to: "TBTC".to_string(),
            preimage_hash: hex::encode(issued.preimage_hash),
            claim_address: issued.claim_address.clone(),
            invoice_amount: prepared.invoice_amount_sats,
            pair_hash: prepared.pair_hash.clone(),
            referral_id: self.config.referral_id.clone(),
            claim_public_key: hex::encode(&issued.claim_public_key),
            description: None,
            invoice_expiry: None,
        };

        let resp = self
            .api_client
            .create_reverse_swap(&create_req)
            .await
            .map_err(|e| match e {
                BoltzError::Api {
                    code: Some(409), ..
                } => BoltzError::DuplicatePreimage,
                other => other,
            })?;

        // The swap id is server-controlled and later interpolated into request
        // URLs; validate it as a plain alphanumeric token at the point it first
        // enters, so a hostile id can't reshape a request path downstream.
        validate_swap_id(&resp.id)?;

        if resp.onchain_amount < prepared.estimated_onchain_amount {
            return Err(BoltzError::Generic(format!(
                "Boltz onchain_amount ({}) less than prepared estimate ({})",
                resp.onchain_amount, prepared.estimated_onchain_amount,
            )));
        }

        // Validate lockup address matches the expected ERC20Swap contract.
        if resp.lockup_address.to_lowercase() != self.erc20swap_address.to_lowercase() {
            return Err(BoltzError::Generic(format!(
                "Boltz lockup_address ({}) does not match expected ERC20Swap ({})",
                resp.lockup_address, self.erc20swap_address,
            )));
        }

        // Parse the BOLT11 invoice and verify the amount matches what we
        // requested. A malicious Boltz server could return an invoice with a
        // higher amount, causing the user to overpay on Lightning.
        let decoded_invoice: Bolt11Invoice = resp
            .invoice
            .parse()
            .map_err(|e| BoltzError::Generic(format!("Failed to parse BOLT11 invoice: {e}")))?;
        let decoded_amount_sats = decoded_invoice
            .amount_milli_satoshis()
            .ok_or_else(|| BoltzError::Generic("BOLT11 invoice missing amount".to_string()))?
            / 1000;
        if decoded_amount_sats != prepared.invoice_amount_sats {
            return Err(BoltzError::Generic(format!(
                "Invoice amount ({decoded_amount_sats} sats) does not match requested amount ({} sats)",
                prepared.invoice_amount_sats,
            )));
        }

        // Early abort: reject an unreasonably short lockup timeout before the
        // invoice is returned, so the user never commits sats to a swap that
        // could let Boltz refund before we can claim. This is a UX fast-fail;
        // the load-bearing protection is re-checked against the live block
        // height immediately before the preimage is revealed at claim time
        // (`claim_and_swap` -> `ensure_timeout_margin`).
        self.ensure_timeout_margin(resp.timeout_block_height)
            .await?;

        let now = current_unix_timestamp();
        Ok(BoltzSwap {
            id: resp.id,
            status: BoltzSwapStatus::Created,
            bridge_kind: prepared.bridge_kind,
            key_source: issued.key_source,
            chain_id: self.config.chain_id,
            claim_address: issued.claim_address,
            destination_address: prepared.destination_address.clone(),
            destination_chain: prepared.destination_chain.clone(),
            asset: prepared.asset,
            refund_address: resp.refund_address.ok_or_else(|| BoltzError::Api {
                reason: "Missing refund_address in swap response".to_string(),
                code: None,
            })?,
            erc20swap_address: self.erc20swap_address.clone(),
            router_address: ARBITRUM_ROUTER_ADDRESS.to_string(),
            invoice: resp.invoice,
            invoice_amount_sats: prepared.invoice_amount_sats,
            onchain_amount: resp.onchain_amount,
            expected_output_amount: prepared.output_amount,
            slippage_bps: prepared.slippage_bps,
            timeout_block_height: resp.timeout_block_height,
            lockup_tx_id: None,
            claim_tx_hash: None,
            pending_call_id: None,
            delivered_amount: None,
            bridge_ref: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Create a throwaway hold invoice for Lightning fee estimation.
    ///
    /// Returns just the BOLT11 invoice string. The preimage is freshly
    /// random and **immediately discarded**, so the invoice cannot be
    /// claimed and **must not be paid**. Useful when the caller needs an
    /// LN routing fee estimate against a real BOLT11 invoice without
    /// committing to a real swap.
    ///
    /// Compared to [`create`](Self::create) this skips:
    /// - HD key index consumption (random preimage hash, no derivation)
    /// - Local-store writes (no `BoltzSwap` is constructed or returned)
    /// - WS swap tracking (caller is expected not to subscribe)
    ///
    /// Sets `invoiceExpiry` to [`PROBE_INVOICE_EXPIRY_SECS`] so the
    /// unfunded swap's server-side state on Boltz self-clears as quickly
    /// as the API allows.
    ///
    /// The `claim_address` and `claim_public_key` fields use a throwaway random
    /// key — Boltz only needs a valid secp256k1 point and EVM address there, and
    /// since the swap will never be claimed, the relationship between those and
    /// the (random) preimage hash is irrelevant. This keeps the probe
    /// independent of the service's secret mode (no seed required).
    pub async fn create_probe_invoice(
        &self,
        prepared: &PreparedSwap,
    ) -> Result<String, BoltzError> {
        if current_unix_timestamp() >= prepared.expires_at {
            return Err(BoltzError::QuoteExpired);
        }
        let throwaway = EvmKeyPair::generate()?;

        // 32 random bytes — no derivation, no recoverability needed: the
        // invoice will never be paid, so a recoverable preimage would serve no
        // purpose.
        let mut preimage_hash = [0u8; 32];
        getrandom::getrandom(&mut preimage_hash).map_err(|e| {
            BoltzError::Generic(format!("Failed to generate random preimage hash: {e}"))
        })?;

        let create_req = crate::api::types::CreateReverseSwapRequest {
            from: "BTC".to_string(),
            to: "TBTC".to_string(),
            preimage_hash: hex::encode(preimage_hash),
            claim_address: throwaway.address_hex(),
            invoice_amount: prepared.invoice_amount_sats,
            pair_hash: prepared.pair_hash.clone(),
            referral_id: self.config.referral_id.clone(),
            claim_public_key: hex::encode(&throwaway.public_key),
            description: None,
            invoice_expiry: Some(PROBE_INVOICE_EXPIRY_SECS),
        };

        let resp = self
            .api_client
            .create_reverse_swap(&create_req)
            .await
            .map_err(|e| match e {
                BoltzError::Api {
                    code: Some(409), ..
                } => BoltzError::DuplicatePreimage,
                other => other,
            })?;

        // Validate the returned invoice amount matches what we asked for —
        // a malicious server could otherwise return an invoice for a
        // different amount, leading to a wrong fee estimate.
        let decoded_invoice: Bolt11Invoice = resp
            .invoice
            .parse()
            .map_err(|e| BoltzError::Generic(format!("Failed to parse BOLT11 invoice: {e}")))?;
        let decoded_amount_sats = decoded_invoice
            .amount_milli_satoshis()
            .ok_or_else(|| BoltzError::Generic("BOLT11 invoice missing amount".to_string()))?
            / 1000;
        if decoded_amount_sats != prepared.invoice_amount_sats {
            return Err(BoltzError::Generic(format!(
                "Invoice amount ({decoded_amount_sats} sats) does not match requested amount ({} sats)",
                prepared.invoice_amount_sats,
            )));
        }

        Ok(resp.invoice)
    }

    // ─── Internal ────────────────────────────────────────────────────────

    async fn fetch_tbtc_pair(&self) -> Result<ReversePairInfo, BoltzError> {
        let pairs = self.api_client.get_reverse_swap_pairs().await?;
        let pair = pairs
            .0
            .get("BTC")
            .and_then(|m| m.get("TBTC"))
            .cloned()
            .ok_or_else(|| BoltzError::Api {
                reason: "BTC/TBTC pair not found. Is referral header configured?".to_string(),
                code: None,
            })?;
        if pair.limits.minimal > pair.limits.maximal {
            return Err(BoltzError::Api {
                reason: format!(
                    "Invalid pair limits: minimal ({}) > maximal ({})",
                    pair.limits.minimal, pair.limits.maximal,
                ),
                code: None,
            });
        }
        Ok(pair)
    }

    async fn fetch_quote_out_tbtc(&self, usdt_amount: u64) -> Result<u128, BoltzError> {
        let quotes = self
            .api_client
            .get_quote_out(
                "ARB",
                ARBITRUM_TBTC_ADDRESS,
                ARBITRUM_USDT_ADDRESS,
                u128::from(usdt_amount),
            )
            .await?;
        // "out" direction: pick the lowest amount (least input needed for
        // the desired output).
        let quote = pick_best_quote(&quotes, QuoteDirection::Out)?;
        if quote == 0 {
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero tBTC".to_string(),
            ));
        }
        Ok(quote)
    }

    async fn fetch_quote_in_usdt(&self, tbtc_evm_units: u128) -> Result<u64, BoltzError> {
        let quotes = self
            .api_client
            .get_quote_in(
                "ARB",
                ARBITRUM_TBTC_ADDRESS,
                ARBITRUM_USDT_ADDRESS,
                tbtc_evm_units,
            )
            .await?;
        let amount = pick_best_quote(&quotes, QuoteDirection::In)?;
        if amount == 0 {
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero USDT".to_string(),
            ));
        }
        amount
            .try_into()
            .map_err(|_| BoltzError::Generic("USDT amount overflow".into()))
    }

    /// Reject a swap whose lockup `timeout_block_height` leaves too little
    /// headroom before the current block, narrowing the preimage-exposure
    /// window (see [`MIN_TIMEOUT_L1_MARGIN`]).
    ///
    /// `timeout_block_height` is denominated in **L1** block height, so this
    /// fetches the current L1 height (NOT the L2 `eth_blockNumber`) and requires
    /// at least `MIN_TIMEOUT_L1_MARGIN` blocks of margin. **Fail-closed:** an
    /// RPC error rejects the swap rather than proceeding blind, because the
    /// whole point is to guard the preimage reveal against a malicious/buggy
    /// server returning a too-short timeout to win a refund-vs-claim race.
    async fn ensure_timeout_margin(&self, timeout_block_height: u64) -> Result<(), BoltzError> {
        let current_l1 = self.evm_provider.eth_l1_block_number().await?;
        check_timeout_margin(timeout_block_height, current_l1)
    }

    /// Claim tBTC locked on-chain and swap to the output token (USDT or USDC).
    /// Returns the claim tx hash on success.
    #[expect(clippy::too_many_lines)]
    pub(crate) async fn claim_and_swap(
        &self,
        swap: &BoltzSwap,
        skip_drift_check: bool,
    ) -> Result<String, BoltzError> {
        // Verify funds are actually locked on-chain BEFORE deriving the
        // preimage. Without this check, a fraudulent `transaction.confirmed` WS
        // message would cause us to reveal the preimage in (reverted) calldata,
        // allowing an attacker to settle the Lightning HTLC.
        //
        // Retry a few times: the public RPC endpoint may lag behind Boltz's node
        // that triggered the `transaction.confirmed` WS event.
        let mut lockup_verified = false;
        for attempt in 0..LOCKUP_CHECK_MAX_ATTEMPTS {
            match is_swap_still_locked_by_swap(&self.evm_provider, swap, self.secrets.as_ref())
                .await
            {
                Ok(true) => {
                    lockup_verified = true;
                    break;
                }
                Ok(false) => {
                    tracing::debug!(
                        swap_id = swap.id,
                        attempt,
                        "Lockup not yet visible on-chain, retrying"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        swap_id = swap.id,
                        attempt,
                        error = %e,
                        "Lockup check RPC error, retrying"
                    );
                }
            }
            if attempt < LOCKUP_CHECK_MAX_ATTEMPTS.saturating_sub(1) {
                sleep_1s().await;
            }
        }
        if !lockup_verified {
            return Err(BoltzError::Generic(
                "On-chain lockup check failed: funds are not locked in ERC20Swap contract"
                    .to_string(),
            ));
        }

        // Load-bearing, fail-safe gate: confirm the lockup timeout still leaves
        // enough block headroom BEFORE revealing the preimage. The lockup check
        // above only proves the funds are locked *right now*; it cannot prove
        // the lock survives until our claim mines. Revealing the preimage with a
        // near-expired timeout lets a malicious/buggy server refund and then
        // settle the LN HTLC with the leaked preimage. Aborting here is loss-
        // free: the preimage stays secret and the LN payment refunds on timeout.
        self.ensure_timeout_margin(swap.timeout_block_height)
            .await?;

        let preimage = self.secrets.preimage(swap)?;
        let gas_key_pair = self.secrets.gas_key_pair(swap)?;
        let gas_signer = EvmSigner::new(&gas_key_pair, swap.chain_id);
        let erc20swap_version = self
            .fetch_erc20swap_version(&swap.erc20swap_address)
            .await?;

        // Addresses (including the DEX output token) come from the resolved
        // destination, which covers every bridge uniformly.
        let addrs = ClaimAddresses::parse(swap, &self.chain_registry)?;
        let tbtc_evm_amount = U256::from(swap.onchain_amount)
            .checked_mul(U256::from(SATS_TO_TBTC_FACTOR))
            .ok_or_else(|| BoltzError::Generic("tBTC EVM amount overflow".into()))?;
        let timelock = U256::from(swap.timeout_block_height);

        for attempt in 0..MAX_CLAIM_RETRIES {
            if attempt > 0 {
                tracing::info!(attempt, swap_id = swap.id, "Retrying claim");
            }

            let result = self
                .try_claim(
                    swap,
                    &gas_signer,
                    &erc20swap_version,
                    &preimage,
                    &addrs,
                    tbtc_evm_amount,
                    timelock,
                    skip_drift_check,
                )
                .await;

            match result {
                Ok(tx_hash) => {
                    return Ok(tx_hash);
                }
                Err(e) => {
                    // Quote drift is not transient — retrying immediately won't
                    // help. Return the error without revealing the preimage; the
                    // caller (`SwapManager::do_claim`) reverts the swap to
                    // `TbtcLocked` so the consumer can accept the new rate via
                    // `accept_degraded_quote`.
                    if matches!(e, BoltzError::QuoteDegradedBeyondSlippage { .. }) {
                        return Err(e);
                    }

                    // The claim was already broadcast (preimage public) but the
                    // confirmation poll failed. Re-submitting would broadcast a
                    // second, reverting claim. Stop here; the call_id is
                    // persisted and the swap stays in `Claiming`. Boltz's
                    // `invoice.settled` WS event then drives `resume_pending_call`
                    // to re-resolve the tx hash (falling back to the on-chain
                    // lock-state check in `check_on_chain_and_retry`).
                    if matches!(e, BoltzError::ClaimBroadcastUnconfirmed { .. }) {
                        tracing::warn!(
                            attempt,
                            swap_id = swap.id,
                            error = %e,
                            "Claim broadcast but unconfirmed; not re-submitting"
                        );
                        return Err(e);
                    }

                    tracing::warn!(attempt, swap_id = swap.id, error = %e, "Claim attempt failed");

                    // Check if funds are still locked on-chain. If not, stop
                    // retrying — the swap was either claimed by another instance
                    // or refunded by Boltz. Don't mark success or failure here;
                    // the WS update will determine the final state.
                    match is_swap_still_locked_by_swap(
                        &self.evm_provider,
                        swap,
                        self.secrets.as_ref(),
                    )
                    .await
                    {
                        Ok(false) => {
                            tracing::info!(
                                swap_id = swap.id,
                                "Funds no longer locked on-chain, stopping retries"
                            );
                            return Err(e);
                        }
                        Ok(true) => {} // Still locked, worth retrying.
                        Err(check_err) => {
                            tracing::warn!(
                                swap_id = swap.id,
                                error = %check_err,
                                "On-chain lock check failed, continuing with retry"
                            );
                        }
                    }

                    if attempt >= MAX_CLAIM_RETRIES.saturating_sub(1) {
                        return Err(e);
                    }
                    // Exponential backoff: 1s, 2s, 4s, 8s, ...
                    let delay_secs = 2u64.pow(attempt);
                    platform_utils::tokio::time::sleep(platform_utils::time::Duration::from_secs(
                        delay_secs,
                    ))
                    .await;
                }
            }
        }
        unreachable!("loop exits via return")
    }

    #[expect(clippy::too_many_arguments)]
    async fn try_claim(
        &self,
        swap: &BoltzSwap,
        gas_signer: &EvmSigner,
        erc20swap_version: &str,
        preimage: &[u8; 32],
        addrs: &ClaimAddresses,
        tbtc_evm_amount: U256,
        timelock: U256,
        skip_drift_check: bool,
    ) -> Result<String, BoltzError> {
        // Dispatch on the resolved destination's bridge — the single source of
        // truth for how this swap is delivered.
        let dest = self.resolve_destination(&swap.destination_chain, swap.asset)?;
        match dest.bridge {
            Bridge::Cctp { domain } => {
                self.try_claim_cctp(
                    swap,
                    domain,
                    dest.transport,
                    gas_signer,
                    erc20swap_version,
                    preimage,
                    addrs,
                    tbtc_evm_amount,
                    timelock,
                    skip_drift_check,
                )
                .await
            }
            Bridge::Direct => {
                self.try_claim_same_chain(
                    swap,
                    gas_signer,
                    erc20swap_version,
                    preimage,
                    addrs,
                    tbtc_evm_amount,
                    timelock,
                    skip_drift_check,
                )
                .await
            }
            Bridge::Oft { .. } => {
                self.try_claim_cross_chain(
                    swap,
                    gas_signer,
                    erc20swap_version,
                    preimage,
                    addrs,
                    tbtc_evm_amount,
                    timelock,
                    skip_drift_check,
                )
                .await
            }
        }
    }

    /// Direct claim: claim tBTC + DEX swap to the output token (USDT or USDC)
    /// + sweep to the destination on Arbitrum. No cross-chain bridge.
    #[expect(clippy::too_many_arguments)]
    async fn try_claim_same_chain(
        &self,
        swap: &BoltzSwap,
        gas_signer: &EvmSigner,
        erc20swap_version: &str,
        preimage: &[u8; 32],
        addrs: &ClaimAddresses,
        tbtc_evm_amount: U256,
        timelock: U256,
        skip_drift_check: bool,
    ) -> Result<String, BoltzError> {
        let amount_in: u128 = tbtc_evm_amount
            .try_into()
            .map_err(|_| BoltzError::Generic("tBTC amount too large".into()))?;

        let quotes = self
            .api_client
            .get_quote_in("ARB", ARBITRUM_TBTC_ADDRESS, addrs.output_token, amount_in)
            .await?;
        let best = pick_best_quote_with_data(&quotes, QuoteDirection::In)?;
        if best.amount == 0 {
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero output".into(),
            ));
        }
        let raw_output_amount = best.amount;

        if !skip_drift_check {
            check_quote_drift(
                swap.expected_output_amount,
                raw_output_amount,
                swap.slippage_bps,
            )?;
        }

        let min_amount_out_u128 = compute_claim_floor(
            raw_output_amount,
            swap.expected_output_amount,
            swap.slippage_bps,
            skip_drift_check,
        )?;
        if min_amount_out_u128 == 0 {
            return Err(BoltzError::Generic(
                "Amount too small: slippage-adjusted minimum is zero".into(),
            ));
        }
        let min_amount_out = U256::from(min_amount_out_u128);

        let encode_resp = self
            .api_client
            .encode_quote(
                "ARB",
                &EncodeRequest {
                    recipient: addrs.router.to_string(),
                    amount_in,
                    amount_out_min: min_amount_out_u128,
                    data: best.data.clone(),
                },
            )
            .await?;
        let dex_calls: Vec<contracts::Call> = encode_resp
            .calls
            .iter()
            .map(quote_calldata_to_call)
            .collect::<Result<Vec<_>, _>>()?;

        // Same-chain claim is Arbitrum-only, which is always EVM, so the
        // destination must round-trip as a 20-byte EVM address. Surface a
        // hard error instead of panicking if the invariant is ever violated.
        let destination_evm = addrs.destination_evm.ok_or_else(|| {
            BoltzError::Generic("Same-chain claim requires an EVM destination address".to_string())
        })?;

        let erc20swap_sig = gas_signer.sign_eip712_erc20swap_claim(
            addrs.erc20swap,
            erc20swap_version,
            preimage,
            tbtc_evm_amount,
            addrs.tbtc,
            addrs.refund,
            timelock,
            addrs.router,
        )?;

        let router_sig = gas_signer.sign_eip712_router_claim(
            addrs.router,
            preimage,
            addrs.output_token_address,
            min_amount_out,
            destination_evm,
        )?;

        let erc20_claim = Erc20Claim {
            preimage: (*preimage).into(),
            amount: tbtc_evm_amount,
            tokenAddress: addrs.tbtc,
            refundAddress: addrs.refund,
            timelock,
            v: erc20swap_sig.v,
            r: erc20swap_sig.r.into(),
            s: erc20swap_sig.s.into(),
        };

        let calldata = encode_claim_erc20_execute(
            &erc20_claim,
            &dex_calls,
            addrs.output_token_address,
            min_amount_out,
            destination_evm,
            router_sig.v,
            router_sig.r,
            router_sig.s,
        );

        self.submit_claim(swap, &swap.router_address.clone(), &calldata, gas_signer)
            .await
    }

    /// Cross-chain CCTP claim: claim tBTC + DEX swap to USDC + CCTP burn to the
    /// destination chain, all in one atomic Router call.
    ///
    /// End-to-end slippage: `delivered = burn - cctpFee`. The Router enforces a
    /// floor (`minAmount`) on the USDC available to burn after the DEX calls;
    /// we set `minAmount = delivered_floor + maxFee` so that, since the burn
    /// deducts at most `maxFee`, the delivered amount stays at or above the
    /// single end-to-end floor anchored on the prepare-time expected amount.
    #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn try_claim_cctp(
        &self,
        swap: &BoltzSwap,
        domain: u32,
        transport: NetworkTransport,
        gas_signer: &EvmSigner,
        erc20swap_version: &str,
        preimage: &[u8; 32],
        addrs: &ClaimAddresses,
        tbtc_evm_amount: U256,
        timelock: U256,
        skip_drift_check: bool,
    ) -> Result<String, BoltzError> {
        let amount_in: u128 = tbtc_evm_amount
            .try_into()
            .map_err(|_| BoltzError::Generic("tBTC amount too large".into()))?;

        // DEX quote: tBTC -> Arbitrum USDC.
        let quotes = self
            .api_client
            .get_quote_in(
                "ARB",
                ARBITRUM_TBTC_ADDRESS,
                ARBITRUM_USDC_ADDRESS,
                amount_in,
            )
            .await?;
        let best = pick_best_quote_with_data(&quotes, QuoteDirection::In)?;
        if best.amount == 0 {
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero USDC".into(),
            ));
        }
        let raw_quote_usdc = best.amount;

        // Re-quote the CCTP burn fee at claim time and cap it with a buffer.
        // The recipient-setup decision is taken once here and reused for both
        // the fee query and the hook bytes so they can't drift apart.
        let needs_recipient_setup = self
            .cctp_needs_recipient_setup(transport, &swap.destination_address)
            .await?;
        let fee = self
            .cctp_fee_client
            .get_fee(
                CCTP_ARBITRUM_DOMAIN,
                domain,
                CCTP_FINALITY_FAST,
                needs_recipient_setup,
            )
            .await?;
        let max_fee = cctp::add_fee_buffer(cctp::compute_total_fee(
            raw_quote_usdc,
            fee.bps_units,
            fee.forward_fee,
        ));

        // The amount that would actually land on the destination = burn - fee.
        let net_quote = raw_quote_usdc.saturating_sub(max_fee);
        if !skip_drift_check {
            check_quote_drift(swap.expected_output_amount, net_quote, swap.slippage_bps)?;
        }
        let delivered_floor = compute_claim_floor(
            net_quote,
            swap.expected_output_amount,
            swap.slippage_bps,
            skip_drift_check,
        )?;
        if delivered_floor == 0 {
            return Err(BoltzError::Generic(
                "Amount too small: slippage-adjusted minimum is zero".into(),
            ));
        }
        // Floor the burn so that delivered (= burn - maxFee) >= delivered_floor.
        let min_amount = delivered_floor
            .checked_add(max_fee)
            .ok_or_else(|| BoltzError::Generic("CCTP min amount overflow".into()))?;

        // Encode the DEX trade routing the USDC output to the Router.
        let encode_resp = self
            .api_client
            .encode_quote(
                "ARB",
                &EncodeRequest {
                    recipient: addrs.router.to_string(),
                    amount_in,
                    amount_out_min: min_amount,
                    data: best.data.clone(),
                },
            )
            .await?;
        let dex_calls: Vec<contracts::Call> = encode_resp
            .calls
            .iter()
            .map(quote_calldata_to_call)
            .collect::<Result<Vec<_>, _>>()?;

        // Build CctpData.
        let token_messenger = parse_address(CCTP_TOKEN_MESSENGER_V2)?;
        let mint_recipient = match transport {
            NetworkTransport::Evm => cctp::evm_mint_recipient(&swap.destination_address)?,
            NetworkTransport::Solana => cctp::solana_mint_recipient(&swap.destination_address)?,
            NetworkTransport::Tron => {
                return Err(BoltzError::Generic(
                    "CCTP does not support Tron destinations".into(),
                ));
            }
        };
        let hook_data = cctp_forward_hook(&swap.destination_address, needs_recipient_setup)?;

        let cctp_data = CctpData {
            destinationDomain: domain,
            mintRecipient: mint_recipient,
            destinationCaller: FixedBytes::<32>::ZERO,
            maxFee: U256::from(max_fee),
            minFinalityThreshold: CCTP_FINALITY_FAST,
            hookData: hook_data.into(),
        };

        let typehash = self.fetch_typehash_cctp_data(&swap.router_address).await?;
        let cctp_data_hash = hash_cctp_data(typehash, &cctp_data);

        // Sign: the ERC20Swap cooperative claim (identical to OFT) + the Router
        // ClaimCctp authorization.
        let erc20swap_sig = gas_signer.sign_eip712_erc20swap_claim(
            addrs.erc20swap,
            erc20swap_version,
            preimage,
            tbtc_evm_amount,
            addrs.tbtc,
            addrs.refund,
            timelock,
            addrs.router,
        )?;
        let router_sig = gas_signer.sign_eip712_router_claim_cctp(
            addrs.router,
            preimage,
            addrs.output_token_address, // = Arbitrum USDC for CCTP swaps
            token_messenger,
            cctp_data_hash,
            U256::from(min_amount),
        )?;

        let erc20_claim = Erc20Claim {
            preimage: (*preimage).into(),
            amount: tbtc_evm_amount,
            tokenAddress: addrs.tbtc,
            refundAddress: addrs.refund,
            timelock,
            v: erc20swap_sig.v,
            r: erc20swap_sig.r.into(),
            s: erc20swap_sig.s.into(),
        };
        let auth = ClaimCctpAuthorization {
            minAmount: U256::from(min_amount),
            v: router_sig.v,
            r: router_sig.r.into(),
            s: router_sig.s.into(),
        };

        let calldata = encode_claim_erc20_execute_cctp(
            &erc20_claim,
            &dex_calls,
            addrs.output_token_address,
            token_messenger,
            &cctp_data,
            &auth,
        );

        self.submit_claim(swap, &swap.router_address.clone(), &calldata, gas_signer)
            .await
    }

    /// Choose the CCTP forwarding `hookData` for a destination. EVM uses the
    /// static forward tag. Solana uses the static tag when the recipient's USDC
    /// ATA already exists, or the ATA-creating variant (carrying the wallet)
    /// when it does not.
    /// Whether the CCTP forwarding hook must also create the destination
    /// recipient's token account: true only for a Solana USDC destination whose
    /// associated token account doesn't exist yet. This single decision drives
    /// BOTH the Iris fee query (`includeRecipientSetup`) and the hook bytes
    /// ([`cctp_forward_hook`]), keeping the quoted `maxFee` and the on-chain
    /// hook in lockstep. Mirrors the web app's `shouldCreateSolanaTokenAccount`.
    async fn cctp_needs_recipient_setup(
        &self,
        transport: NetworkTransport,
        destination: &str,
    ) -> Result<bool, BoltzError> {
        if transport != NetworkTransport::Solana {
            return Ok(false);
        }
        let ata = cctp::solana_mint_recipient(destination)?;
        let ata_base58 = bs58::encode(ata.as_slice()).into_string();
        Ok(!self.solana_rpc.account_exists(&ata_base58).await?)
    }

    /// Fetch the Iris burn fee for a CCTP `destination` on `domain`, requesting
    /// the recipient-setup tier when a first-time Solana ATA must be created.
    /// Shared by both prepare paths so the fee query stays consistent.
    async fn cctp_prepare_fee(
        &self,
        transport: NetworkTransport,
        destination: &str,
        domain: u32,
    ) -> Result<cctp::CctpFee, BoltzError> {
        let needs_recipient_setup = self
            .cctp_needs_recipient_setup(transport, destination)
            .await?;
        self.cctp_fee_client
            .get_fee(
                CCTP_ARBITRUM_DOMAIN,
                domain,
                CCTP_FINALITY_FAST,
                needs_recipient_setup,
            )
            .await
    }

    /// Cross-chain claim: claim tBTC + DEX swap to USDT + OFT bridge to destination chain.
    ///
    /// Two-pass approach:
    /// - Pass 1: estimate `LayerZero` messaging fee cost in tBTC
    /// - Pass 2: re-quote with adjusted tBTC split (trade vs fee)
    #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn try_claim_cross_chain(
        &self,
        swap: &BoltzSwap,
        gas_signer: &EvmSigner,
        erc20swap_version: &str,
        preimage: &[u8; 32],
        addrs: &ClaimAddresses,
        tbtc_evm_amount: U256,
        timelock: U256,
        skip_drift_check: bool,
    ) -> Result<String, BoltzError> {
        let dst_info = self.resolve_destination(&swap.destination_chain, swap.asset)?;
        let (mesh, dst_eid) = dst_info.oft().ok_or_else(|| {
            BoltzError::Generic(format!(
                "Destination '{}' is not an OFT route",
                swap.destination_chain
            ))
        })?;

        // The legacy and native USDT0 meshes deploy distinct OFT contracts
        // on the source chain — pick the one matching the destination's mesh
        // so quoting and sending bridge through the right contract.
        let source_oft_address = self.chain_registry.oft_for(mesh).ok_or_else(|| {
            BoltzError::Generic(format!("Source chain has no {mesh:?} mesh OFT deployment"))
        })?;
        let oft_addr = parse_address(source_oft_address)?;

        let tbtc_amount: u128 = tbtc_evm_amount
            .try_into()
            .map_err(|_| BoltzError::Generic("tBTC amount too large".into()))?;
        let router_str = addrs.router.to_string();

        // Compute the LayerZero executor options once for this claim. Must be
        // the same bytes on every subsequent `quoteOFT` / `quoteSend` call and
        // in the `SendData` that the router signs over, or the on-chain
        // `claimERC20ExecuteOft` signature verification fails. For Solana
        // destinations whose ATA is missing, this carries the `lzReceive`
        // option with `solanaAtaRentExemptLamports` so the destination
        // executor creates the recipient's token account on arrival.
        let extra_options = self
            .compute_extra_options(dst_info, &swap.destination_address)
            .await?;

        // ─── Pass 1: estimate LZ fee cost ─────────────────────────────
        // Get initial DEX quote with full tBTC to estimate USDT output
        let initial_trade = pick_best_quote_with_data(
            &self
                .api_client
                .get_quote_in(
                    "ARB",
                    ARBITRUM_TBTC_ADDRESS,
                    ARBITRUM_USDT_ADDRESS,
                    tbtc_amount,
                )
                .await?,
            QuoteDirection::In,
        )?;

        // Quote OFT with initial USDT amount to get messaging fee
        let initial_send_param = contracts::build_oft_send_param(
            dst_eid,
            addrs.destination_bytes32,
            U256::from(initial_trade.amount),
            U256::ZERO,
            extra_options.clone(),
        );
        let (_, initial_receipt) = self
            .quote_oft(source_oft_address, &initial_send_param)
            .await?;

        let mut quoted_send_param = initial_send_param.clone();
        quoted_send_param.minAmountLD = initial_receipt.amountReceivedLD;
        let msg_fee = self
            .quote_send(source_oft_address, &quoted_send_param)
            .await?;

        // Buffer the messaging fee against Pass-1 → execution drift in the
        // LayerZero native fee. The user's slippage knob doubles as the
        // buffer size: it determines how much fee variance we absorb before
        // reverting. Crucially this does NOT affect the user-facing
        // promise — `min_amount_ld_slipped` below is anchored on
        // `expected_output_amount`, so a bigger buffer just shrinks
        // `raw_dest_amount` and is caught by the drift check, never by
        // delivering less than `expected × (1 − s)`.
        let native_fee: u128 = msg_fee
            .nativeFee
            .try_into()
            .map_err(|_| BoltzError::Generic("LZ fee too large".into()))?;
        let fee_with_slippage = apply_slippage_up(native_fee, u128::from(swap.slippage_bps))?;

        // Quote DEX: how much tBTC for the ETH messaging fee
        let fee_dex = pick_best_quote_with_data(
            &self
                .api_client
                .get_quote_out(
                    "ARB",
                    ARBITRUM_TBTC_ADDRESS,
                    ZERO_ADDRESS,
                    fee_with_slippage,
                )
                .await?,
            QuoteDirection::Out,
        )?;

        // ─── Pass 2: final quotes with adjusted tBTC split ────────────
        let fee_tbtc = fee_dex.amount;
        let trade_tbtc = tbtc_amount.checked_sub(fee_tbtc).ok_or_else(|| {
            BoltzError::Generic("Amount too small to cover OFT cross-chain messaging fee".into())
        })?;
        if trade_tbtc == 0 {
            return Err(BoltzError::Generic(
                "Amount too small to cover OFT cross-chain messaging fee".into(),
            ));
        }

        // Final trade DEX quote: trade_tbtc -> USDT
        let trade_best = pick_best_quote_with_data(
            &self
                .api_client
                .get_quote_in(
                    "ARB",
                    ARBITRUM_TBTC_ADDRESS,
                    ARBITRUM_USDT_ADDRESS,
                    trade_tbtc,
                )
                .await?,
            QuoteDirection::In,
        )?;
        if trade_best.amount == 0 {
            return Err(BoltzError::InvalidQuote("DEX returned zero USDT".into()));
        }

        // ─── End-to-end OFT floor ────────────────────────────────────
        // Quote OFT once with the raw trade amount to learn what the user
        // would actually receive on the destination chain. This single
        // value drives both the drift abort and the on-chain `minAmountLd`
        // floor — slippage is applied exactly once, end-to-end, against
        // the destination amount the user perceives.
        let oft_quote_param = contracts::build_oft_send_param(
            dst_eid,
            addrs.destination_bytes32,
            U256::from(trade_best.amount),
            U256::ZERO,
            extra_options.clone(),
        );
        let (_, oft_receipt) = self.quote_oft(source_oft_address, &oft_quote_param).await?;
        let raw_dest_amount: u128 = oft_receipt
            .amountReceivedLD
            .try_into()
            .map_err(|_| BoltzError::Generic("OFT amount too large".into()))?;

        if !skip_drift_check {
            check_quote_drift(
                swap.expected_output_amount,
                raw_dest_amount,
                swap.slippage_bps,
            )?;
        }

        let min_amount_ld_slipped = compute_claim_floor(
            raw_dest_amount,
            swap.expected_output_amount,
            swap.slippage_bps,
            skip_drift_check,
        )?;
        if min_amount_ld_slipped == 0 {
            return Err(BoltzError::Generic(
                "Amount too small: cross-chain slippage-adjusted minimum is zero".into(),
            ));
        }

        // The DEX-leg `amount_out_min` is set to a loose sentinel: the
        // user's slippage budget is enforced atomically by the OFT contract
        // via `minAmountLd`. Any DEX output too small to clear that floor
        // reverts the whole atomic tx, so a tight DEX min only adds revert
        // frequency without adding user protection. `1` (rather than `0`)
        // keeps the encode API happy and rejects pathological zero-output
        // DEX paths.
        let min_usdt_out: u128 = 1;

        // Quote send for the LZ fee with the actual trade amount and the
        // signed-floor min — this is the message that will execute on-chain.
        let mut final_quoted_param = oft_quote_param.clone();
        final_quoted_param.minAmountLD = U256::from(min_amount_ld_slipped);
        let final_msg_fee = self
            .quote_send(source_oft_address, &final_quoted_param)
            .await?;

        // ─── Encode DEX calls ─────────────────────────────────────────
        // Trade calls: tBTC -> USDT
        let trade_encode = self
            .api_client
            .encode_quote(
                "ARB",
                &EncodeRequest {
                    recipient: router_str.clone(),
                    amount_in: trade_tbtc,
                    amount_out_min: min_usdt_out,
                    data: trade_best.data,
                },
            )
            .await?;
        let trade_calls: Vec<contracts::Call> = trade_encode
            .calls
            .iter()
            .map(quote_calldata_to_call)
            .collect::<Result<Vec<_>, _>>()?;

        // Fee calls: tBTC -> ETH (for LZ messaging)
        // NOTE: amount_out_min uses the Pass-1 native_fee, not the final_msg_fee
        // re-quoted above. If the LZ fee increases between passes the fee
        // swap may yield less ETH than needed and the transaction reverts
        // (no fund loss — just wasted sponsored gas). The fee_with_slippage
        // buffer on the input side absorbs typical fee movement.
        let fee_encode = self
            .api_client
            .encode_quote(
                "ARB",
                &EncodeRequest {
                    recipient: router_str.clone(),
                    amount_in: fee_tbtc,
                    amount_out_min: native_fee,
                    data: fee_dex.data,
                },
            )
            .await?;
        let fee_calls: Vec<contracts::Call> = fee_encode
            .calls
            .iter()
            .map(quote_calldata_to_call)
            .collect::<Result<Vec<_>, _>>()?;

        // Combine all DEX calls
        let mut all_calls = trade_calls;
        all_calls.extend(fee_calls);

        // ─── Optional OFT approval top-up ─────────────────────────────
        // For the current Arbitrum native-mesh OFT this is a no-op
        // (`approvalRequired()` returns false because the mint/burn variant
        // bypasses ERC20 transfers). It kicks in for legacy-mesh destinations
        // (Solana/Tron/Celo/TON), whose source OFT is a classical Adapter
        // that internally does `token.transferFrom(msg.sender, oft, amount)`
        // and therefore needs a standing ERC20 allowance.
        //
        // The amount passed to the gate is the raw pre-slippage DEX quote
        // (`trade_best.amount`) — matching against the slippage-reduced
        // value would under-approve when slippage eats a big chunk.
        if let Some(approval_call) = self
            .build_oft_approval_call(
                addrs.router,
                oft_addr,
                addrs.output_token_address,
                U256::from(trade_best.amount),
            )
            .await?
        {
            all_calls.push(approval_call);
        }

        // ─── Build SendData + hash ────────────────────────────────────
        // `extraOptions` here MUST match the bytes used in every `quoteOFT` /
        // `quoteSend` call above — the router signs over SendData, so any
        // drift between what is signed and what the OFT executes flips the
        // on-chain signature verification to revert.
        let send_data = SendData {
            dstEid: dst_eid,
            to: addrs.destination_bytes32,
            extraOptions: extra_options,
            composeMsg: vec![].into(),
            oftCmd: vec![].into(),
        };

        let typehash = self.fetch_typehash_send_data(&swap.router_address).await?;
        let send_data_hash = contracts::hash_send_data(typehash, &send_data);

        // ─── Sign ─────────────────────────────────────────────────────
        let erc20swap_sig = gas_signer.sign_eip712_erc20swap_claim(
            addrs.erc20swap,
            erc20swap_version,
            preimage,
            tbtc_evm_amount,
            addrs.tbtc,
            addrs.refund,
            timelock,
            addrs.router,
        )?;

        let router_sig = gas_signer.sign_eip712_router_claim_send(
            addrs.router,
            preimage,
            addrs.output_token_address,
            oft_addr,
            send_data_hash,
            U256::from(min_amount_ld_slipped),
            final_msg_fee.lzTokenFee,
            addrs.refund,
        )?;

        // ─── Encode calldata ──────────────────────────────────────────
        let erc20_claim = Erc20Claim {
            preimage: (*preimage).into(),
            amount: tbtc_evm_amount,
            tokenAddress: addrs.tbtc,
            refundAddress: addrs.refund,
            timelock,
            v: erc20swap_sig.v,
            r: erc20swap_sig.r.into(),
            s: erc20swap_sig.s.into(),
        };

        let auth = ClaimSendAuthorization {
            minAmountLd: U256::from(min_amount_ld_slipped),
            lzTokenFee: final_msg_fee.lzTokenFee,
            refundAddress: addrs.refund,
            v: router_sig.v,
            r: router_sig.r.into(),
            s: router_sig.s.into(),
        };

        let calldata = encode_claim_erc20_execute_oft(
            &erc20_claim,
            &all_calls,
            addrs.output_token_address,
            oft_addr,
            &send_data,
            &auth,
        );

        self.submit_claim(swap, &swap.router_address.clone(), &calldata, gas_signer)
            .await
    }

    /// Submit encoded calldata via Alchemy gas abstraction.
    ///
    /// Submission and confirmation are split so the gas-sponsor `call_id` can
    /// be durably persisted in the window between them: if the process dies
    /// after `wallet_sendPreparedCalls` but before the confirming poll, the
    /// claim still mines, and on resume the manager re-polls the persisted
    /// `call_id` to recover the tx hash instead of trusting the WS event.
    async fn submit_claim(
        &self,
        swap: &BoltzSwap,
        router_address: &str,
        calldata: &[u8],
        gas_signer: &EvmSigner,
    ) -> Result<String, BoltzError> {
        let evm_call = EvmCall {
            to: router_address.to_string(),
            value: None,
            data: Some(format!("0x{}", hex::encode(calldata))),
        };

        let call_id = self
            .alchemy_client
            .submit_calls(vec![evm_call], swap.chain_id, gas_signer)
            .await?;

        // Durably record the in-flight call_id before polling. Best-effort:
        // a persistence failure only forfeits the resume optimization (the
        // on-chain rescan fallback still applies), so it must not abort the
        // claim.
        self.persist_pending_call_id(&swap.id, &call_id).await;

        // The preimage is now public (broadcast in the claim UserOp). A poll
        // failure here must NOT bubble up as a generic claim error: that would
        // let the retry loop re-enter `submit_calls` and broadcast a second,
        // reverting claim. Signal "broadcast, unconfirmed" so the loop stops and
        // recovery proceeds via the persisted call_id.
        let result = self
            .alchemy_client
            .poll_call_status(&call_id)
            .await
            .map_err(|_| BoltzError::ClaimBroadcastUnconfirmed {
                call_id: call_id.clone(),
            })?;

        tracing::info!(
            tx_hash = result.tx_hash,
            swap_id = swap.id,
            "Claim submitted"
        );
        Ok(result.tx_hash)
    }

    /// Persist the in-flight gas-sponsor `call_id` onto the stored swap.
    /// Best-effort — logs and swallows errors.
    async fn persist_pending_call_id(&self, swap_id: &str, call_id: &str) {
        match self.store.get_swap(swap_id).await {
            Ok(Some(mut s)) => {
                s.pending_call_id = Some(call_id.to_string());
                s.updated_at = current_unix_timestamp();
                if let Err(e) = self.store.upsert_swap(&s).await {
                    tracing::warn!(swap_id, error = %e, "Failed to persist pending call_id");
                }
            }
            Ok(None) => {
                tracing::warn!(swap_id, "Swap missing while persisting pending call_id");
            }
            Err(e) => {
                tracing::warn!(swap_id, error = %e, "Failed to load swap for pending call_id");
            }
        }
    }

    /// Recover the claim tx hash for a previously persisted gas-sponsor
    /// `call_id` by re-polling `wallet_getCallsStatus`. Used on resume when a
    /// swap is mid-claim with no tx hash yet recorded.
    pub(crate) async fn poll_pending_call(&self, call_id: &str) -> Result<String, BoltzError> {
        Ok(self.alchemy_client.poll_call_status(call_id).await?.tx_hash)
    }

    /// Query Circle Iris for the destination-delivery status of a CCTP swap,
    /// given its stored guid `"<source_domain>:<source_tx_hash>"`. Lets callers
    /// confirm the cross-chain mint was forwarded — the client only verifies
    /// the Arbitrum burn on-chain; CCTP delivery lands asynchronously.
    pub(crate) async fn cctp_delivery_status(
        &self,
        guid: &str,
    ) -> Result<cctp::CctpMessageStatus, BoltzError> {
        let (domain_str, tx_hash) = guid
            .split_once(':')
            .ok_or_else(|| BoltzError::Generic(format!("Malformed CCTP guid '{guid}'")))?;
        let source_domain: u32 = domain_str.parse().map_err(|_| {
            BoltzError::Generic(format!("Invalid CCTP source domain in guid '{guid}'"))
        })?;
        self.cctp_fee_client
            .get_message_status(source_domain, tx_hash)
            .await
    }

    /// Fallback delivery confirmation for a CCTP swap whose Circle forward
    /// stalled: read the *destination chain* for proof the mint landed, rather
    /// than trusting Circle's forwarder-specific `forwardTxHash`. Returns the
    /// authoritative delivered amount once provably delivered, else `None`.
    ///
    /// Only meaningful once attested — the nonce and the delivered amount both
    /// come from the attested `message`. Only Solana is probed today (its
    /// ATA-creating forward is the one that stalls); EVM/Tron destinations have
    /// no destination-chain probe and rely on the forwarder fast path, so they
    /// return `None` here. This is a drop-in seam: an EVM probe (a
    /// `usedNonces` `eth_call` on the destination chain) slots in per-chain
    /// when a chain's RPC is supplied and the risk justifies it.
    pub(crate) async fn cctp_destination_delivered(
        &self,
        swap: &BoltzSwap,
        status: &cctp::CctpMessageStatus,
    ) -> Result<Option<u64>, BoltzError> {
        let (Some(message), Some(delivered)) = (status.message.as_deref(), status.delivered_amount)
        else {
            return Ok(None);
        };
        let dest = self.resolve_destination(&swap.destination_chain, swap.asset)?;
        match dest.transport {
            NetworkTransport::Solana => {
                if self.cctp_nonce_consumed_on_solana(message).await? {
                    Ok(Some(delivered))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Whether the CCTP message's nonce has been consumed on Solana: its
    /// `used_nonce` PDA exists under the destination `MessageTransmitter`, which
    /// proves `receiveMessage` ran (the mint landed) regardless of who
    /// submitted it. Sweep-proof, unlike a recipient balance check.
    async fn cctp_nonce_consumed_on_solana(&self, message: &str) -> Result<bool, BoltzError> {
        let nonce = contracts::decode_cctp_nonce_from_message(message)
            .ok_or_else(|| BoltzError::Generic("CCTP message missing nonce".into()))?;
        let transmitter = decode_solana_pubkey(SOLANA_MESSAGE_TRANSMITTER_V2)?;
        let pda = derive_used_nonce_pda(&nonce, &transmitter)?;
        let pda_base58 = bs58::encode(pda.as_slice()).into_string();
        // Single attempt: this runs inside the sequential delivery-poll loop, so
        // a multi-second 429 backoff would stall every later swap. The poll
        // retries next tick anyway.
        self.solana_rpc.account_exists_no_retry(&pda_base58).await
    }

    /// Query `LayerZero` Scan for whether an OFT message (by its GUID) has been
    /// delivered on the destination chain. Confirmation-only: the OFT delivered
    /// amount is already known from the source `OFTSent` log at claim time.
    pub(crate) async fn oft_delivery_status(
        &self,
        guid: &str,
    ) -> Result<crate::evm::lz_scan::LzMessageStatus, BoltzError> {
        self.lz_scan_client.get_message_status(guid).await
    }

    // ─── OFT fee estimation (for prepare-time quoting) ─────────────────

    /// Find the OFT send amount required to deliver `target_amount` on the
    /// destination chain. Native-mesh routes binary-search the on-chain
    /// `quoteOFT`; legacy-mesh routes use the closed-form 3 bps inverse
    /// because the legacy bridge fee is not deducted by the staticcall.
    async fn estimate_oft_required_send_amount(
        &self,
        dest: &Destination,
        target_amount: u128,
        extra_options: &Bytes,
    ) -> Result<u128, BoltzError> {
        if target_amount == 0 {
            return Ok(0);
        }

        let (mesh, _) = dest.oft().ok_or_else(|| {
            BoltzError::Generic(format!(
                "Destination '{}/{}' is not an OFT route",
                dest.chain_label, dest.asset
            ))
        })?;

        // Legacy-mesh routes: skip the binary search and apply the closed-form
        // 3 bps inverse. The legacy `quoteOFT` does not deduct the bridge fee,
        // so the search would converge to a too-low source amount.
        if mesh == Usdt0Kind::Legacy {
            return legacy_mesh_source_amount(target_amount)
                .ok_or_else(|| BoltzError::Generic("Legacy mesh source amount overflow".into()));
        }

        // Binary search: find the minimum send amount where OFT receive >= target
        let mut low = target_amount;
        let mut high = target_amount;

        // Phase 1: find upper bound
        // Safety: `attempts` is bounded to 32 iterations, and `low`/`high`
        // use checked arithmetic. The unchecked `+= 1` on a u32 capped at
        // 32 cannot overflow.
        let mut attempts = 0u32;
        loop {
            let (_, received) = self
                .quote_oft_messaging_fee(dest, high, extra_options)
                .await?;
            if received >= target_amount {
                break;
            }
            low = high
                .checked_add(1)
                .ok_or_else(|| BoltzError::Generic("OFT amount search overflow".into()))?;
            high = high
                .checked_mul(2)
                .ok_or_else(|| BoltzError::Generic("OFT amount search overflow".into()))?;
            #[expect(clippy::arithmetic_side_effects)]
            {
                attempts += 1;
            }
            if attempts > 32 {
                return Err(BoltzError::Generic(
                    "Could not find OFT send amount for target".into(),
                ));
            }
        }

        // Phase 2: binary search
        // Safety: `high >= low` is guaranteed by the while condition, so
        // `high - low` cannot underflow. `mid` is between `low` and `high`,
        // so `mid + 1 <= high` which fits in u128.
        #[expect(clippy::arithmetic_side_effects)]
        while low < high {
            let mid = low + (high - low) / 2;
            let (_, received) = self
                .quote_oft_messaging_fee(dest, mid, extra_options)
                .await?;
            if received >= target_amount {
                high = mid;
            } else {
                low = mid + 1;
            }
        }

        Ok(low)
    }

    /// Quote OFT messaging fee and received amount for a given USDT amount and destination chain.
    /// Returns `(native_fee, amount_received_on_destination)`.
    ///
    /// `extra_options` must match whatever will eventually be submitted in
    /// the router's `claimERC20ExecuteOft` — for Solana destinations with a
    /// missing ATA, this contains the lzReceive option that pre-funds the
    /// account creation, which materially affects the fee.
    async fn quote_oft_messaging_fee(
        &self,
        dest: &Destination,
        usdt_amount: u128,
        extra_options: &Bytes,
    ) -> Result<(u128, u128), BoltzError> {
        let (mesh, lz_eid) = dest.oft().ok_or_else(|| {
            BoltzError::Generic(format!(
                "Destination '{}/{}' is not an OFT route",
                dest.chain_label, dest.asset
            ))
        })?;

        // Match the source OFT to the destination's mesh: legacy and native
        // USDT0 use distinct contracts on the source chain.
        let source_oft_address = self.chain_registry.oft_for(mesh).ok_or_else(|| {
            BoltzError::Generic(format!("Source chain has no {mesh:?} mesh OFT deployment"))
        })?;

        // The recipient is irrelevant for messaging-fee estimation; an all-zero
        // 32-byte placeholder is fine for both EVM and non-EVM destinations.
        let send_param = contracts::build_oft_send_param(
            lz_eid,
            FixedBytes::<32>::ZERO,
            U256::from(usdt_amount),
            U256::ZERO,
            extra_options.clone(),
        );
        let (_, receipt) = self.quote_oft(source_oft_address, &send_param).await?;

        let mut quoted_param = send_param;
        quoted_param.minAmountLD = receipt.amountReceivedLD;
        let msg_fee = self.quote_send(source_oft_address, &quoted_param).await?;

        let native_fee: u128 = msg_fee
            .nativeFee
            .try_into()
            .map_err(|_| BoltzError::Generic("LZ fee too large".into()))?;
        let amount_received: u128 = receipt
            .amountReceivedLD
            .try_into()
            .map_err(|_| BoltzError::Generic("OFT amount too large".into()))?;

        Ok((native_fee, amount_received))
    }

    /// DEX quote: how much tBTC (in wei) needed to buy the given ETH amount.
    /// Used at prepare time to convert the LZ messaging fee directly into
    /// tBTC claim cost, keeping cross-chain quotes on the tBTC↔ETH pool
    /// instead of hopping through USDT↔ETH.
    async fn fetch_quote_out_tbtc_for_eth(&self, eth_amount: u128) -> Result<u128, BoltzError> {
        let quotes = self
            .api_client
            .get_quote_out("ARB", ARBITRUM_TBTC_ADDRESS, ZERO_ADDRESS, eth_amount)
            .await?;
        let quote = pick_best_quote(&quotes, QuoteDirection::Out)?;
        if quote == 0 {
            // A degenerate "0" quote would value the LZ messaging-fee leg at 0
            // tBTC, producing a too-optimistic prepare quote (the on-chain floor
            // would only catch it later as a degraded/aborted claim). Fail fast,
            // like the sibling DEX-quote helpers.
            return Err(BoltzError::InvalidQuote(
                "DEX quote returned zero tBTC for ETH".to_string(),
            ));
        }
        Ok(quote)
    }

    // ─── LayerZero executor options ───────────────────────────────────

    /// Build the `extraOptions` bytes for an OFT `SendParam` targeting the
    /// given destination. For EVM and Tron destinations this is always empty.
    /// For Solana, the destination's Associated Token Account must exist
    /// before the LZ executor can deliver tokens; if the ATA is missing, the
    /// returned blob carries an `lzReceive` option pre-funding the account
    /// creation with `solanaAtaRentExemptLamports`.
    ///
    /// The result is cached per-recipient: once we've observed an ATA to
    /// exist, future checks for the same recipient in-process skip the RPC.
    /// "Doesn't exist" is not cached — the user may create the ATA between
    /// calls.
    /// Validate the destination address for the given chain's transport before
    /// committing to a swap, and reject addresses that are themselves a known
    /// token *contract* address (sending tokens there would burn them).
    fn validate_destination(
        &self,
        dest: &Destination,
        destination: &str,
    ) -> Result<(), BoltzError> {
        if !is_valid_destination_address(dest.transport, destination) {
            return Err(BoltzError::Generic(format!(
                "Invalid destination address '{destination}' for {} ({})",
                dest.chain_label, dest.asset
            )));
        }

        if self.is_known_token_address(dest.transport, destination) {
            return Err(BoltzError::Generic(format!(
                "Destination '{destination}' is a known token contract address; \
                 sending there would burn funds"
            )));
        }

        Ok(())
    }

    /// Whether `addr` matches a known token-contract address on the same
    /// transport. Compares against the source chain's USDT and tBTC (EVM), the
    /// Solana USDT0 mint, and every destination's published USDT0 token
    /// address, each normalized per transport. Cross-transport addresses never
    /// match (their encodings differ), so only same-transport tokens are
    /// gathered.
    fn is_known_token_address(&self, transport: NetworkTransport, addr: &str) -> bool {
        let mut known: Vec<&str> = Vec::new();
        match transport {
            NetworkTransport::Evm => {
                known.push(ARBITRUM_USDT_ADDRESS);
                known.push(ARBITRUM_TBTC_ADDRESS);
            }
            NetworkTransport::Solana => known.push(SOLANA_USDT0_MINT),
            NetworkTransport::Tron => {}
        }
        for dest in &self.chain_registry.destinations {
            if dest.transport == transport
                && let Some(token) = &dest.dest_token_address
            {
                known.push(token.as_str());
            }
        }

        matches_any_known_token(transport, addr, &known)
    }

    async fn compute_extra_options(
        &self,
        dest: &Destination,
        destination: &str,
    ) -> Result<Bytes, BoltzError> {
        // Temporary workaround: OFT sends to Polygon need a bumped `lzReceive`
        // gas limit (boltz-web-app#1500). Polygon is an EVM destination, so it
        // never overlaps the Solana ATA branch below.
        let polygon_gas_bump = dest.evm_chain_id == Some(POLYGON_EVM_CHAIN_ID);

        if dest.transport != NetworkTransport::Solana {
            return Ok(Bytes::from(build_extra_options(false, polygon_gas_bump)));
        }

        // Fast path: if we've already confirmed the ATA exists for this
        // recipient, skip the RPC.
        let cache_hit = self
            .ata_cache
            .lock()
            .is_ok_and(|guard| guard.contains(destination));
        if cache_hit {
            return Ok(Bytes::new());
        }

        let owner = decode_solana_pubkey(destination)?;
        let mint = decode_solana_pubkey(SOLANA_USDT0_MINT)?;
        let ata_bytes = derive_ata(&owner, &mint)?;
        let ata_base58 = bs58::encode(ata_bytes).into_string();

        let exists = self.solana_rpc.account_exists(&ata_base58).await?;
        if exists {
            if let Ok(mut guard) = self.ata_cache.lock() {
                guard.insert(destination.to_string());
            }
            return Ok(Bytes::new());
        }

        Ok(Bytes::from(build_extra_options(true, false)))
    }

    // ─── OFT quoting helpers ──────────────────────────────────────────

    /// Call `quoteOFT` on the OFT contract via `eth_call`.
    async fn quote_oft(
        &self,
        oft_address: &str,
        send_param: &contracts::OftSendParam,
    ) -> Result<(contracts::OftLimit, contracts::OftReceipt), BoltzError> {
        let calldata = contracts::encode_quote_oft(send_param);
        let result = self.evm_provider.eth_call(oft_address, &calldata).await?;
        let (limit, _fees, receipt) = contracts::decode_quote_oft_return(&result)?;
        Ok((limit, receipt))
    }

    /// Call `quoteSend` on the OFT contract via `eth_call`.
    async fn quote_send(
        &self,
        oft_address: &str,
        send_param: &contracts::OftSendParam,
    ) -> Result<contracts::MessagingFee, BoltzError> {
        let calldata = contracts::encode_quote_send(send_param, false);
        let result = self.evm_provider.eth_call(oft_address, &calldata).await?;
        contracts::decode_quote_send_return(&result)
    }

    /// Build an `approve(oft, MaxUint256)` call to top up the Router's
    /// allowance on the source OFT when the OFT is a classical Adapter.
    ///
    /// The OFT's `approvalRequired()` distinguishes mint/burn variants
    /// (return false — no `transferFrom` path, no allowance consumed) from
    /// classical Adapter variants (return true — `send` internally does
    /// `token.transferFrom(msg.sender, oft, amount)`). For the current
    /// Arbitrum → EVM native-mesh flow this is always a no-op; it becomes
    /// load-bearing once legacy-mesh destinations (Solana/Tron/Celo/TON) are
    /// routed, since those bridge through the legacy-mesh Arbitrum OFT
    /// Adapter.
    ///
    /// The actual decision (`allowance < amount * 10` → top up to
    /// `MaxUint256`) is factored out into [`decide_oft_approval_top_up`] so
    /// the gate can be exhaustively unit-tested without a live RPC.
    async fn build_oft_approval_call(
        &self,
        router: alloy_primitives::Address,
        oft: alloy_primitives::Address,
        token: alloy_primitives::Address,
        amount: U256,
    ) -> Result<Option<contracts::Call>, BoltzError> {
        let oft_str = oft.to_string();
        let approval_required_data = contracts::encode_approval_required();
        let result = self
            .evm_provider
            .eth_call(&oft_str, &approval_required_data)
            .await?;
        let required = contracts::decode_approval_required_return(&result)?;
        if !required {
            // Short-circuit: native-mesh mint/burn OFTs never need an ERC20
            // allowance, so skip the allowance `eth_call` entirely.
            return Ok(None);
        }

        let token_str = token.to_string();
        let allowance_data = contracts::encode_allowance(router, oft);
        let result = self
            .evm_provider
            .eth_call(&token_str, &allowance_data)
            .await?;
        let current_allowance = contracts::decode_allowance_return(&result)?;

        Ok(decide_oft_approval_top_up(
            current_allowance,
            amount,
            oft,
            token,
        ))
    }

    /// Fetch `TYPEHASH_SEND_DATA` from the Router contract.
    async fn fetch_typehash_send_data(&self, router_address: &str) -> Result<[u8; 32], BoltzError> {
        let calldata = contracts::encode_typehash_send_data_call();
        let result = self
            .evm_provider
            .eth_call(router_address, &calldata)
            .await?;
        contracts::decode_typehash_send_data(&result)
    }

    /// Fetch `TYPEHASH_CCTP_DATA` from the Router contract.
    async fn fetch_typehash_cctp_data(&self, router_address: &str) -> Result<[u8; 32], BoltzError> {
        let calldata = contracts::encode_typehash_cctp_data_call();
        let result = self
            .evm_provider
            .eth_call(router_address, &calldata)
            .await?;
        contracts::decode_typehash_cctp_data(&result)
    }

    async fn fetch_erc20swap_version(&self, erc20swap_address: &str) -> Result<String, BoltzError> {
        let calldata = contracts::encode_version_call();
        let result = self
            .evm_provider
            .eth_call(erc20swap_address, &calldata)
            .await?;
        let version = contracts::decode_version_return(&result)?;
        Ok(version.to_string())
    }
}

// ─── OFT approval gate (pure) ────────────────────────────────────────────

/// Decide whether the Router needs an `approve(oft, MaxUint256)` top-up on
/// the source OFT, given the current on-chain allowance and the amount
/// about to flow through the OFT. Gate: `allowance < amount * 10`.
///
/// Caller must have already verified `oft.approvalRequired() == true`;
/// this function unconditionally assumes the OFT is an Adapter.
///
/// A 10x runway is used so one `MaxUint256` approval amortises the SSTORE
/// across roughly ten equal-size claims. `saturating_mul` protects against
/// absurdly large `amount` values by saturating the threshold to
/// `U256::MAX`, which errs on the side of topping up.
fn decide_oft_approval_top_up(
    current_allowance: U256,
    amount: U256,
    oft: alloy_primitives::Address,
    token: alloy_primitives::Address,
) -> Option<contracts::Call> {
    let threshold = amount.saturating_mul(U256::from(10u64));
    if current_allowance >= threshold {
        return None;
    }
    Some(contracts::Call {
        target: token,
        value: U256::ZERO,
        callData: contracts::encode_approve(oft, U256::MAX).into(),
    })
}

// ─── Parsed addresses for claim ──────────────────────────────────────────

struct ClaimAddresses {
    erc20swap: alloy_primitives::Address,
    router: alloy_primitives::Address,
    tbtc: alloy_primitives::Address,
    /// The DEX output token on Arbitrum — Arbitrum USDT for OFT/USDT-direct
    /// routes, Arbitrum USDC for CCTP/USDC-direct routes. See also
    /// [`Self::output_token`] for its string form.
    output_token_address: alloy_primitives::Address,
    /// Canonical string form of the DEX output token, for DEX quote requests.
    output_token: &'static str,
    refund: alloy_primitives::Address,
    /// EVM destination address. `Some` for EVM transports (used by the
    /// same-chain claim path which signs over the destination as an
    /// `address`); `None` for Solana / Tron whose recipients aren't EVM
    /// addresses.
    destination_evm: Option<alloy_primitives::Address>,
    /// Transport-encoded 32-byte destination, ready to feed into both
    /// `OftSendParam.to` and `SendData.to` on the cross-chain claim path.
    destination_bytes32: FixedBytes<32>,
}

impl ClaimAddresses {
    /// Resolve claim addresses for any bridge from the unified registry. The
    /// DEX output token comes from the resolved [`Destination`] (USDT or USDC).
    fn parse(swap: &BoltzSwap, registry: &DestinationRegistry) -> Result<Self, BoltzError> {
        let dest = registry
            .find(&swap.destination_chain, swap.asset)
            .ok_or_else(|| {
                BoltzError::Generic(format!(
                    "Unknown destination '{}' for swap {}",
                    swap.destination_chain, swap.id
                ))
            })?;
        let destination_bytes32 = encode_oft_recipient(dest.transport, &swap.destination_address)?;
        let destination_evm = match dest.transport {
            NetworkTransport::Evm => Some(parse_address(&swap.destination_address)?),
            NetworkTransport::Solana | NetworkTransport::Tron => None,
        };
        Ok(Self {
            erc20swap: parse_address(&swap.erc20swap_address)?,
            router: parse_address(&swap.router_address)?,
            tbtc: parse_address(ARBITRUM_TBTC_ADDRESS)?,
            output_token_address: parse_address(dest.dex_output_token)?,
            output_token: dest.dex_output_token,
            refund: parse_address(&swap.refund_address)?,
            destination_evm,
            destination_bytes32,
        })
    }
}

// ─── Fee computation ─────────────────────────────────────────────────────

struct FeeCalc {
    invoice_sats: u64,
    boltz_fee_sats: u64,
    onchain_sats: u64,
}

/// Compute the total sats needed from an already-floored tBTC-sats claim
/// amount and Boltz pair info.
///
/// Formula (integer math):
///   `invoiceAmount = ceil((receiveAmount + minerFee) / (1 - percentage/100))`
///
/// The percentage from the API (e.g. `0.25` for 0.25%) is parsed from its
/// string representation to avoid floating-point imprecision.
///
/// Callers are responsible for converting tBTC EVM units (wei) to sats by
/// flooring with `SATS_TO_TBTC_FACTOR`. Cross-chain quotes floor each DEX
/// leg independently before summing.
fn compute_invoice_amount(pair: &ReversePairInfo, tbtc_sats: u64) -> Result<FeeCalc, BoltzError> {
    let tbtc_sats = u128::from(tbtc_sats);

    let miner_fees = u128::from(pair.fees.miner_fees.claim)
        .checked_add(u128::from(pair.fees.miner_fees.lockup))
        .ok_or_else(|| BoltzError::Generic("Miner fees overflow".into()))?;

    // Parse percentage from f64 to integer basis points to avoid floating-point imprecision.
    // The API returns values like 0.25 (meaning 0.25%). We need basis points of 100%,
    // i.e., 0.25% → 25 out of 10000.
    let pct_bps = parse_percentage_to_bps(pair.fees.percentage)?;

    // invoiceAmount = ceil((receiveAmount + minerFee) / (1 - pct/100))
    // In integer form: ceil((base * 10000) / (10000 - pct_bps))
    let base = tbtc_sats
        .checked_add(miner_fees)
        .ok_or_else(|| BoltzError::Generic("Fee base overflow".to_string()))?;
    let denominator = 10000u64
        .checked_sub(pct_bps)
        .ok_or_else(|| BoltzError::Generic("Invalid fee percentage (>= 100%)".to_string()))?;
    if denominator == 0 {
        return Err(BoltzError::Generic(
            "Invalid fee percentage (100%)".to_string(),
        ));
    }

    // ceil(base * 10000 / denominator)
    let numerator = base
        .checked_mul(10000)
        .ok_or_else(|| BoltzError::Generic("Invoice computation overflow".to_string()))?;
    let invoice = numerator.div_ceil(u128::from(denominator));
    let boltz_fee = invoice
        .checked_sub(base)
        .ok_or_else(|| BoltzError::Generic("Fee computation underflow".to_string()))?;
    let onchain = invoice
        .checked_sub(boltz_fee)
        .and_then(|v| v.checked_sub(miner_fees))
        .ok_or_else(|| BoltzError::Generic("Onchain amount underflow".to_string()))?;

    let to_u64 = |v: u128, name: &str| -> Result<u64, BoltzError> {
        v.try_into()
            .map_err(|_| BoltzError::Generic(format!("{name} overflow")))
    };

    Ok(FeeCalc {
        invoice_sats: to_u64(invoice, "Invoice amount")?,
        boltz_fee_sats: to_u64(boltz_fee, "Boltz fee")?,
        onchain_sats: to_u64(onchain, "Onchain amount")?,
    })
}

/// Parse a fee percentage (e.g. 0.25 meaning 0.25%) to basis points of 100% (e.g. 25).
/// Uses string formatting to avoid floating-point imprecision when converting to integer.
fn parse_percentage_to_bps(percentage: f64) -> Result<u64, BoltzError> {
    // Format with enough precision to capture the API value, then parse as integer.
    // percentage * 100 gives basis points (0.25% * 100 = 25 bps).
    let s = format!("{:.4}", percentage * 100.0);
    let parts: Vec<&str> = s.split('.').collect();
    let whole: u64 = parts[0]
        .parse()
        .map_err(|_| BoltzError::Generic(format!("Invalid fee percentage: {percentage}")))?;
    // Check if fractional part is non-zero (meaning the percentage has sub-bps precision)
    if parts.len() > 1 && !parts[1].trim_end_matches('0').is_empty() {
        return Err(BoltzError::Generic(format!(
            "Fee percentage {percentage} has sub-basis-point precision, cannot represent exactly"
        )));
    }
    Ok(whole)
}

/// Compute the onchain amount from invoice sats (forward direction).
///
/// Formula:
///   `receiveAmount = sendAmount - ceil(sendAmount * percentage / 100) - minerFee`
fn compute_onchain_amount(
    pair: &ReversePairInfo,
    invoice_sats: u64,
) -> Result<FeeCalc, BoltzError> {
    let invoice = u128::from(invoice_sats);

    let pct_bps = parse_percentage_to_bps(pair.fees.percentage)?;
    let miner_fees = u128::from(pair.fees.miner_fees.claim)
        .checked_add(u128::from(pair.fees.miner_fees.lockup))
        .ok_or_else(|| BoltzError::Generic("Miner fees overflow".into()))?;

    // boltz_fee = ceil(invoice * pct_bps / 10000)
    let boltz_fee = invoice
        .checked_mul(u128::from(pct_bps))
        .ok_or_else(|| BoltzError::Generic("Fee computation overflow".into()))?
        .div_ceil(10000);

    let onchain = invoice
        .checked_sub(boltz_fee)
        .and_then(|v| v.checked_sub(miner_fees))
        .ok_or_else(|| BoltzError::Generic("Invoice amount too small to cover fees".into()))?;

    let to_u64 = |v: u128, name: &str| -> Result<u64, BoltzError> {
        v.try_into()
            .map_err(|_| BoltzError::Generic(format!("{name} overflow")))
    };

    Ok(FeeCalc {
        invoice_sats,
        boltz_fee_sats: to_u64(boltz_fee, "Boltz fee")?,
        onchain_sats: to_u64(onchain, "Onchain amount")?,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────

fn to_chain_id_u32(chain_id: u64) -> Result<u32, BoltzError> {
    chain_id
        .try_into()
        .map_err(|_| BoltzError::Generic("Chain ID overflow".to_string()))
}

/// Validate a server-supplied swap id before it is persisted and later
/// interpolated into request URLs (`v2/swap/{id}`...). Boltz ids are short
/// alphanumeric tokens; rejecting anything else stops a hostile server from
/// returning path/query characters that reshape a request path (the fixed base
/// URL already bounds the host, so this is defense-in-depth).
fn validate_swap_id(id: &str) -> Result<(), BoltzError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(BoltzError::Generic(format!(
            "Boltz returned an invalid swap id: {id:?}"
        )));
    }
    Ok(())
}

/// Pure check behind [`ReverseSwapExecutor::ensure_timeout_margin`]: reject the
/// swap unless the lockup timeout sits at least [`MIN_TIMEOUT_L1_MARGIN`] L1
/// blocks above the current L1 height. `saturating_sub` makes a timeout at or
/// below the current block yield a zero margin (rejected).
fn check_timeout_margin(timeout_block_height: u64, current_l1: u64) -> Result<(), BoltzError> {
    let margin = timeout_block_height.saturating_sub(current_l1);
    if margin < MIN_TIMEOUT_L1_MARGIN {
        return Err(BoltzError::Generic(format!(
            "Lockup timeout_block_height ({timeout_block_height}) leaves only {margin} L1 \
             block(s) of headroom over the current L1 height ({current_l1}); require at least \
             {MIN_TIMEOUT_L1_MARGIN}"
        )));
    }
    Ok(())
}

/// Build the CCTP forwarding hook bytes from the recipient-setup decision
/// (see [`ReverseSwapExecutor::cctp_needs_recipient_setup`]). With setup, the
/// Solana ATA-creating hook; otherwise the plain EVM-style forward tag (also
/// used for Solana recipients whose ATA already exists).
fn cctp_forward_hook(
    destination: &str,
    needs_recipient_setup: bool,
) -> Result<Vec<u8>, BoltzError> {
    if needs_recipient_setup {
        cctp::solana_forward_hook_data(destination)
    } else {
        Ok(cctp::evm_forward_hook_data().to_vec())
    }
}

/// Floor tBTC wei (18-decimal EVM units) to tBTC sats (8-decimal), for
/// converting DEX quote outputs to claim amounts before summing.
fn tbtc_wei_to_sats_u64(tbtc_wei: u128) -> Result<u64, BoltzError> {
    let sats_factor = u128::from(SATS_TO_TBTC_FACTOR);
    let tbtc_sats = tbtc_wei.checked_div(sats_factor).unwrap_or(0);
    u64::try_from(tbtc_sats).map_err(|_| BoltzError::Generic("tBTC sats overflow".into()))
}

/// Whether `addr` matches any address in `known`, normalized for `transport`
/// (EVM case-insensitive; Solana/Tron exact). An empty/blank `addr` never
/// matches.
fn matches_any_known_token(transport: NetworkTransport, addr: &str, known: &[&str]) -> bool {
    let target = normalize_token_address(transport, addr);
    if target.is_empty() {
        return false;
    }
    known
        .iter()
        .any(|k| normalize_token_address(transport, k) == target)
}

/// Decode a base58 Solana pubkey into its 32-byte form.
fn decode_solana_pubkey(s: &str) -> Result<[u8; 32], BoltzError> {
    let decoded = bs58::decode(s)
        .into_vec()
        .map_err(|e| BoltzError::Generic(format!("Invalid Solana pubkey '{s}': {e}")))?;
    if decoded.len() != 32 {
        return Err(BoltzError::Generic(format!(
            "Solana pubkey '{s}' must decode to 32 bytes, got {}",
            decoded.len()
        )));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&decoded);
    Ok(out)
}

pub(crate) fn current_unix_timestamp() -> u64 {
    use platform_utils::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(e) => {
            tracing::error!("System clock before UNIX epoch: {e}, returning 0");
            0
        }
    }
}

async fn sleep_1s() {
    platform_utils::tokio::time::sleep(platform_utils::time::Duration::from_secs(1)).await;
}

/// Check that the fresh DEX quote hasn't degraded beyond the slippage
/// tolerance compared to the creation-time estimate. Rejects a swap whose
/// effective receive amount would drift below `expected * (1 - slippage)`
/// before we commit the claim transaction.
fn check_quote_drift(
    expected_usd: u64,
    fresh_quote_usd: u128,
    slippage_bps: u32,
) -> Result<(), BoltzError> {
    let factor = 10000u128
        .checked_sub(u128::from(slippage_bps))
        .ok_or_else(|| BoltzError::Generic("slippage_bps exceeds 10000".to_string()))?;
    let threshold = slippage_mul_div(u128::from(expected_usd), factor)?;
    if fresh_quote_usd < threshold {
        let quoted = fresh_quote_usd.try_into().unwrap_or(u64::MAX);
        return Err(BoltzError::QuoteDegradedBeyondSlippage {
            expected_usd,
            quoted_usd: quoted,
        });
    }
    Ok(())
}

/// `amount * numerator / 10000` with overflow checked rather than wrapping.
/// The multiplicands can be RPC-controlled u128 quote values, so an absurd
/// response must surface a bounded error identically in debug and release
/// rather than silently wrapping (release has no `overflow-checks`).
fn slippage_mul_div(amount: u128, numerator: u128) -> Result<u128, BoltzError> {
    amount
        .checked_mul(numerator)
        .and_then(|scaled| scaled.checked_div(10000))
        .ok_or_else(|| BoltzError::Generic("slippage arithmetic overflow".to_string()))
}

/// Apply slippage upward (for fees that might increase).
/// Returns `amount * (10000 + slippage_bps) / 10000`.
fn apply_slippage_up(amount: u128, slippage_bps: u128) -> Result<u128, BoltzError> {
    slippage_mul_div(amount, 10000u128.saturating_add(slippage_bps))
}

/// Apply slippage downward (for output floors).
/// Returns `amount * (10000 - slippage_bps) / 10000`.
fn apply_slippage_down(amount: u128, slippage_bps: u32) -> Result<u128, BoltzError> {
    let factor = 10000u32
        .checked_sub(slippage_bps)
        .ok_or_else(|| BoltzError::Generic("slippage_bps exceeds 10000".to_string()))?;
    slippage_mul_div(amount, u128::from(factor))
}

/// Compute the on-chain output floor for a claim. In normal mode the
/// floor is anchored on `expected_amount` (locked at prepare time) so the
/// user is guaranteed to receive at least `expected × (1 − slippage)`
/// end-to-end — drift between prepare and claim, and any internal buffers
/// that shrink the live quote, are caught by the drift check rather than
/// quietly delivering less than the user agreed to. When the drift check is
/// skipped (the user explicitly accepted the degraded live quote via
/// `accept_degraded_quote`), the floor falls back to the live quote.
fn compute_claim_floor(
    raw_quote: u128,
    expected_amount: u64,
    slippage_bps: u32,
    skip_drift_check: bool,
) -> Result<u128, BoltzError> {
    if skip_drift_check {
        apply_slippage_down(raw_quote, slippage_bps)
    } else {
        apply_slippage_down(u128::from(expected_amount), slippage_bps)
    }
}

/// Pick the slippage tolerance for a new swap. Returns the per-swap
/// `override_bps` when `Some`, otherwise falls back to `config_default`.
/// Enforces the same `10..=MAX_SLIPPAGE_BPS` bounds in both cases so a bad
/// per-swap value can't bypass the validation that previously ran on the
/// config-level default.
pub(crate) fn resolve_slippage_bps(
    override_bps: Option<u32>,
    config_default: u32,
) -> Result<u32, BoltzError> {
    let bps = override_bps.unwrap_or(config_default);
    if !(10..=MAX_SLIPPAGE_BPS).contains(&bps) {
        return Err(BoltzError::Generic(format!(
            "slippage_bps must be >= 10 and <= {MAX_SLIPPAGE_BPS}"
        )));
    }
    Ok(bps)
}

// ─── DEX quote selection ─────────────────────────────────────────────────
// - "in" direction (quoting by input):  pick highest output (best return)
// - "out" direction (quoting by output): pick lowest input  (cheapest route)

#[derive(Clone, Copy)]
enum QuoteDirection {
    In,
    Out,
}

struct ParsedQuote {
    amount: u128,
    data: serde_json::Value,
}

fn pick_best_quote(
    quotes: &[QuoteResponse],
    direction: QuoteDirection,
) -> Result<u128, BoltzError> {
    Ok(pick_best_quote_with_data(quotes, direction)?.amount)
}

fn pick_best_quote_with_data(
    quotes: &[QuoteResponse],
    direction: QuoteDirection,
) -> Result<ParsedQuote, BoltzError> {
    if quotes.is_empty() {
        return Err(BoltzError::Api {
            reason: "No DEX quote returned".to_string(),
            code: None,
        });
    }

    let mut best: Option<ParsedQuote> = None;
    for q in quotes {
        let amount: u128 = q.quote.parse().map_err(|_| BoltzError::Api {
            reason: format!("Invalid quote amount: {}", q.quote),
            code: None,
        })?;
        let is_better = match best {
            None => true,
            Some(ref b) => match direction {
                QuoteDirection::In => amount > b.amount,
                QuoteDirection::Out => amount < b.amount,
            },
        };
        if is_better {
            best = Some(ParsedQuote {
                amount,
                data: q.data.clone(),
            });
        }
    }

    best.ok_or_else(|| BoltzError::Api {
        reason: "No DEX quote returned".to_string(),
        code: None,
    })
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn validate_swap_id_accepts_alphanumeric_rejects_path_chars() {
        assert!(validate_swap_id("gEzPzETWJGgy").is_ok());
        assert!(validate_swap_id("abc123XYZ").is_ok());
        // Empty, path/query separators, and other punctuation are rejected.
        assert!(validate_swap_id("").is_err());
        assert!(validate_swap_id("../v2/swap").is_err());
        assert!(validate_swap_id("id/with/slash").is_err());
        assert!(validate_swap_id("id?q=1").is_err());
        assert!(validate_swap_id("id#frag").is_err());
        assert!(validate_swap_id("id with space").is_err());
    }

    #[macros::test_all]
    fn check_timeout_margin_enforces_l1_headroom() {
        let l1 = 25_273_396; // a realistic Arbitrum-reported L1 height

        // Honest Boltz timeout (~7200 blocks out) passes comfortably.
        assert!(check_timeout_margin(l1 + 7200, l1).is_ok());
        // Exactly the minimum margin is accepted (boundary is inclusive).
        assert!(check_timeout_margin(l1 + MIN_TIMEOUT_L1_MARGIN, l1).is_ok());

        // One block under the minimum is rejected.
        assert!(check_timeout_margin(l1 + MIN_TIMEOUT_L1_MARGIN - 1, l1).is_err());
        // A timeout equal to or below the current block (zero/negative headroom)
        // is rejected — `saturating_sub` floors the margin at 0.
        assert!(check_timeout_margin(l1, l1).is_err());
        assert!(check_timeout_margin(l1 - 1_000, l1).is_err());
    }

    #[macros::test_all]
    fn check_timeout_margin_does_not_confuse_l1_and_l2() {
        // Regression guard for the L1/L2 denomination trap: an L1-denominated
        // timeout (~25.3M) compared against an L2 height (~471M, what
        // `eth_blockNumber` returns) yields a huge negative headroom and must be
        // rejected — proving we must feed `eth_l1_block_number`, not the L2 one.
        let timeout_l1 = 25_280_598;
        let l2_height = 471_376_588;
        assert!(check_timeout_margin(timeout_l1, l2_height).is_err());
    }

    #[macros::test_all]
    fn cctp_forward_hook_picks_setup_vs_plain() {
        // No recipient setup → the fixed 32-byte EVM forward tag (also used for
        // Solana recipients whose ATA already exists). Destination is irrelevant.
        let plain = cctp_forward_hook("0x1111111111111111111111111111111111111111", false).unwrap();
        assert_eq!(plain, cctp::evm_forward_hook_data().to_vec());

        // Recipient setup → the Solana ATA-creating hook, which is longer than
        // the 32-byte tag and varies by recipient.
        let recipient = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let setup = cctp_forward_hook(recipient, true).unwrap();
        assert_eq!(setup, cctp::solana_forward_hook_data(recipient).unwrap());
        assert_ne!(setup, plain);
    }

    #[macros::test_all]
    fn matches_any_known_token_evm_case_insensitive() {
        let known = [ARBITRUM_USDT_ADDRESS, ARBITRUM_TBTC_ADDRESS];
        // Exact, lowercased, and uppercased all match (EVM is case-insensitive).
        assert!(matches_any_known_token(
            NetworkTransport::Evm,
            ARBITRUM_USDT_ADDRESS,
            &known
        ));
        assert!(matches_any_known_token(
            NetworkTransport::Evm,
            &ARBITRUM_USDT_ADDRESS.to_uppercase(),
            &known
        ));
        // Surrounding whitespace tolerated.
        assert!(matches_any_known_token(
            NetworkTransport::Evm,
            &format!("  {ARBITRUM_TBTC_ADDRESS}  "),
            &known
        ));
        // A normal recipient address is not a known token.
        assert!(!matches_any_known_token(
            NetworkTransport::Evm,
            "0x1234567890AbCdEf1234567890aBcDeF12345678",
            &known
        ));
    }

    #[macros::test_all]
    fn matches_any_known_token_solana_case_sensitive() {
        let known = [SOLANA_USDT0_MINT];
        assert!(matches_any_known_token(
            NetworkTransport::Solana,
            SOLANA_USDT0_MINT,
            &known
        ));
        // base58 is case-sensitive: a lowercased mint must NOT match.
        assert!(!matches_any_known_token(
            NetworkTransport::Solana,
            &SOLANA_USDT0_MINT.to_lowercase(),
            &known
        ));
    }

    #[macros::test_all]
    fn matches_any_known_token_empty_addr() {
        assert!(!matches_any_known_token(
            NetworkTransport::Evm,
            "   ",
            &[ARBITRUM_USDT_ADDRESS]
        ));
    }

    #[macros::test_all]
    fn test_current_unix_timestamp() {
        let ts = current_unix_timestamp();
        assert!(ts > 1_704_067_200);
    }

    #[macros::test_all]
    fn test_compute_invoice_amount() {
        let pair = ReversePairInfo {
            hash: "abc".to_string(),
            rate: 1.0,
            limits: crate::api::types::PairLimits {
                minimal: 10000,
                maximal: 25_000_000,
            },
            fees: crate::api::types::ReversePairFees {
                percentage: 0.25,
                miner_fees: crate::api::types::MinerFees {
                    claim: 170,
                    lockup: 171,
                },
            },
        };

        // 0.001 BTC = 100_000 sats
        let tbtc_sats: u64 = 100_000;
        let result = compute_invoice_amount(&pair, tbtc_sats).unwrap();

        // invoice should be > base (100_000 + 170 + 171 = 100_341)
        assert!(result.invoice_sats > 100_341);
        assert!(result.boltz_fee_sats > 0);
        assert!(result.onchain_sats > 0);
    }

    fn make_quote(amount: &str) -> QuoteResponse {
        QuoteResponse {
            quote: amount.to_string(),
            data: serde_json::json!({"type": "test"}),
        }
    }

    #[macros::test_all]
    fn test_pick_best_quote_in_direction() {
        // "in" direction: highest output wins
        let quotes = vec![make_quote("100"), make_quote("300"), make_quote("200")];
        let best = pick_best_quote(&quotes, QuoteDirection::In).unwrap();
        assert_eq!(best, 300);
    }

    #[macros::test_all]
    fn test_pick_best_quote_out_direction() {
        // "out" direction: lowest input wins
        let quotes = vec![make_quote("300"), make_quote("100"), make_quote("200")];
        let best = pick_best_quote(&quotes, QuoteDirection::Out).unwrap();
        assert_eq!(best, 100);
    }

    #[macros::test_all]
    fn test_pick_best_quote_single() {
        let quotes = vec![make_quote("42")];
        assert_eq!(pick_best_quote(&quotes, QuoteDirection::In).unwrap(), 42);
        assert_eq!(pick_best_quote(&quotes, QuoteDirection::Out).unwrap(), 42);
    }

    #[macros::test_all]
    fn test_pick_best_quote_empty() {
        let quotes: Vec<QuoteResponse> = vec![];
        assert!(pick_best_quote(&quotes, QuoteDirection::In).is_err());
    }

    #[macros::test_all]
    fn test_parse_percentage_to_bps() {
        assert_eq!(parse_percentage_to_bps(0.25).unwrap(), 25);
        assert_eq!(parse_percentage_to_bps(0.5).unwrap(), 50);
        assert_eq!(parse_percentage_to_bps(1.0).unwrap(), 100);
        assert_eq!(parse_percentage_to_bps(0.0).unwrap(), 0);
        assert_eq!(parse_percentage_to_bps(0.1).unwrap(), 10);
    }

    #[macros::test_all]
    fn test_parse_percentage_to_bps_sub_bps_rejected() {
        // 0.125% = 12.5 bps — sub-bps precision
        assert!(parse_percentage_to_bps(0.125).is_err());
    }

    fn test_pair(percentage: f64) -> ReversePairInfo {
        ReversePairInfo {
            hash: "abc".to_string(),
            rate: 1.0,
            limits: crate::api::types::PairLimits {
                minimal: 10000,
                maximal: 25_000_000,
            },
            fees: crate::api::types::ReversePairFees {
                percentage,
                miner_fees: crate::api::types::MinerFees {
                    claim: 2,
                    lockup: 6,
                },
            },
        }
    }

    #[macros::test_all]
    fn test_compute_onchain_amount() {
        let pair = test_pair(0.25);
        let result = compute_onchain_amount(&pair, 10000).unwrap();

        // boltz_fee = ceil(10000 * 25 / 10000) = 25
        assert_eq!(result.boltz_fee_sats, 25);
        // onchain = 10000 - 25 - 8 = 9967
        assert_eq!(result.onchain_sats, 9967);
        assert_eq!(result.invoice_sats, 10000);
    }

    #[macros::test_all]
    fn test_compute_onchain_amount_too_small() {
        let pair = test_pair(0.25);
        // Amount too small to cover miner fees
        assert!(compute_onchain_amount(&pair, 5).is_err());
    }

    #[macros::test_all]
    fn test_compute_invoice_and_onchain_roundtrip() {
        // Verify that compute_invoice_amount and compute_onchain_amount are consistent:
        // compute_onchain_amount(compute_invoice_amount(x).invoice_sats).onchain_sats
        // should be close to the original tbtc_sats (within rounding).
        let pair = test_pair(0.25);
        let invoice = compute_invoice_amount(&pair, 100_000).unwrap();
        let back = compute_onchain_amount(&pair, invoice.invoice_sats).unwrap();

        // onchain_sats from roundtrip should match the original (100_000)
        // Allow 1 sat tolerance for ceiling rounding
        let diff = invoice.onchain_sats.abs_diff(back.onchain_sats);
        assert!(
            diff <= 1,
            "roundtrip diff={diff}, invoice_onchain={}, back_onchain={}",
            invoice.onchain_sats,
            back.onchain_sats
        );
    }

    #[macros::test_all]
    fn test_pick_best_quote_preserves_data() {
        let quotes = vec![
            QuoteResponse {
                quote: "100".to_string(),
                data: serde_json::json!({"route": "A"}),
            },
            QuoteResponse {
                quote: "200".to_string(),
                data: serde_json::json!({"route": "B"}),
            },
        ];
        let best = pick_best_quote_with_data(&quotes, QuoteDirection::In).unwrap();
        assert_eq!(best.amount, 200);
        assert_eq!(best.data, serde_json::json!({"route": "B"}));
    }

    #[macros::test_all]
    fn test_check_quote_drift_within_tolerance() {
        // Expected 1000 USDT, got 995 (0.5% drop), slippage 1% → OK
        assert!(check_quote_drift(1_000_000, 995_000, 100).is_ok());
    }

    #[macros::test_all]
    fn test_check_quote_drift_at_boundary() {
        // Expected 1000 USDT, got 990 (exactly 1% drop), slippage 1% → OK
        assert!(check_quote_drift(1_000_000, 990_000, 100).is_ok());
    }

    #[macros::test_all]
    fn test_check_quote_drift_beyond_tolerance() {
        // Expected 1000 USD, got 980 (2% drop), slippage 1% → error
        let err = check_quote_drift(1_000_000, 980_000, 100).unwrap_err();
        assert!(matches!(
            err,
            BoltzError::QuoteDegradedBeyondSlippage {
                expected_usd: 1_000_000,
                quoted_usd: 980_000,
            }
        ));
    }

    #[macros::test_all]
    fn test_check_quote_drift_better_quote_ok() {
        // Expected 1000 USDT, got 1050 (better!) → always OK
        assert!(check_quote_drift(1_000_000, 1_050_000, 100).is_ok());
    }

    #[macros::test_all]
    fn test_check_quote_drift_zero_expected() {
        // A swap with no prepare-time expected amount (expected=0) should
        // always pass the drift check.
        assert!(check_quote_drift(0, 500_000, 100).is_ok());
    }

    // ─── Slippage override resolution ────────────────────────────────

    #[macros::test_all]
    fn resolve_slippage_override_wins_over_config() {
        assert_eq!(resolve_slippage_bps(Some(250), 100).unwrap(), 250);
    }

    #[macros::test_all]
    fn resolve_slippage_falls_back_to_config_when_none() {
        assert_eq!(resolve_slippage_bps(None, 100).unwrap(), 100);
    }

    #[macros::test_all]
    fn resolve_slippage_accepts_bounds() {
        assert_eq!(resolve_slippage_bps(Some(10), 100).unwrap(), 10);
        assert_eq!(
            resolve_slippage_bps(Some(MAX_SLIPPAGE_BPS), 100).unwrap(),
            MAX_SLIPPAGE_BPS
        );
    }

    #[macros::test_all]
    fn resolve_slippage_rejects_below_min() {
        assert!(resolve_slippage_bps(Some(9), 100).is_err());
    }

    #[macros::test_all]
    fn resolve_slippage_rejects_above_max() {
        assert!(resolve_slippage_bps(Some(MAX_SLIPPAGE_BPS + 1), 100).is_err());
    }

    #[macros::test_all]
    fn resolve_slippage_rejects_out_of_range_config_default() {
        // Override is None but config default is out of range — validation
        // applies to the resolved value regardless of origin.
        assert!(resolve_slippage_bps(None, 5).is_err());
        assert!(resolve_slippage_bps(None, MAX_SLIPPAGE_BPS + 1).is_err());
    }

    #[macros::test_all]
    fn resolve_slippage_override_accepted_even_when_config_is_out_of_range() {
        // If a caller explicitly sets a valid per-swap override, a broken
        // config default must not poison the prepare call.
        assert_eq!(resolve_slippage_bps(Some(150), 10_000).unwrap(), 150);
    }

    // ─── Slippage helpers ────────────────────────────────────────────

    #[macros::test_all]
    fn apply_slippage_down_zero_bps_is_identity() {
        assert_eq!(apply_slippage_down(1_000_000, 0).unwrap(), 1_000_000);
    }

    #[macros::test_all]
    fn apply_slippage_down_one_percent() {
        assert_eq!(apply_slippage_down(1_000_000, 100).unwrap(), 990_000);
    }

    #[macros::test_all]
    fn apply_slippage_down_max_bound() {
        assert_eq!(
            apply_slippage_down(1_000_000, MAX_SLIPPAGE_BPS).unwrap(),
            950_000
        );
    }

    #[macros::test_all]
    fn apply_slippage_down_floors_to_zero_for_tiny_amounts() {
        assert_eq!(apply_slippage_down(1, 100).unwrap(), 0);
    }

    #[macros::test_all]
    fn apply_slippage_down_is_single_application_not_compound() {
        // End-to-end semantics check for the cross-chain claim path:
        // applying slippage once must yield `amount * (1 - s)`, NOT
        // `amount * (1 - s)²`. Regression guard against any future
        // re-introduction of per-leg compounding.
        let raw = 1_000_000_u128;
        let single = apply_slippage_down(raw, 100).unwrap(); // 990_000
        let compound = apply_slippage_down(single, 100).unwrap(); // 980_100
        assert_eq!(single, 990_000);
        assert_eq!(compound, 980_100);
        assert!(compound < single);
    }

    #[macros::test_all]
    fn apply_slippage_overflow_errors_instead_of_wrapping() {
        // A pathological (RPC-controlled) amount near u128::MAX must surface a
        // bounded error, not wrap silently in release.
        assert!(apply_slippage_up(u128::MAX, 100).is_err());
        assert!(apply_slippage_down(u128::MAX, 100).is_err());
        // Out-of-range slippage can't underflow the factor either.
        assert!(apply_slippage_down(1_000_000, 10_001).is_err());
    }

    #[macros::test_all]
    fn apply_slippage_up_one_percent() {
        assert_eq!(apply_slippage_up(1_000_000, 100).unwrap(), 1_010_000);
    }

    // ─── Claim floor (promise honoring) ──────────────────────────────

    #[macros::test_all]
    fn claim_floor_anchors_on_expected_in_normal_mode() {
        // Even when raw_quote dropped below expected (still passes drift),
        // the floor must equal `expected × (1 − s)` — never `raw × (1 − s)`,
        // which would compound and break the user's promise.
        let expected = 1_000_000_u64;
        let raw = 992_000_u128; // dropped 0.8% from prepare quote, within 1% drift
        let floor = compute_claim_floor(raw, expected, 100, false).unwrap();
        assert_eq!(floor, 990_000); // expected × 0.99
        assert_ne!(floor, 982_080); // would be raw × 0.99 (the broken case)
    }

    #[macros::test_all]
    fn claim_floor_anchors_on_expected_when_raw_is_higher() {
        // Favorable movement: raw_quote > expected. Floor stays at the
        // promise — the user is guaranteed at least the promise; anything
        // above that is a bonus.
        let expected = 1_000_000_u64;
        let raw = 1_010_000_u128;
        assert_eq!(
            compute_claim_floor(raw, expected, 100, false).unwrap(),
            990_000
        );
    }

    #[macros::test_all]
    fn claim_floor_falls_back_to_raw_when_drift_check_skipped() {
        // When the drift check is skipped (accept_degraded_quote), the floor
        // is anchored on the live quote so the claim still has a meaningful min.
        let raw = 500_000_u128;
        assert_eq!(compute_claim_floor(raw, 0, 100, true).unwrap(), 495_000);
    }

    #[macros::test_all]
    fn claim_floor_zero_expected_in_normal_mode_is_zero() {
        // Defensive: a normal-mode claim with expected=0 returns floor=0,
        // and the call site rejects that as "amount too small".
        assert_eq!(compute_claim_floor(500_000, 0, 100, false).unwrap(), 0);
    }

    // ─── OFT approval gate tests ─────────────────────────────────────

    fn approval_test_addrs() -> (alloy_primitives::Address, alloy_primitives::Address) {
        let oft = contracts::parse_address("0x77652D5aba086137b595875263FC200182919B92").unwrap();
        let token = contracts::parse_address("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9").unwrap();
        (oft, token)
    }

    #[macros::test_all]
    fn decide_approval_skips_when_allowance_far_above_threshold() {
        let (oft, token) = approval_test_addrs();
        let amount = U256::from(1_000_000u64);
        // 100x the amount — well above the 10x threshold.
        let allowance = U256::from(100_000_000u64);
        assert!(decide_oft_approval_top_up(allowance, amount, oft, token).is_none());
    }

    #[macros::test_all]
    fn decide_approval_skips_at_exact_threshold() {
        let (oft, token) = approval_test_addrs();
        let amount = U256::from(1_000_000u64);
        // allowance == amount * 10 exactly — `>=` means no top-up.
        let allowance = U256::from(10_000_000u64);
        assert!(decide_oft_approval_top_up(allowance, amount, oft, token).is_none());
    }

    #[macros::test_all]
    fn decide_approval_tops_up_just_below_threshold() {
        let (oft, token) = approval_test_addrs();
        let amount = U256::from(1_000_000u64);
        // allowance == amount * 10 - 1 — one wei below the gate.
        let allowance = U256::from(9_999_999u64);
        let call =
            decide_oft_approval_top_up(allowance, amount, oft, token).expect("should top up");
        assert_eq!(call.target, token);
        assert_eq!(call.value, U256::ZERO);
    }

    #[macros::test_all]
    fn decide_approval_tops_up_when_allowance_is_zero() {
        let (oft, token) = approval_test_addrs();
        let amount = U256::from(1u64);
        let call = decide_oft_approval_top_up(U256::ZERO, amount, oft, token)
            .expect("should top up with zero allowance");
        assert_eq!(call.target, token);
    }

    #[macros::test_all]
    fn decide_approval_skips_when_amount_is_zero() {
        // Degenerate edge: amount == 0 → threshold == 0. Any allowance
        // (including 0) satisfies `>=`, so no top-up is emitted.
        let (oft, token) = approval_test_addrs();
        assert!(decide_oft_approval_top_up(U256::ZERO, U256::ZERO, oft, token).is_none());
    }

    #[macros::test_all]
    fn decide_approval_saturates_on_pathological_amount() {
        // amount * 10 overflows U256; `saturating_mul` clamps to U256::MAX.
        // Only an allowance == U256::MAX can satisfy `>= U256::MAX`.
        let (oft, token) = approval_test_addrs();
        let huge_amount = U256::MAX;
        // allowance strictly below MAX → top up.
        assert!(
            decide_oft_approval_top_up(U256::MAX - U256::from(1u64), huge_amount, oft, token)
                .is_some()
        );
        // allowance at MAX → no top up.
        assert!(decide_oft_approval_top_up(U256::MAX, huge_amount, oft, token).is_none());
    }

    #[macros::test_all]
    fn decide_approval_emits_max_uint256_for_the_oft_spender() {
        // Verify the generated `Call` really encodes `approve(oft, MaxUint256)`
        // on the underlying token — catches target/spender mix-ups. We rely
        // on `contracts::encode_approve` being independently validated in
        // its own unit tests (selector + length + args roundtrip).
        let (oft, token) = approval_test_addrs();
        let call =
            decide_oft_approval_top_up(U256::ZERO, U256::from(1_000_000u64), oft, token).unwrap();

        assert_eq!(call.target, token);
        assert_eq!(call.value, U256::ZERO);

        let expected_calldata = contracts::encode_approve(oft, U256::MAX);
        assert_eq!(call.callData.as_ref(), expected_calldata.as_slice());

        // Cross-check: swapping oft/token would change the calldata, so a
        // mix-up where we accidentally target `oft` and spend `token` would
        // be caught by this byte-equality assertion against the
        // independently-tested `encode_approve`.
        let wrong_way = contracts::encode_approve(token, U256::MAX);
        assert_ne!(call.callData.as_ref(), wrong_way.as_slice());
    }
}
