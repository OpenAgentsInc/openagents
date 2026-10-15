//! What a link to any page shows when it is pasted into a chat app
//! (iMessage, Slack, X, Discord, WhatsApp, our own phone app).
//!
//! Every page's `<head>` names a preview picture (`og:image`) drawn here
//! for the kind of page it is ([`image`]); a shared chat draws its own
//! ([`crate::pages::chat::share`]).
//!
//! Link readers fetch pages signed out, so a page only signed-in people
//! may open sends them to `/login?return_to=...` like any signed-out
//! visitor. The sign-in page then describes the page it returns to, by
//! its kind and never by its contents ([`signed_out`]): a chat link reads
//! "A chat on OpenAgents", not "Log in", and nothing private shows. No
//! request is told apart by its user agent.
//!
//! The kind of page comes from the first segment of its path: a fixed
//! table of the site's own sections, not a reading of what anyone wrote.

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::App;
use crate::pages::chat::share::card;

/// The pictures' address prefix.
pub(crate) const PREFIX: &str = "/og";
/// Changes when the drawing changes, so link readers fetch it again.
const VERSION: &str = "1";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/og/{file}", get(kind_image))
        .route("/og/docs/{file}", get(doc_image))
}

/// The kinds of page a preview names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Home,
    Chat,
    Projects,
    Settings,
    Environments,
    Work,
    Device,
    Docs,
    Roadmap,
    Promises,
    Download,
    Pricing,
    Trace,
    Stats,
    Page,
}

impl Kind {
    const ALL: [Self; 15] = [
        Self::Home,
        Self::Chat,
        Self::Projects,
        Self::Settings,
        Self::Environments,
        Self::Work,
        Self::Device,
        Self::Docs,
        Self::Roadmap,
        Self::Promises,
        Self::Download,
        Self::Pricing,
        Self::Trace,
        Self::Stats,
        Self::Page,
    ];

    /// The kind of the page at `path` (a site path, `/chat/...`).
    pub(crate) fn of(path: &str) -> Self {
        let path = path.split(['?', '#']).next().unwrap_or(path);
        match path.trim_start_matches('/').split('/').next().unwrap_or("") {
            "" => Self::Home,
            "chat" => Self::Chat,
            "projects" => Self::Projects,
            "settings" | "account" => Self::Settings,
            "environments" | "studios" => Self::Environments,
            "work" => Self::Work,
            "device" => Self::Device,
            "docs" => Self::Docs,
            "roadmap" => Self::Roadmap,
            "promises" => Self::Promises,
            "download" => Self::Download,
            "pricing" => Self::Pricing,
            "trace" => Self::Trace,
            "stats" | "live" => Self::Stats,
            _ => Self::Page,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Chat => "chat",
            Self::Projects => "projects",
            Self::Settings => "settings",
            Self::Environments => "environments",
            Self::Work => "work",
            Self::Device => "device",
            Self::Docs => "docs",
            Self::Roadmap => "roadmap",
            Self::Promises => "promises",
            Self::Download => "download",
            Self::Pricing => "pricing",
            Self::Trace => "trace",
            Self::Stats => "stats",
            Self::Page => "page",
        }
    }

    /// The picture's large words and the line under them.
    fn drawn(self) -> (&'static str, &'static str) {
        match self {
            Self::Home | Self::Page => (
                "An open network of agents",
                "You work with them through one conversation, from anywhere: this website, your phone, or a terminal.",
            ),
            Self::Chat => (
                "A chat",
                "One conversation with your agents. Log in to open it.",
            ),
            Self::Projects => (
                "Projects",
                "Your repositories and the chats about them. Log in to open them.",
            ),
            Self::Settings => (
                "Settings",
                "Your account, keys and plan. Log in to open them.",
            ),
            Self::Environments => ("Environments", "Set up a repository and run agents in it."),
            Self::Work => (
                "Work",
                "What your agents are working on. Log in to follow it.",
            ),
            Self::Device => ("Connect a device", "Log in to finish connecting it."),
            Self::Docs => ("Docs", "How to use OpenAgents and Coder."),
            Self::Roadmap => (
                "Roadmap",
                "What we're building next, what comes after, and what we set aside.",
            ),
            Self::Promises => (
                "What works today",
                "Everything here works today, and each line links to the check that proves it.",
            ),
            Self::Download => (
                "Download",
                "Coder runs on macOS, Linux, and Windows. One command installs it.",
            ),
            Self::Pricing => (
                "Pricing",
                "Chatting with OpenAgents is free, and so is Coder on your own computer.",
            ),
            Self::Trace => (
                "A shared trace",
                "An agent's work, step by step, shared from OpenAgents.",
            ),
            Self::Stats => (
                "Stats",
                "What OpenAgents has received for plugin calls and paid out to plugin authors.",
            ),
        }
    }
}

