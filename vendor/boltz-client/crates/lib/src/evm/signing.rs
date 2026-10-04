use alloy_primitives::{Address, B256, U256};
use alloy_signer::SignerSync;
use alloy_signer_local::LocalSigner;
use alloy_sol_types::Eip712Domain;
use k256::ecdsa::SigningKey;

use crate::error::BoltzError;
use crate::keys::EvmKeyPair;

// ─── EIP-712 typed structs ──────────────────────────────────────────────
// These are defined as `sol!` structs so alloy automatically generates the
// correct EIP-712 type hashes and struct encoding via `SolStruct`.
//
// The `ERC20Swap` and Router contracts both have a struct named `Claim` but
// with different fields. We use separate modules to avoid name collisions.

/// EIP-712 types for the `ERC20Swap` contract cooperative claim.
pub(crate) mod erc20swap_eip712 {
    use alloy_sol_types::sol;
    sol! {
        /// `ERC20Swap` cooperative claim authorization.
        /// Domain: `{ name: "ERC20Swap", version: <runtime>, chainId, verifyingContract }`
        struct Claim {
            bytes32 preimage;
            uint256 amount;
            address tokenAddress;
            address refundAddress;
            uint256 timelock;
            address destination;
        }
    }
}

/// EIP-712 types for the Router contract.
pub(crate) mod router_eip712 {
    use alloy_sol_types::sol;
    sol! {
        /// Router same-chain sweep authorization.
        /// Domain: `{ name: "Router", version: "2", chainId, verifyingContract }`
        struct Claim {
            bytes32 preimage;
            address token;
            uint256 minAmountOut;
            address destination;
        }

        /// Router cross-chain OFT bridge authorization.
        /// Domain: `{ name: "Router", version: "2", chainId, verifyingContract }`
        struct ClaimSend {
            bytes32 preimage;
            address token;
            address oft;
            bytes32 sendData;
            uint256 minAmountLD;
            uint256 lzTokenFee;
            address refundAddress;
        }

        /// Router cross-chain CCTP bridge authorization.
        /// Domain: `{ name: "Router", version: "2", chainId, verifyingContract }`
        /// `cctpData` is the `hash_cctp_data` struct hash.
        struct ClaimCctp {
            bytes32 preimage;
            address token;
            address tokenMessenger;
            bytes32 cctpData;
            uint256 minAmount;
        }
    }
}

/// An ECDSA signature with recovery id, suitable for EIP-712 / EIP-191 / raw signing.
#[derive(Debug, Clone)]
pub(crate) struct EvmSignature {
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

/// EVM signer backed by `alloy-signer-local`'s `LocalSigner`.
/// Provides EIP-712, EIP-191, and raw ECDSA signing via battle-tested alloy libraries.
pub(crate) struct EvmSigner {
    inner: LocalSigner<SigningKey>,
    address_bytes: [u8; 20],
    chain_id: u64,
}

impl EvmSigner {
    pub fn new(key_pair: &EvmKeyPair, chain_id: u64) -> Self {
        let inner = LocalSigner::from_signing_key(key_pair.signing_key().clone());
        Self {
            inner,
            address_bytes: key_pair.address,
            chain_id,
        }
    }

    #[cfg(test)]
    pub fn address(&self) -> [u8; 20] {
        self.address_bytes
    }

    pub fn address_hex(&self) -> String {
        Address::from(self.address_bytes).to_checksum(None)
    }

    // ─── 1. Raw ECDSA (no prefix) ───────────────────────────────────────

    /// Sign a raw 32-byte digest with no prefix.
    /// Used for: EIP-7702 authorization digests from Alchemy.
    pub fn sign_raw_digest(&self, digest: &[u8; 32]) -> Result<EvmSignature, BoltzError> {
        let hash = B256::from(*digest);
        let sig = self
            .inner
            .sign_hash_sync(&hash)
            .map_err(|e| BoltzError::Signing(format!("Raw signing failed: {e}")))?;
        Ok(alloy_sig_to_evm_sig(&sig))
    }

    // ─── 2. EIP-191 personal_sign ───────────────────────────────────────

    /// Sign a message with EIP-191 `personal_sign` prefix.
    /// Used for: Alchemy `UserOperation` payloads.
    pub fn sign_message(&self, message: &[u8]) -> Result<EvmSignature, BoltzError> {
        let sig = self
            .inner
            .sign_message_sync(message)
            .map_err(|e| BoltzError::Signing(format!("EIP-191 signing failed: {e}")))?;
        Ok(alloy_sig_to_evm_sig(&sig))
    }

