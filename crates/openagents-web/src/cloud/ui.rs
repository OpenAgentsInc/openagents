//! Shared parts of the Cloud pages in the Coder Light / Coder Noir design
//! language (UI-10). Every Cloud page builds its content with these and the
//! `openagents_ui` components, then answers through [`document`].
//!
//! - [`document`]: the whole Cloud page through [`crate::ui_page::UiPage`],
//!   with the `.cloud` root, the Cloud and Rust Native stylesheets, HTMX, and
//!   `cloud-start.js`, under the Cloud [`super::protect`] headers.
//! - [`status`]: a status [`Badge`] with a [`Tone`].
//! - [`BoundForm`]: a POST form bound to its CSRF ticket and the hidden
//!   request id, epoch, or revision fields the owner checks; [`retry`] is
//!   the "Retry the same request" form.
//! - [`outcome_unknown`]: the "Outcome unknown" alert, with an optional
//!   retry form that recovers the original request.
//! - [`Details`]: a key/value list (`<dl>`).
//! - [`table`]: a labelled [`Table`].
//! - [`empty`], [`denied`], [`unavailable`]: the empty, refused, and
//!   unavailable states.
//! - [`native`]: a validated Rust Native view (already HTML) in a nested
//!   Coder Noir panel, since those views carry the Coder Noir palette.
//! - [`card`], [`section`], [`links`], [`csrf`], [`hidden`]: small layout
//!   and form pieces.
//!
//! Every function escapes the text it is given; pass [`Markup`] (or
//! [`maud::PreEscaped`] for HTML that is already escaped) for rich content.
//! Nothing here emits an inline `style` attribute or an inline script.

use axum::http::HeaderMap;
use axum::response::Response;
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{
    Alert, Badge, Button, ButtonType, ButtonVariant, Color, ControlSize, EmptyMessage, Variant,
};
pub(crate) use openagents_ui::content::Table;

use crate::ui_page::UiPage;

/// The Cloud page: `body` inside the `.cloud` root that `cloud-start.js`
/// and the Wasm privacy guard look for, with the Cloud headers (`no-store`,
/// the Cloud CSP, `Vary: Cookie`). The theme comes from the request's cookie,
/// else the system setting.
pub(crate) fn document(headers: &HeaderMap, body: impl Render) -> Response {
    let page = UiPage::new("Workspace")
        .path("/cloud/app")
        .head(html! {
            (crate::chat_html::head())
            link rel="stylesheet" href="/cloud/assets/cloud.css";
            link rel="stylesheet" href="/cloud/assets/native.css";
        })
        .content(html! {
            div class="cloud" { (body) }
            script type="module" src="/cloud/assets/start.js" {}
        });
    super::protect(page.respond(headers))
}

/// The color of a [`status`] badge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Recorded, inactive, or not yet checked.
    #[default]
    Neutral,
    /// Running, pending, or in review.
    Info,
    /// Succeeded or admitted.
    Success,
    /// Needs attention: unknown, stale, or waiting for input.
    Warning,
    /// Failed or refused.
    Danger,
}

impl Tone {
    fn color(self) -> Color {
        match self {
            Self::Neutral => Color::Secondary,
            Self::Info => Color::Info,
            Self::Success => Color::Success,
            Self::Warning => Color::Warning,
            Self::Danger => Color::Danger,
        }
    }
}

/// A soft status badge, such as `running` or `Outcome unknown`.
pub(crate) fn status(label: &str, tone: Tone) -> Markup {
    Badge::new(label)
        .color(tone.color())
        .variant(Variant::Soft)
        .render()
}

/// The hidden CSRF ticket every Cloud form carries (`name="csrf"`).
pub(crate) fn csrf(token: &str) -> Markup {
    hidden("csrf", token)
}

/// One hidden form field.
pub(crate) fn hidden(name: &str, value: &str) -> Markup {
    html! { input type="hidden" name=(name) value=(value); }
}

/// A submit button: solid primary, or soft secondary when not `primary`.
pub(crate) fn submit(label: &str, primary: bool) -> Button {
    let button = Button::new(label)
        .kind(ButtonType::Submit)
        .size(ControlSize::Sm);
    if primary {
        button
    } else {
        button.variant(ButtonVariant::Soft).color(Color::Secondary)
    }
}

