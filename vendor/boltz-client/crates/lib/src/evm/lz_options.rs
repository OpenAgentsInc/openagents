//! `LayerZero` v2 type-3 executor options encoder for OFT
//! `SendParam.extraOptions`.
//!
//! Two `lzReceive` directives are emitted, each as an option-type-1 entry
//! carrying `(gas, value)` packed as two big-endian uint128s:
//!
//! - **Solana ATA creation** `(gas=0, value=solanaAtaRentExemptLamports)`:
//!   the LZ executor creates the recipient's SPL ATA on Solana before landing
//!   the cross-chain tokens. Without it, sending USDT0 to a Solana wallet that
//!   has never held USDT fails silently on the destination side.
//! - **Polygon gas bump** `(gas=30000, value=0)`: a temporary workaround for
//!   failed OFT txs on Polygon (mirrors boltz-web-app#1500). Remove once the
//!   OFT options are bumped on-chain upstream.
//!
//! Native-drop (option type 2), gas top-up, and compose messages are not
//! emitted — the reverse-swap flow needs neither.

/// Lamports required to rent-exempt a 165-byte SPL Token account. Static on
/// Solana mainnet. If the runtime ever raises the rent-exempt floor this
/// becomes load-bearing and should move to a `getMinimumBalanceForRentExemption`
/// runtime query.
pub const SOLANA_ATA_RENT_EXEMPT_LAMPORTS: u128 = 2_039_280;

/// `lzReceive` gas limit bumped for Polygon OFT sends. Temporary on-chain
/// workaround for failed txs (boltz-web-app#1500); remove once the OFT options
/// are bumped on-chain upstream.
pub const POLYGON_LZ_RECEIVE_GAS_BUMP: u128 = 30_000;

/// `LayerZero` v2 options header — type-3 options.
const TYPE3_HEADER: [u8; 2] = [0x00, 0x03];
/// Executor worker id (constant in LZ v2; only one executor worker exists).
const EXECUTOR_WORKER_ID: u8 = 1;
/// Option type: `lzReceive`.
const OPTION_TYPE_LZ_RECEIVE: u8 = 1;

/// Build the `extraOptions` bytes for an OFT `SendParam`.
///
/// Each requested directive contributes one type-3 `lzReceive` option. When no
/// directive is requested, returns an empty byte vector — the default for EVM
/// destinations other than Polygon, Tron, and Solana destinations whose ATA
/// already exists.
///
/// - `create_solana_ata`: append `(gas=0, value=SOLANA_ATA_RENT_EXEMPT_LAMPORTS)`
///   so the executor pre-funds and creates the recipient's ATA.
/// - `polygon_gas_bump`: append `(gas=POLYGON_LZ_RECEIVE_GAS_BUMP, value=0)`.
#[must_use]
pub fn build_extra_options(create_solana_ata: bool, polygon_gas_bump: bool) -> Vec<u8> {
    // (gas, value) pairs for each lzReceive option, in append order.
    let mut options: Vec<(u128, u128)> = Vec::new();
    if create_solana_ata {
        options.push((0, SOLANA_ATA_RENT_EXEMPT_LAMPORTS));
    }
    if polygon_gas_bump {
        options.push((POLYGON_LZ_RECEIVE_GAS_BUMP, 0));
    }

    if options.is_empty() {
        return Vec::new();
    }

    // 2-byte type-3 header + 36 bytes per option (1 worker id + 2 size
    // + 1 option type + 32 payload).
    let mut out = Vec::with_capacity(options.len().saturating_mul(36).saturating_add(2));
    out.extend_from_slice(&TYPE3_HEADER);
    for (gas, value) in options {
        append_lz_receive_option(&mut out, gas, value);
    }
    out
}

