//! Reads the pages the chat's link cards ask for (#11126,
//! `openagents_chat_app::links`): the page's preview tags, then its
//! preview picture, each within a time and size limit. Only `https` pages
//! on a named host are read (no addresses, `localhost`, or local names),
//! redirects stay on `https` and stop after three, and no cookies or
//! credentials are sent. A page that fails any step gets a plain card.

use std::sync::Arc;
use std::time::Duration;

use openagents_chat_app::links::{self, LinkPreviews, Preview};
use tokio::runtime::Handle;
use tokio::sync::Semaphore;

/// The most pages read at once.
pub(crate) const AT_ONCE: usize = 3;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const PAGE_TIMEOUT: Duration = Duration::from_secs(6);
const IMAGE_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_REDIRECTS: usize = 3;

/// Start reading each page the cards newly asked for.
pub(crate) fn fetch_wanted(previews: &LinkPreviews, runtime: &Handle, gate: &Arc<Semaphore>) {
    for url in previews.take_wanted() {
        let previews = previews.clone();
        let gate = gate.clone();
        runtime.spawn(async move {
            let _permit = gate.acquire_owned().await;
            let preview = read(&url).await;
            previews.finish(&url, preview);
            crate::wake::ring();
        });
    }
}

/// Whether `url` may be read: `https`, a named public host, no credentials.
pub(crate) fn allowed(url: &url::Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(url::Host::Domain(host)) = url.host() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host.contains('.')
        && !host.ends_with(".local")
        && !host.ends_with(".localhost")
        && !host.ends_with(".internal")
        && !host.ends_with(".lan")
        && !host.ends_with(".home.arpa")
        && host != "localhost"
}

fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS || !allowed(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .user_agent("OpenAgents link preview")
        .build()
        .ok()
}

async fn read(link: &str) -> Option<Preview> {
    let url = url::Url::parse(link).ok().filter(allowed)?;
    let client = client()?;
    let (page, base) = tokio::time::timeout(PAGE_TIMEOUT, async {
        let response = client
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await
            .ok()?;
        if !response.status().is_success() || !allowed(response.url()) {
            return None;
        }
        let html = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|kind| kind.contains("html"));
        if !html {
            return None;
        }
        let base = response.url().clone();
        let body = bounded(response, links::MAX_PAGE_BYTES).await?;
        Some((String::from_utf8_lossy(&body).into_owned(), base))
    })
    .await
    .ok()
    .flatten()?;
    let meta = links::page_meta(&page);
    let image = match meta
        .image
        .as_deref()
        .and_then(|image| base.join(image).ok())
        .filter(allowed)
    {
        // The download has its time limit; the decode, on a blocking
        // thread, is bounded by the picture's size limits instead.
        Some(image) => match tokio::time::timeout(IMAGE_TIMEOUT, picture(&client, image)).await {
            Ok(Some(bytes)) => tokio::task::spawn_blocking(move || links::card_image(&bytes))
                .await
                .ok()
                .flatten(),
            _ => None,
        },
        None => None,
    };
    Some(Preview {
        title: meta.title,
        site: meta.site,
        image: image.map(Arc::new),
    })
}

/// The page's preview picture's bytes, at most [`links::MAX_IMAGE_BYTES`];
/// [`links::card_image`] makes them safe to show.
async fn picture(client: &reqwest::Client, url: url::Url) -> Option<Vec<u8>> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "image/png,image/jpeg")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() || !allowed(response.url()) {
        return None;
    }
    let image = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|kind| kind.starts_with("image/"));
    if !image
        || response
            .content_length()
            .is_some_and(|length| length > links::MAX_IMAGE_BYTES as u64)
    {
        return None;
    }
    bounded(response, links::MAX_IMAGE_BYTES).await
}

/// At most `limit` bytes of the body. A page longer than that is cut (its
/// head comes first); a picture longer than that is refused by
/// [`links::card_image`], which sees only the first `limit + 1` bytes.
async fn bounded(mut response: reqwest::Response, limit: usize) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        let room = (limit + 1).saturating_sub(body.len());
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if body.len() > limit {
            break;
        }
    }
    Some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_https_pages_are_read() {
        let ok = |link: &str| allowed(&url::Url::parse(link).unwrap());
        assert!(ok("https://openagents.com/roadmap"));
        assert!(ok("https://en.wikipedia.org/wiki/Rust"));
        assert!(!ok("http://openagents.com/"));
        assert!(!ok("https://127.0.0.1/"));
        assert!(!ok("https://[::1]/"));
        assert!(!ok("https://localhost/"));
        assert!(!ok("https://printer.local/"));
        assert!(!ok("https://intranet/"));
        assert!(!ok("https://user:pass@example.com/"));
        assert!(!ok("file:///etc/passwd"));
    }

    /// Reads a real page: `cargo test ... link_fetch -- --ignored`.
    #[test]
    #[ignore = "reaches the network"]
    fn reads_a_real_page() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let preview = runtime
            .block_on(read("https://github.com/OpenAgentsInc/openagents"))
            .expect("a preview");
        eprintln!(
            "{:?} {:?} {:?}",
            preview.title,
            preview.site,
            preview.image.as_ref().map(|i| i.len())
        );
        assert!(preview.image.is_some());
    }
}
