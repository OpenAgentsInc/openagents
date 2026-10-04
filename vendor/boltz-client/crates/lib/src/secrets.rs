//! Source of a swap's preimage and claim/gas signing key.
//!
//! Two mutually-exclusive providers back the two [`SwapKeySource`] modes:
//! [`DerivedSecretProvider`] re-derives everything from a BIP-32 seed (nothing
//! secret persisted, but a durable HD key-index counter is required), while
//! [`RandomSecretProvider`] generates random per-swap secrets that the caller
//! persists inline on the swap (no counter, store-only recoverability).
//!
//! A provider only handles its own [`SwapKeySource`] variant; being handed the
//! other (which can't happen for a correctly-constructed service) is a hard
//! error rather than silent misbehaviour.

use std::sync::Arc;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::BoltzError;
use crate::keys::{EvmKeyManager, EvmKeyPair};
use crate::models::{BoltzSwap, Secret, SwapKeySource, SwapSecrets};
use crate::store::DerivedKeyStore;
use platform_utils::tokio::sync::Mutex;

/// Crypto material committed when creating a new swap, before the Boltz create
/// call. The caller copies these into the `CreateReverseSwapRequest` and the
/// persisted [`BoltzSwap`].
pub(crate) struct IssuedSecrets {
    /// Persisted on the swap; later resolved back into the live secrets.
    pub key_source: SwapKeySource,
    pub preimage_hash: [u8; 32],
    /// Sent to Boltz as `claimPublicKey`. Not used on-chain for EVM swaps, so
    /// it need not relate to the preimage — the gas key's pubkey is fine.
    pub claim_public_key: Vec<u8>,
    pub claim_address: String,
}

#[macros::async_trait]
pub(crate) trait SecretProvider: Send + Sync {
    /// Commit to a new swap's secrets for `chain_id`.
    ///
    /// Derived mode durably reserves an HD index (serialized in-process here).
    /// Random mode generates the preimage + gas key — the caller MUST persist
    /// the resulting [`SwapKeySource`] with the swap before the invoice is
    /// handed out, or the swap becomes unclaimable.
    async fn issue(&self, chain_id: u32) -> Result<IssuedSecrets, BoltzError>;

    /// Resolve the HTLC preimage for a persisted swap (claim time).
    fn preimage(&self, swap: &BoltzSwap) -> Result<[u8; 32], BoltzError>;

    /// Resolve the preimage hash for a persisted swap (on-chain lockup checks).
    fn preimage_hash(&self, swap: &BoltzSwap) -> Result<[u8; 32], BoltzError>;

    /// Resolve the gas/claim signing key pair for a persisted swap.
    fn gas_key_pair(&self, swap: &BoltzSwap) -> Result<EvmKeyPair, BoltzError>;
}

fn to_chain_id_u32(chain_id: u64) -> Result<u32, BoltzError> {
    chain_id
        .try_into()
        .map_err(|_| BoltzError::Generic("Chain ID overflow".into()))
}

fn wrong_mode(expected: &str) -> BoltzError {
    BoltzError::Generic(format!(
        "Swap key source does not match provider (expected {expected})"
    ))
}

/// Seed-derived secrets: preimage and the global gas signer come from BIP-32
/// derivation. Requires a [`DerivedKeyStore`] for durable index issuance.
pub(crate) struct DerivedSecretProvider {
    key_manager: EvmKeyManager,
    store: Arc<dyn DerivedKeyStore>,
    /// Serializes index issuance in-process so concurrent creates can't draw
    /// the same index from a non-transactional store and reuse a preimage.
    index_lock: Mutex<()>,
}

impl DerivedSecretProvider {
    pub(crate) fn new(key_manager: EvmKeyManager, store: Arc<dyn DerivedKeyStore>) -> Self {
        Self {
            key_manager,
            store,
            index_lock: Mutex::new(()),
        }
    }

