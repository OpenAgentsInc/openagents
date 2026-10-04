//! Circle CCTP v2 helpers: `mintRecipient` encoding, forwarding-service
//! `hookData`, and burn-fee math.
//!
//! Mirrors boltz-web-app `boltz-swaps` `cctp/evm.ts` + the fee math in
//! `CctpBridgeDriver`. The Arbitrum -> destination burn is driven by the
//! Router's `claimERC20ExecuteCctp`; these helpers build the `CctpData`
//! field values for it.

use std::collections::HashMap;

use alloy_primitives::FixedBytes;
use serde::Deserialize;

use platform_utils::http::HttpClient;

use crate::config::{CCTP_FEE_BPS_DENOMINATOR, CCTP_MAX_FEE_BUFFER_BPS, SOLANA_USDC_MINT};
use crate::error::BoltzError;
use crate::evm::contracts::{address_to_bytes32, parse_address};
use crate::solana::ata::derive_ata;

/// Basis-points denominator for the `maxFee` buffer.
const BPS_DENOMINATOR: u128 = 10_000;

/// Byte length of the Solana hookData prefix that precedes the payload: the
/// `"cctp-forward"` tag zero-padded — the first 28 bytes of the EVM forward
/// hookData.
///
/// Equals the web app's `cctpForwardHookData.slice(0, 58)`, but note that slices
/// a `"0x"`-prefixed hex *string*: 58 chars is `"0x"` + 56 hex = **28 bytes**,
/// not 29 (the count includes the `0x`). A 29-byte prefix shifts the 4-byte
/// dataLength that follows by one, so Circle reads length 0 and drops the
/// payload (the ATA-creation flag + wallet), and the forward fails.
const SOLANA_FORWARD_PREFIX_LEN: usize = 28;

/// Decode a base58 Solana pubkey into its 32-byte form.
fn decode_solana_pubkey(s: &str) -> Result<[u8; 32], BoltzError> {
    let decoded = bs58::decode(s)
        .into_vec()
        .map_err(|e| BoltzError::Generic(format!("Invalid Solana pubkey '{s}': {e}")))?;
    let arr: [u8; 32] = decoded.try_into().map_err(|v: Vec<u8>| {
        BoltzError::Generic(format!(
            "Solana pubkey '{s}' must decode to 32 bytes, got {}",
            v.len()
        ))
    })?;
    Ok(arr)
}

/// CCTP `mintRecipient` for an EVM destination: the 20-byte recipient address
/// left-padded to `bytes32`.
pub fn evm_mint_recipient(address: &str) -> Result<FixedBytes<32>, BoltzError> {
    Ok(address_to_bytes32(parse_address(address)?))
}

/// CCTP `mintRecipient` for a Solana destination: the recipient's USDC
/// Associated Token Account (32-byte pubkey). CCTP mints into a token account,
/// so the recipient must be the ATA, not the wallet; the wallet rides in the
/// Solana forwarding `hookData` so Circle's forwarder can create the ATA.
pub fn solana_mint_recipient(recipient_wallet: &str) -> Result<FixedBytes<32>, BoltzError> {
    let owner = decode_solana_pubkey(recipient_wallet)?;
    let mint = decode_solana_pubkey(SOLANA_USDC_MINT)?;
    let ata = derive_ata(&owner, &mint)?;
    Ok(FixedBytes::from(ata))
}

/// EVM forwarding-service `hookData`: the ASCII tag `"cctp-forward"` (12 bytes)
/// right-padded to 32 bytes. Built directly (rather than decoding the hex
/// const) to avoid a fallible decode in a hot path; a unit test pins it equal
/// to [`CCTP_FORWARD_HOOK_DATA_HEX`].
#[must_use]
pub fn evm_forward_hook_data() -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..12].copy_from_slice(b"cctp-forward");
    out
}

