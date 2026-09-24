//! Configured NIP-PL executor.
//!
//! Leases decrypt under the relay's executor key. A match posts the fixed
//! APNs reconnect constant to the configured gateway. The request body does
//! not contain the event.

use std::time::Duration;

use secp256k1::XOnlyPublicKey;
use tokio::{io::AsyncWriteExt, net::TcpStream, time::timeout};

use crate::domain::Event;
use nostr::push_lease::{self, AcceptedLease, application_body};

use super::{config::PushExecutor, db::DbPool, subscription::event_visible_to_reader};

const LEASE_SCAN: i64 = 1_024;

/// Refuse a lease the executor cannot bind, or return when it may be stored.
///
/// # Errors
///
/// Returns the specification reason without the `invalid:` prefix.
pub async fn prepare_lease(
    db: &DbPool,
    executor: &PushExecutor,
    event: &Event,
    now: u64,
) -> Result<(), String> {
    let author = xonly(&event.pubkey)?;
    let plaintext = push_lease::open_lease(&event.content, &executor.secret, &author)
        .map_err(|_| "undecryptable content".to_owned())?;
    let stored = db
        .push_leases(now, LEASE_SCAN)
        .await
        .map_err(|_| "lease state is unavailable".to_owned())?;
    let mut previous = None;
    let mut other_endpoints = Vec::new();
    let mut active_others = 0_usize;
    for existing in &stored {
        if existing.id == event.id {
            continue;
        }
        let Ok(existing_author) = xonly(&existing.pubkey) else {
            return Err("stored lease is unreadable".to_owned());
        };
        let Ok(opened) =
            push_lease::open_lease(&existing.content, &executor.secret, &existing_author)
        else {
            return Err("stored lease is unreadable".to_owned());
        };
        let reread_now = existing
            .expiration()
            .unwrap_or(now)
            .saturating_sub(1)
            .min(now);
        let same_address = existing.pubkey == event.pubkey
            && existing.distinct_parameter() == event.distinct_parameter();
        let accepted = match push_lease::accept_lease(
            existing,
            &opened,
            reread_now,
            &executor.descriptor(),
            None,
            &[],
            0,
        ) {
            Ok(accepted) => accepted,
            Err(_) if same_address => return Err("stored lease is unreadable".to_owned()),
            Err(_) => continue,
        };
        if same_address {
            previous = Some(accepted);
        } else if accepted.active && accepted.author == event.pubkey {
            active_others += 1;
            if let Some(endpoint) = accepted.endpoint.clone() {
                other_endpoints.push(endpoint);
            }
        }
    }
    push_lease::accept_lease(
        event,
        &plaintext,
        now,
        &executor.descriptor(),
        previous.as_ref(),
        &other_endpoints,
        active_others,
    )?;
    Ok(())
}

/// Post one fixed reconnect body for each matching active lease.
pub async fn deliver_wakes(db: &DbPool, executor: &PushExecutor, event: &Event) {
    if event.kind == crate::domain::PUSH_LEASE_KIND {
        return;
    }
    let now = super::server::unix_now();
    let Ok(stored) = db.push_leases(now, LEASE_SCAN).await else {
        return;
    };
    let mut endpoints = Vec::new();
    for existing in stored {
        let Ok(author) = xonly(&existing.pubkey) else {
            continue;
        };
        let Ok(opened) = push_lease::open_lease(&existing.content, &executor.secret, &author)
        else {
            continue;
        };
        let Ok(accepted) = push_lease::accept_lease(
            &existing,
            &opened,
            now,
            &executor.descriptor(),
            None,
            &[],
            0,
        ) else {
            continue;
        };
        if !matching(&accepted, event, now) {
            continue;
        }
        if let Some(endpoint) = accepted.endpoint.clone()
            && !endpoints.iter().any(|existing| existing == &endpoint)
        {
            endpoints.push(endpoint);
        }
    }
    for endpoint in endpoints {
        let _ = post_reconnect(&executor.gateway, &endpoint).await;
    }
}

/// POST the APNs reconnect constant. The body is not derived from an event.
///
/// # Errors
///
/// Returns an error when the gateway URL is not `http://` or the POST fails.
pub async fn post_reconnect(gateway: &str, endpoint: &str) -> Result<(), String> {
    if endpoint.contains(['\r', '\n', ' ']) {
        return Err("endpoint".to_owned());
    }
    let body = application_body("apns")?;
    let rest = gateway
        .strip_prefix("http://")
        .ok_or_else(|| "gateway must be http".to_owned())?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_owned()),
    };
    if authority.is_empty() {
        return Err("gateway host is empty".to_owned());
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>()
                .map_err(|_| "the push gateway port is not a number".to_owned())?,
        ),
        None => (authority, 80),
    };
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Push-Endpoint: {endpoint}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = timeout(Duration::from_secs(2), TcpStream::connect((host, port)))
        .await
        .map_err(|_| "gateway timed out".to_owned())?
        .map_err(|error| error.to_string())?;
    timeout(Duration::from_secs(2), stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| "gateway timed out".to_owned())?
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn matching(lease: &AcceptedLease, event: &Event, now: u64) -> bool {
    let mut readers = std::collections::HashSet::new();
    readers.insert(lease.author.clone());
    push_lease::lease_matches(lease, event, now)
        && push_lease::author_may_read(event, &lease.author)
        && event_visible_to_reader(event, &readers)
}

fn xonly(pubkey: &str) -> Result<XOnlyPublicKey, String> {
    let mut bytes = [0_u8; 32];
    if pubkey.len() != 64 {
        return Err("the author public key is not valid".to_owned());
    }
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&pubkey[index * 2..index * 2 + 2], 16)
            .map_err(|_| "the author public key is not valid".to_owned())?;
    }
    XOnlyPublicKey::from_byte_array(bytes)
        .map_err(|_| "the author public key is not valid".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::push_lease::APNS_BODY;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn the_wake_posts_only_the_fixed_reconnect_constant() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let gateway = format!("http://{address}/wake");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0_u8; 4_096];
            let read = stream.read(&mut buffer).await.unwrap();
            buffer.truncate(read);
            String::from_utf8(buffer).unwrap()
        });
        post_reconnect(&gateway, "device-token").await.unwrap();
        let request = server.await.unwrap();
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        assert_eq!(body, APNS_BODY);
        assert!(!request.contains("event"));
        assert!(request.contains("X-Push-Endpoint: device-token"));
    }
}
