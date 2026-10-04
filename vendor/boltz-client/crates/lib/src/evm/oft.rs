//! USDT0 deployments API → [`DestinationRegistry`] builder.
//!
//! Destination chains are discovered at runtime from
//! `https://docs.usdt0.to/api/deployments`, so adding a new EVM destination
//! requires no client release: once USDT0 publishes it, the next service
//! init picks it up. Non-EVM destinations (Solana, Tron) still require a
//! code-level encoder in [`crate::evm::recipient`], but adding a new chain
//! on an existing transport is pure data.
//!
//! The USDT0 response exposes two meshes:
//! - `native` — the `OFTv2` mesh that carries most EVM chains.
//! - `legacyMesh` — the older `OFTv1` mesh that hosts Solana, TON, Tron,
//!   Celo, and duplicate Arbitrum/Ethereum entries with a different OFT
//!   contract used when bridging into the legacy mesh.
//!
//! Non-EVM legacy chains have `chainId: null` in the response; we infer
//! [`NetworkTransport`] from the lowercased name and drop entries for which
//! no encoder exists.

use std::collections::{HashMap, HashSet};

use platform_utils::http::HttpClient;
use serde::Deserialize;

use crate::config::{
    ARBITRUM_USDC_ADDRESS, ARBITRUM_USDT_ADDRESS, ARBITRUM_USDT0_LEGACY_OFT,
    ARBITRUM_USDT0_NATIVE_OFT,
};
use crate::error::BoltzError;
use crate::models::{
    Asset, Bridge, CCTP_DESTINATIONS, Destination, DestinationRegistry, NetworkTransport, Usdt0Kind,
};

/// Default OFT token name to look up.
const DEFAULT_OFT_NAME: &str = "usdt0";

/// Primary OFT contract names, tried in order. `OFT Program` covers Solana's
/// legacy-mesh deployment.
const PRIMARY_OFT_CONTRACT_NAMES: &[&str] = &["OFT", "OFT Adapter", "OFT Program"];

/// Contract name under which the USDT0 token address is published in the
/// deployments registry. Not every chain publishes one: on Ethereum (and
/// similar adapter-only deployments) the OFT wraps the canonical USDT and
/// there is no separate `Token` entry.
const TOKEN_CONTRACT_NAME: &str = "Token";

/// Flat per-route fee charged by the legacy mesh USDT0 bridge, in basis
/// points. The legacy `quoteOFT` staticcall does not deduct this, so any
/// inverse-quote (destination amount → required source amount) for a legacy
/// mesh route must add it back via [`legacy_mesh_source_amount`].
pub const LEGACY_MESH_FEE_BPS: u32 = 3;

/// Denominator for basis-point math.
pub const HUNDRED_PERCENT_BPS: u32 = 10_000;

/// Closed-form inverse of the legacy mesh OFT bridge fee: given a desired
/// destination amount, return the source amount needed to deliver it after
/// the flat 3 bps fee:
///
/// ```text
///     source = ceilDiv(dest * 10_000, 10_000 - 3)
/// ```
///
/// Used to short-circuit the binary-search inverse-quote for legacy mesh
/// routes, where the on-chain `quoteOFT` does not account for the bridge
/// fee. Returns `None` on `u128` overflow.
#[must_use]
pub fn legacy_mesh_source_amount(destination_amount: u128) -> Option<u128> {
    let numerator = destination_amount.checked_mul(u128::from(HUNDRED_PERCENT_BPS))?;
    let denominator = u128::from(HUNDRED_PERCENT_BPS - LEGACY_MESH_FEE_BPS);
    ceil_div(numerator, denominator)
}

/// Integer ceiling division: `(num + den - 1) / den`. Returns `None` on
/// `u128` overflow or division by zero.
#[must_use]
#[expect(clippy::arithmetic_side_effects)]
pub fn ceil_div(numerator: u128, denominator: u128) -> Option<u128> {
    if denominator == 0 {
        return None;
    }
    // `denominator - 1` cannot underflow (checked above); the final
    // division by `denominator` cannot panic for the same reason.
    let bumped = numerator.checked_add(denominator - 1)?;
    Some(bumped / denominator)
}

/// Fetch the USDT0 deployments JSON from `url` and build a
/// [`DestinationRegistry`] anchored at `source_evm_chain_id` (Arbitrum in
/// practice).
///
/// Fails if the fetch errors, the body is not parseable, the `usdt0` token
/// config is missing, or `source_evm_chain_id` is not present in USDT0's
/// `native` section.
pub async fn fetch_chain_registry(
    http_client: &dyn HttpClient,
    url: &str,
    source_evm_chain_id: u64,
) -> Result<DestinationRegistry, BoltzError> {
    let response = http_client.get(url.to_string(), None).await?;

    if !response.is_success() {
        return Err(BoltzError::Api {
            reason: format!("Failed to fetch OFT deployments: HTTP {}", response.status),
            code: None,
        });
    }

    parse_chain_registry(&response.body, source_evm_chain_id)
}