/// A page's preview picture: its address and its words for screen readers.
pub(crate) struct Image {
    pub(crate) url: String,
    pub(crate) alt: String,
}

/// The preview picture for the page at `path` (none: the home page's).
pub(crate) fn image(path: Option<&str>) -> Image {
    let site = crate::agent_ready::SITE;
    let path = path.unwrap_or("/");
    if let Some(slug) = path.strip_prefix("/docs/")
        && let Some(title) = doc_title(slug)
    {
        return Image {
            url: format!("{site}{PREFIX}/docs/{slug}.png?v={VERSION}"),
            alt: format!("{title}: OpenAgents docs"),
        };
    }
    let kind = Kind::of(path);
    let (heading, _) = kind.drawn();
    Image {
        url: format!("{site}{PREFIX}/{}.png?v={VERSION}", kind.slug()),
        alt: if heading.starts_with("An open") {
            "OpenAgents".to_owned()
        } else {
            format!("{heading}: OpenAgents")
        },
    }
}

/// What a link to a page only signed-in people may open shows signed out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SignedOut {
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    /// The page's own path, its query left off.
    pub(crate) path: String,
}

/// The preview the sign-in page gives for `return_to` when that page is
/// one only signed-in people open; `None` for the home page and for public
/// pages, whose sign-in links read "Log in" as they are.
pub(crate) fn signed_out(return_to: &str) -> Option<SignedOut> {
    let path = return_to
        .split(['?', '#'])
        .next()
        .unwrap_or(return_to)
        .trim_end_matches('/');
    let (title, description) = match Kind::of(path) {
        Kind::Chat => (
            "A chat on OpenAgents",
            "Log in to OpenAgents to open this chat.",
        ),
        Kind::Projects if path == "/projects" => (
            "Your projects on OpenAgents",
            "Log in to OpenAgents to see your projects and their repositories.",
        ),
        Kind::Projects => (
            "A project on OpenAgents",
            "Log in to OpenAgents to open this project.",
        ),
        Kind::Settings => (
            "Your settings on OpenAgents",
            "Log in to OpenAgents to change your account, keys and plan.",
        ),
        Kind::Environments => (
            "Environments on OpenAgents",
            "Log in to OpenAgents to set up a repository and run agents in it.",
        ),
        Kind::Work => (
            "Work on OpenAgents",
            "Log in to OpenAgents to follow what your agents are working on.",
        ),
        Kind::Device => (
            "Connect a device to OpenAgents",
            "Log in to OpenAgents to finish connecting your device.",
        ),
        Kind::Page if !path.is_empty() => (
            "A page on OpenAgents",
            "Log in to OpenAgents to open this page.",
        ),
        _ => return None,
    };
    Some(SignedOut {
        title,
        description,
        path: path.to_owned(),
    })
}

/// A guide's title by its slug, when there is such a guide.
fn doc_title(slug: &str) -> Option<String> {
    crate::pages::content::DOCS
        .iter()
        .find(|(name, _)| *name == slug)
        .map(|(name, source)| crate::markdown::title(source, name))
}

fn png(bytes: std::sync::Arc<Vec<u8>>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes.as_ref().clone(),
    )
        .into_response()
}

