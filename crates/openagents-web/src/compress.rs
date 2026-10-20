//! Response compression for the whole site (#12241).
//!
//! A text response (HTML, CSS, JavaScript, JSON, SVG, wasm, plain text)
//! goes out brotli-compressed when the request accepts `br`, otherwise
//! gzip-compressed when it accepts `gzip`, with `Vary: Accept-Encoding`.
//!
//! Only a body whose length is known is compressed: a string or bytes the
//! handler built whole. A streamed body (an event stream, a live chat
//! answer, a file read in chunks) has no exact length and passes through
//! untouched, so each event still reaches the browser when it's written.
//! A response that already names a `Content-Encoding` (`/lander4`,
//! `/lander5`, the Foreman page, Everglade's precompressed builds, an
//! upstream's answer) is left alone.
//!
//! A response marked `Cache-Control: public` is a static asset or a page
//! the same for everyone. Its compressed forms are made once at the best
//! quality and kept in memory by the SHA-256 of the body, so `ui.css`
//! costs one compression per process; [`warm`] makes the largest ones at
//! start-up. Everything else is compressed per response at a fast level.

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};

/// Bodies smaller than this gain too little to be worth compressing.
const MIN_SIZE: u64 = 1024;
/// Bodies larger than this are sent as they are rather than held and
/// compressed in memory.
const MAX_SIZE: u64 = 16 * 1024 * 1024;
/// Bodies up to this size are compressed on the request's own task; larger
/// ones on the blocking pool.
const INLINE_SIZE: usize = 64 * 1024;
/// The most compressed bytes the static cache keeps.
const CACHE_BUDGET: usize = 64 * 1024 * 1024;

/// A content coding the site sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Coding {
    Brotli,
    Gzip,
}

impl Coding {
    fn name(self) -> &'static str {
        match self {
            Coding::Brotli => "br",
            Coding::Gzip => "gzip",
        }
    }
}

/// The coding to answer a request with: brotli when it's accepted, else
/// gzip, else none. A `q=0` entry refuses that coding; `*` accepts both.
pub(crate) fn negotiate(headers: &HeaderMap) -> Option<Coding> {
    let mut brotli = None;
    let mut gzip = None;
    let mut any = None;
    for entry in headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
    {
        let mut parts = entry.split(';');
        let name = parts.next().unwrap_or("").trim();
        let accepted = !parts.any(|param| {
            param
                .trim()
                .strip_prefix("q=")
                .is_some_and(|q| q.trim().parse::<f32>().is_ok_and(|q| q <= 0.0))
        });
        if name.eq_ignore_ascii_case("br") {
            brotli = Some(accepted);
        } else if name.eq_ignore_ascii_case("gzip") || name.eq_ignore_ascii_case("x-gzip") {
            gzip = Some(accepted);
        } else if name == "*" {
            any = Some(accepted);
        }
    }
    let any = any.unwrap_or(false);
    if brotli.unwrap_or(any) {
        Some(Coding::Brotli)
    } else if gzip.unwrap_or(any) {
        Some(Coding::Gzip)
    } else {
        None
    }
}

/// Whether a `Content-Type` is text worth compressing.
fn compressible(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if essence == "text/event-stream" {
        return false;
    }
    essence.starts_with("text/")
        || essence.ends_with("+json")
        || essence.ends_with("+xml")
        || matches!(
            essence.as_str(),
            "application/javascript"
                | "application/json"
                | "application/wasm"
                | "application/xml"
                | "application/x-ndjson"
                | "image/svg+xml"
        )
}

/// Compresses what [`compressible`] admits; see the module documentation.
pub(crate) async fn layer(request: Request, next: Next) -> Response {
    let coding = negotiate(request.headers());
    let head = request.method() == Method::HEAD;
    let response = next.run(request).await;
    let Some(length) = eligible(&response) else {
        return response;
    };
    let (mut parts, body) = response.into_parts();
    add_vary(&mut parts.headers);
    let Some(coding) = coding else {
        return Response::from_parts(parts, body);
    };
    // The length is exact, so the body is already in memory.
    let Ok(bytes) = axum::body::to_bytes(body, length as usize).await else {
        return Response::from_parts(parts, Body::empty());
    };
    let shared = parts
        .headers
        .get_all(header::CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|directive| directive.trim().eq_ignore_ascii_case("public"));
    // A HEAD answer has no body to compress.
    let compressed = if head {
        None
    } else if shared {
        cached(&bytes, coding).await
    } else {
        fast(&bytes, coding).await
    };
    let Some(compressed) = compressed.filter(|out| out.len() < bytes.len()) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    parts.headers.insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static(coding.name()),
    );
    parts.headers.remove(header::CONTENT_LENGTH);
    weaken_etag(&mut parts.headers);
    Response::from_parts(parts, Body::from(compressed))
}

/// The body's length when this response may be compressed.
fn eligible(response: &Response) -> Option<u64> {
    let status = response.status();
    if status.is_informational()
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::PARTIAL_CONTENT
        || status == StatusCode::NOT_MODIFIED
    {
        return None;
    }
    let headers = response.headers();
    if headers.contains_key(header::CONTENT_ENCODING) || headers.contains_key(header::CONTENT_RANGE)
    {
        return None;
    }
    let no_transform = headers
        .get_all(header::CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.to_ascii_lowercase().contains("no-transform"));
    if no_transform {
        return None;
    }
    let content_type = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    if !compressible(content_type) {
        return None;
    }
    // A streamed body has no exact length: leave it to flow as written.
    let length = response.body().size_hint().exact()?;
    (MIN_SIZE..=MAX_SIZE).contains(&length).then_some(length)
}

