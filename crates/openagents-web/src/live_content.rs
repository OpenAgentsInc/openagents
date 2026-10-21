//! Content read from the `openagents` tenant's livehost manifest (#12435).
//!
//! With `OPENAGENTS_LIVE_RELAY`, `OPENAGENTS_LIVE_STORE`, and
//! `OPENAGENTS_LIVE_PUBKEY` set, the server follows the tenant's signed
//! deploy pointer on `OPENAGENTS_LIVE_CHANNEL` (default `staging`) and reads
//! docs, legal text, the blog, and the design-language stylesheet and
//! script from the newest manifest. A path the manifest lacks, or a server
//! with no follower, reads the copy compiled into the binary, so a content
//! deploy can only replace text, never remove a page.
//!
//! Paths are relative to this crate, as the compiled-in copies are:
//! `content/docs/mac.md`, `static/fonts.css`. The design-language bundle is
//! `openagents-ui.css` and `openagents-ui.js` (`openagents-ui-bundle`).

use std::borrow::Cow;
use std::sync::{Arc, OnceLock, RwLock};

use livehost::follow::{FollowConfig, Follower};
use livehost::store::Store;

/// The tenant this site is in its manifests.
pub const TENANT: &str = "openagents";

static FOLLOWER: OnceLock<Arc<Follower>> = OnceLock::new();

/// Starts following the pointer the environment names, on the current
/// Tokio runtime. Returns whether it started; without the variables the
/// site serves its compiled-in content only.
pub fn start_from_env() -> Result<bool, String> {
    let variable = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let (Some(relay), Some(store), Some(pubkey)) = (
        variable("OPENAGENTS_LIVE_RELAY"),
        variable("OPENAGENTS_LIVE_STORE"),
        variable("OPENAGENTS_LIVE_PUBKEY"),
    ) else {
        return Ok(false);
    };
    let channel = variable("OPENAGENTS_LIVE_CHANNEL").unwrap_or_else(|| "staging".to_owned());
    let store = Store::parse(&store).map_err(|error| format!("live content store: {error:#}"))?;
    let follower = Follower::start(FollowConfig {
        relay,
        store,
        tenant: TENANT.to_owned(),
        channel,
        pubkey,
    })
    .map_err(|error| format!("live content: {error:#}"))?;
    install(follower);
    Ok(true)
}

/// Uses `follower` for live content. The first call wins.
pub fn install(follower: Arc<Follower>) {
    let _ = FOLLOWER.set(follower);
}

fn follower() -> Option<Arc<Follower>> {
    #[cfg(test)]
    if let Some(follower) = TEST_FOLLOWER.with(|cell| cell.borrow().clone()) {
        return Some(follower);
    }
    FOLLOWER.get().cloned()
}

#[cfg(test)]
thread_local! {
    /// A follower for one test thread, so other tests keep the compiled copies.
    static TEST_FOLLOWER: std::cell::RefCell<Option<Arc<Follower>>> =
        const { std::cell::RefCell::new(None) };
}