    // ─── 3. EIP-712: ERC20Swap cooperative claim ────────────────────────

    /// Sign the `ERC20Swap` cooperative claim EIP-712 typed data.
    /// The gas signer signs this; it authorizes the Router to claim tokens.
    /// `destination` = Router address (will be `msg.sender` at claim time).
    #[expect(clippy::too_many_arguments)]
    pub fn sign_eip712_erc20swap_claim(
        &self,
        erc20swap_address: Address,
        erc20swap_version: &str,
        preimage: &[u8; 32],
        amount: U256,
        token_address: Address,
        refund_address: Address,
        timelock: U256,
        destination: Address,
    ) -> Result<EvmSignature, BoltzError> {
        let domain = Eip712Domain {
            name: Some("ERC20Swap".into()),
            version: Some(erc20swap_version.to_string().into()),
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(erc20swap_address),
            salt: None,
        };

        let claim = erc20swap_eip712::Claim {
            preimage: (*preimage).into(),
            amount,
            tokenAddress: token_address,
            refundAddress: refund_address,
            timelock,
            destination,
        };

        let sig = self
            .inner
            .sign_typed_data_sync(&claim, &domain)
            .map_err(|e| BoltzError::Signing(format!("ERC20Swap EIP-712 signing failed: {e}")))?;
        Ok(alloy_sig_to_evm_sig(&sig))
    }

    // ─── 4. EIP-712: Router Claim (same-chain) ──────────────────────────

    /// Sign the Router Claim EIP-712 typed data (same-chain sweep).
    /// Authorizes the Router to sweep output token to `destination`.
    pub fn sign_eip712_router_claim(
        &self,
        router_address: Address,
        preimage: &[u8; 32],
        token: Address,
        min_amount_out: U256,
        destination: Address,
    ) -> Result<EvmSignature, BoltzError> {
        let domain = Eip712Domain {
            name: Some("Router".into()),
            version: Some("2".into()),
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(router_address),
            salt: None,
        };

        let claim = router_eip712::Claim {
            preimage: (*preimage).into(),
            token,
            minAmountOut: min_amount_out,
            destination,
        };

        let sig = self
            .inner
            .sign_typed_data_sync(&claim, &domain)
            .map_err(|e| {
                BoltzError::Signing(format!("Router Claim EIP-712 signing failed: {e}"))
            })?;
        Ok(alloy_sig_to_evm_sig(&sig))
    }

    // ─── 5. EIP-712: Router ClaimSend (cross-chain OFT) ─────────────────

    /// Sign the Router `ClaimSend` EIP-712 typed data (cross-chain OFT bridging).
    #[expect(clippy::too_many_arguments)]
    pub fn sign_eip712_router_claim_send(
        &self,
        router_address: Address,
        preimage: &[u8; 32],
        token: Address,
        oft: Address,
        send_data_hash: [u8; 32],
        min_amount_ld: U256,
        lz_token_fee: U256,
        refund_address: Address,
    ) -> Result<EvmSignature, BoltzError> {
        let domain = Eip712Domain {
            name: Some("Router".into()),
            version: Some("2".into()),
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(router_address),
            salt: None,
        };

        let claim_send = router_eip712::ClaimSend {
            preimage: (*preimage).into(),
            token,
            oft,
            sendData: send_data_hash.into(),
            minAmountLD: min_amount_ld,
            lzTokenFee: lz_token_fee,
            refundAddress: refund_address,
        };

        let sig = self
            .inner
            .sign_typed_data_sync(&claim_send, &domain)
            .map_err(|e| {
                BoltzError::Signing(format!("Router ClaimSend EIP-712 signing failed: {e}"))
            })?;
        Ok(alloy_sig_to_evm_sig(&sig))
    }

    // ─── 6. EIP-712: Router ClaimCctp (cross-chain CCTP) ────────────────