/// Adds `Accept-Encoding` to `Vary` unless it's already named.
fn add_vary(headers: &mut HeaderMap) {
    let named = headers
        .get_all(header::VARY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|name| {
            let name = name.trim();
            name == "*" || name.eq_ignore_ascii_case("accept-encoding")
        });
    if !named {
        headers.append(header::VARY, HeaderValue::from_static("accept-encoding"));
    }
}

/// A strong validator names the exact bytes, which compression changes.
fn weaken_etag(headers: &mut HeaderMap) {
    let Some(tag) = headers.get(header::ETAG).and_then(|v| v.to_str().ok()) else {
        return;
    };
    if tag.starts_with("W/") {
        return;
    }
    if let Ok(weak) = HeaderValue::from_str(&format!("W/{tag}")) {
        headers.insert(header::ETAG, weak);
    }
}

/// The body in `coding`, at the best level for a body compressed once
/// (`best`) or a fast level for one compressed per response.
pub(crate) fn encode(input: &[u8], coding: Coding, best: bool) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 + 64);
    match coding {
        Coding::Brotli => {
            let quality = if best { 11 } else { 5 };
            let mut writer = brotli::CompressorWriter::new(&mut out, 16 * 1024, quality, 22);
            writer.write_all(input).ok()?;
            writer.flush().ok()?;
        }
        Coding::Gzip => {
            let level = flate2::Compression::new(if best { 9 } else { 6 });
            let mut writer = flate2::write::GzEncoder::new(&mut out, level);
            writer.write_all(input).ok()?;
            writer.finish().ok()?;
        }
    }
    Some(out)
}

/// Both compressed forms of one body, each made the first time it's asked
/// for.
#[derive(Default)]
struct Entry {
    brotli: OnceLock<Bytes>,
    gzip: OnceLock<Bytes>,
}

impl Entry {
    fn slot(&self, coding: Coding) -> &OnceLock<Bytes> {
        match coding {
            Coding::Brotli => &self.brotli,
            Coding::Gzip => &self.gzip,
        }
    }

    fn kept(&self) -> bool {
        self.brotli.get().is_some() || self.gzip.get().is_some()
    }
}

#[derive(Default)]
struct Cache {
    entries: HashMap<[u8; 32], Arc<Entry>>,
    bytes: usize,
}

/// The most bodies the cache tracks.
const CACHE_ENTRIES: usize = 2048;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// The kept form of a shared body. A body seen once is compressed at the
/// fast level, so a public page that differs on every request never costs
/// the best level; from its second time it's compressed once at the best
/// level on the blocking pool and kept. Past the cache's budget bodies are
/// compressed per response at the fast level.
async fn cached(input: &Bytes, coding: Coding) -> Option<Bytes> {
    let key: [u8; 32] = Sha256::digest(input).into();
    let entry = {
        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = cache.entries.get(&key) {
            Some(entry.clone())
        } else {
            if cache.entries.len() >= CACHE_ENTRIES {
                cache.entries.retain(|_, entry| entry.kept());
            }
            if cache.entries.len() < CACHE_ENTRIES && cache.bytes < CACHE_BUDGET {
                cache.entries.insert(key, Arc::default());
            }
            None
        }
    };
    let Some(entry) = entry else {
        return fast(input, coding).await;
    };
    if let Some(done) = entry.slot(coding).get() {
        return Some(done.clone());
    }
    if cache().lock().unwrap_or_else(|e| e.into_inner()).bytes >= CACHE_BUDGET {
        return fast(input, coding).await;
    }
    keep(&entry, input, coding).await
}

/// The best-level form of `input`, made and kept in `entry`.
async fn keep(entry: &Entry, input: &Bytes, coding: Coding) -> Option<Bytes> {
    let owned = input.clone();
    let made = tokio::task::spawn_blocking(move || encode(&owned, coding, true))
        .await
        .ok()
        .flatten()
        .map(Bytes::from)?;
    if entry.slot(coding).set(made.clone()).is_ok() {
        cache().lock().unwrap_or_else(|e| e.into_inner()).bytes += made.len();
    }
    Some(made)
}

/// `input` at the fast level, on the blocking pool when it's large.
async fn fast(input: &Bytes, coding: Coding) -> Option<Bytes> {
    if input.len() <= INLINE_SIZE {
        return encode(input, coding, false).map(Bytes::from);
    }
    let input = input.clone();
    tokio::task::spawn_blocking(move || encode(&input, coding, false))
        .await
        .ok()
        .flatten()
        .map(Bytes::from)
}

/// Makes the compressed forms of the largest static assets on a thread of
/// their own, so the first visitors after a start don't wait for them.
pub fn warm() {
    std::thread::spawn(|| {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread().build() else {
            return;
        };
        let assets: [&'static str; 3] = [
            openagents_ui::stylesheet(),
            openagents_ui::script(),
            openagents_ui::assets::ALPINE_CSP_JS,
        ];
        runtime.block_on(async {
            for asset in assets {
                let bytes = Bytes::from_static(asset.as_bytes());
                let key: [u8; 32] = Sha256::digest(&bytes).into();
                let entry = cache()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .entries
                    .entry(key)
                    .or_default()
                    .clone();
                for coding in [Coding::Brotli, Coding::Gzip] {
                    let _ = keep(&entry, &bytes, coding).await;
                }
            }
        });
    });
}

#[cfg(test)]
mod tests;
