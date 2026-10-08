//! The aggregator: count a pool's beacons, receipts, and trusted check
//! verdicts over a window, publish the `30201` aggregate, and verify one by
//! recomputing it.

use nostr::domain::Event;
use nostr::pylon::{
    AggregateInputs, BEACON_KIND, BEACON_MARKER, CHECK_KIND, CHECK_NAMESPACE, POOL_KIND,
    POOL_MARKER, PoolAggregate, PoolPolicy, RECEIPT_KIND, RECEIPT_MARKER, Window, aggregate_event,
    compute_aggregate, verify_aggregate,
};
use serde_json::json;

use crate::identity::Identity;
use crate::now;
use crate::relay::{self, LIFETIME};

/// The default number of rate slices per window.
pub const SLICES: u32 = 12;

/// The window ending at the next whole minute and `minutes` long. The end
/// rounds up so the newest beacon, which replaced any older one on the
/// relay, was sampled inside the window.
#[must_use]
pub fn window(at: u64, minutes: u64) -> Window {
    let to = at.div_ceil(60) * 60;
    Window {
        from: to - minutes.clamp(1, 60) * 60,
        to,
    }
}

/// The beacons, the receipts since the window opened, and the policy's
/// checkers' verdicts made inside it.
async fn inputs(
    conn: &mut nostr_transport::Connection,
    window: Window,
    policy: &PoolPolicy,
) -> Result<(Vec<Event>, Vec<Event>, Vec<Event>), String> {
    let beacons = relay::query(
        conn,
        "pool-beacons",
        &[json!({"kinds": [BEACON_KIND], "#t": [BEACON_MARKER], "limit": 4_096})],
    )
    .await?;
    let receipts = relay::query(
        conn,
        "pool-receipts",
        &[json!({
            "kinds": [RECEIPT_KIND],
            "#t": [RECEIPT_MARKER],
            "since": window.from,
            "limit": 65_536,
        })],
    )
    .await?;
    let checks = if policy.checkers.is_empty() {
        Vec::new()
    } else {
        relay::query(
            conn,
            "pool-checks",
            &[json!({
                "kinds": [CHECK_KIND],
                "authors": policy.checkers,
                "#L": [CHECK_NAMESPACE],
                "since": window.from,
                "until": window.to,
                "limit": 65_536,
            })],
        )
        .await?
    };
    Ok((beacons, receipts, checks))
}

/// Compute the pool's aggregate now and, when `publish`, sign and publish it.
///
/// # Errors
///
/// When the relay cannot be read or refuses the aggregate.
pub async fn aggregate(
    aggregator: &Identity,
    relay_url: &str,
    policy: &PoolPolicy,
    minutes: u64,
    publish: bool,
) -> Result<(PoolAggregate, Option<Event>), String> {
    let mut conn = relay::connect(relay_url, aggregator, LIFETIME).await?;
    let window = window(now(), minutes);
    let (beacons, receipts, checks) = inputs(&mut conn, window, policy).await?;
    let aggregate = compute_aggregate(
        aggregator.pubkey(),
        policy,
        window,
        &AggregateInputs {
            beacons: &beacons,
            receipts: &receipts,
            checks: &checks,
        },
        now(),
    )?;
    let mut event = None;
    if publish {
        let signed = aggregate_event(aggregator.signer(), &aggregate)?;
        relay::publish(&mut conn, &signed).await?;
        event = Some(signed);
    }
    let _ = conn.close().await;
    Ok((aggregate, event))
}

/// Fetch the newest aggregate for `policy.pool` from `aggregator` and
/// recompute it from the relay's beacons and receipts.
///
/// # Errors
///
/// When no aggregate is found or it does not recompute.
pub async fn verify(
    reader: &Identity,
    relay_url: &str,
    aggregator: &str,
    policy: &PoolPolicy,
) -> Result<PoolAggregate, String> {
    let mut conn = relay::connect(relay_url, reader, LIFETIME).await?;
    let found = relay::query(
        &mut conn,
        "pool",
        &[json!({
            "kinds": [POOL_KIND],
            "authors": [aggregator],
            "#d": [policy.pool],
            "#t": [POOL_MARKER],
            "limit": 1,
        })],
    )
    .await?;
    let event = found
        .into_iter()
        .max_by_key(|e| e.created_at)
        .ok_or("no aggregate for this pool from that aggregator")?;
    let content: serde_json::Value =
        serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
    let window: Window =
        serde_json::from_value(content["window"].clone()).map_err(|e| e.to_string())?;
    let (beacons, receipts, checks) = inputs(&mut conn, window, policy).await?;
    let _ = conn.close().await;
    verify_aggregate(
        &event,
        policy,
        &AggregateInputs {
            beacons: &beacons,
            receipts: &receipts,
            checks: &checks,
        },
    )
}
