use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// A 32-byte secret that redacts its `Debug` output and zeroizes on drop.
///
/// Money-critical material (an HTLC preimage or a claim/gas signing key in
/// seedless mode) lives behind this so it can't leak through the `Debug` that
/// flows into logs and events, and is wiped from memory when dropped.
#[derive(Clone, Serialize, Deserialize)]
pub struct Secret([u8; 32]);

impl Secret {
    pub(crate) fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for Secret {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Where a swap's preimage and claim/gas signing key come from.
///
/// The two modes are mutually exclusive per [`BoltzService`](crate::BoltzService)
/// instance and persisted with the swap, so the manager loop and recovery paths
/// resolve secrets without any ambient mode state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SwapKeySource {
    /// Seed-derived (HD). The preimage and the global gas signer are re-derived
    /// on demand from the master seed + this index; nothing secret is stored.
    /// Requires a durable, monotonic key-index counter
    /// (see [`DerivedKeyStore`](crate::store::DerivedKeyStore)).
    Derived {
        /// HD derivation index for the per-swap preimage key.
        claim_key_index: u32,
    },
    /// Seedless. The preimage and a per-swap gas/claim key are generated
    /// randomly at create time and stored inline — recoverable only from the
    /// local store, never re-derivable.
    Stored(SwapSecrets),
}

/// Inline per-swap secrets backing [`SwapKeySource::Stored`]. Both are
/// money-critical: losing them makes an in-flight swap unclaimable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapSecrets {
    pub(crate) preimage: Secret,
    /// Private key (32 bytes) of the per-swap gas/claim signer.
    pub(crate) gas_key: Secret,
}

/// Persisted state for a single Boltz reverse swap.
///
/// Whether the preimage and claim key are stored inline depends on
/// [`key_source`](Self::key_source): in [`SwapKeySource::Derived`] mode nothing
/// secret is stored (both re-derived from the seed); in
/// [`SwapKeySource::Stored`] mode they are persisted with the swap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoltzSwap {
    /// Swap ID — the Boltz backend ID for normal swaps, or a `recovery-*` ID for recovered swaps.
    pub id: String,
    pub status: BoltzSwapStatus,
    /// Which bridge carries the Arbitrum -> destination leg.
    pub bridge_kind: BridgeKind,
    /// Source of this swap's preimage and claim/gas signing key.
    pub key_source: SwapKeySource,
    /// EVM chain ID (42161 for Arbitrum).
    pub chain_id: u64,

    // Addresses
    /// Gas signer address (used as claimAddress with Boltz).
    pub claim_address: String,
    /// User's final destination address for the delivered stablecoin.
    pub destination_address: String,
    /// Destination chain label (e.g. `"Arbitrum One"`, `"Base"`, `"Solana"`).
    /// Together with [`asset`](Self::asset) this is the destination identity.
    pub destination_chain: String,
    /// Stablecoin delivered on the destination chain. The same chain can host
    /// more than one (e.g. USDT0 via OFT and USDC via CCTP), so asset is part
    /// of the destination identity, not derivable from the chain alone.
    pub asset: Asset,
    /// Boltz's refund address (from swap response).
    pub refund_address: String,

    // Contract addresses (snapshot at creation time)
    pub erc20swap_address: String,
    pub router_address: String,

    // Invoice
    pub invoice: String,
    pub invoice_amount_sats: u64,

    // Amounts
    /// tBTC amount locked on-chain (sats, from swap response `onchainAmount`).
    pub onchain_amount: u64,
    /// Expected stablecoin output (6 decimals).
    pub expected_output_amount: u64,
    /// DEX slippage tolerance (basis points) snapshot at `prepare` time.
    /// Used for the claim-time quote drift check and on-chain `minOut`
    /// values so per-swap overrides survive across service restarts.
    pub slippage_bps: u32,

    // Timing
    pub timeout_block_height: u64,

    // Results
    pub lockup_tx_id: Option<String>,
    pub claim_tx_hash: Option<String>,
    /// Gas-sponsor `call_id` for an in-flight claim, persisted right after
    /// `wallet_sendPreparedCalls` and before the confirming poll. If the
    /// process dies in that window the claim still mines, but the tx hash is
    /// not yet known locally; on resume the manager re-polls this `call_id` to
    /// recover the tx hash (and verify on-chain) instead of trusting the WS
    /// `invoice.settled` event. Cleared once `claim_tx_hash` is persisted.
    pub pending_call_id: Option<String>,
    /// Actual stablecoin amount delivered on the destination chain (6
    /// decimals). `None` until the claim receipt is processed. For OFT
    /// destinations this is `amountReceivedLD`; for CCTP it's the attested
    /// delivered amount; for `Direct` delivery it's the final ERC20 `Transfer`
    /// value to the user.
    pub delivered_amount: Option<u64>,
    /// Cross-chain bridge tracking handle, set only for bridged swaps (`None`
    /// for `Direct` Arbitrum-destination swaps). Encoding depends on the
    /// bridge: for `Oft` it's the `LayerZero` message GUID (`0x`-prefixed hex,
    /// looked up on `LayerZero` Scan); for `Cctp` it's `"<source_domain>:<burn_tx_hash>"`,
    /// the key Circle Iris indexes the message by. Used to confirm destination
    /// delivery while the swap is `Settling`.
    pub bridge_ref: Option<String>,

    // Timestamps (unix seconds)
    pub created_at: u64,
    pub updated_at: u64,
}