/// Parse a USDT0 deployments JSON body into a [`DestinationRegistry`]. Split out
/// from [`fetch_chain_registry`] for unit testing without an HTTP roundtrip.
pub fn parse_chain_registry(
    body: &str,
    source_evm_chain_id: u64,
) -> Result<DestinationRegistry, BoltzError> {
    let registry: OftRegistry = serde_json::from_str(body).map_err(|e| BoltzError::Api {
        reason: format!("Failed to parse OFT deployments: {e}"),
        code: None,
    })?;

    let token_config = registry
        .0
        .get(DEFAULT_OFT_NAME)
        .ok_or_else(|| BoltzError::Api {
            reason: format!("OFT token '{DEFAULT_OFT_NAME}' not found in deployments"),
            code: None,
        })?;

    let source_evm_chain_id_u32 = source_evm_chain_id_as_u32(source_evm_chain_id)?;

    // Locate the source chain in the native section to derive its display
    // name. The source must be present in the native mesh — if it isn't,
    // there's no native destination to bridge to and the claim path is
    // non-functional.
    let source_native_entry = token_config
        .native
        .iter()
        .find(|c| c.chain_id == Some(source_evm_chain_id_u32))
        .ok_or_else(|| BoltzError::Api {
            reason: format!(
                "Source chain ID {source_evm_chain_id} not found in USDT0 native deployments",
            ),
            code: None,
        })?;
    let source_chain_label = source_native_entry.name.clone();

    let source_native_oft = resolve_chain_info(source_native_entry).map(|info| info.oft_address);
    let source_legacy_oft = token_config
        .legacy_mesh
        .iter()
        .find(|c| c.chain_id == Some(source_evm_chain_id_u32))
        .and_then(resolve_chain_info)
        .map(|info| info.oft_address);

    // Pin the source-side OFT contracts. These are the addresses the Router
    // approves and calls `send` on during a cross-chain claim, so a hijacked
    // deployments feed substituting them is the only feed-compromise path that
    // yields theft. They never change per mesh: verify the feed matches the
    // pinned constant and refuse to start on a mismatch. A feed that simply
    // omits a mesh leaves that mesh unavailable (no `send` happens without an
    // address) rather than failing init, so a benign upstream restructuring
    // can't brick the client.
    let source_native_oft =
        verify_pinned_source_oft(source_native_oft, ARBITRUM_USDT0_NATIVE_OFT, "native")?;
    let source_legacy_oft =
        verify_pinned_source_oft(source_legacy_oft, ARBITRUM_USDT0_LEGACY_OFT, "legacy")?;

    // Build destinations from both sections in one pass, native first so a
    // chain appearing in both keeps the native spec. Dedup uniformly — both
    // across and within sections — by `chainId` (a chain can show up under
    // different names: "Arbitrum One" in native, "Arbitrum" in legacyMesh) and
    // by label (an exact-name repeat). `HashSet::insert` returns `false` when
    // the key was already present, which is the "already seen — skip" signal.
    let mut destinations: Vec<Destination> = Vec::new();
    let mut seen_evm_chain_ids: HashSet<u64> = HashSet::new();
    let mut seen_labels: HashSet<String> = HashSet::new();

    let sections = token_config
        .native
        .iter()
        .map(|entry| (entry, Usdt0Kind::Native))
        .chain(
            token_config
                .legacy_mesh
                .iter()
                .map(|entry| (entry, Usdt0Kind::Legacy)),
        );
    for (entry, kind) in sections {
        let Some(dest) = build_destination(entry, kind, &source_chain_label) else {
            continue;
        };
        if let Some(cid) = dest.evm_chain_id
            && !seen_evm_chain_ids.insert(cid)
        {
            continue;
        }
        if !seen_labels.insert(dest.chain_label.to_lowercase()) {
            continue;
        }
        destinations.push(dest);
    }

    // The source chain must be reachable (it's the Arbitrum USDT direct
    // destination). Verify it landed as a (source chain, USDT) destination.
    if !destinations
        .iter()
        .any(|d| d.asset == Asset::Usdt && d.chain_label.eq_ignore_ascii_case(&source_chain_label))
    {
        return Err(BoltzError::Api {
            reason: format!(
                "Source chain '{source_chain_label}' missing from USDT0 destinations after registry build",
            ),
            code: None,
        });
    }

    // Fold in the static USDC (CCTP) destinations — not published by the
    // USDT0 deployments API. Each bridges Arbitrum USDC to its Circle domain.
    for d in CCTP_DESTINATIONS {
        destinations.push(Destination {
            chain_label: d.chain_label.to_string(),
            asset: Asset::Usdc,
            transport: d.transport,
            evm_chain_id: d.evm_chain_id,
            dex_output_token: ARBITRUM_USDC_ADDRESS,
            dest_token_address: Some(d.token_address.to_string()),
            bridge: Bridge::Cctp { domain: d.domain },
        });
    }

    // USDC on Arbitrum: the CCTP burn *source* domain, so there's no burn —
    // the DEX output USDC is delivered directly, like same-chain USDT. Not in
    // the CCTP table (which lists only burn destinations), so add it here.
    // Identified as (source chain, USDC), distinct from the (source chain,
    // USDT) direct entry above.
    destinations.push(Destination {
        chain_label: source_chain_label.clone(),
        asset: Asset::Usdc,
        transport: NetworkTransport::Evm,
        evm_chain_id: Some(source_evm_chain_id),
        dex_output_token: ARBITRUM_USDC_ADDRESS,
        dest_token_address: Some(ARBITRUM_USDC_ADDRESS.to_string()),
        bridge: Bridge::Direct,
    });

    Ok(DestinationRegistry {
        source_chain_label,
        source_evm_chain_id,
        source_native_oft,
        source_legacy_oft,
        destinations,
    })
}