/// `GET /og/{kind}.png`: a kind of page's picture.
async fn kind_image(Path(file): Path<String>) -> Response {
    let Some(kind) = file
        .strip_suffix(".png")
        .and_then(|slug| Kind::ALL.into_iter().find(|kind| kind.slug() == slug))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (heading, line) = kind.drawn();
    let key = format!("og:{}:{VERSION}", kind.slug());
    match card::cached(&key, || card::render_page(None, heading, line).ok()) {
        Some(bytes) => png(bytes),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /og/docs/{slug}.png`: a guide's picture, with its title.
async fn doc_image(Path(file): Path<String>) -> Response {
    let Some((slug, title)) = file
        .strip_suffix(".png")
        .and_then(|slug| Some((slug, doc_title(slug)?)))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let source = crate::pages::content::DOCS
        .iter()
        .find(|(name, _)| *name == slug)
        .map_or("", |(_, source)| *source);
    let line = crate::agent_ready::summary(source);
    let key = format!("og:docs:{slug}:{VERSION}");
    match card::cached(&key, || card::render_page(Some("Docs"), &title, &line).ok()) {
        Some(bytes) => png(bytes),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_get_their_kind_from_their_first_segment() {
        assert_eq!(Kind::of("/"), Kind::Home);
        assert_eq!(
            Kind::of("/chat/3f2b8c1e-1a2b-4c3d-8e9f-0123456789ab"),
            Kind::Chat
        );
        assert_eq!(Kind::of("/settings/api-keys?x=1"), Kind::Settings);
        assert_eq!(Kind::of("/docs/coder"), Kind::Docs);
        assert_eq!(Kind::of("/something-else"), Kind::Page);
        for kind in Kind::ALL {
            assert!(Kind::ALL.iter().filter(|k| k.slug() == kind.slug()).count() == 1);
        }
    }

    #[test]
    fn sign_in_only_pages_are_named_by_kind_and_never_by_their_contents() {
        let chat = signed_out("/chat/3f2b8c1e-1a2b-4c3d-8e9f-0123456789ab?secret=1").unwrap();
        assert_eq!(chat.title, "A chat on OpenAgents");
        assert_eq!(chat.path, "/chat/3f2b8c1e-1a2b-4c3d-8e9f-0123456789ab");
        assert_eq!(
            signed_out("/projects").unwrap().title,
            "Your projects on OpenAgents"
        );
        assert_eq!(
            signed_out("/settings/memory").unwrap().title,
            "Your settings on OpenAgents"
        );
        assert_eq!(
            signed_out("/environments").unwrap().title,
            "Environments on OpenAgents"
        );
        // Public pages and the home page keep the plain sign-in preview.
        for public in [
            "/",
            "",
            "/docs/coder",
            "/roadmap",
            "/download",
            "/trace/abc",
        ] {
            assert_eq!(signed_out(public), None, "{public}");
        }
    }

    #[test]
    fn every_page_names_a_picture_that_draws() {
        let docs = image(Some("/docs/coder"));
        assert!(docs.url.ends_with("/og/docs/coder.png?v=1"), "{}", docs.url);
        assert!(
            image(Some("/docs/no-such-guide"))
                .url
                .ends_with("/og/docs.png?v=1")
        );
        assert!(image(None).url.ends_with("/og/home.png?v=1"));
        assert!(image(Some("/chat/x")).url.ends_with("/og/chat.png?v=1"));
        for kind in Kind::ALL {
            let (heading, line) = kind.drawn();
            let png = card::render_page(None, heading, line).unwrap();
            let picture = image::load_from_memory(&png).unwrap();
            assert_eq!(
                (picture.width(), picture.height()),
                (card::WIDTH, card::HEIGHT)
            );
        }
        if let Ok(dir) = std::env::var("OA_CARD_OUT") {
            for kind in [Kind::Home, Kind::Chat, Kind::Settings] {
                let (heading, line) = kind.drawn();
                std::fs::write(
                    format!("{dir}/og-{}.png", kind.slug()),
                    card::render_page(None, heading, line).unwrap(),
                )
                .unwrap();
            }
            let source = crate::pages::content::DOCS[0].1;
            std::fs::write(
                format!("{dir}/og-doc.png"),
                card::render_page(
                    Some("Docs"),
                    &crate::markdown::title(source, crate::pages::content::DOCS[0].0),
                    &crate::agent_ready::summary(source),
                )
                .unwrap(),
            )
            .unwrap();
        }
    }
}
