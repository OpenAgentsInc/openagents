//! `/gpui-fonts/`: the font files the GPUI web apps share (#12233).
//!
//! AgentGraph (`/agent/foreman`), `/demo/gpui`, and `/gpui` draw the
//! desktop's Geist and Geist Mono faces. Instead of compiling them into
//! each module, `apps/openagents/scripts/build-web-fonts.sh` writes them once
//! under content-hashed names with a `fonts.json` map, and each page names
//! the faces its app needs with `<link rel=preload as=fetch crossorigin
//! data-gpui-font>`. The browser starts those downloads with the page and
//! keeps one copy of each face for every app; the hashed names let them be
//! cached for good.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, html};

use crate::App;

/// Where the shared fonts are served.
pub(crate) const PREFIX: &str = "/gpui-fonts";

/// The faces `/gpui` and `/demo/gpui` draw, in the order their modules take
/// them (`openagents_web_chat_view::assets::FONT_FILES`).
pub(crate) const CHAT_FACES: [&str; 10] = [
    "Geist.ttf",
    "Geist-Italic.ttf",
    "Geist-Medium.ttf",
    "Geist-SemiBold.ttf",
    "Geist-SemiBoldItalic.ttf",
    "Geist-Bold.ttf",
    "Geist-BoldItalic.ttf",
    "GeistMono.ttf",
    "GeistMono-Medium.ttf",
    "GeistMono-SemiBold.ttf",
];

/// The faces AgentGraph draws, in the order its module takes them
/// (`openagents_web_agent_graph::assets::FONT_FILES`).
pub(crate) const GRAPH_FACES: [&str; 4] = [
    "Geist.ttf",
    "Geist-Medium.ttf",
    "Geist-SemiBold.ttf",
    "GeistMono.ttf",
];

/// Hashed names never change content, so a year and `immutable`.
const CACHE: &str = "public, max-age=31536000, immutable";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/gpui-fonts/{file}", get(font_file))
}

/// A font directory's `fonts.json`: plain names to hashed file names.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Fonts {
    hashed: HashMap<String, String>,
}

impl Fonts {
    /// Reads `dir/fonts.json`, or `None` when it's missing or names a file
    /// that isn't a plain font name in `dir`.
    pub(crate) fn read(dir: &Path) -> Option<Fonts> {
        let text = std::fs::read(dir.join("fonts.json")).ok()?;
        let map: HashMap<String, String> = serde_json::from_slice(&text).ok()?;
        for hashed in map.values() {
            font_type(hashed)?;
            if !dir.join(hashed).is_file() {
                return None;
            }
        }
        Some(Fonts { hashed: map })
    }

    /// [`Fonts::read`], once per directory.
    pub(crate) fn cached(dir: &Path) -> Option<Arc<Fonts>> {
        static CACHE: Mutex<Option<HashMap<PathBuf, Option<Arc<Fonts>>>>> = Mutex::new(None);
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .get_or_insert_with(HashMap::new)
            .entry(dir.to_path_buf())
            .or_insert_with(|| Fonts::read(dir).map(Arc::new))
            .clone()
    }

    /// The URL of the face `name` (such as `Geist.ttf`).
    pub(crate) fn url(&self, name: &str) -> Option<String> {
        self.hashed.get(name).map(|h| format!("{PREFIX}/{h}"))
    }

    /// Preloads for `names`, in order, which the app's start script reads
    /// back as the faces to hand its module; `None` if any is missing.
    pub(crate) fn preloads(&self, names: &[&str]) -> Option<Markup> {
        let urls: Vec<String> = names.iter().map(|n| self.url(n)).collect::<Option<_>>()?;
        Some(html! {
            @for (name, url) in names.iter().zip(&urls) {
                link rel="preload" as="fetch" crossorigin="anonymous" href=(url) data-gpui-font=(name);
            }
        })
    }

    fn serves(&self, file: &str) -> bool {
        self.hashed.values().any(|h| h == file)
    }
}

/// The site's shared fonts, when it has them.
pub(crate) fn fonts(app: &App) -> Option<Arc<Fonts>> {
    Fonts::cached(app.config.gpui_fonts.as_deref()?)
}

/// A font file's type, for a plain name ending in `.ttf` or `.otf`.
fn font_type(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.') {
        return None;
    }
    if name.ends_with(".ttf") {
        Some("font/ttf")
    } else if name.ends_with(".otf") {
        Some("font/otf")
    } else {
        None
    }
}

async fn font_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(dir), Some(fonts), Some(content_type)) =
        (app.config.gpui_fonts.clone(), fonts(&app), font_type(&file))
    else {
        return crate::not_found().await;
    };
    if !fonts.serves(&file) {
        return crate::not_found().await;
    }
    super::everglade::serve_build_cached(&dir, &file, content_type, &request, CACHE).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Geist.0123456789ab.ttf"), b"geist").unwrap();
        std::fs::write(dir.path().join("GeistMono.ba9876543210.ttf"), b"mono").unwrap();
        std::fs::write(
            dir.path().join("fonts.json"),
            r#"{"Geist.ttf": "Geist.0123456789ab.ttf", "GeistMono.ttf": "GeistMono.ba9876543210.ttf"}"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn pages_name_shared_faces_in_order_under_hashed_names() {
        let dir = dir();
        let fonts = Fonts::read(dir.path()).unwrap();
        assert_eq!(
            fonts.url("Geist.ttf").as_deref(),
            Some("/gpui-fonts/Geist.0123456789ab.ttf")
        );
        let html = fonts
            .preloads(&["GeistMono.ttf", "Geist.ttf"])
            .unwrap()
            .into_string();
        let mono = html.find("GeistMono.ba9876543210.ttf").unwrap();
        let geist = html.find("Geist.0123456789ab.ttf").unwrap();
        assert!(mono < geist);
        assert!(html.contains(r#"rel="preload" as="fetch" crossorigin="anonymous""#));
        assert!(html.contains(r#"data-gpui-font="Geist.ttf""#));
        assert!(fonts.preloads(&["Geist-Bold.ttf"]).is_none());
        assert!(fonts.serves("Geist.0123456789ab.ttf"));
        assert!(!fonts.serves("Geist.ttf") && !fonts.serves("fonts.json"));
    }

    #[test]
    fn a_manifest_naming_anything_but_a_font_beside_it_is_refused() {
        let dir = dir();
        std::fs::write(
            dir.path().join("fonts.json"),
            r#"{"Geist.ttf": "../secret.ttf"}"#,
        )
        .unwrap();
        assert!(Fonts::read(dir.path()).is_none());
        std::fs::write(
            dir.path().join("fonts.json"),
            r#"{"Geist.ttf": "missing.ttf"}"#,
        )
        .unwrap();
        assert!(Fonts::read(dir.path()).is_none());
        assert!(Fonts::read(Path::new("/nowhere")).is_none());
        assert_eq!(font_type("Geist.ttf"), Some("font/ttf"));
        assert_eq!(font_type("start.js"), None);
    }
}