/// Lifecycle status of a reverse swap.
///
/// **Variant order is load-bearing.** The derived `PartialOrd`/`Ord` follow
/// declaration order, and the swap manager's WS monotonicity guard
/// (`handle_ws_update` -> `ws_progress_stage`) relies on the non-terminal
/// progression `Created < InvoicePaid < TbtcLocked < Claiming < Settling` to
/// reject a stale/replayed forward-progress event that would regress an
/// already-advanced swap. Do NOT reorder these variants without updating that
/// guard; the ordering is pinned by `status_order_is_monotonic`. (Terminal
/// variants are never compared by the guard — they short-circuit earlier — so
/// their relative order is irrelevant.)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoltzSwapStatus {
    /// Swap created on Boltz, invoice ready to pay.
    Created,
    /// Hold invoice paid, waiting for Boltz to lock tBTC.
    InvoicePaid,
    /// tBTC locked on Arbitrum, ready to claim.
    TbtcLocked,
    /// Claim tx submitted, waiting for confirmation.
    Claiming,
    /// Claim mined on Arbitrum (burn/OFT-send committed), awaiting confirmation
    /// that the cross-chain bridge delivered on the destination chain. Only
    /// `Oft`/`Cctp` swaps pass through this state; `Direct` (same-chain)
    /// completes immediately. Non-terminal: the background manager polls the
    /// bridge's status API and advances to `Completed` once delivery confirms.
    Settling,
    /// Stablecoin delivered to destination.
    Completed,
    /// Swap failed.
    Failed { reason: String },
    /// Swap expired (Boltz timeout reached).
    Expired,
}

impl BoltzSwapStatus {
    /// Whether this status is terminal (no further transitions expected).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed { .. } | Self::Expired)
    }
}

/// Underlying transport for a chain. Determines recipient encoding, RPC
/// dispatch, and OFT source-contract selection.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkTransport {
    Evm,
    Solana,
    Tron,
}

/// Which USDT0 mesh a destination belongs to. Native-mesh and legacy-mesh
/// deployments live on distinct source-side OFT contracts with different
/// fee models, so the destination's mesh determines which source contract
/// the claim path quotes and bridges through.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Usdt0Kind {
    Native,
    Legacy,
}

/// Stablecoin the user receives on the destination. First-class dimension:
/// the same physical chain can offer more than one (USDT0 via OFT *and* USDC
/// via CCTP), so asset is tracked independently of the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Asset {
    /// Canonical Tether.
    Usdt,
    /// `LayerZero` USDT0 (distinct ERC20/SPL from canonical Tether).
    Usdt0,
    /// Circle USD Coin.
    Usdc,
}

impl Asset {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Asset::Usdt => "USDT",
            Asset::Usdt0 => "USDT0",
            Asset::Usdc => "USDC",
        }
    }
}

impl std::fmt::Display for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Inverse of [`Asset::as_str`]: parse a ticker back into the enum. Kept next to
/// `as_str` so adding an asset updates both directions in one place. Case
/// insensitive. `Err(())` for tickers Boltz does not deliver.
impl TryFrom<&str> for Asset {
    type Error = ();