/// Solana forwarding-service `hookData`: the 28-byte `"cctp-forward"` prefix
/// (tag zero-padded) + 4-byte payload length `0x00000021` (33) + ATA-creation
/// flag `0x01` + the 32-byte recipient wallet pubkey. Mirrors the web app's
/// `createCctpSolanaForwardHookData`. Used when the recipient's USDC ATA does
/// not yet exist so Circle's forwarder creates it on delivery.
pub fn solana_forward_hook_data(recipient_wallet: &str) -> Result<Vec<u8>, BoltzError> {
    let wallet = decode_solana_pubkey(recipient_wallet)?;
    let prefix = evm_forward_hook_data();

    let mut out = Vec::with_capacity(SOLANA_FORWARD_PREFIX_LEN + 4 + 1 + 32);
    out.extend_from_slice(&prefix[..SOLANA_FORWARD_PREFIX_LEN]);
    out.extend_from_slice(&[0x00, 0x00, 0x00, 0x21]); // length = 0x21 = 33
    out.push(0x01); // ATA-creation flag
    out.extend_from_slice(&wallet);
    Ok(out)
}

/// Total CCTP burn fee deducted from the burned amount: the protocol fee
/// (`amount * bpsUnits / CCTP_FEE_BPS_DENOMINATOR`) plus the flat forwarding
/// fee. `bps_units` is Circle's `minimumFee` scaled by `CCTP_FEE_SCALE`.
#[must_use]
pub fn compute_total_fee(amount: u128, bps_units: u128, forward_fee: u128) -> u128 {
    amount
        .saturating_mul(bps_units)
        .checked_div(CCTP_FEE_BPS_DENOMINATOR)
        .unwrap_or(0)
        .saturating_add(forward_fee)
}

/// Invert the burn fee: the USDC amount the DEX must produce so that, after
/// the CCTP fee is deducted from the burn, at least `target` arrives on the
/// destination. Solves `burn * (denom - bps_units) / denom - forward_fee >=
/// target` for `burn`, rounding up. Used at prepare time to size the swap.
#[must_use]
pub fn cctp_required_burn(target: u128, fee: &CctpFee) -> u128 {
    let net = CCTP_FEE_BPS_DENOMINATOR.saturating_sub(fee.bps_units);
    if net == 0 {
        return u128::MAX;
    }
    let numerator = target
        .saturating_add(fee.forward_fee)
        .saturating_mul(CCTP_FEE_BPS_DENOMINATOR);
    // Ceiling division so we never under-size the burn.
    numerator
        .saturating_add(net.saturating_sub(1))
        .checked_div(net)
        .unwrap_or(u128::MAX)
}

/// Add the `maxFee` cushion (`CCTP_MAX_FEE_BUFFER_BPS`) on top of the quoted
/// fee, rounding up, so fee movement between prepare and claim doesn't make the
/// burn revert. This is a fee cap, NOT user slippage.
#[must_use]
pub fn add_fee_buffer(amount: u128) -> u128 {
    let numerator =
        amount.saturating_mul(BPS_DENOMINATOR.saturating_add(u128::from(CCTP_MAX_FEE_BUFFER_BPS)));
    // Ceiling division: (numerator + denom - 1) / denom.
    numerator
        .saturating_add(BPS_DENOMINATOR.saturating_sub(1))
        .checked_div(BPS_DENOMINATOR)
        .unwrap_or(0)
}

/// Circle CCTP burn fee for one route/finality, resolved from the Iris API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CctpFee {
    /// Protocol fee in basis points, scaled by [`CCTP_FEE_SCALE`].
    pub bps_units: u128,
    /// Flat forwarding-service fee, in the burn token's smallest units.
    pub forward_fee: u128,
}