    /// Resolve this swap's HD index, erroring if the swap is not a derived one.
    fn index_of(swap: &BoltzSwap) -> Result<u32, BoltzError> {
        match &swap.key_source {
            SwapKeySource::Derived { claim_key_index } => Ok(*claim_key_index),
            SwapKeySource::Stored(_) => Err(wrong_mode("derived")),
        }
    }
}

#[macros::async_trait]
impl SecretProvider for DerivedSecretProvider {
    async fn issue(&self, chain_id: u32) -> Result<IssuedSecrets, BoltzError> {
        let index = {
            let _guard = self.index_lock.lock().await;
            self.store.increment_key_index().await?
        };
        let preimage_hash = self.key_manager.derive_preimage_hash(chain_id, index)?;
        let preimage_key = self.key_manager.derive_preimage_key(chain_id, index)?;
        let gas = self.key_manager.derive_gas_signer(chain_id)?;
        Ok(IssuedSecrets {
            key_source: SwapKeySource::Derived {
                claim_key_index: index,
            },
            preimage_hash,
            claim_public_key: preimage_key.public_key,
            claim_address: gas.address_hex(),
        })
    }

    fn preimage(&self, swap: &BoltzSwap) -> Result<[u8; 32], BoltzError> {
        let index = Self::index_of(swap)?;
        self.key_manager
            .derive_preimage(to_chain_id_u32(swap.chain_id)?, index)
    }

    fn preimage_hash(&self, swap: &BoltzSwap) -> Result<[u8; 32], BoltzError> {
        let index = Self::index_of(swap)?;
        self.key_manager
            .derive_preimage_hash(to_chain_id_u32(swap.chain_id)?, index)
    }

    fn gas_key_pair(&self, swap: &BoltzSwap) -> Result<EvmKeyPair, BoltzError> {
        // The gas signer is global (per chain, not per index), so the index
        // isn't needed here — this call only rejects a `Stored` swap that
        // doesn't belong to a seeded provider.
        Self::index_of(swap)?;
        self.key_manager
            .derive_gas_signer(to_chain_id_u32(swap.chain_id)?)
    }
}

/// Seedless secrets: a random preimage and a per-swap random gas/claim key,
/// stored inline on the swap. Stateless — no key-index counter is needed.
pub(crate) struct RandomSecretProvider;

impl RandomSecretProvider {
    fn secrets_of(swap: &BoltzSwap) -> Result<&SwapSecrets, BoltzError> {
        match &swap.key_source {
            SwapKeySource::Stored(secrets) => Ok(secrets),
            SwapKeySource::Derived { .. } => Err(wrong_mode("stored")),
        }
    }
}

#[macros::async_trait]
impl SecretProvider for RandomSecretProvider {
    async fn issue(&self, _chain_id: u32) -> Result<IssuedSecrets, BoltzError> {
        // Hold the plaintext in `Zeroizing` so the only lasting copies are the
        // ones moved into `Secret` (which zeroizes on its own drop).
        let mut preimage = Zeroizing::new([0u8; 32]);
        getrandom::getrandom(&mut *preimage)
            .map_err(|e| BoltzError::Generic(format!("Failed to generate preimage: {e}")))?;
        let preimage_hash: [u8; 32] = Sha256::digest(preimage.as_slice()).into();

        let gas = EvmKeyPair::generate()?;
        let claim_public_key = gas.public_key.clone();
        let claim_address = gas.address_hex();
        let gas_key = gas.private_key_bytes(); // Zeroizing<[u8; 32]>

        Ok(IssuedSecrets {
            key_source: SwapKeySource::Stored(SwapSecrets {
                preimage: Secret::from(*preimage),
                gas_key: Secret::from(*gas_key),
            }),
            preimage_hash,
            claim_public_key,
            claim_address,
        })
    }

    fn preimage(&self, swap: &BoltzSwap) -> Result<[u8; 32], BoltzError> {
        Ok(*Self::secrets_of(swap)?.preimage.expose())
    }

    fn preimage_hash(&self, swap: &BoltzSwap) -> Result<[u8; 32], BoltzError> {
        Ok(Sha256::digest(self.preimage(swap)?).into())
    }