    fn try_from(ticker: &str) -> Result<Self, Self::Error> {
        match ticker.to_ascii_uppercase().as_str() {
            "USDT" => Ok(Asset::Usdt),
            "USDT0" => Ok(Asset::Usdt0),
            "USDC" => Ok(Asset::Usdc),
            _ => Err(()),
        }
    }
}

/// How a destination's Arbitrum DEX output reaches the user. Carries the
/// per-bridge routing data the claim path needs. Internal to the registry and
/// claim paths; the public API surfaces only the coarse [`BridgeKind`].
#[derive(Clone, Debug)]
pub enum Bridge {
    /// Delivered on Arbitrum itself — no cross-chain hop. The DEX output is
    /// swept straight to the user (both same-chain USDT and USDC-on-Arbitrum).
    Direct,
    /// `LayerZero` USDT0 OFT cross-chain bridge.
    Oft {
        /// Which USDT0 mesh this destination belongs to (selects the
        /// source-side OFT contract via [`DestinationRegistry::oft_for`]).
        mesh: Usdt0Kind,
        /// `LayerZero` endpoint ID for the destination.
        lz_eid: u32,
    },
    /// Circle CCTP v2 burn + mint.
    Cctp {
        /// Circle CCTP domain id of the destination chain.
        domain: u32,
    },
}

impl Bridge {
    /// Coarse public category for this bridge.
    #[must_use]
    pub fn kind(&self) -> BridgeKind {
        match self {
            Bridge::Direct => BridgeKind::Direct,
            Bridge::Oft { .. } => BridgeKind::Oft,
            Bridge::Cctp { .. } => BridgeKind::Cctp,
        }
    }
}

/// A single selectable destination: an asset delivered on a chain via a
/// specific bridge. Unifies what used to be two parallel registries (OFT
/// `ChainSpec` and static `CctpDestination`). Built once at service init.
#[derive(Clone, Debug)]
pub struct Destination {
    /// Human chain label (`"Arbitrum One"`, `"Base"`, `"Solana"`). Together
    /// with [`asset`](Self::asset) this is the destination identity.
    pub chain_label: String,
    /// Asset the user receives.
    pub asset: Asset,
    pub transport: NetworkTransport,
    /// EVM chain ID. `None` for non-EVM transports (Solana, Tron).
    pub evm_chain_id: Option<u64>,
    /// Arbitrum token the DEX leg must produce before the bridge/delivery
    /// (`ARBITRUM_USDT_ADDRESS` for USDT/USDT0 routes, `ARBITRUM_USDC_ADDRESS`
    /// for USDC routes).
    pub dex_output_token: &'static str,
    /// Token contract on the *destination* chain (`0x…` EVM, base58 Solana),
    /// when known. Used for the "don't send to a token contract" guard.
    pub dest_token_address: Option<String>,
    pub bridge: Bridge,
}

impl Destination {
    /// OFT routing data `(mesh, lz_eid)`, present only for `Bridge::Oft`.
    #[must_use]
    pub fn oft(&self) -> Option<(Usdt0Kind, u32)> {
        match &self.bridge {
            Bridge::Oft { mesh, lz_eid } => Some((*mesh, *lz_eid)),
            _ => None,
        }
    }
}

/// Runtime registry of every supported destination across all bridges, plus
/// the source-chain OFT contracts. Built once at service init by merging the
/// USDT0 deployments API, the static [`CCTP_DESTINATIONS`] table, and the
/// Arbitrum-direct entries; stable for the process lifetime.
#[derive(Clone, Debug)]
pub struct DestinationRegistry {
    /// Source-chain label (Arbitrum USDT direct).
    pub source_chain_label: String,
    pub source_evm_chain_id: u64,
    /// Source-side native-mesh OFT contract (`None` if not on the native mesh).
    pub source_native_oft: Option<String>,
    /// Source-side legacy-mesh OFT contract (`None` if not on the legacy mesh).
    pub source_legacy_oft: Option<String>,
    pub destinations: Vec<Destination>,
}

impl DestinationRegistry {
    /// Look up a destination by its `(chain, asset)` identity. Chain match is
    /// case-insensitive; asset must match exactly. `None` if unsupported.
    #[must_use]
    pub fn find(&self, chain: &str, asset: Asset) -> Option<&Destination> {
        self.destinations
            .iter()
            .find(|d| d.asset == asset && d.chain_label.eq_ignore_ascii_case(chain))
    }