/// A POST form bound to its CSRF ticket and the exact hidden fields the
/// owner checks (request id, members epoch, record revision). The hidden
/// fields render in the order given, before the body and the submit row.
///
/// ```ignore
/// BoundForm::new("/cloud/app/hosts/a/confirm")
///     .csrf(&token)
///     .bind("request", &request)
///     .bind("revision", &revision.to_string())
///     .submit("Confirm this exact request")
/// ```
#[must_use]
pub(crate) struct BoundForm {
    action: Option<String>,
    csrf: Option<String>,
    bound: Vec<(String, String)>,
    body: Option<Markup>,
    submit: Option<Markup>,
    id: Option<String>,
    label: Option<String>,
}

impl BoundForm {
    /// A form that posts to `action`.
    pub(crate) fn new(action: impl Into<String>) -> Self {
        Self {
            action: Some(action.into()),
            csrf: None,
            bound: Vec::new(),
            body: None,
            submit: None,
            id: None,
            label: None,
        }
    }

    /// A form that posts back to the current URL (no `action` attribute).
    pub(crate) fn here() -> Self {
        Self {
            action: None,
            ..Self::new("")
        }
    }

    /// The CSRF ticket.
    pub(crate) fn csrf(mut self, token: &str) -> Self {
        self.csrf = Some(token.to_owned());
        self
    }

    /// A hidden field the owner checks: request id, epoch, revision.
    pub(crate) fn bind(mut self, name: &str, value: &str) -> Self {
        self.bound.push((name.to_owned(), value.to_owned()));
        self
    }

    /// The visible fields and text between the hidden fields and the submit.
    pub(crate) fn body(mut self, body: impl Render) -> Self {
        self.body = Some(body.render());
        self
    }

    /// A primary submit button labelled `label`.
    pub(crate) fn submit(self, label: &str) -> Self {
        self.submit_with(submit(label, true))
    }

    /// Any submit control: a [`Button`], or a Rust Native submit view.
    pub(crate) fn submit_with(mut self, control: impl Render) -> Self {
        self.submit = Some(control.render());
        self
    }

    /// The form's `id`.
    pub(crate) fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_owned());
        self
    }

    /// The form's accessible name (`aria-label`).
    pub(crate) fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_owned());
        self
    }
}

impl Render for BoundForm {
    fn render(&self) -> Markup {
        html! {
            form class="cloud-form" method="post" action=[self.action.as_deref()]
                id=[self.id.as_deref()] aria-label=[self.label.as_deref()] {
                @if let Some(token) = &self.csrf { (csrf(token)) }
                @for (name, value) in &self.bound { (hidden(name, value)) }
                @if let Some(body) = &self.body { (body) }
                @if let Some(submit) = &self.submit {
                    div class="cloud-form-actions" { (submit) }
                }
            }
        }
    }
}

/// The "Retry the same request" form: it posts the original request again
/// so the owner recovers it, and never creates a new one.
pub(crate) fn retry(action: impl Into<String>, token: &str) -> BoundForm {
    BoundForm::new(action)
        .csrf(token)
        .submit_with(submit("Retry the same request", true))
}

/// The "Outcome unknown" alert: `description` says what was asked and
/// where it can be recovered; `retry` (usually [`retry`]) recovers it.
pub(crate) fn outcome_unknown(description: impl Render, retry: Option<BoundForm>) -> Markup {
    let mut alert = Alert::new()
        .color(Color::Warning)
        .variant(Variant::Soft)
        .title("Outcome unknown")
        .description_markup(description)
        .attr("role", "status");
    if let Some(retry) = retry {
        alert = alert.actions(retry);
    }
    alert.render()
}

/// A key/value list: `<dl class="cloud-details">`.
#[derive(Default)]
#[must_use]
pub(crate) struct Details {
    rows: Vec<(String, Markup)>,
}

impl Details {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// One `<dt>`/`<dd>` pair.
    pub(crate) fn row(mut self, label: &str, value: impl Render) -> Self {
        self.rows.push((label.to_owned(), value.render()));
        self
    }
}

impl Render for Details {
    fn render(&self) -> Markup {
        html! {
            dl class="cloud-details" {
                @for (label, value) in &self.rows {
                    dt { (label) }
                    dd { (value) }
                }
            }
        }
    }
}

/// A data table whose scroll region is named `label`.
pub(crate) fn table(label: &str) -> Table {
    Table::new().label(label)
}