/// Parse a decimal value (number or numeric string) into an integer scaled by
/// `10^scale_digits`, truncating any fraction beyond `scale_digits`. Used for
/// Circle's `minimumFee` (bps, scaled by 9) and `forwardFee` (scale 0).
fn parse_scaled(value: &serde_json::Value, scale_digits: u32) -> Result<u128, BoltzError> {
    let s = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        other => {
            return Err(BoltzError::Generic(format!(
                "invalid CCTP fee value: {other}"
            )));
        }
    };

    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s.as_str(), ""),
    };

    let parse_digits = |d: &str| -> Result<u128, BoltzError> {
        if d.is_empty() {
            return Ok(0);
        }
        d.parse::<u128>()
            .map_err(|_| BoltzError::Generic(format!("invalid CCTP fee number '{s}'")))
    };

    let scale = scale_digits as usize;
    // Truncate or right-pad the fractional part to exactly `scale` digits.
    let mut frac = String::with_capacity(scale);
    frac.push_str(frac_part.get(..scale.min(frac_part.len())).unwrap_or(""));
    while frac.len() < scale {
        frac.push('0');
    }

    let int_scaled = parse_digits(int_part)?
        .checked_mul(10u128.pow(scale_digits))
        .ok_or_else(|| BoltzError::Generic("CCTP fee overflow".into()))?;
    let frac_scaled = if frac.is_empty() {
        0
    } else {
        parse_digits(&frac)?
    };

    int_scaled
        .checked_add(frac_scaled)
        .ok_or_else(|| BoltzError::Generic("CCTP fee overflow".into()))
}

/// Client for Circle's Iris CCTP fee API.
pub struct CctpFeeClient {
    http: Box<dyn HttpClient>,
    api_url: String,
}

impl CctpFeeClient {
    pub fn new(http: Box<dyn HttpClient>, api_url: String) -> Self {
        // Normalize: drop a single trailing slash so path joins are clean.
        let api_url = match api_url.strip_suffix('/') {
            Some(trimmed) => trimmed.to_string(),
            None => api_url,
        };
        Self { http, api_url }
    }

    /// Fetch the burn fee for `source_domain -> dest_domain` at the given
    /// `finality_threshold` (Fast=1000 / Standard=2000), in Forwarded mode.
    ///
    /// `include_recipient_setup` must be set when the forwarding hook will also
    /// create the destination recipient's token account (a first-time Solana
    /// USDC recipient whose ATA doesn't exist yet). Circle then quotes the
    /// higher forward fee that covers the account-creation rent, so the
    /// `maxFee` we cap the burn with matches what will actually be deducted.
    /// Mirrors the web app's `getCctpFee(..., includeRecipientSetup)`.
    pub async fn get_fee(
        &self,
        source_domain: u32,
        dest_domain: u32,
        finality_threshold: u32,
        include_recipient_setup: bool,
    ) -> Result<CctpFee, BoltzError> {
        let url = Self::fee_url(
            &self.api_url,
            source_domain,
            dest_domain,
            include_recipient_setup,
        );
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "application/json".to_string());

        let response = self
            .http
            .get(url, Some(headers))
            .await
            .map_err(|e| BoltzError::Generic(format!("CCTP fee request failed: {e}")))?;

        if !response.is_success() {
            return Err(BoltzError::Generic(format!(
                "CCTP fee HTTP error {}: {}",
                response.status, response.body
            )));
        }