    fn gas_key_pair(&self, swap: &BoltzSwap) -> Result<EvmKeyPair, BoltzError> {
        EvmKeyPair::from_private_key_bytes(Self::secrets_of(swap)?.gas_key.expose())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::models::{Asset, BoltzSwapStatus, BridgeKind};
    use crate::store::MemoryBoltzStorage;

    const CHAIN_ID: u32 = 42161;

    fn swap_with(key_source: SwapKeySource, claim_address: &str) -> BoltzSwap {
        BoltzSwap {
            id: "s".to_string(),
            status: BoltzSwapStatus::Created,
            bridge_kind: BridgeKind::Oft,
            key_source,
            chain_id: u64::from(CHAIN_ID),
            claim_address: claim_address.to_string(),
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
            lockup_tx_id: None,
            claim_tx_hash: None,
            pending_call_id: None,
            delivered_amount: None,
            bridge_ref: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        }
    }

    fn derived() -> DerivedSecretProvider {
        DerivedSecretProvider::new(
            EvmKeyManager::from_seed(&[7u8; 32]).unwrap(),
            Arc::new(MemoryBoltzStorage::new()),
        )
    }

    #[macros::test_all]
    fn secret_debug_is_redacted() {
        let s = Secret::from([0xABu8; 32]);
        assert_eq!(format!("{s:?}"), "Secret([redacted])");
    }

    #[macros::async_test_all]
    async fn random_issue_round_trips_and_is_consistent() {
        let provider = RandomSecretProvider;
        let issued = provider.issue(CHAIN_ID).await.unwrap();

        // Random mode stores its secrets inline.
        assert!(matches!(issued.key_source, SwapKeySource::Stored(_)));

        let swap = swap_with(issued.key_source, &issued.claim_address);

        // preimage_hash == SHA256(preimage), and matches what `issue` sent to Boltz.
        let preimage = provider.preimage(&swap).unwrap();
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        assert_eq!(provider.preimage_hash(&swap).unwrap(), hash);
        assert_eq!(issued.preimage_hash, hash);

        // The stored gas key resolves to the claim address baked into the swap.
        let gas = provider.gas_key_pair(&swap).unwrap();
        assert_eq!(gas.address_hex(), swap.claim_address);
        assert_eq!(gas.public_key, issued.claim_public_key);
    }

    #[macros::async_test_all]
    async fn random_issue_is_unique_per_call() {
        let provider = RandomSecretProvider;
        let a = provider.issue(CHAIN_ID).await.unwrap();
        let b = provider.issue(CHAIN_ID).await.unwrap();
        assert_ne!(a.preimage_hash, b.preimage_hash);
        assert_ne!(a.claim_address, b.claim_address);
    }

    #[macros::test_all]
    fn providers_reject_the_other_mode() {
        let stored_swap = {
            // Build a Stored swap without async by reusing a hand-made secret.
            let secrets = SwapSecrets {
                preimage: Secret::from([1u8; 32]),
                gas_key: Secret::from([2u8; 32]),
            };
            swap_with(SwapKeySource::Stored(secrets), "0xabc")
        };
        let derived_swap = swap_with(SwapKeySource::Derived { claim_key_index: 0 }, "0xabc");

        // Derived provider can't resolve a seedless swap, and vice versa.
        assert!(derived().preimage(&stored_swap).is_err());
        assert!(RandomSecretProvider.preimage(&derived_swap).is_err());
    }

    #[macros::test_all]
    fn derived_resolution_is_deterministic() {
        let swap = swap_with(SwapKeySource::Derived { claim_key_index: 3 }, "0xabc");
        let p = derived();
        assert_eq!(p.preimage(&swap).unwrap(), p.preimage(&swap).unwrap());
        let expected: [u8; 32] = Sha256::digest(p.preimage(&swap).unwrap()).into();
        assert_eq!(p.preimage_hash(&swap).unwrap(), expected);
    }
}