fn source_evm_chain_id_as_u32(source: u64) -> Result<u32, BoltzError> {
    source
        .try_into()
        .map_err(|_| BoltzError::Generic(format!("Source chain ID {source} exceeds u32")))
}

/// Verify a feed-provided source OFT address against the compile-time pin.
///
/// - feed omits it (`None`) → `Ok(None)`: that mesh is simply unavailable; no
///   `send` is ever signed without an address, so there's nothing to steal.
/// - feed matches the pin (case-insensitive) → `Ok(Some(pin))`, canonicalized
///   to the pinned casing.
/// - feed provides a *different* address → `Err`: the substitution we refuse
///   to trust, since the Router would approve and `send` on it.
fn verify_pinned_source_oft(
    feed_value: Option<String>,
    pinned: &str,
    mesh: &str,
) -> Result<Option<String>, BoltzError> {
    match feed_value {
        None => Ok(None),
        Some(addr) if addr.eq_ignore_ascii_case(pinned) => Ok(Some(pinned.to_string())),
        Some(addr) => Err(BoltzError::Api {
            reason: format!(
                "USDT0 {mesh}-mesh source OFT {addr} does not match the pinned Arbitrum \
                 contract {pinned}; refusing to use a substituted bridge contract"
            ),
            code: None,
        }),
    }
}

/// Build a USDT0 `Destination` for a single deployments entry, or `None` if
/// the entry is unsupported (missing `lzEid`, missing primary OFT contract, or
/// non-EVM transport with no encoder).
///
/// The source chain (Arbitrum) becomes a `Bridge::Direct` USDT destination —
/// same-chain delivery, no OFT hop. Every other entry is a `Bridge::Oft`
/// cross-chain destination. Asset is `USDT` for the source and adapter-only
/// deployments (`token_address.is_none()`, e.g. Ethereum) where the OFT
/// unwraps canonical Tether, and `USDT0` everywhere a distinct OFT token is
/// published (labeled accurately so a USDT0 balance isn't conflated with
/// canonical Tether).
fn build_destination(
    entry: &OftApiChain,
    mesh: Usdt0Kind,
    source_chain_label: &str,
) -> Option<Destination> {
    let (transport, evm_chain_id) = classify_transport(entry)?;
    let info = resolve_chain_info(entry)?;
    let is_source = entry.name.eq_ignore_ascii_case(source_chain_label);

    let asset = if is_source || info.token_address.is_none() {
        Asset::Usdt
    } else {
        Asset::Usdt0
    };
    let bridge = if is_source {
        Bridge::Direct
    } else {
        Bridge::Oft {
            mesh,
            lz_eid: info.lz_eid,
        }
    };

    Some(Destination {
        chain_label: entry.name.clone(),
        asset,
        transport,
        evm_chain_id,
        // OFT and same-chain USDT both DEX into Arbitrum USDT.
        dex_output_token: ARBITRUM_USDT_ADDRESS,
        dest_token_address: info.token_address,
        bridge,
    })
}

/// Infer the underlying transport from a USDT0 entry:
/// - `chainId: Some(…)` → EVM.
/// - `chainId: null` + known non-EVM name → matching variant.
/// - Anything else → `None`, causing the entry to be dropped from the
///   registry (no code-level encoder).
fn classify_transport(entry: &OftApiChain) -> Option<(NetworkTransport, Option<u64>)> {
    if let Some(id) = entry.chain_id {
        return Some((NetworkTransport::Evm, Some(u64::from(id))));
    }
    match entry.name.to_lowercase().as_str() {
        "solana" => Some((NetworkTransport::Solana, None)),
        "tron" => Some((NetworkTransport::Tron, None)),
        _ => None,
    }
}