        Self::parse_fee_response(&response.body, finality_threshold)
    }

    /// Build the Iris burn-fee request URL. Always Forwarded (`forward=true`);
    /// appends `includeRecipientSetup=true` when the forwarding hook will also
    /// create the recipient's token account. Split out for testing.
    fn fee_url(
        api_url: &str,
        source_domain: u32,
        dest_domain: u32,
        include_recipient_setup: bool,
    ) -> String {
        let mut url =
            format!("{api_url}/v2/burn/USDC/fees/{source_domain}/{dest_domain}?forward=true");
        if include_recipient_setup {
            url.push_str("&includeRecipientSetup=true");
        }
        url
    }

    /// Query the status of a CCTP message by its source-chain burn tx hash.
    /// `GET /v2/messages/{source_domain}?transactionHash=...`. A 404 means the
    /// burn hasn't been indexed yet (treated as not-yet-found, still pending).
    pub async fn get_message_status(
        &self,
        source_domain: u32,
        source_tx_hash: &str,
    ) -> Result<CctpMessageStatus, BoltzError> {
        let url = format!(
            "{}/v2/messages/{source_domain}?transactionHash={source_tx_hash}",
            self.api_url
        );
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "application/json".to_string());

        let response = self
            .http
            .get(url, Some(headers))
            .await
            .map_err(|e| BoltzError::Generic(format!("CCTP message request failed: {e}")))?;

        // Not indexed yet — the message is still pending, not an error.
        if response.status == 404 {
            return Ok(CctpMessageStatus::default());
        }
        if !response.is_success() {
            return Err(BoltzError::Generic(format!(
                "CCTP message HTTP error {}: {}",
                response.status, response.body
            )));
        }

        Self::parse_message_status(&response.body)
    }

    /// Parse the Iris `/v2/messages` response. Split out for testing against
    /// recorded JSON.
    fn parse_message_status(body: &str) -> Result<CctpMessageStatus, BoltzError> {
        let parsed: CctpMessagesResponse = serde_json::from_str(body).map_err(|e| {
            BoltzError::Generic(format!("failed to parse CCTP messages response: {e}"))
        })?;
        let Some(msg) = parsed.messages.into_iter().next() else {
            return Ok(CctpMessageStatus::default());
        };
        // The authoritative delivered amount comes from the attested message's
        // finalized feeExecuted; filled in by the caller (which owns the
        // message decoder) when `message` is present.
        let delivered_amount = msg
            .message
            .as_deref()
            .and_then(crate::evm::contracts::decode_cctp_delivered_from_message);

        Ok(CctpMessageStatus {
            found: true,
            status: msg.status,
            attestation: msg.attestation,
            forward_tx_hash: msg.forward_tx_hash,
            message: msg.message,
            delivered_amount,
            forward_state: msg.forward_state,
            forward_error_code: msg.forward_error_code,
        })
    }

    /// Parse the Iris fee response body and extract the fee for
    /// `finality_threshold`. Split out for testing against recorded JSON.
    fn parse_fee_response(body: &str, finality_threshold: u32) -> Result<CctpFee, BoltzError> {
        let entries: Vec<CctpFeeEntry> = serde_json::from_str(body).map_err(|e| {
            BoltzError::Generic(format!(
                "failed to parse CCTP fee response: {e} (body: {body})"
            ))
        })?;

        let entry = entries
            .into_iter()
            .find(|e| e.finality_threshold == finality_threshold)
            .ok_or_else(|| {
                BoltzError::Generic(format!(
                    "missing CCTP fee for finality threshold {finality_threshold}"
                ))
            })?;

        let bps_units = parse_scaled(&entry.minimum_fee, CCTP_FEE_SCALE_DIGITS)?;
        let forward_fee = match entry.forward_fee {
            Some(f) => parse_scaled(&f.med, 0)?,
            None => {
                return Err(BoltzError::Generic(format!(
                    "missing CCTP forward fee for finality threshold {finality_threshold}"
                )));
            }
        };

        Ok(CctpFee {
            bps_units,
            forward_fee,
        })
    }
}

/// Scale (in decimal digits) Circle applies to `minimumFee`. `CCTP_FEE_SCALE`
/// is `10^CCTP_FEE_SCALE_DIGITS`.
const CCTP_FEE_SCALE_DIGITS: u32 = 9;