    /// Sign the Router `ClaimCctp` EIP-712 typed data (cross-chain CCTP/USDC
    /// bridging). Same domain as the OFT `ClaimSend` path.
    pub fn sign_eip712_router_claim_cctp(
        &self,
        router_address: Address,
        preimage: &[u8; 32],
        token: Address,
        token_messenger: Address,
        cctp_data_hash: [u8; 32],
        min_amount: U256,
    ) -> Result<EvmSignature, BoltzError> {
        let domain = Eip712Domain {
            name: Some("Router".into()),
            version: Some("2".into()),
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(router_address),
            salt: None,
        };

        let claim_cctp = router_eip712::ClaimCctp {
            preimage: (*preimage).into(),
            token,
            tokenMessenger: token_messenger,
            cctpData: cctp_data_hash.into(),
            minAmount: min_amount,
        };

        let sig = self
            .inner
            .sign_typed_data_sync(&claim_cctp, &domain)
            .map_err(|e| {
                BoltzError::Signing(format!("Router ClaimCctp EIP-712 signing failed: {e}"))
            })?;
        Ok(alloy_sig_to_evm_sig(&sig))
    }
}

/// Convert an alloy `Signature` to our `EvmSignature` (v=27/28, r, s).
fn alloy_sig_to_evm_sig(sig: &alloy_primitives::Signature) -> EvmSignature {
    let v = if sig.v() { 28 } else { 27 };
    let r: [u8; 32] = sig.r().to_be_bytes();
    let s: [u8; 32] = sig.s().to_be_bytes();
    EvmSignature { v, r, s }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::keys::EvmKeyManager;

    const TEST_SEED_HEX: &str = "5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4";

    // Dummy router address used as the EIP-712 `verifyingContract` in these tests.
    // The tests only assert signature validity / recovery, not fixture equality,
    // so any 20-byte value works — keeps the real router address out of tests.
    const TEST_ROUTER: &str = "0x0000000000000000000000000000000000000042";

    fn test_seed() -> Vec<u8> {
        hex::decode(TEST_SEED_HEX).unwrap()
    }

    fn test_signer() -> EvmSigner {
        let manager = EvmKeyManager::from_seed(&test_seed()).unwrap();
        let key_pair = manager.derive_gas_signer(42161).unwrap();
        EvmSigner::new(&key_pair, 42161)
    }

    fn addr(s: &str) -> Address {
        s.parse().unwrap()
    }

    // ─── Basic signing tests ─────────────────────────────────────────

    #[macros::test_all]
    #[allow(clippy::similar_names)]
    fn test_sign_raw_digest_deterministic() {
        let signer = test_signer();
        let digest = alloy_primitives::keccak256(b"test message");

        let sig1 = signer.sign_raw_digest(digest.as_ref()).unwrap();
        let sig2 = signer.sign_raw_digest(digest.as_ref()).unwrap();

        assert_eq!(sig1.r, sig2.r);
        assert_eq!(sig1.s, sig2.s);
        assert_eq!(sig1.v, sig2.v);
    }

    #[macros::test_all]
    fn test_sign_raw_digest_v_value() {
        let signer = test_signer();
        let sig = signer.sign_raw_digest(&[0u8; 32]).unwrap();
        assert!(sig.v == 27 || sig.v == 28);
    }

    #[macros::test_all]
    fn test_sign_message_eip191() {
        let signer = test_signer();
        let sig = signer.sign_message(b"hello world").unwrap();
        assert!(sig.v == 27 || sig.v == 28);
        assert_ne!(sig.r, [0u8; 32]);
    }

    #[macros::test_all]
    fn test_sign_message_differs_from_raw() {
        let signer = test_signer();
        let data = alloy_primitives::keccak256(b"test data");

        let raw_sig = signer.sign_raw_digest(data.as_ref()).unwrap();
        let msg_sig = signer.sign_message(data.as_ref()).unwrap();

        // EIP-191 prefixing means different signatures
        assert_ne!(raw_sig.r, msg_sig.r);
    }

    // ─── EIP-712 signing tests ───────────────────────────────────────

    #[macros::test_all]
    fn test_sign_eip712_erc20swap_claim() {
        let signer = test_signer();

        let sig = signer
            .sign_eip712_erc20swap_claim(
                addr("0x6398B76DF91C5eBe9f488e3656658E79284dDc0F"),
                "6",
                &[1u8; 32],
                U256::from(100_000_000_000_000u64),
                addr("0x6c84a8f1c29108F47a79964b5Fe888D4f4D0dE40"),
                addr("0x0000000000000000000000000000000000000002"),
                U256::from(12345u64),
                addr(TEST_ROUTER),
            )
            .unwrap();

        assert!(sig.v == 27 || sig.v == 28);
        assert_ne!(sig.r, [0u8; 32]);
    }

    #[macros::test_all]
    #[allow(clippy::similar_names)]
    fn test_sign_eip712_erc20swap_claim_deterministic() {
        let signer = test_signer();

        let sign = || {
            signer.sign_eip712_erc20swap_claim(
                addr("0x6398B76DF91C5eBe9f488e3656658E79284dDc0F"),
                "6",
                &[1u8; 32],
                U256::from(100u64),
                addr("0x6c84a8f1c29108F47a79964b5Fe888D4f4D0dE40"),
                addr("0x0000000000000000000000000000000000000002"),
                U256::from(100u64),
                addr(TEST_ROUTER),
            )
        };

        let sig1 = sign().unwrap();
        let sig2 = sign().unwrap();
        assert_eq!(sig1.r, sig2.r);
        assert_eq!(sig1.s, sig2.s);
        assert_eq!(sig1.v, sig2.v);
    }

    #[macros::test_all]
    fn test_sign_eip712_router_claim() {
        let signer = test_signer();

        let sig = signer
            .sign_eip712_router_claim(
                addr(TEST_ROUTER),
                &[1u8; 32],
                addr("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
                U256::from(71_000_000u64),
                addr("0x0000000000000000000000000000000000000004"),
            )
            .unwrap();

        assert!(sig.v == 27 || sig.v == 28);
        assert_ne!(sig.r, [0u8; 32]);
    }

    #[macros::test_all]
    fn test_sign_eip712_router_claim_send() {
        let signer = test_signer();

        let sig = signer
            .sign_eip712_router_claim_send(
                addr(TEST_ROUTER),
                &[1u8; 32],
                addr("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
                addr("0x0000000000000000000000000000000000000005"),
                [2u8; 32],
                U256::from(1000u64),
                U256::ZERO,
                addr("0x0000000000000000000000000000000000000006"),
            )
            .unwrap();

        assert!(sig.v == 27 || sig.v == 28);
        assert_ne!(sig.r, [0u8; 32]);
    }

    #[macros::test_all]
    fn test_erc20swap_and_router_signatures_differ() {
        let signer = test_signer();
        let preimage = [1u8; 32];

        let erc20_sig = signer
            .sign_eip712_erc20swap_claim(
                addr("0x6398B76DF91C5eBe9f488e3656658E79284dDc0F"),
                "6",
                &preimage,
                U256::from(100u64),
                addr("0x6c84a8f1c29108F47a79964b5Fe888D4f4D0dE40"),
                addr("0x0000000000000000000000000000000000000002"),
                U256::from(100u64),
                addr(TEST_ROUTER),
            )
            .unwrap();

        let router_sig = signer
            .sign_eip712_router_claim(
                addr(TEST_ROUTER),
                &preimage,
                addr("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
                U256::from(100u64),
                addr("0x0000000000000000000000000000000000000004"),
            )
            .unwrap();

        assert_ne!(erc20_sig.r, router_sig.r);
    }

    // ─── Signature recovery verification ─────────────────────────────

    #[macros::test_all]
    fn test_signature_recovers_to_correct_address() {
        let signer = test_signer();
        let digest = alloy_primitives::keccak256(b"test recovery");
        let sig = signer.sign_raw_digest(digest.as_ref()).unwrap();

        // Recover using alloy
        let alloy_sig = alloy_primitives::Signature::new(
            U256::from_be_bytes(sig.r),
            U256::from_be_bytes(sig.s),
            sig.v == 28,
        );

        let recovered = alloy_sig.recover_address_from_prehash(&digest).unwrap();
        assert_eq!(recovered.as_slice(), &signer.address());
    }

    #[macros::test_all]
    fn test_eip191_signature_recovers_correctly() {
        let signer = test_signer();
        let message = b"test eip191 recovery";
        let sig = signer.sign_message(message).unwrap();

        let digest = alloy_primitives::eip191_hash_message(message);

        let alloy_sig = alloy_primitives::Signature::new(
            U256::from_be_bytes(sig.r),
            U256::from_be_bytes(sig.s),
            sig.v == 28,
        );

        let recovered = alloy_sig.recover_address_from_prehash(&digest).unwrap();
        assert_eq!(recovered.as_slice(), &signer.address());
    }

    #[macros::test_all]
    fn test_eip712_signature_recovers_correctly() {
        use alloy_sol_types::SolStruct;

        let signer = test_signer();
        let preimage = [1u8; 32];

        let domain = Eip712Domain {
            name: Some("Router".into()),
            version: Some("2".into()),
            chain_id: Some(U256::from(42161u64)),
            verifying_contract: Some(addr(TEST_ROUTER)),
            salt: None,
        };

        let claim = router_eip712::Claim {
            preimage: preimage.into(),
            token: addr("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
            minAmountOut: U256::from(100u64),
            destination: addr("0x0000000000000000000000000000000000000004"),
        };

        // The digest that alloy computes for EIP-712
        let digest = claim.eip712_signing_hash(&domain);

        let sig = signer
            .sign_eip712_router_claim(
                addr(TEST_ROUTER),
                &preimage,
                addr("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
                U256::from(100u64),
                addr("0x0000000000000000000000000000000000000004"),
            )
            .unwrap();

        let alloy_sig = alloy_primitives::Signature::new(
            U256::from_be_bytes(sig.r),
            U256::from_be_bytes(sig.s),
            sig.v == 28,
        );

        let recovered = alloy_sig.recover_address_from_prehash(&digest).unwrap();
        assert_eq!(recovered.as_slice(), &signer.address());
    }

    // ─── EIP-712 type hash pinning ─────────────────────────────────
    // These type hashes are derived from the EIP-712 type strings and must
    // match the constants embedded in the deployed contracts. The ERC20Swap
    // hash was verified against the contract bytecode from the Boltz regtest
    // stack. All three were cross-referenced with `cast keccak`.

    #[macros::test_all]
    fn test_erc20swap_claim_type_hash_matches_contract() {
        use alloy_sol_types::SolStruct;

        let claim = erc20swap_eip712::Claim {
            preimage: [0u8; 32].into(),
            amount: U256::ZERO,
            tokenAddress: Address::ZERO,
            refundAddress: Address::ZERO,
            timelock: U256::ZERO,
            destination: Address::ZERO,
        };

        // keccak256("Claim(bytes32 preimage,uint256 amount,address tokenAddress,address refundAddress,uint256 timelock,address destination)")
        // Verified against deployed ERC20Swap contract bytecode in Boltz regtest stack.
        assert_eq!(
            format!("{}", claim.eip712_type_hash()),
            "0x88d2eb81eeaf48c24a8e0c241c49b9f515812cf57db155ee3bba213131a67cf1"
        );
    }

    #[macros::test_all]
    fn test_router_claim_type_hash() {
        use alloy_sol_types::SolStruct;

        let claim = router_eip712::Claim {
            preimage: [0u8; 32].into(),
            token: Address::ZERO,
            minAmountOut: U256::ZERO,
            destination: Address::ZERO,
        };

        // keccak256("Claim(bytes32 preimage,address token,uint256 minAmountOut,address destination)")
        assert_eq!(
            format!("{}", claim.eip712_type_hash()),
            "0xa47e6983ae363f2a4c612ca9d2c8acd221ad9a6dd1ae17020ccfa05f14074fcf"
        );
    }

    #[macros::test_all]
    fn test_router_claim_send_type_hash() {
        use alloy_sol_types::SolStruct;

        let claim_send = router_eip712::ClaimSend {
            preimage: [0u8; 32].into(),
            token: Address::ZERO,
            oft: Address::ZERO,
            sendData: [0u8; 32].into(),
            minAmountLD: U256::ZERO,
            lzTokenFee: U256::ZERO,
            refundAddress: Address::ZERO,
        };

        // keccak256("ClaimSend(bytes32 preimage,address token,address oft,bytes32 sendData,uint256 minAmountLD,uint256 lzTokenFee,address refundAddress)")
        assert_eq!(
            format!("{}", claim_send.eip712_type_hash()),
            "0xd574e98ae922e812083482a53f290e4a94af4ec6bc2d9490b0386edcf40dfecf"
        );
    }

    #[macros::test_all]
    fn test_router_claim_cctp_type_hash() {
        use alloy_sol_types::SolStruct;

        let claim_cctp = router_eip712::ClaimCctp {
            preimage: [0u8; 32].into(),
            token: Address::ZERO,
            tokenMessenger: Address::ZERO,
            cctpData: [0u8; 32].into(),
            minAmount: U256::ZERO,
        };

        // keccak256("ClaimCctp(bytes32 preimage,address token,address tokenMessenger,bytes32 cctpData,uint256 minAmount)")
        // Independently computed with `cast keccak` — must match boltz-core
        // v5.0.0 Router.TYPEHASH_CLAIM_CCTP.
        assert_eq!(
            format!("{}", claim_cctp.eip712_type_hash()),
            "0xf854d53d13bfc357f12f22b9d29b6f5c46693d79fb5dfd1153ba80151e59528c"
        );
    }

    // ─── Misc ────────────────────────────────────────────────────────

    #[macros::test_all]
    fn test_signer_address_matches_key_pair() {
        let manager = EvmKeyManager::from_seed(&test_seed()).unwrap();
        let key_pair = manager.derive_gas_signer(42161).unwrap();
        let signer = EvmSigner::new(&key_pair, 42161);

        // The alloy LocalSigner should derive the same address as our EvmKeyPair
        assert_eq!(signer.inner.address().as_slice(), &key_pair.address);
    }
}