/// Flat OFT fields the registry needs from one USDT0 entry.
struct ResolvedOftInfo {
    lz_eid: u32,
    oft_address: String,
    token_address: Option<String>,
}

fn resolve_chain_info(entry: &OftApiChain) -> Option<ResolvedOftInfo> {
    let lz_eid_str = entry.lz_eid.as_ref()?;
    let lz_eid: u32 = lz_eid_str.parse().ok()?;
    let contract = find_primary_contract(&entry.contracts)?;

    Some(ResolvedOftInfo {
        lz_eid,
        oft_address: contract.address.clone(),
        token_address: find_token_contract(&entry.contracts).map(|c| c.address.clone()),
    })
}

fn find_primary_contract(contracts: &[OftApiContract]) -> Option<&OftApiContract> {
    PRIMARY_OFT_CONTRACT_NAMES
        .iter()
        .find_map(|name| contracts.iter().find(|c| c.name == *name))
}

fn find_token_contract(contracts: &[OftApiContract]) -> Option<&OftApiContract> {
    contracts.iter().find(|c| c.name == TOKEN_CONTRACT_NAME)
}

// ─── API response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct OftRegistry(HashMap<String, OftTokenConfig>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OftTokenConfig {
    native: Vec<OftApiChain>,
    #[serde(default)]
    legacy_mesh: Vec<OftApiChain>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OftApiChain {
    name: String,
    chain_id: Option<u32>,
    lz_eid: Option<String>,
    contracts: Vec<OftApiContract>,
}

#[derive(Deserialize)]
struct OftApiContract {
    name: String,
    address: String,
    #[allow(dead_code)]
    explorer: String,
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    const ARBITRUM_CHAIN_ID: u64 = 42161;