/// Status of a CCTP message from the Iris `/v2/messages` endpoint, used to
/// confirm destination-side delivery for a USDC swap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CctpMessageStatus {
    /// `false` when Iris hasn't indexed the burn yet (HTTP 404 or empty list);
    /// the message is still pending.
    pub found: bool,
    /// Circle status string, e.g. `"pending_confirmations"` / `"complete"`.
    pub status: Option<String>,
    /// Attestation signature, present once the message is attested.
    pub attestation: Option<String>,
    /// Forwarding-service tx hash on the destination chain, present once the
    /// forward (mint) has been submitted.
    pub forward_tx_hash: Option<String>,
    /// The attested CCTP message hex, present once attested. Its burn body
    /// carries the finalized `feeExecuted`, from which the authoritative
    /// delivered amount is derived.
    pub message: Option<String>,
    /// Authoritative delivered amount (burn amount minus the finalized CCTP
    /// fee), parsed from `message`. Present only once attested.
    pub delivered_amount: Option<u64>,
    /// Circle forwarding-service state (`"PENDING"`/`"COMPLETE"`/`"FAILED"`).
    /// A `FAILED` forward means the mint must land some other way (e.g. a
    /// third-party `receiveMessage`); the funds are attested and recoverable,
    /// not lost — so it is surfaced, never treated as a swap failure.
    pub forward_state: Option<String>,
    /// Forwarding error code when `forward_state` is `FAILED` (e.g.
    /// `"INTERNAL_ERROR"`). Diagnostic only.
    pub forward_error_code: Option<String>,
}

impl CctpMessageStatus {
    /// Whether the destination mint has been forwarded (delivery on its way /
    /// done).
    #[must_use]
    pub fn is_forwarded(&self) -> bool {
        self.forward_tx_hash.is_some()
    }

    /// Whether Circle's forwarding service reported the forward as failed. The
    /// funds are still attested and recoverable; delivery must be confirmed
    /// from the destination chain instead.
    #[must_use]
    pub fn forward_failed(&self) -> bool {
        self.forward_state
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("FAILED"))
    }
}

#[derive(Deserialize)]
struct CctpMessagesResponse {
    #[serde(default)]
    messages: Vec<CctpMessageSnapshot>,
}

#[derive(Deserialize)]
struct CctpMessageSnapshot {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    attestation: Option<String>,
    #[serde(default, rename = "forwardTxHash")]
    forward_tx_hash: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default, rename = "forwardState")]
    forward_state: Option<String>,
    #[serde(default, rename = "forwardErrorCode")]
    forward_error_code: Option<String>,
}

#[derive(Deserialize)]
struct CctpFeeEntry {
    #[serde(rename = "finalityThreshold")]
    finality_threshold: u32,
    #[serde(rename = "minimumFee")]
    minimum_fee: serde_json::Value,
    #[serde(rename = "forwardFee")]
    forward_fee: Option<CctpForwardFee>,
}

#[derive(Deserialize)]
struct CctpForwardFee {
    med: serde_json::Value,
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::config::{CCTP_FEE_SCALE, CCTP_FORWARD_HOOK_DATA_HEX};

    // Sanity: the scale constant and digit count agree.
    #[macros::test_all]
    fn fee_scale_matches_digits() {
        assert_eq!(CCTP_FEE_SCALE, 10u128.pow(CCTP_FEE_SCALE_DIGITS));
    }

    #[macros::test_all]
    fn evm_forward_hook_matches_config_hex() {
        let expected = hex::decode(CCTP_FORWARD_HOOK_DATA_HEX).unwrap();
        assert_eq!(evm_forward_hook_data().to_vec(), expected);
    }

    #[macros::test_all]
    fn evm_mint_recipient_left_pads() {
        let r = evm_mint_recipient("0x1234567890AbCdEf1234567890aBcDeF12345678").unwrap();
        assert_eq!(&r[..12], &[0u8; 12]);
        assert_eq!(
            &r[12..],
            &hex::decode("1234567890abcdef1234567890abcdef12345678").unwrap()[..]
        );
    }

    #[macros::test_all]
    fn evm_mint_recipient_rejects_garbage() {
        assert!(evm_mint_recipient("not-an-address").is_err());
    }

