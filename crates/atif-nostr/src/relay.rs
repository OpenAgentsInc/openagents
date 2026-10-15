//! Bounded relay exchange. External locators require a separate source allowlist.
use crate::{MAX_CHUNKS, Result, Verified, carriage};
use futures_util::StreamExt;
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::Duration;

const LIFETIME: Duration = Duration::from_secs(60);

pub async fn publish(url: &str, secret: &SecretKey, bundle: &carriage::Bundle) -> Result<()> {
    carriage::validate_bundle(bundle, Some(secret))?;
    // Reconnect in batches so the transport's bounded frame counter never
    // turns a valid large trajectory into an unbounded subscription.
    for batch in bundle.chunks.chunks(128) {
        let mut socket = Connection::connect(url, secret, LIFETIME).await?;
        for event in batch {
            send(&mut socket, event).await?;
        }
        socket.close().await?;
    }
    let mut socket = Connection::connect(url, secret, LIFETIME).await?;
    send(&mut socket, &bundle.manifest).await?;
    socket.close().await
}
async fn send(socket: &mut Connection, event: &Event) -> Result<()> {
    socket.send(json!(["EVENT", event])).await?;
    for _ in 0..16 {
        let value = socket.next().await?;
        if value[0] == "OK" && value[1] == event.id {
            return if value[2] == true {
                Ok(())
            } else {
                Err("relay refused trajectory publication".into())
            };
        }
    }
    Err("relay did not acknowledge the trajectory event".into())
}
async fn query(url: &str, secret: &SecretKey, filter: Value, limit: usize) -> Result<Vec<Event>> {
    let mut socket = Connection::connect(url, secret, LIFETIME)
        .await?
        .with_frame_budget(limit + 4);
    socket.send(json!(["REQ", "atif", filter])).await?;
    let mut events = Vec::new();
    loop {
        let value = socket.next().await?;
        if value[0] == "EVENT" && value[1] == "atif" {
            let event: Event =
                serde_json::from_value(value[2].clone()).map_err(|e| e.to_string())?;
            if events.iter().any(|held: &Event| held.id == event.id) {
                continue;
            }
            if events.len() >= limit {
                return Err("relay exceeded the trajectory event bound".into());
            }
            events.push(event);
            if events.len() == limit {
                break;
            }
        } else if value[0] == "EOSE" && value[1] == "atif" {
            break;
        } else if value[0] == "CLOSED" && value[1] == "atif" {
            return Err("trajectory is unavailable to this reader".into());
        }
    }
    socket.send(json!(["CLOSE", "atif"])).await?;
    socket.close().await?;
    Ok(events)
}

/// Retrieve exact events. No URL named by a manifest is followed unless its
/// HTTPS hostname is explicitly allowed by this reader.
pub async fn fetch(
    url: &str,
    secret: &SecretKey,
    event_id: &str,
    source_hosts: &BTreeSet<String>,
) -> Result<Verified> {
    tokio::time::timeout(LIFETIME, fetch_inner(url, secret, event_id, source_hosts))
        .await
        .map_err(|_| "trajectory retrieval deadline exceeded".to_string())?
}
async fn fetch_inner(
    url: &str,
    secret: &SecretKey,
    event_id: &str,
    source_hosts: &BTreeSet<String>,
) -> Result<Verified> {
    crate::trajectory::hex(event_id)?;
    let events = query(
        url,
        secret,
        json!({"ids":[event_id],"kinds":[3198,3188],"limit":1}),
        1,
    )
    .await?;
    let event = events
        .into_iter()
        .find(|e| e.id == event_id)
        .ok_or("content_unavailable: trajectory declaration")?;
    let manifest = carriage::manifest(&event, Some(secret))?;
    if manifest.chunks().is_empty() {
        return from_sources(&manifest, source_hosts).await;
    }
    if event.kind == 3188 && manifest.chunks().iter().any(|c| c["event"].is_null()) {
        let tag = |name: &str| {
            event
                .tags
                .iter()
                .find(|t| t.name() == Some(name))
                .and_then(|t| t.value())
        };
        let found=query(url,secret,json!({"authors":[event.pubkey],"kinds":[3188],"#h":[tag("h")],"#p":[tag("p")],"limit":manifest.chunks().len()+1}),manifest.chunks().len()+1).await?;
        let chunks = found
            .into_iter()
            .filter(|e| e.id != event.id)
            .collect::<Vec<_>>();
        return carriage::receive(&event, &chunks, Some(secret));
    }
    let mut chunks = Vec::new();
    if manifest.chunks().iter().any(|c| c["event"].is_null()) {
        let hashes = manifest
            .chunks()
            .iter()
            .filter_map(|c| c["digest"].as_str().and_then(|s| s.strip_prefix("sha256:")))
            .collect::<Vec<_>>();
        chunks=query(url,secret,json!({"authors":[event.pubkey],"kinds":[3199],"#x":hashes,"limit":manifest.chunks().len()}),manifest.chunks().len()).await?;
    } else {
        for batch in manifest.chunks().chunks(128) {
            let ids = batch
                .iter()
                .filter_map(|v| v["event"].as_str())
                .collect::<Vec<_>>();
            chunks.extend(query(url,secret,json!({"ids":ids,"authors":[event.pubkey],"kinds":[if event.kind==3188 {3188}else{3199}],"limit":batch.len()}),batch.len()).await?);
            if chunks.len() > MAX_CHUNKS {
                return Err("too many trajectory chunks".into());
            }
        }
    }
    carriage::receive(&event, &chunks, Some(secret))
}
async fn from_sources(manifest: &crate::Manifest, hosts: &BTreeSet<String>) -> Result<Verified> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(LIFETIME)
        .build()
        .map_err(|e| e.to_string())?;
    for source in manifest.body()["artifact"]["sources"]
        .as_array()
        .ok_or("content_unavailable: trajectory sources")?
    {
        let Some(url) = source["url"]
            .as_str()
            .and_then(|u| reqwest::Url::parse(u).ok())
        else {
            continue;
        };
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || !url.host_str().is_some_and(|h| hosts.contains(h))
        {
            continue;
        }
        let Ok(response) = client.get(url).send().await else {
            continue;
        };
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > manifest.body()["artifact"]["size"].as_u64().unwrap_or(0))
        {
            continue;
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(part) = stream.next().await {
            let part = part.map_err(|_| "trajectory source transfer failed")?;
            if bytes.len().saturating_add(part.len())
                > manifest.body()["artifact"]["size"].as_u64().unwrap_or(0) as usize
            {
                return Err("trajectory source exceeded its byte bound".into());
            }
            bytes.extend_from_slice(&part);
        }
        return crate::trajectory::verify_bytes(manifest, bytes);
    }
    Err("content_unavailable: no permitted trajectory source".into())
}
