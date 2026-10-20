//! `/@{handle}/picture`: a profile's picture, served from this origin.
//!
//! The site's policy loads images from its own origin only, so this server
//! fetches the picture the profile names and serves it. It fetches only
//! `https://` addresses whose host resolves to public addresses, pins the
//! connection to the address it checked, follows no redirects, stops at
//! [`MAX_BYTES`], and serves only what starts like a PNG, JPEG, GIF, or
//! WebP, under the type it found and `nosniff`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt as _;

use super::relays::{host_port, public_ip};
use crate::App;

/// The largest picture served.
pub(crate) const MAX_BYTES: usize = 2 * 1024 * 1024;
/// How long a fetched picture is kept.
const KEEP: Duration = Duration::from_secs(600);
/// The most pictures kept.
const KEPT_MAX: usize = 128;

/// The image type `bytes` start like.
pub(crate) fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

type Kept = Mutex<HashMap<String, (Instant, Option<Arc<(&'static str, Bytes)>>)>>;

fn kept() -> &'static Kept {
    static KEPT: OnceLock<Kept> = OnceLock::new();
    KEPT.get_or_init(Kept::default)
}

/// The public address `url`'s host resolves to, when every one is public.
async fn public_address(url: &str) -> Option<(String, SocketAddr)> {
    let (host, port) = host_port(url, "https://", 443)?;
    let addresses: Vec<SocketAddr> = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    .ok()?
    .ok()?
    .collect();
    if addresses.is_empty() || !addresses.iter().all(|a| public_ip(a.ip())) {
        return None;
    }
    Some((host, addresses[0]))
}

/// Fetch `url` under the rules above.
pub(crate) async fn fetch(url: &str) -> Option<(&'static str, Bytes)> {
    if !nostr::metadata::https_url(url) {
        return None;
    }
    let (host, address) = public_address(url).await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(&host, address)
        .timeout(Duration::from_secs(5))
        .user_agent("OpenAgents profile pictures")
        .build()
        .ok()?;
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return None;
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if body.len() + chunk.len() > MAX_BYTES {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    let kind = sniff(&body)?;
    Some((kind, Bytes::from(body)))
}

async fn kept_or_fetched(url: &str) -> Option<Arc<(&'static str, Bytes)>> {
    if let Some((at, found)) = kept()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(url)
        && at.elapsed() < KEEP
    {
        return found.clone();
    }
    let found = fetch(url).await.map(Arc::new);
    let mut held = kept().lock().unwrap_or_else(PoisonError::into_inner);
    if held.len() >= KEPT_MAX {
        held.retain(|_, (at, _)| at.elapsed() < KEEP);
        if held.len() >= KEPT_MAX {
            held.clear();
        }
    }
    held.insert(url.to_string(), (Instant::now(), found.clone()));
    found
}

pub(super) async fn serve(State(app): State<App>, Path(text): Path<String>) -> Response {
    let Ok(who) = super::who(&app, &text).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let loaded = super::load(&app.config.profile_relays, &who.pubkey).await;
    let Some(url) = loaded.metadata.as_ref().and_then(|m| m.picture.clone()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(found) = kept_or_fetched(&url).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (kind, bytes) = (found.0, found.1.clone());
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=600"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_images_are_served() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF89a.."), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b"<html>"), None);
    }

    #[tokio::test]
    async fn local_and_plain_addresses_are_never_fetched() {
        for url in [
            "http://example.com/p.png",
            "https://127.0.0.1/p.png",
            "https://localhost/p.png",
            "https://169.254.169.254/latest/meta-data",
            "https://10.0.0.1/p.png",
            "https://user@example.com/p.png",
        ] {
            assert!(fetch(url).await.is_none(), "{url}");
        }
    }
}