/// Nothing to show yet: an outlined empty message.
pub(crate) fn empty(title: &str, description: impl Into<String>) -> Markup {
    EmptyMessage::new()
        .title(title)
        .description(description)
        .class("cloud-empty")
        .render()
}

/// The owner refused this view or action (`role="alert"`).
pub(crate) fn denied(title: &str, description: impl Render) -> Markup {
    Alert::new()
        .color(Color::Danger)
        .variant(Variant::Soft)
        .title(title)
        .description_markup(description)
        .render()
}

/// This view or action is not available here (`role="status"`).
pub(crate) fn unavailable(title: &str, description: impl Render) -> Markup {
    Alert::new()
        .color(Color::Secondary)
        .variant(Variant::Outline)
        .title(title)
        .description_markup(description)
        .attr("role", "status")
        .render()
}

/// A Rust Native view rendered by `rust_native_web::render_view` (validated,
/// already-escaped HTML), in a nested dark panel: those views use the
/// Coder Noir palette in both themes.
pub(crate) fn native(view: &str) -> Markup {
    html! { div class="cloud-native" data-theme="dark" { (PreEscaped(view)) } }
}

/// A bordered card: `<section class="cloud-card">`.
pub(crate) fn card(body: impl Render) -> Markup {
    html! { section class="cloud-card" { (body.render()) } }
}

/// A titled section: `<section aria-labelledby=id><h3 id=id>title</h3>…`.
pub(crate) fn section(id: &str, title: impl Render, body: impl Render) -> Markup {
    html! {
        section aria-labelledby=(id) {
            h3 id=(id) { (title.render()) }
            (body.render())
        }
    }
}

/// A row of links separated by ` · `: `<p class="cloud-links">`.
pub(crate) fn links<'a>(items: impl IntoIterator<Item = (&'a str, &'a str)>) -> Markup {
    html! {
        p class="cloud-links" {
            @for (index, (href, label)) in items.into_iter().enumerate() {
                @if index > 0 { " \u{b7} " }
                a href=(href) { (label) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_form_carries_ticket_and_exact_fields_in_order() {
        let html = BoundForm::new("/x/confirm")
            .csrf("t<1>")
            .bind("request", "r-1")
            .bind("revision", "7")
            .body(html! { p { "Exact" } })
            .submit("Confirm")
            .render()
            .into_string();
        assert!(html.starts_with(
            "<form class=\"cloud-form\" method=\"post\" action=\"/x/confirm\"><input type=\"hidden\" name=\"csrf\" value=\"t&lt;1&gt;\"><input type=\"hidden\" name=\"request\" value=\"r-1\"><input type=\"hidden\" name=\"revision\" value=\"7\"><p>Exact</p>"
        ), "{html}");
        assert!(html.contains("type=\"submit\""), "{html}");
        assert!(html.contains(">Confirm</span></button></div></form>"), "{html}");
        let here = BoundForm::here().render().into_string();
        assert!(!here.contains("action="), "{here}");
    }

    #[test]
    fn outcome_unknown_offers_the_same_request_again() {
        let html = outcome_unknown(
            "request r-1",
            Some(retry("/x", "t").bind("request", "r-1")),
        )
        .into_string();
        for needle in [
            "Outcome unknown",
            "request r-1",
            "Retry the same request",
            "name=\"request\" value=\"r-1\"",
            "role=\"status\"",
        ] {
            assert!(html.contains(needle), "{needle}: {html}");
        }
    }

    #[test]
    fn details_states_and_links_escape_text() {
        let html = Details::new()
            .row("Owner", "<b>")
            .render()
            .into_string();
        assert_eq!(
            html,
            "<dl class=\"cloud-details\"><dt>Owner</dt><dd>&lt;b&gt;</dd></dl>"
        );
        assert!(denied("No", "x").into_string().contains("role=\"alert\""));
        assert!(unavailable("Off", "<i>").into_string().contains("&lt;i&gt;"));
        assert!(empty("None", "yet").into_string().contains("oa-empty-message"));
        assert_eq!(
            links([("/a", "A"), ("/b", "B")]).into_string(),
            "<p class=\"cloud-links\"><a href=\"/a\">A</a> \u{b7} <a href=\"/b\">B</a></p>"
        );
        assert!(status("failed", Tone::Danger).into_string().contains("data-color=\"danger\""));
    }
}