    /// Source OFT contract for the given mesh, or `None` if the source chain
    /// doesn't participate in that mesh.
    #[must_use]
    pub fn oft_for(&self, mesh: Usdt0Kind) -> Option<&str> {
        match mesh {
            Usdt0Kind::Native => self.source_native_oft.as_deref(),
            Usdt0Kind::Legacy => self.source_legacy_oft.as_deref(),
        }
    }
}

/// Coarse public category of a swap's Arbitrum -> destination leg, for
/// display and delivery-status UX (`Cctp` → Circle Iris, `Oft` → `LayerZero`
/// GUID, `Direct` → none). The data-carrying detail lives in [`Bridge`];
/// claim dispatch resolves that from the destination, not this field.
///
/// - `Direct` — delivered on Arbitrum, no cross-chain hop (USDT or USDC).
/// - `Oft`    — `LayerZero` USDT0 bridge.
/// - `Cctp`   — Circle CCTP v2 (USDC).
///
/// Defaults to `Oft` so swaps persisted before CCTP/Direct existed deserialize
/// correctly (a missing field means a pre-CCTP OFT swap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeKind {
    Direct,
    Oft,
    Cctp,
}

/// A USDC (CCTP) destination chain. Static compile-time table — CCTP routes
/// are not published by the USDT0 deployments API. Identified, like every
/// destination, by its `(chain_label, asset = USDC)` pair.
#[derive(Clone, Debug)]
pub struct CctpDestination {
    /// Human chain label (`"Base"`, `"Optimism"`). Matches the USDT0
    /// deployments-API spelling for chains that also support OFT, so a chain
    /// never appears under two different names across the two bridges.
    pub chain_label: &'static str,
    pub transport: NetworkTransport,
    /// EVM chain id of the destination chain. `None` for non-EVM transports
    /// (Solana), which expose no numeric chain id.
    pub evm_chain_id: Option<u64>,
    /// Circle CCTP domain id of the destination chain.
    pub domain: u32,
    /// USDC token contract on the destination chain (EVM `0x…`, Solana base58
    /// mint). Used to decode the delivered amount during recovery.
    pub token_address: &'static str,
}

/// Static registry of USDC (CCTP) destinations, mirroring boltz-web-app
/// `boltz-swaps` `cctp/variants.ts`. Addresses and Circle domains are verified
/// against that source.
pub const CCTP_DESTINATIONS: &[CctpDestination] = &[
    CctpDestination {
        chain_label: "Base",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(8453),
        domain: 6,
        token_address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    },
    CctpDestination {
        chain_label: "Ethereum",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(1),
        domain: 0,
        token_address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    },
    CctpDestination {
        chain_label: "Avalanche",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(43114),
        domain: 1,
        token_address: "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E",
    },
    CctpDestination {
        chain_label: "Optimism",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(10),
        domain: 2,
        token_address: "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85",
    },
    CctpDestination {
        chain_label: "Polygon PoS",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(137),
        domain: 7,
        token_address: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
    },
    CctpDestination {
        chain_label: "Unichain",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(130),
        domain: 10,
        token_address: "0x078D782b760474a361dDA0AF3839290b0EF57AD6",
    },
    CctpDestination {
        chain_label: "Linea",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(59144),
        domain: 11,
        token_address: "0x176211869cA2b568f2A7D4EE941E073a821EE1ff",
    },
    CctpDestination {
        chain_label: "Codex",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(81224),
        domain: 12,
        token_address: "0xd996633a415985DBd7D6D12f4A4343E31f5037cf",
    },
    CctpDestination {
        chain_label: "Sonic",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(146),
        domain: 13,
        token_address: "0x29219dd400f2Bf60E5a23d13be72b486d4038894",
    },
    CctpDestination {
        chain_label: "World Chain",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(480),
        domain: 14,
        token_address: "0x79A02482A880bCe3F13E09da970dC34dB4cD24D1",
    },
    CctpDestination {
        chain_label: "Monad",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(143),
        domain: 15,
        token_address: "0x754704Bc059F8C67012fEd69BC8A327a5aafb603",
    },
    CctpDestination {
        chain_label: "Sei",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(1329),
        domain: 16,
        token_address: "0xe15fC38F6D8c56aF07bbCBe3BAf5708A2Bf42392",
    },
    CctpDestination {
        chain_label: "XDC",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(50),
        domain: 18,
        token_address: "0xfA2958CB79b0491CC627c1557F441eF849Ca8eb1",
    },
    CctpDestination {
        chain_label: "Ink",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(57073),
        domain: 21,
        token_address: "0x2D270e6886d130D724215A266106e6832161EAEd",
    },
    CctpDestination {
        chain_label: "Plume",
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(98866),
        domain: 22,
        token_address: "0x222365EF19F7947e5484218551B56bb3965Aa7aF",
    },
    CctpDestination {
        chain_label: "Solana",
        transport: NetworkTransport::Solana,
        evm_chain_id: None,
        domain: 5,
        token_address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    },
];