    #[macros::test_all]
    fn solana_mint_recipient_is_recipient_ata() {
        // Arbitrary valid Solana wallet pubkey.
        let wallet = "BZkwksSEeHrCVS3HeewBJKEBTEEuwnEqpkHqEg1dRpuE";
        let r = solana_mint_recipient(wallet).unwrap();
        // The ATA is derived (off-curve), so it must differ from the wallet itself.
        let wallet_bytes = bs58::decode(wallet).into_vec().unwrap();
        assert_ne!(r.as_slice(), &wallet_bytes[..]);
        // Deterministic.
        assert_eq!(r, solana_mint_recipient(wallet).unwrap());
    }

    #[macros::test_all]
    fn solana_forward_hook_structure() {
        let wallet = "BZkwksSEeHrCVS3HeewBJKEBTEEuwnEqpkHqEg1dRpuE";
        let hook = solana_forward_hook_data(wallet).unwrap();

        assert_eq!(hook.len(), 28 + 4 + 1 + 32);
        // Prefix = "cctp-forward" tag (12B) zero-padded to 28 bytes.
        assert_eq!(&hook[..12], b"cctp-forward");
        assert_eq!(&hook[12..28], &[0u8; 16]);
        // dataLength = 0x00000021 (33), flag = 0x01.
        assert_eq!(&hook[28..32], &[0x00, 0x00, 0x00, 0x21]);
        assert_eq!(hook[32], 0x01);
        // Trailing 32 bytes = the wallet pubkey.
        let wallet_bytes = bs58::decode(wallet).into_vec().unwrap();
        assert_eq!(&hook[33..], &wallet_bytes[..]);
    }

    /// Byte-parity with boltz-web-app's own golden vectors
    /// (`boltz-swaps` `cctp/evm.spec.ts`, `0x` stripped) — the reference
    /// implementation this is ported from. Pins the exact on-wire bytes: a
    /// prior 29-byte prefix shifted `dataLength` to 0, so Circle dropped the
    /// payload and the forward failed. See `SOLANA_FORWARD_PREFIX_LEN`.
    #[macros::test_all]
    fn solana_forward_hook_matches_web_app_vectors() {
        for (recipient, expected) in [
            (
                "11111111111111111111111111111111",
                "636374702d666f7277617264000000000000000000000000000000000000002101\
0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "EwwMqF8sFZRBGLchFfq61g5U7mPB14EnXxLQDWb5VAe5",
                "636374702d666f7277617264000000000000000000000000000000000000002101\
cf3ac201d92eadcae0cd69b431f4c0e6d96c06bdb2fa28271b00409b5f1622ca",
            ),
        ] {
            let hook = solana_forward_hook_data(recipient).unwrap();
            assert_eq!(hex::encode(hook), expected, "recipient {recipient}");
        }
    }

    #[macros::test_all]
    fn solana_forward_hook_rejects_bad_wallet() {
        assert!(solana_forward_hook_data("0xnot-base58").is_err());
    }

    #[macros::test_all]
    fn compute_total_fee_protocol_plus_forward() {
        // bps_units = minimumFee (in bps) scaled by CCTP_FEE_SCALE (10^9).
        // So `1` bps -> 1 * 10^9. Protocol fee = amount * bps_units / denom
        // = amount * bps / 10_000. On 1_000_000 units at 1 bps that is 100.
        // Plus a flat forward fee of 7 -> 107.
        let one_bps_scaled = 1_000_000_000u128;
        let fee = compute_total_fee(1_000_000, one_bps_scaled, 7);
        assert_eq!(fee, 107);
    }

    #[macros::test_all]
    fn compute_total_fee_zero_protocol() {
        assert_eq!(compute_total_fee(0, 1_000_000_000, 5), 5);
    }

    #[macros::test_all]
    fn add_fee_buffer_rounds_up() {
        // 2 bps buffer on 10_000 = exactly 10_002.
        assert_eq!(add_fee_buffer(10_000), 10_002);
        // On a value where the buffer is fractional, it rounds up.
        // 1 * (10002) / 10000 = 1.0002 -> ceil -> 2.
        assert_eq!(add_fee_buffer(1), 2);
        assert_eq!(add_fee_buffer(0), 0);
    }