/// Append a single type-3 `lzReceive` executor option. The 32-byte payload is
/// two big-endian uint128s: `gas` (high 16 bytes) then `value` (low 16 bytes),
/// matching `encodePacked(["uint128","uint128"], [gas, value])`.
fn append_lz_receive_option(out: &mut Vec<u8>, gas: u128, value: u128) {
    let mut payload = [0u8; 32];
    payload[0..16].copy_from_slice(&gas.to_be_bytes());
    payload[16..32].copy_from_slice(&value.to_be_bytes());

    // 32-byte payload + 1 for the option type; cannot overflow `u16`.
    let option_size: u16 = 32 + 1;

    out.push(EXECUTOR_WORKER_ID);
    out.extend_from_slice(&option_size.to_be_bytes());
    out.push(OPTION_TYPE_LZ_RECEIVE);
    out.extend_from_slice(&payload);
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn no_directives_returns_empty() {
        assert!(build_extra_options(false, false).is_empty());
    }

    /// Structural assertions for the `LayerZero` v2 type-3 options blob
    /// carrying a single ATA-creation `lzReceive` entry:
    ///   - Type-3 header `0003`
    ///   - Executor worker id `01`
    ///   - Option size `0021` (= 33 = 32-byte payload + 1 for optionType)
    ///   - Option type `01` (lzReceive)
    ///   - Payload: 16 zero bytes gas + `2_039_280` as uint128 BE (0x1F1DF0)
    #[macros::test_all]
    fn ata_creation_matches_structural_layout() {
        let bytes = build_extra_options(true, false);

        assert_eq!(bytes.len(), 38);
        assert_eq!(&bytes[0..2], &[0x00, 0x03]);
        assert_eq!(bytes[2], 0x01);
        assert_eq!(&bytes[3..5], &[0x00, 0x21]);
        assert_eq!(bytes[5], 0x01);
        assert_eq!(&bytes[6..22], &[0u8; 16]);
        let mut expected_lamports = [0u8; 16];
        expected_lamports[13..16].copy_from_slice(&[0x1f, 0x1d, 0xf0]);
        assert_eq!(&bytes[22..38], &expected_lamports);
    }

    /// Byte-exact vector decoded from a literal hex string. Cross-checks the
    /// same bytes against an independent encoding path.
    #[macros::test_all]
    fn ata_creation_full_hex_vector() {
        let bytes = build_extra_options(true, false);
        let expected = hex::decode(
            "00030100210100000000000000000000000000000000000000000000000000000000001f1df0",
        )
        .expect("hex");
        assert_eq!(bytes, expected);
    }

    /// Polygon-only gas bump: type-3 header + one lzReceive option carrying
    /// `(gas=30000, value=0)`. 30000 = 0x7530 in the high uint128.
    #[macros::test_all]
    fn polygon_gas_bump_matches_layout() {
        let bytes = build_extra_options(false, true);

        assert_eq!(bytes.len(), 38);
        assert_eq!(&bytes[0..2], &[0x00, 0x03]); // type-3 header
        assert_eq!(bytes[2], 0x01); // worker id
        assert_eq!(&bytes[3..5], &[0x00, 0x21]); // option size 33
        assert_eq!(bytes[5], 0x01); // lzReceive
        // gas = 30000 in the high uint128 (last two bytes of the first 16).
        let mut expected_gas = [0u8; 16];
        expected_gas[14..16].copy_from_slice(&[0x75, 0x30]);
        assert_eq!(&bytes[6..22], &expected_gas);
        // value = 0 in the low uint128.
        assert_eq!(&bytes[22..38], &[0u8; 16]);
    }

    /// Both directives concatenate: ATA option first, then the Polygon bump.
    /// (Not a real combination — Solana is never Polygon — but verifies the
    /// multi-option append path.)
    #[macros::test_all]
    fn both_directives_concatenate() {
        let bytes = build_extra_options(true, true);
        assert_eq!(bytes.len(), 2 + 36 * 2);
        assert_eq!(&bytes[0..2], &[0x00, 0x03]);
        // First option == the ATA-only blob (minus its header).
        assert_eq!(&bytes[2..38], &build_extra_options(true, false)[2..]);
        // Second option == the Polygon-only blob (minus its header).
        assert_eq!(&bytes[38..], &build_extra_options(false, true)[2..]);
    }
}