/// A selectable swap destination. Returned by the discovery API so callers can
/// present every asset/chain/bridge combination uniformly. The `(chain_label,
/// asset)` pair is the destination identity to feed back into
/// `prepare_reverse_swap`.
#[derive(Clone, Debug)]
pub struct DestinationOption {
    /// Human chain label (`"Arbitrum One"`, `"Base"`, `"Solana"`). With
    /// [`asset`](Self::asset), the destination identity for the prepare API.
    pub chain_label: String,
    /// Asset delivered there.
    pub asset: Asset,
    pub transport: NetworkTransport,
    /// EVM chain ID of the destination chain. `None` for non-EVM transports
    /// (Solana, Tron), which expose no numeric chain id.
    pub evm_chain_id: Option<u64>,
    /// Token contract on the destination chain (`0x…` EVM, base58 Solana),
    /// when known.
    pub dest_token_address: Option<String>,
    /// Coarse bridge category (for delivery-status UX).
    pub bridge_kind: BridgeKind,
}

/// Quote result returned to caller before committing to a swap.
#[derive(Clone, Debug, Serialize)]
pub struct PreparedSwap {
    pub destination_address: String,
    /// Destination chain label; with [`asset`](Self::asset), the destination
    /// identity carried into `create_reverse_swap`.
    pub destination_chain: String,
    /// Stablecoin delivered on the destination chain.
    pub asset: Asset,
    /// Coarse bridge category for the destination leg.
    pub bridge_kind: BridgeKind,
    /// Requested stablecoin output (6 decimals).
    pub output_amount: u64,
    /// Total sats to pay (includes all fees).
    pub invoice_amount_sats: u64,
    /// Boltz service fee in sats.
    pub boltz_fee_sats: u64,
    /// tBTC amount after Boltz fee (sats).
    pub estimated_onchain_amount: u64,
    pub slippage_bps: u32,
    /// Pins fee/rate snapshot for `POST /swap/reverse`.
    pub pair_hash: String,
    /// Quote expiry (unix timestamp seconds).
    pub expires_at: u64,
}

/// Result of creating a swap on Boltz.
#[derive(Clone, Debug, Serialize)]
pub struct CreatedSwap {
    /// Swap ID (Boltz backend ID).
    pub swap_id: String,
    /// Hold invoice to pay.
    pub invoice: String,
    pub invoice_amount_sats: u64,
    pub timeout_block_height: u64,
}

/// Min/max swap limits from the Boltz pairs endpoint.
#[derive(Clone, Debug, Serialize)]
pub struct SwapLimits {
    pub min_sats: u64,
    pub max_sats: u64,
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn status_order_is_monotonic() {
        // Pins the non-terminal lifecycle progression the WS monotonicity guard
        // relies on (see `BoltzSwapStatus` doc + `ws_progress_stage`). If a
        // refactor reorders the variants, this fails loudly instead of silently
        // letting a stale `invoice.paid` regress an advanced swap.
        use BoltzSwapStatus::{Claiming, Created, InvoicePaid, Settling, TbtcLocked};
        assert!(Created < InvoicePaid);
        assert!(InvoicePaid < TbtcLocked);
        assert!(TbtcLocked < Claiming);
        assert!(Claiming < Settling);
    }

