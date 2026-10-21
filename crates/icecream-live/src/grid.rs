//! `icecream-live grid-check`: the Grid's arch to the zone, end to end, in
//! a browser from `openagents browser run`. It opens the Grid with the
//! arch, stands the player in front of it so the page prefetches the
//! zone's first play, walks through, waits for the zone to report its
//! first play from inside the Grid, and comes back. One JSON line.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::{Args, cdp, epoch_ms, host, runtime};

pub fn run(mut args: Args) -> Result<()> {
    let host = args.host();
    let port = cdp::port().context("run this under `openagents browser run`")?;
    let line = runtime()?.block_on(check(&host, port))?;
    println!("{line}");
    Ok(())
}

fn hosted(value: &Value) -> Value {
    value
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

async fn check(host: &str, port: u16) -> Result<Value> {
    let wait = Duration::from_secs(60);
    let mut acks = host::Acks::open(host).await?;
    let base = format!(
        "{host}/t/grid/?zone=grid&offline&hosted={}",
        icecream_zone::TENANT
    );
    let opened = Instant::now();
    let mut page = cdp::Page::open(port, &base).await?;
    let first = hosted(&page.until("window.verse_hosted", wait).await?);
    let grid_ms = opened.elapsed().as_millis();
    let far = first["near"] == false && first["prefetch"] == "idle";

    // In front of the arch, facing it: near enough to prefetch.
    let front = &first["front"];
    let at = format!(
        "{:.2},{:.2},{:.3}",
        front[0].as_f64().unwrap_or(0.0),
        front[1].as_f64().unwrap_or(0.0),
        front[2].as_f64().unwrap_or(0.0)
    );
    page.navigate(&format!("{base}&at={at}")).await?;
    let prefetch = acks
        .wait(
            &json!({ "tenant": icecream_zone::TENANT, "kind": "prefetch" }),
            wait,
        )
        .await?;
    page.until(
        "window.verse_hosted && JSON.parse(window.verse_hosted).prefetch === 'prefetched'",
        wait,
    )
    .await?;

    // Walk through.
    let walked = epoch_ms();
    page.eval("document.getElementById('everglade-canvas').focus(), 1")
        .await?;
    page.key("w", "KeyW", true).await?;
    let opened = page
        .until(
            "window.verse_hosted && JSON.parse(window.verse_hosted).open",
            Duration::from_secs(15),
        )
        .await;
    page.key("w", "KeyW", false).await?;
    opened?;
    let crossed = epoch_ms();
    // The zone's own first play, timed from the moment its frame began
    // loading.
    let zone = acks
        .wait(
            &json!({ "tenant": icecream_zone::TENANT, "kind": "first_play" }),
            wait,
        )
        .await?;

    // And back.
    page.eval("[...document.querySelectorAll('button')].find(b => b.textContent === 'Back to the Grid').click(), 1")
        .await?;
    let back = page
        .until(
            "window.verse_hosted && !JSON.parse(window.verse_hosted).open",
            Duration::from_secs(10),
        )
        .await
        .is_ok();
    Ok(json!({
        "grid_ready_ms": grid_ms,
        "arch_far_and_idle_at_spawn": far,
        "prefetch": prefetch,
        "walk_to_cross_ms": (crossed - walked).round(),
        "zone_first_play_ms": zone["first_play_ms"],
        "zone_first_play_network_bytes": zone["bytes"],
        "zone_first_play": zone,
        "back_on_the_grid": back,
    }))
}
