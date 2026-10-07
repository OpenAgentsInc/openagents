//! The Pylon Field's relay source: verified beacons and receipts, projected
//! into NIP-PYLON `pylon` world states. Verse polls a [`RelayField`] and
//! draws what it returns; nothing else feeds a pylon's glow.

use std::collections::{BTreeMap, BTreeSet};

use nostr::pylon::{
    self, Outcome, PylonState, RECEIPT_KIND, RECEIPT_MARKER, parse_receipt, project,
};
use serde_json::json;

use crate::client::beacons;
use crate::identity::Identity;
use crate::now;
use crate::relay::{self, LIFETIME};

/// How far back receipts count toward a pylon's job total.
pub const RECEIPT_WINDOW_SECS: u64 = 24 * 3_600;

/// A relay-backed source of pylon states for one pool.
pub struct RelayField {
    pub relay: String,
    /// Only pylons whose beacon asks to join this pool; `None` keeps all.
    pub pool: Option<String>,
    reader: Identity,
}

impl RelayField {
    /// A field over `relay`, reading as `reader` (any key; the relay needs
    /// one for NIP-42).
    #[must_use]
    pub fn new(relay: &str, pool: Option<&str>, reader: Identity) -> Self {
        Self {
            relay: relay.into(),
            pool: pool.map(str::to_string),
            reader,
        }
    }

    /// Fetch, verify, and project. Stale beacons come back as `unknown`.
    /// At most 256 pylons, newest first, as the world projection allows.
    ///
    /// # Errors
    ///
    /// When the relay cannot be read.
    pub async fn poll(&self) -> Result<Vec<PylonState>, String> {
        let mut conn = relay::connect(&self.relay, &self.reader, LIFETIME).await?;
        let book = beacons(&mut conn, None).await?;
        let receipts = relay::query(
            &mut conn,
            "receipts",
            &[json!({
                "kinds": [RECEIPT_KIND],
                "#t": [RECEIPT_MARKER],
                "since": now().saturating_sub(RECEIPT_WINDOW_SECS),
                "limit": 5_000,
            })],
        )
        .await?;
        let _ = conn.close().await;

        let mut seen = BTreeSet::new();
        let mut jobs: BTreeMap<String, u64> = BTreeMap::new();
        for event in &receipts {
            let Ok(receipt) = parse_receipt(event, None) else {
                continue;
            };
            if receipt.outcome == Outcome::Accepted
                && seen.insert((receipt.buyer.clone(), receipt.request.clone()))
            {
                *jobs.entry(receipt.address()).or_default() += 1;
            }
        }
        let at = now();
        let mut states: Vec<(u64, PylonState)> = book
            .iter()
            .filter(|(_, b)| self.pool.as_ref().is_none_or(|p| b.pools.contains(p)))
            .map(|(_, b)| {
                (
                    b.observed_at,
                    project(b, at, jobs.get(&b.address()).copied().unwrap_or(0)),
                )
            })
            .collect();
        states.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(states.into_iter().take(256).map(|(_, s)| s).collect())
    }
}

/// Whether a pylon state should glow: fresh and serving at least one job.
#[must_use]
pub fn glowing(state: &PylonState) -> bool {
    state.status == "online" && state.busy > 0
}

/// Re-exported so Verse needs only this crate for the state type.
pub use pylon::PylonState as State;