/// The text at `path` in the live manifest, or `fallback`.
pub fn text(path: &str, fallback: &'static str) -> Cow<'static, str> {
    follower()
        .and_then(|follower| follower.current())
        .and_then(|content| content.text(path).map(str::to_owned))
        .map_or(Cow::Borrowed(fallback), Cow::Owned)
}

/// The live manifest's hash, if one is loaded.
pub fn manifest() -> Option<String> {
    let content = follower()?.current()?;
    Some(content.manifest_hash.clone())
}

/// One part of the design-language bundle: its text and cache-busting
/// version.
pub struct Part {
    pub text: Cow<'static, str>,
    pub version: Cow<'static, str>,
}

struct Live {
    manifest: String,
    css: Option<Arc<Part>>,
    js: Option<Arc<Part>>,
}

/// The live bundle, worked out once per manifest rather than per page.
static BUNDLE: RwLock<Option<Arc<Live>>> = RwLock::new(None);

fn live_bundle() -> Option<Arc<Live>> {
    let content = follower()?.current()?;
    if let Some(live) = BUNDLE.read().ok()?.as_ref()
        && live.manifest == content.manifest_hash
    {
        return Some(live.clone());
    }
    let part = |path: &str, compiled: &str| {
        let text = content.text(path).filter(|text| *text != compiled)?;
        Some(Arc::new(Part {
            version: Cow::Owned(openagents_ui::bundle::version(text.as_bytes())),
            text: Cow::Owned(text.to_owned()),
        }))
    };
    let live = Arc::new(Live {
        manifest: content.manifest_hash.clone(),
        css: part("openagents-ui.css", openagents_ui::stylesheet()),
        js: part("openagents-ui.js", openagents_ui::script()),
    });
    if let Ok(mut bundle) = BUNDLE.write() {
        *bundle = Some(live.clone());
    }
    Some(live)
}

/// The design-language stylesheet: the live manifest's, else the compiled one.
pub fn ui_stylesheet() -> Arc<Part> {
    live_bundle()
        .and_then(|live| live.css.clone())
        .unwrap_or_else(|| {
            Arc::new(Part {
                text: Cow::Borrowed(openagents_ui::stylesheet()),
                version: Cow::Borrowed(openagents_ui::stylesheet_version()),
            })
        })
}

/// The design-language script: the live manifest's, else the compiled one.
pub fn ui_script() -> Arc<Part> {
    live_bundle()
        .and_then(|live| live.js.clone())
        .unwrap_or_else(|| {
            Arc::new(Part {
                text: Cow::Borrowed(openagents_ui::script()),
                version: Cow::Borrowed(openagents_ui::script_version()),
            })
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use axum::http::HeaderMap;
    use livehost::manifest::{HOST_VERSION, Manifest, SCHEMA};
    use nostr::domain::RelaySigner;

    use super::*;

    #[tokio::test]
    async fn docs_and_the_stylesheet_come_from_the_live_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().to_str().unwrap();
        let store = Store::parse(store_path).unwrap();
        let mut objects = BTreeMap::new();
        for (path, body) in [
            (
                "content/docs/mac.md",
                "# Mac, live\n\nThis guide came from a manifest.\n",
            ),
            ("openagents-ui.css", "body { color: rebeccapurple; }\n"),
        ] {
            objects.insert(path.to_owned(), store.put(body.as_bytes()).await.unwrap().0);
        }
        let document = Manifest {
            schema: SCHEMA.into(),
            tenant: TENANT.into(),
            host_min: HOST_VERSION,
            parent: None,
            objects,
            routes: Vec::new(),
            modules: BTreeMap::new(),
            handlers: BTreeMap::new(),
            encodings: BTreeMap::new(),
            meta: None,
        };
        let (hash, _) = store.put(&document.to_bytes()).await.unwrap();
        let key = RelaySigner::from_secret_hex(&"5a".repeat(32)).unwrap();
        let follower = Follower::new(FollowConfig {
            relay: "ws://127.0.0.1:9".into(),
            store: Store::parse(store_path).unwrap(),
            tenant: TENANT.into(),
            channel: "staging".into(),
            pubkey: key.pubkey().into(),
        })
        .unwrap();

        // Before any pointer, and for a key that isn't the tenant's, the
        // compiled copies stay.
        TEST_FOLLOWER.with(|cell| *cell.borrow_mut() = Some(follower.clone()));
        assert_eq!(text("content/docs/mac.md", "compiled"), "compiled");
        let stranger = RelaySigner::from_secret_hex(&"6b".repeat(32)).unwrap();
        let forged =
            livehost::pointer::sign(&stranger, TENANT, "staging", &hash, None, 100).unwrap();
        assert!(follower.receive(&forged).await.is_err());
        assert_eq!(text("content/docs/mac.md", "compiled"), "compiled");

        let pointer = livehost::pointer::sign(&key, TENANT, "staging", &hash, None, 100).unwrap();
        follower.receive(&pointer).await.unwrap();
        assert_eq!(manifest(), Some(hash));
        assert_eq!(text("content/docs/missing.md", "compiled"), "compiled");
        let page = crate::pages::content::doc_page(&HeaderMap::new(), "mac");
        let body = axum::body::to_bytes(page.into_body(), 1 << 22)
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&body);
        assert!(body.contains("This guide came from a manifest."), "{body}");

        let css = ui_stylesheet();
        assert_eq!(css.text, "body { color: rebeccapurple; }\n");
        assert_ne!(css.version, openagents_ui::stylesheet_version());
        assert!(crate::theme::style_tag().contains(css.version.as_ref()));
        assert_eq!(ui_script().text, openagents_ui::script());
        TEST_FOLLOWER.with(|cell| *cell.borrow_mut() = None);
    }
}
