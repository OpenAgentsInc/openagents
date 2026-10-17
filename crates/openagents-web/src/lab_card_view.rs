//! One web renderer for the shared Lab card views.

use crate::App;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use lab_kit::{Card, View};
use maud::{Markup, html};
use serde::Deserialize;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/labs/{kind}/{document}", get(document))
        .route("/labs/files/{id}/play", get(play))
        .route("/chat/{id}/labs/{message}", get(saved))
}

#[derive(Deserialize)]
struct Page {
    #[serde(default = "personal")]
    space: String,
    v: Option<u64>,
}
fn personal() -> String {
    "me".into()
}

async fn document(
    State(app): State<App>,
    headers: HeaderMap,
    Path((kind, document)): Path<(String, String)>,
    Query(page): Query<Page>,
) -> Response {
    let authority = crate::labs_api::caller_headers(&app, &headers);
    let api = crate::labs_api::runtime(&app);
    let card = match page.v {
        Some(version) => {
            api.read_card(&authority, &page.space, &document, version)
                .await
        }
        None => api.latest_card(&authority, &page.space, &document).await,
    };
    let card = match card {
        Ok(card) if card.kind == kind => card,
        Ok(_) => return lab_api::ApiError::missing().into_response(),
        Err(error) => return error.into_response(),
    };
    crate::chat_html::protect(
        crate::ui_page::UiPage::new(&card.title)
            .path(format!("/labs/{}/{}", encoded(&kind), encoded(&document)))
            .head(crate::chat_html::head())
            .content(openagents_ui::content::PageColumn::new(markup(&card, &page.space)).wide())
            .respond(&headers),
    )
}

async fn saved(
    State(app): State<App>,
    headers: HeaderMap,
    Path((chat, message)): Path<(String, usize)>,
) -> Response {
    let cards = match crate::history_api::cards_for_message(&app, &headers, &chat, message).await {
        Ok(cards) => cards,
        Err(response) => return response,
    };
    let content = html! {
        @for value in cards {
            @let space = value["space"].as_str().unwrap_or("me");
            @if let Ok(card) = serde_json::from_value::<Card>(value.clone()) { (markup(&card, space)) }
        }
    };
    crate::chat_html::protect(Html(content.into_string()).into_response())
}