    #[macros::test_all]
    fn cctp_required_burn_covers_target_plus_fee() {
        // 1 bps protocol + flat forward fee of 50, target 1_000_000.
        let fee = CctpFee {
            bps_units: 1_000_000_000, // 1 bps scaled
            forward_fee: 50,
        };
        let burn = cctp_required_burn(1_000_000, &fee);
        // Burning `burn` and deducting the fee must leave >= target.
        let actual_fee = compute_total_fee(burn, fee.bps_units, fee.forward_fee);
        assert!(burn.saturating_sub(actual_fee) >= 1_000_000);
        // ...and not be wildly over-sized (within fee + 1 of target).
        assert!(burn <= 1_000_000 + actual_fee + 1);
    }

    #[macros::test_all]
    fn cctp_required_burn_zero_fee_is_target() {
        let fee = CctpFee {
            bps_units: 0,
            forward_fee: 0,
        };
        assert_eq!(cctp_required_burn(1_000_000, &fee), 1_000_000);
    }

    #[macros::test_all]
    fn parse_scaled_handles_integers_and_decimals() {
        use serde_json::json;
        // Integer bps "1" scaled by 9.
        assert_eq!(parse_scaled(&json!("1"), 9).unwrap(), 1_000_000_000);
        // Numeric (not string) integer.
        assert_eq!(parse_scaled(&json!(2), 0).unwrap(), 2);
        // Decimal bps "0.5" scaled by 9.
        assert_eq!(parse_scaled(&json!("0.5"), 9).unwrap(), 500_000_000);
        // Fraction longer than the scale is truncated.
        assert_eq!(
            parse_scaled(&json!("1.2345678915"), 9).unwrap(),
            1_234_567_891
        );
        // Zero.
        assert_eq!(parse_scaled(&json!("0"), 9).unwrap(), 0);
    }

    #[macros::test_all]
    fn fee_url_appends_recipient_setup_only_when_needed() {
        // Default: Forwarded fee, no setup param.
        assert_eq!(
            CctpFeeClient::fee_url("https://iris-api.circle.com", 3, 5, false),
            "https://iris-api.circle.com/v2/burn/USDC/fees/3/5?forward=true"
        );
        // First-time Solana recipient: request the recipient-setup fee tier,
        // matching the web app's `...?forward=true&includeRecipientSetup=true`.
        assert_eq!(
            CctpFeeClient::fee_url("https://iris-api.circle.com", 3, 5, true),
            "https://iris-api.circle.com/v2/burn/USDC/fees/3/5?forward=true&includeRecipientSetup=true"
        );
    }

    #[macros::test_all]
    fn parse_fee_response_picks_finality_and_tier() {
        // Recorded-shape Iris response: two finality tiers, each with a
        // minimumFee (bps) and low/med/high forwardFee.
        let body = r#"[
            {"finalityThreshold":1000,"minimumFee":"1","forwardFee":{"low":"100","med":"150","high":"200"}},
            {"finalityThreshold":2000,"minimumFee":"0","forwardFee":{"low":"50","med":"75","high":"100"}}
        ]"#;

        let fast = CctpFeeClient::parse_fee_response(body, 1000).unwrap();
        assert_eq!(fast.bps_units, 1_000_000_000); // 1 bps scaled by 10^9
        assert_eq!(fast.forward_fee, 150); // med tier