    #[macros::test_all]
    fn test_swap_status_terminal() {
        assert!(!BoltzSwapStatus::Created.is_terminal());
        assert!(!BoltzSwapStatus::InvoicePaid.is_terminal());
        assert!(!BoltzSwapStatus::TbtcLocked.is_terminal());
        assert!(!BoltzSwapStatus::Claiming.is_terminal());
        assert!(!BoltzSwapStatus::Settling.is_terminal());
        assert!(BoltzSwapStatus::Completed.is_terminal());
        assert!(BoltzSwapStatus::Expired.is_terminal());
        assert!(
            BoltzSwapStatus::Failed {
                reason: "test".to_string()
            }
            .is_terminal()
        );
    }

    #[macros::test_all]
    fn test_boltz_swap_serialization() {
        let swap = BoltzSwap {
            id: "boltz-1".to_string(),
            status: BoltzSwapStatus::Created,
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
            invoice: "lnbc1000n1...".to_string(),
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
        };

        let json = serde_json::to_string(&swap).unwrap();
        let deserialized: BoltzSwap = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "boltz-1");
        assert_eq!(deserialized.status, BoltzSwapStatus::Created);
        assert_eq!(deserialized.chain_id, 42161);
        assert_eq!(deserialized.destination_chain, "Arbitrum One");
        assert_eq!(deserialized.asset, Asset::Usdt);
    }

    /// A `Settling` swap with a `bridge_ref` must round-trip (new variant +
    /// new field name on the way out, not just the legacy alias inward).
    #[macros::test_all]
    fn settling_swap_with_bridge_ref_round_trips() {
        let mut swap = BoltzSwap {
            id: "s2".to_string(),
            status: BoltzSwapStatus::Settling,
            bridge_kind: BridgeKind::Cctp,
            key_source: SwapKeySource::Derived { claim_key_index: 0 },
            chain_id: 42161,
            claim_address: "0xabc".to_string(),
            destination_address: "0xdef".to_string(),
            destination_chain: "Base".to_string(),
            asset: Asset::Usdc,
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
            bridge_ref: Some("6:0xburn".to_string()),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        };
        let json = serde_json::to_string(&swap).unwrap();
        assert!(json.contains("\"bridge_ref\":\"6:0xburn\""));
        let back: BoltzSwap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status, BoltzSwapStatus::Settling);
        assert_eq!(back.bridge_ref.as_deref(), Some("6:0xburn"));
        // Sanity: still non-terminal.
        swap.status = back.status;
        assert!(!swap.status.is_terminal());
    }

    #[macros::test_all]
    fn bridge_kind_from_bridge() {
        assert_eq!(Bridge::Direct.kind(), BridgeKind::Direct);
        assert_eq!(
            Bridge::Oft {
                mesh: Usdt0Kind::Native,
                lz_eid: 30110
            }
            .kind(),
            BridgeKind::Oft
        );
        assert_eq!(Bridge::Cctp { domain: 6 }.kind(), BridgeKind::Cctp);
    }

    #[macros::test_all]
    fn cctp_destinations_table_is_well_formed() {
        use std::collections::HashSet;

        // 15 EVM destinations + Solana.
        assert_eq!(CCTP_DESTINATIONS.len(), 16);

        let mut labels = HashSet::new();
        let mut domains = HashSet::new();
        let mut solana_count = 0;
        for d in CCTP_DESTINATIONS {
            // Chain labels are unique: with the implicit USDC asset they form
            // each destination's identity.
            assert!(
                labels.insert(d.chain_label),
                "duplicate chain_label {}",
                d.chain_label
            );
            // Circle domains are unique per destination.
            assert!(domains.insert(d.domain), "duplicate domain {}", d.domain);
            match d.transport {
                NetworkTransport::Evm => {
                    assert!(d.token_address.starts_with("0x"));
                    assert_eq!(d.token_address.len(), 42);
                    // Every EVM destination carries a numeric chain id.
                    assert!(
                        d.evm_chain_id.is_some(),
                        "{} missing evm_chain_id",
                        d.chain_label
                    );
                }
                NetworkTransport::Solana => {
                    solana_count += 1;
                    assert!(!d.token_address.starts_with("0x"));
                    // Non-EVM transports expose no chain id.
                    assert!(
                        d.evm_chain_id.is_none(),
                        "{} has evm_chain_id",
                        d.chain_label
                    );
                }
                NetworkTransport::Tron => panic!("CCTP does not support Tron"),
            }
        }
        // Exactly one Solana destination (USDC on Solana).
        assert_eq!(solana_count, 1);
    }
}