async fn play(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> Response {
    let authority = crate::labs_api::caller_headers(&app, &headers);
    let (file, bytes) = match crate::labs_api::runtime(&app)
        .read_artifact(&authority, &page.space, &id, 1024 * 1024)
        .await
    {
        Ok(file) => file,
        Err(error) => return error.into_response(),
    };
    if file.media_type != "text/html" {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; media-src data: blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

pub(crate) fn thread_entry(
    chat: &str,
    message: usize,
    meta: Option<&openagents_chat::router::Meta>,
) -> Markup {
    if !meta.is_some_and(|meta| {
        meta.cards
            .iter()
            .any(|card| card["schema"] == lab_kit::card::SCHEMA)
    }) {
        return html! {};
    }
    html! { div hx-get=(format!("/chat/{chat}/labs/{message}")) hx-trigger="load" hx-swap="innerHTML" {
        p role="status" { "Loading document…" }
    } }
}

fn encoded(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

pub(crate) fn artifact(space: &str, id: &str) -> String {
    format!(
        "/v1/labs/artifacts/{}/content?space={}",
        encoded(id),
        encoded(space)
    )
}

pub(crate) fn markup(card: &Card, space: &str) -> Markup {
    let Ok(projection) = lab_ui::card(card, space, "lab-web-card", 1) else {
        return html! { p { "This document can't be displayed." } };
    };
    html! {
        section.oa-lab-card data-lab-kind=(card.kind) data-lab-document=(card.document) data-lab-version=(card.version) {
            (node(&projection.view.view().root, &projection))
        }
    }
}

// This adapter only draws the primitives emitted by lab-ui. Card composition
// and control availability belong to that shared Rust Native view.
fn node(value: &rust_native::Node<lab_ui::Intent>, projection: &lab_ui::Projection) -> Markup {
    use rust_native::{Element, TextRole};
    match &value.element {
        Element::Stack { children, .. } => html! { div data-rn-node=(value.key) {
            @for child in children { (node(child, projection)) }
        } },
        Element::Text {
            value,
            role: TextRole::Heading,
        } => html! { p { strong { (value) } } },
        Element::Text { value, .. } => html! { p { (value) } },
        Element::Surface { resource, .. } => match projection.media.get(resource) {
            Some(resource) => media(&resource.card, &resource.space),
            None => html! {},
        },
        Element::Button {
            label,
            enabled: true,
            intent:
                lab_ui::Intent::Open {
                    space,
                    kind,
                    document,
                    version,
                },
            ..
        } => {
            let href = format!(
                "/labs/{}/{}?v={version}&space={}",
                encoded(kind),
                encoded(document),
                encoded(space)
            );
            html! { p { a href=(href) data-rn-node=(value.key) { (label) } } }
        }
        _ => html! {},
    }
}

fn media(card: &Card, space: &str) -> Markup {
    let primary = card.primary.as_deref().map(|id| artifact(space, id));
    let poster = card.poster.as_deref().map(|id| artifact(space, id));
    html! {
        @match card.view.drawn() {
            View::Video => {
                @if let Some(primary) = &primary {
                    video controls preload="metadata" src=(primary) poster=[poster.as_deref()] aria-label=(card.title) {}
                } @else if let Some(poster) = &poster {
                    img src=(poster) alt=(card.title);
                }
            },
            View::Image => {
                @if let Some(image) = primary.as_deref().or(poster.as_deref()) {
                    img src=(image) alt=(card.title);
                }
            },
            View::Compare => {
                @if let Some(compare) = &card.compare {
                    div.oa-lab-compare {
                        @for (label, id) in [("Before", &compare.a_primary), ("After", &compare.b_primary)] {
                            @if let Some(id) = id {
                                figure { img src=(artifact(space, id)) alt={ (card.title) ": " (label) }; figcaption { (label) } }
                            }
                        }
                    }
                }
            },
            View::Model3d => {
                @if let Some(primary) = &primary {
                    link rel="stylesheet" href="/chat/cad/card.css";
                    script src="/chat/cad/card.js" defer {}
                    (crate::cad_lab::viewer(primary, poster.as_deref(), &card.title, &card.document, card.version))
                } @else if let Some(poster) = &poster {
                    img src=(poster) alt=(card.title);
                }
            },
            View::Playable => {
                @if let Some(primary) = &card.primary {
                    iframe src=(format!("/labs/files/{}/play?space={}", encoded(primary), encoded(space))) title=(card.title) sandbox="allow-scripts" loading="lazy" {}
                }
            },
            View::Report | View::None | View::Unknown => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_views_show_a_report_and_media_use_credential_checked_artifacts() {
        let mut card: Card = serde_json::from_value(json!({"kind":"future","document":"example","version":1,
            "title":"Example","summary":"A saved result","view":"future_view","primary":"file?escape=1",
            "page":"//outside.example"})).unwrap();
        let report = markup(&card, "project:shared").into_string();
        assert!(report.contains("A saved result"));
        assert!(!report.contains("outside.example"));
        assert!(!report.contains("<iframe"));
        assert!(!report.contains("/artifacts/"));
        card.view = View::Playable;
        let playable = markup(&card, "project:shared").into_string();
        assert!(playable.contains("sandbox=\"allow-scripts\""));
        assert!(playable.contains("/labs/files/file%3Fescape%3D1/play?space=project%3Ashared"));
        assert!(!playable.contains("allow-same-origin"));
        card.view = View::Model3d;
        let model = markup(&card, "me").into_string();
        assert!(model.contains("data-cad-model="));
        assert!(model.contains("/chat/cad/card.js"));
    }
}