    const SAMPLE_DEPLOYMENTS: &str = r#"{
        "usdt0": {
            "native": [
                {
                    "name": "Arbitrum One",
                    "chainId": 42161,
                    "lzEid": "30110",
                    "contracts": [
                        {"name": "Token", "address": "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9", "explorer": "https://arbiscan.io/"},
                        {"name": "OFT", "address": "0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92", "explorer": "https://arbiscan.io/"}
                    ]
                },
                {
                    "name": "Ethereum",
                    "chainId": 1,
                    "lzEid": "30101",
                    "contracts": [
                        {"name": "OFT Adapter", "address": "0x6C96dE32CEa08842dcc4058c14d3aaAD7Fa41dee", "explorer": "https://etherscan.io/"}
                    ]
                },
                {
                    "name": "Tempo",
                    "chainId": 4217,
                    "lzEid": "30410",
                    "contracts": [
                        {"name": "Token", "address": "0x20C00000000000000000000014f22CA97301EB73", "explorer": "https://explore.mainnet.tempo.xyz/"},
                        {"name": "OFT", "address": "0xaf37E8B6C9ED7f6318979f56Fc287d76c30847ff", "explorer": "https://explore.mainnet.tempo.xyz/"}
                    ]
                },
                {
                    "name": "Optimism",
                    "chainId": 10,
                    "lzEid": "30111",
                    "contracts": [
                        {"name": "Token", "address": "0x01bFF41798a0BcF287b996046Ca68b395DbC1071", "explorer": "https://optimistic.etherscan.io/"},
                        {"name": "OFT", "address": "0xF03b4d9AC1D5d1E7c4cEf54C2A313b9fe051A0aD", "explorer": "https://optimistic.etherscan.io/"}
                    ]
                },
                {
                    "name": "Polygon PoS",
                    "chainId": 137,
                    "lzEid": "30109",
                    "contracts": [
                        {"name": "Token", "address": "0xc2132D05D31c914a87C6611C10748AEb04B58e8F", "explorer": "https://polygonscan.com/"},
                        {"name": "OFT", "address": "0x6BA10300f0DC58B7a1e4c0e41f5daBb7D7829e13", "explorer": "https://polygonscan.com/"}
                    ]
                },
                {
                    "name": "HyperCore",
                    "contracts": [
                        {"name": "Token", "address": "0x25faedc3f054130dbb4e4203aca63567", "explorer": "https://app.hyperliquid.xyz/"}
                    ]
                }
            ],
            "legacyMesh": [
                {
                    "name": "Arbitrum",
                    "chainId": 42161,
                    "lzEid": "30110",
                    "contracts": [
                        {"name": "OFT", "address": "0x77652D5aba086137b595875263FC200182919B92", "explorer": "https://arbiscan.io/"},
                        {"name": "Composer", "address": "0x759BA420bF1ded1765F18C2DC3Fc57A1964A2Ad1", "explorer": "https://arbiscan.io/"}
                    ]
                },
                {
                    "name": "Celo",
                    "chainId": 42220,
                    "lzEid": "30125",
                    "contracts": [
                        {"name": "OFT", "address": "0xf10E161027410128E63E75D0200Fb6d34b2db243", "explorer": "https://celoscan.io/"}
                    ]
                },
                {
                    "name": "Solana",
                    "lzEid": "30168",
                    "contracts": [
                        {"name": "OFT Store", "address": "HyXJcgYpURfDhgzuyRL7zxP4FhLg7LZQMeDrR4MXZcMN", "explorer": "https://solscan.io/"},
                        {"name": "OFT Program", "address": "Fuww9mfc8ntAwxPUzFia7VJFAdvLppyZwhPJoXySZXf7", "explorer": "https://solscan.io/"}
                    ]
                },
                {
                    "name": "Tron",
                    "lzEid": "30420",
                    "contracts": [
                        {"name": "OFT", "address": "TFG4wBaDQ8sHWWP1ACeSGnoNR6RRzevLPt", "explorer": "https://tronscan.org/"}
                    ]
                },
                {
                    "name": "TON",
                    "lzEid": "30343",
                    "contracts": [
                        {"name": "OFT", "address": "EQCxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", "explorer": "https://tonviewer.com/"}
                    ]
                }
            ]
        }
    }"#;

    fn sample_registry() -> DestinationRegistry {
        parse_chain_registry(SAMPLE_DEPLOYMENTS, ARBITRUM_CHAIN_ID).unwrap()
    }

    /// First destination matching a chain label (case-insensitive), regardless
    /// of asset. Sufficient for the OFT fixtures here where each chain carries a
    /// single asset; the `Arbitrum One` USDT/USDC pair is looked up via
    /// `registry.find(chain, asset)` instead.
    fn by_label<'a>(reg: &'a DestinationRegistry, chain: &str) -> Option<&'a Destination> {
        reg.destinations
            .iter()
            .find(|d| d.chain_label.eq_ignore_ascii_case(chain))
    }

    #[macros::test_all]
    fn native_evm_entries_are_registered() {
        let registry = sample_registry();

        let tempo = by_label(&registry, "tempo").expect("tempo");
        assert_eq!(tempo.transport, NetworkTransport::Evm);
        assert_eq!(tempo.evm_chain_id, Some(4217));
        assert_eq!(tempo.oft(), Some((Usdt0Kind::Native, 30410)));
        assert_eq!(
            tempo.dest_token_address.as_deref(),
            Some("0x20C00000000000000000000014f22CA97301EB73")
        );
        assert_eq!(tempo.chain_label, "Tempo");
    }

    #[macros::test_all]
    fn native_duplicate_in_legacy_prefers_native() {
        // Arbitrum appears in both `native` (as "Arbitrum One") and
        // `legacyMesh` (as "Arbitrum"). The "arbitrum one" key wins as the
        // source (Direct USDT), the native OFT is recorded as the source
        // contract, and the legacy alias "arbitrum" does not leak into
        // destinations as a second Arbitrum (dedup-by-chainId).
        let registry = sample_registry();

        let arb = by_label(&registry, "arbitrum one").expect("arbitrum one");
        assert!(matches!(arb.bridge, Bridge::Direct));
        assert_eq!(arb.asset, Asset::Usdt);
        assert_eq!(
            registry.source_native_oft.as_deref(),
            Some("0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92")
        );

        assert!(
            by_label(&registry, "arbitrum").is_none(),
            "legacy-mesh alias `arbitrum` must not leak as a second destination"
        );
    }

    #[macros::test_all]
    fn legacy_only_evm_chain_falls_through_to_legacy_mesh() {
        // Celo is only in `legacyMesh`, so it lands in the destinations
        // map with `mesh == Legacy`.
        let registry = sample_registry();

        let celo = by_label(&registry, "celo").expect("celo");
        assert_eq!(celo.oft(), Some((Usdt0Kind::Legacy, 30125)));
        assert_eq!(celo.transport, NetworkTransport::Evm);
        assert_eq!(celo.evm_chain_id, Some(42220));
    }

    #[macros::test_all]
    fn non_evm_legacy_chains_infer_transport_from_name() {
        let registry = sample_registry();

        let solana = by_label(&registry, "solana").expect("solana");
        assert_eq!(solana.transport, NetworkTransport::Solana);
        assert_eq!(solana.evm_chain_id, None);
        assert_eq!(solana.oft(), Some((Usdt0Kind::Legacy, 30168)));

        let tron = by_label(&registry, "tron").expect("tron");
        assert_eq!(tron.transport, NetworkTransport::Tron);
        assert_eq!(tron.evm_chain_id, None);
    }

    #[macros::test_all]
    fn unsupported_non_evm_family_is_dropped() {
        // TON is present in the fixture but has no NetworkTransport variant,
        // so `classify_transport` returns None and the entry is skipped.
        let registry = sample_registry();
        assert!(by_label(&registry, "ton").is_none());
    }

    #[macros::test_all]
    fn entry_without_lz_eid_is_dropped() {
        // HyperCore in the fixture has no `lzEid` and no `chainId`. It would
        // classify as non-EVM, but since its name isn't in the encoder map,
        // it drops anyway — and even if the name were recognised, the missing
        // `lzEid` would keep it out.
        let registry = sample_registry();
        assert!(by_label(&registry, "hypercore").is_none());
    }

    #[macros::test_all]
    fn source_fields_aggregate_native_and_legacy_oft_addresses() {
        let registry = sample_registry();

        assert_eq!(registry.source_chain_label, "Arbitrum One");
        assert_eq!(registry.source_evm_chain_id, ARBITRUM_CHAIN_ID);
        assert_eq!(
            registry.source_native_oft.as_deref(),
            Some("0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92")
        );
        assert_eq!(
            registry.source_legacy_oft.as_deref(),
            Some("0x77652D5aba086137b595875263FC200182919B92")
        );
    }

    #[macros::test_all]
    fn oft_for_picks_source_contract_by_mesh() {
        let registry = sample_registry();

        assert_eq!(
            registry.oft_for(Usdt0Kind::Native),
            Some("0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92")
        );
        assert_eq!(
            registry.oft_for(Usdt0Kind::Legacy),
            Some("0x77652D5aba086137b595875263FC200182919B92")
        );
    }

    #[macros::test_all]
    fn asset_is_usdt_for_source_chain() {
        // Arbitrum is the source: Direct, same-chain USDT delivery, no bridge.
        let registry = sample_registry();
        let arb = by_label(&registry, "arbitrum one").expect("arbitrum one");
        assert!(matches!(arb.bridge, Bridge::Direct));
        assert_eq!(arb.asset, Asset::Usdt);
        assert_eq!(arb.dex_output_token, ARBITRUM_USDT_ADDRESS);
    }

    #[macros::test_all]
    fn asset_is_usdt_for_adapter_only_destination() {
        // Ethereum publishes only `OFT Adapter` (no `Token` entry), so the
        // adapter unwraps to canonical underlying USDT on delivery.
        let registry = sample_registry();
        let eth = by_label(&registry, "ethereum").expect("ethereum");
        assert!(matches!(eth.bridge, Bridge::Oft { .. }));
        assert!(eth.dest_token_address.is_none());
        assert_eq!(eth.asset, Asset::Usdt);
    }

    #[macros::test_all]
    fn asset_is_usdt0_for_distinct_oft_destinations() {
        // Destinations where USDT0 publishes its own OFT ERC20 (distinct
        // from any canonical Tether contract on that chain) must be
        // labeled USDT0 so users don't end up with two indistinguishable
        // "USDT" balances in their wallet.
        let registry = sample_registry();
        for id in ["tempo", "optimism"] {
            let dest = by_label(&registry, id).unwrap_or_else(|| panic!("{id}"));
            assert!(matches!(dest.bridge, Bridge::Oft { .. }), "{id}");
            assert!(dest.dest_token_address.is_some(), "{id}");
            assert_eq!(dest.asset, Asset::Usdt0, "{id}");
        }
    }

    #[macros::test_all]
    fn asset_is_usdt0_for_polygon_pos() {
        // Polygon PoS is a native-mesh destination with its own `Token`
        // entry, so users receive the distinct USDT0 OFT — not canonical
        // Tether — even though other clients sometimes label it plain
        // "USDT" on Polygon.
        let registry = sample_registry();
        let polygon = by_label(&registry, "polygon pos").expect("polygon pos");
        assert_eq!(polygon.evm_chain_id, Some(137));
        assert!(polygon.dest_token_address.is_some());
        assert_eq!(polygon.asset, Asset::Usdt0);
    }

    #[macros::test_all]
    fn cctp_and_usdc_arb_are_folded_in() {
        let registry = sample_registry();

        // CCTP burn destinations fold in from the static table.
        let base = registry.find("Base", Asset::Usdc).expect("base");
        assert_eq!(base.asset, Asset::Usdc);
        assert_eq!(base.chain_label, "Base");
        assert!(matches!(base.bridge, Bridge::Cctp { domain: 6 }));
        assert_eq!(base.dex_output_token, ARBITRUM_USDC_ADDRESS);

        // USDC on Arbitrum is Direct (no burn) — the CCTP source domain.
        let arb_usdc = registry
            .find("Arbitrum One", Asset::Usdc)
            .expect("usdc-arb");
        assert_eq!(arb_usdc.asset, Asset::Usdc);
        assert_eq!(arb_usdc.chain_label, "Arbitrum One");
        assert!(matches!(arb_usdc.bridge, Bridge::Direct));
        assert_eq!(arb_usdc.transport, NetworkTransport::Evm);
        assert_eq!(arb_usdc.evm_chain_id, Some(ARBITRUM_CHAIN_ID));
        assert_eq!(arb_usdc.dex_output_token, ARBITRUM_USDC_ADDRESS);

        // The same chain hosts both (Arbitrum One, USDT) Direct and
        // (Arbitrum One, USDC) Direct — distinct destinations keyed by asset.
        let arb_tether = registry.find("Arbitrum One", Asset::Usdt).expect("usdt");
        assert!(matches!(arb_tether.bridge, Bridge::Direct));
        assert_eq!(arb_tether.dex_output_token, ARBITRUM_USDT_ADDRESS);
    }

    #[macros::test_all]
    fn destinations_map_lists_registered_destinations_only() {
        let registry = sample_registry();
        let has = |id: &str| by_label(&registry, id).is_some();

        assert!(has("arbitrum one"));
        assert!(has("ethereum"));
        assert!(has("tempo"));
        assert!(has("celo"));
        assert!(has("solana"));
        assert!(has("tron"));
        // TON and HyperCore are unsupported and must be absent.
        assert!(!has("ton"));
        assert!(!has("hypercore"));
    }

    #[macros::test_all]
    fn missing_source_chain_errors() {
        // Source chain ID 99999 is not in the fixture → init must fail hard.
        let err = parse_chain_registry(SAMPLE_DEPLOYMENTS, 99999).unwrap_err();
        match err {
            BoltzError::Api { reason, .. } => {
                assert!(reason.contains("Source chain ID 99999"), "reason: {reason}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[macros::test_all]
    fn unknown_evm_chain_requires_zero_code_changes() {
        // Smoke test: an EVM entry the client has never seen before still
        // lands in the registry keyed by its lowercased name, fully wired up.
        let body = r#"{
            "usdt0": {
                "native": [
                    {
                        "name": "Arbitrum One",
                        "chainId": 42161,
                        "lzEid": "30110",
                        "contracts": [
                            {"name": "OFT", "address": "0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92", "explorer": ""}
                        ]
                    },
                    {
                        "name": "FutureChain",
                        "chainId": 9999,
                        "lzEid": "30999",
                        "contracts": [
                            {"name": "OFT", "address": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "explorer": ""}
                        ]
                    }
                ]
            }
        }"#;
        let registry = parse_chain_registry(body, ARBITRUM_CHAIN_ID).unwrap();
        let future = by_label(&registry, "futurechain").expect("futurechain");
        assert_eq!(future.transport, NetworkTransport::Evm);
        assert_eq!(future.evm_chain_id, Some(9999));
        assert_eq!(future.oft(), Some((Usdt0Kind::Native, 30999)));
    }

    #[macros::test_all]
    fn missing_legacy_mesh_array_parses_successfully() {
        let body = r#"{
            "usdt0": {
                "native": [
                    {
                        "name": "Arbitrum One",
                        "chainId": 42161,
                        "lzEid": "30110",
                        "contracts": [
                            {"name": "OFT", "address": "0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92", "explorer": ""}
                        ]
                    }
                ]
            }
        }"#;
        let registry = parse_chain_registry(body, ARBITRUM_CHAIN_ID).unwrap();
        // The source lands as a Direct USDT destination (no OFT hop).
        assert!(
            by_label(&registry, "arbitrum one").is_some_and(|d| matches!(d.bridge, Bridge::Direct))
        );
        assert!(registry.source_legacy_oft.is_none());
    }

    #[macros::test_all]
    fn substituted_native_source_oft_is_rejected() {
        // A hijacked feed swapping in a different Arbitrum native OFT — the
        // one address the Router approves and `send`s on — must hard-fail
        // init rather than let the claim path sign a send to an attacker
        // contract.
        let body = r#"{
            "usdt0": {
                "native": [
                    {
                        "name": "Arbitrum One",
                        "chainId": 42161,
                        "lzEid": "30110",
                        "contracts": [
                            {"name": "OFT", "address": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "explorer": ""}
                        ]
                    }
                ]
            }
        }"#;
        let err = parse_chain_registry(body, ARBITRUM_CHAIN_ID).unwrap_err();
        match err {
            BoltzError::Api { reason, .. } => {
                assert!(
                    reason.contains("native-mesh source OFT"),
                    "reason: {reason}"
                );
                assert!(reason.contains("pinned"), "reason: {reason}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[macros::test_all]
    fn substituted_legacy_source_oft_is_rejected() {
        // Native matches the pin, but the legacy-mesh source OFT is
        // substituted → still rejected (the legacy OFT is an equally valid
        // approve+send target for legacy-mesh routes).
        let body = r#"{
            "usdt0": {
                "native": [
                    {
                        "name": "Arbitrum One",
                        "chainId": 42161,
                        "lzEid": "30110",
                        "contracts": [
                            {"name": "OFT", "address": "0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92", "explorer": ""}
                        ]
                    }
                ],
                "legacyMesh": [
                    {
                        "name": "Arbitrum",
                        "chainId": 42161,
                        "lzEid": "30110",
                        "contracts": [
                            {"name": "OFT", "address": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "explorer": ""}
                        ]
                    }
                ]
            }
        }"#;
        let err = parse_chain_registry(body, ARBITRUM_CHAIN_ID).unwrap_err();
        match err {
            BoltzError::Api { reason, .. } => {
                assert!(
                    reason.contains("legacy-mesh source OFT"),
                    "reason: {reason}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[macros::test_all]
    fn source_oft_match_is_case_insensitive() {
        // The pin compare must ignore EIP-55 casing: a feed returning the same
        // address lowercased is accepted and canonicalized to the pin.
        let body = r#"{
            "usdt0": {
                "native": [
                    {
                        "name": "Arbitrum One",
                        "chainId": 42161,
                        "lzEid": "30110",
                        "contracts": [
                            {"name": "OFT", "address": "0x14e4a1b13bf7f943c8ff7c51fb60fa964a298d92", "explorer": ""}
                        ]
                    }
                ]
            }
        }"#;
        let registry = parse_chain_registry(body, ARBITRUM_CHAIN_ID).unwrap();
        assert_eq!(
            registry.source_native_oft.as_deref(),
            Some("0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92")
        );
    }

    fn contract(name: &str, address: &str) -> OftApiContract {
        OftApiContract {
            name: name.to_string(),
            address: address.to_string(),
            explorer: String::new(),
        }
    }

    #[macros::test_all]
    fn primary_contract_name_precedence_prefers_oft_over_adapter() {
        // When a chain advertises both `OFT` and `OFT Adapter`, the resolver
        // must pick `OFT` (first entry in `PRIMARY_OFT_CONTRACT_NAMES`),
        // regardless of array order. Guards against a future reorder of the
        // precedence list or a `.find()` → `.rfind()` slip.
        let contracts = vec![
            contract("OFT Adapter", "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            contract("OFT", "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ];
        assert_eq!(
            find_primary_contract(&contracts).map(|c| c.address.as_str()),
            Some("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            "must prefer `OFT` over `OFT Adapter` regardless of array order"
        );
    }

    #[macros::test_all]
    fn primary_contract_falls_back_to_adapter_when_oft_absent() {
        // Only an adapter published → resolver falls back to it.
        let contracts = vec![contract("OFT Adapter", "0xadapter")];
        assert_eq!(
            find_primary_contract(&contracts).map(|c| c.address.as_str()),
            Some("0xadapter")
        );

        // Ethereum in the fixture is adapter-only with no separate Token entry.
        let registry = sample_registry();
        let eth = by_label(&registry, "ethereum").expect("ethereum");
        assert!(eth.dest_token_address.is_none());
    }

    #[macros::test_all]
    fn missing_token_config_fails() {
        let body = r#"{"other": {"native": [], "legacyMesh": []}}"#;
        let err = parse_chain_registry(body, ARBITRUM_CHAIN_ID).unwrap_err();
        match err {
            BoltzError::Api { reason, .. } => assert!(reason.contains("usdt0")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[macros::test_all]
    fn legacy_mesh_source_amount_matches_known_vector() {
        assert_eq!(
            legacy_mesh_source_amount(1_000_000_000),
            Some(1_000_300_091)
        );
    }

    #[macros::test_all]
    fn legacy_mesh_source_amount_zero() {
        assert_eq!(legacy_mesh_source_amount(0), Some(0));
    }

    #[macros::test_all]
    fn legacy_mesh_source_amount_rounds_up() {
        assert_eq!(legacy_mesh_source_amount(1), Some(2));
    }

    #[macros::test_all]
    fn legacy_mesh_source_amount_returns_none_on_overflow() {
        assert_eq!(legacy_mesh_source_amount(u128::MAX), None);
    }

    #[macros::test_all]
    fn ceil_div_basic_cases() {
        assert_eq!(ceil_div(0, 5), Some(0));
        assert_eq!(ceil_div(6, 3), Some(2));
        assert_eq!(ceil_div(7, 3), Some(3));
        assert_eq!(ceil_div(1, 1), Some(1));
    }

    #[macros::test_all]
    fn ceil_div_division_by_zero_is_none() {
        assert_eq!(ceil_div(42, 0), None);
    }

    #[macros::test_all]
    fn ceil_div_overflow_is_none() {
        assert_eq!(ceil_div(u128::MAX, 3), None);
    }
}