        let standard = CctpFeeClient::parse_fee_response(body, 2000).unwrap();
        assert_eq!(standard.bps_units, 0);
        assert_eq!(standard.forward_fee, 75);
    }

    #[macros::test_all]
    fn parse_fee_response_missing_finality_errors() {
        let body = r#"[{"finalityThreshold":2000,"minimumFee":"0","forwardFee":{"low":"1","med":"2","high":"3"}}]"#;
        assert!(CctpFeeClient::parse_fee_response(body, 1000).is_err());
    }

    #[macros::test_all]
    fn parse_message_status_forwarded() {
        let body = r#"{"messages":[{"status":"complete","attestation":"0xabcd","forwardTxHash":"0xdead"}]}"#;
        let s = CctpFeeClient::parse_message_status(body).unwrap();
        assert!(s.found);
        assert_eq!(s.status.as_deref(), Some("complete"));
        assert_eq!(s.attestation.as_deref(), Some("0xabcd"));
        assert_eq!(s.forward_tx_hash.as_deref(), Some("0xdead"));
        assert!(s.is_forwarded());
    }

    #[macros::test_all]
    fn parse_message_status_pending_not_forwarded() {
        let body = r#"{"messages":[{"status":"pending_confirmations","attestation":"PENDING"}]}"#;
        let s = CctpFeeClient::parse_message_status(body).unwrap();
        assert!(s.found);
        assert_eq!(s.status.as_deref(), Some("pending_confirmations"));
        assert!(s.forward_tx_hash.is_none());
        assert!(!s.is_forwarded());
    }

    #[macros::test_all]
    fn parse_message_status_empty_is_not_found() {
        let s = CctpFeeClient::parse_message_status(r#"{"messages":[]}"#).unwrap();
        assert!(!s.found);
        assert!(!s.is_forwarded());
    }

    #[macros::test_all]
    fn parse_message_status_derives_delivered_from_attested_message() {
        // Attested message: burn amount at byte 216, finalized feeExecuted at
        // byte 312. delivered = amount - fee.
        let mut msg = vec![0u8; 344];
        msg[216..248].copy_from_slice(&[0u8; 32]);
        msg[244..248].copy_from_slice(&1_000_000u32.to_be_bytes()); // amount = 1_000_000
        msg[340..344].copy_from_slice(&250u32.to_be_bytes()); // feeExecuted = 250
        let message_hex = format!("0x{}", hex::encode(&msg));
        let body = format!(
            r#"{{"messages":[{{"status":"complete","attestation":"0xabcd","forwardTxHash":"0xdead","message":"{message_hex}"}}]}}"#
        );

        let s = CctpFeeClient::parse_message_status(&body).unwrap();
        assert_eq!(s.message.as_deref(), Some(message_hex.as_str()));
        assert_eq!(s.delivered_amount, Some(1_000_000 - 250));
    }

    #[macros::test_all]
    fn parse_message_status_reads_failed_forward() {
        // The stuck-swap shape: attested + status complete, but the forward
        // failed and no forwardTxHash. Must parse as not-forwarded, failed.
        let body = r#"{"messages":[{"status":"complete","attestation":"0xabcd","forwardState":"FAILED","forwardErrorCode":"INTERNAL_ERROR"}]}"#;
        let s = CctpFeeClient::parse_message_status(body).unwrap();
        assert!(!s.is_forwarded());
        assert!(s.forward_failed());
        assert_eq!(s.forward_error_code.as_deref(), Some("INTERNAL_ERROR"));

        // A pending (not failed) forward is not `forward_failed`.
        let pending = r#"{"messages":[{"status":"complete","forwardState":"PENDING"}]}"#;
        assert!(
            !CctpFeeClient::parse_message_status(pending)
                .unwrap()
                .forward_failed()
        );
    }

    #[macros::test_all]
    fn parse_fee_response_forwarded_fee_with_numeric_values() {
        // minimumFee / forwardFee can arrive as JSON numbers, not strings.
        let body = r#"[{"finalityThreshold":1000,"minimumFee":0.5,"forwardFee":{"low":10,"med":20,"high":30}}]"#;
        let fee = CctpFeeClient::parse_fee_response(body, 1000).unwrap();
        assert_eq!(fee.bps_units, 500_000_000);
        assert_eq!(fee.forward_fee, 20);
    }
}
