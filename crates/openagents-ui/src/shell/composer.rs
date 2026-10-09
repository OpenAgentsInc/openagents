//! The composer: one form used by the home page, chat and the Cloud/Coder
//! work views.
//!
//! Without JavaScript it is a plain `<form method="post">`; the server answers
//! a failed submit by re-rendering the page with [`Composer::draft`] set so
//! nothing typed is lost. With HTMX it posts with `hx-swap="none"`, so a
//! failed request never touches the text box: the draft stays until the
//! server answers success (usually `HX-Redirect`). A failure message goes in
//! the status region ([`Composer::status_id`]) through an out-of-band swap.

use maud::{Markup, Render, html};

use super::glyph;

/// An HTMX `GET` that loads a panel, as the composer's selectors and pickers
/// do (`hx-get`, `hx-target`, `hx-include`, `hx-swap`, `hx-sync`).
#[derive(Clone, Debug)]
pub struct HxGet {
    pub(super) url: String,
    pub(super) target: Option<String>,
    pub(super) include: Option<String>,
    pub(super) swap: Option<String>,
    pub(super) sync: Option<String>,
}

impl HxGet {
    /// `hx-get=url`.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            target: None,
            include: None,
            swap: None,
            sync: None,
        }
    }

    /// `hx-target`.
    #[must_use]
    pub fn target(mut self, selector: impl Into<String>) -> Self {
        self.target = Some(selector.into());
        self
    }

    /// `hx-include`.
    #[must_use]
    pub fn include(mut self, selector: impl Into<String>) -> Self {
        self.include = Some(selector.into());
        self
    }

    /// `hx-swap`.
    #[must_use]
    pub fn swap(mut self, swap: impl Into<String>) -> Self {
        self.swap = Some(swap.into());
        self
    }

    /// `hx-sync`.
    #[must_use]
    pub fn sync(mut self, sync: impl Into<String>) -> Self {
        self.sync = Some(sync.into());
        self
    }
}

/// A trigger button with an optional HTMX request or popover target.
fn trigger(
    class: &str,
    aria_label: Option<&str>,
    title: Option<&str>,
    hx: Option<&HxGet>,
    popover: Option<&str>,
    inner: Markup,
) -> Markup {
    html! {
        button type="button" class=(class) aria-label=[aria_label] title=[title]
            aria-haspopup=[(hx.is_some() || popover.is_some()).then_some("dialog")]
            popovertarget=[popover]
            hx-get=[hx.map(|hx| hx.url.as_str())]
            hx-target=[hx.and_then(|hx| hx.target.as_deref())]
            hx-include=[hx.and_then(|hx| hx.include.as_deref())]
            hx-swap=[hx.and_then(|hx| hx.swap.as_deref())]
            hx-sync=[hx.and_then(|hx| hx.sync.as_deref())] {
            (inner)
        }
    }
}

/// A composer dropdown label: a quiet button reading `label: value` with a
/// chevron. The repository, branch and environment selectors use it.
#[derive(Clone, Debug)]
pub struct ComposerDropdown {
    label: String,
    value: String,
    icon: Option<Markup>,
    hx: Option<HxGet>,
    popover: Option<String>,
    show_label: bool,
}

impl ComposerDropdown {
    /// A dropdown whose accessible name is `label` and which shows `value`.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            icon: None,
            hx: None,
            popover: None,
            show_label: false,
        }
    }

    /// A leading icon.
    #[must_use]
    pub fn icon(mut self, icon: impl Render) -> Self {
        self.icon = Some(icon.render());
        self
    }

    /// Shows the label before the value as well as naming the button.
    #[must_use]
    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// Loads the choices with HTMX.
    #[must_use]
    pub fn hx(mut self, hx: HxGet) -> Self {
        self.hx = Some(hx);
        self
    }

    /// Opens a native popover by id instead.
    #[must_use]
    pub fn popover(mut self, id: impl Into<String>) -> Self {
        self.popover = Some(id.into());
        self
    }
}

impl Render for ComposerDropdown {
    fn render(&self) -> Markup {
        let name = format!("{}: {}", self.label, self.value);
        let inner = html! {
            @if let Some(icon) = &self.icon {
                span class="oa-composer-dropdown-icon" aria-hidden="true" { (icon) }
            }
            span class="oa-composer-dropdown-text" {
                @if self.show_label {
                    span class="oa-composer-dropdown-label" { (self.label) }
                }
                span class="oa-composer-dropdown-value" { (self.value) }
            }
            span class="oa-composer-dropdown-chevron" aria-hidden="true" { (glyph::chevron_down()) }
        };
        trigger(
            "oa-composer-dropdown",
            Some(&name),
            Some(&name),
            self.hx.as_ref(),
            self.popover.as_deref(),
            inner,
        )
    }
}

/// The model (or runtime) picker trigger in the composer footer: the model
/// name, an optional effort or mode, and a chevron.
#[derive(Clone, Debug)]
pub struct ModelPickerTrigger {
    model: String,
    effort: Option<String>,
    hx: Option<HxGet>,
    popover: Option<String>,
}

impl ModelPickerTrigger {
    /// A trigger showing `model` ("Auto", "GPT-5", a runtime profile).
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            effort: None,
            hx: None,
            popover: None,
        }
    }

    /// A secondary value after the model, such as the reasoning effort.
    #[must_use]
    pub fn effort(mut self, effort: impl Into<String>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    /// Loads the picker with HTMX.
    #[must_use]
    pub fn hx(mut self, hx: HxGet) -> Self {
        self.hx = Some(hx);
        self
    }

    /// Opens a native popover by id instead.
    #[must_use]
    pub fn popover(mut self, id: impl Into<String>) -> Self {
        self.popover = Some(id.into());
        self
    }
}

impl Render for ModelPickerTrigger {
    fn render(&self) -> Markup {
        let name = match &self.effort {
            Some(effort) => format!("Model: {}, {}", self.model, effort),
            None => format!("Model: {}", self.model),
        };
        let inner = html! {
            span class="oa-model-picker-content" {
                span class="oa-model-picker-label" { (self.model) }
                @if let Some(effort) = &self.effort {
                    span class="oa-model-picker-effort" { (effort) }
                }
            }
            span class="oa-composer-dropdown-chevron" aria-hidden="true" { (glyph::chevron_down()) }
        };
        trigger(
            "oa-model-picker-trigger",
            Some(&name),
            Some(&name),
            self.hx.as_ref(),
            self.popover.as_deref(),
            inner,
        )
    }
}

/// The composer form.
#[derive(Clone, Debug)]
pub struct Composer {
    id: String,
    action: String,
    label: String,
    enhanced: bool,
    hx_include: Option<String>,
    input_id: Option<String>,
    body_id: Option<String>,
    name: String,
    placeholder: String,
    input_label: String,
    draft: String,
    max_chars: Option<usize>,
    rows: u8,
    autofocus: bool,
    disabled: bool,
    hidden: Vec<Markup>,
    selectors: Option<Markup>,
    attachments: Option<Markup>,
    leading: Vec<Markup>,
    dropdowns: Vec<ComposerDropdown>,
    model_picker: Option<ModelPickerTrigger>,
    trailing: Vec<Markup>,
    send: Option<Markup>,
    send_label: String,
    status: Option<Markup>,
    after: Option<Markup>,
}

impl Composer {
    /// A composer whose `<form id>` is `id` and which posts to `action`.
    #[must_use]
    pub fn new(id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            action: action.into(),
            label: "Composer".to_owned(),
            enhanced: true,
            hx_include: None,
            input_id: None,
            body_id: None,
            name: "q".to_owned(),
            placeholder: String::new(),
            input_label: "Message".to_owned(),
            draft: String::new(),
            max_chars: None,
            rows: 1,
            autofocus: false,
            disabled: false,
            hidden: Vec::new(),
            selectors: None,
            attachments: None,
            leading: Vec::new(),
            dropdowns: Vec::new(),
            model_picker: None,
            trailing: Vec::new(),
            send: None,
            send_label: "Send".to_owned(),
            status: None,
            after: None,
        }
    }

    /// The accessible name of the composer region ("Start a chat").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Whether to add the HTMX attributes (`hx-post`, `hx-swap="none"`,
    /// `hx-disabled-elt` naming this form's submit button, `hx-sync`). On by
    /// default; the plain form works
    /// either way.
    #[must_use]
    pub fn enhanced(mut self, enhanced: bool) -> Self {
        self.enhanced = enhanced;
        self
    }

    /// `hx-include` for fields that live outside the form.
    #[must_use]
    pub fn hx_include(mut self, selector: impl Into<String>) -> Self {
        self.hx_include = Some(selector.into());
        self
    }

    /// The text box id, `{id}-input` by default.
    #[must_use]
    pub fn input_id(mut self, id: impl Into<String>) -> Self {
        self.input_id = Some(id.into());
        self
    }

    /// The composer body (card) id, `{id}-body` by default.
    #[must_use]
    pub fn body_id(mut self, id: impl Into<String>) -> Self {
        self.body_id = Some(id.into());
        self
    }

    /// The text box's field name, `q` by default.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// The placeholder.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The text box's visually hidden label, "Message" by default.
    #[must_use]
    pub fn input_label(mut self, label: impl Into<String>) -> Self {
        self.input_label = label.into();
        self
    }

    /// Text to put back in the box, for a plain-form submit that failed.
    #[must_use]
    pub fn draft(mut self, draft: impl Into<String>) -> Self {
        self.draft = draft.into();
        self
    }

    /// `maxlength` on the text box.
    #[must_use]
    pub fn max_chars(mut self, max: usize) -> Self {
        self.max_chars = Some(max);
        self
    }

    /// Initial text box rows.
    #[must_use]
    pub fn rows(mut self, rows: u8) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Focuses the text box on load.
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Disables the text box and the send button (for example while a turn
    /// runs).
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Hidden fields inside the form (CSRF, request id, selection token).
    #[must_use]
    pub fn hidden(mut self, fields: impl Render) -> Self {
        self.hidden.push(fields.render());
        self
    }

    /// The selector row above the body: repository, branch and environment
    /// [`ComposerDropdown`]s, or the caller's own row (for example one that
    /// is replaced out of band).
    #[must_use]
    pub fn selectors(mut self, selectors: impl Render) -> Self {
        self.selectors = Some(selectors.render());
        self
    }

    /// The attachments row inside the body, above the text box.
    #[must_use]
    pub fn attachments(mut self, attachments: impl Render) -> Self {
        self.attachments = Some(attachments.render());
        self
    }

    /// A leading footer control (the "add context" button).
    #[must_use]
    pub fn leading(mut self, control: impl Render) -> Self {
        self.leading.push(control.render());
        self
    }

    /// A dropdown label in the footer.
    #[must_use]
    pub fn dropdown(mut self, dropdown: ComposerDropdown) -> Self {
        self.dropdowns.push(dropdown);
        self
    }

    /// The model or runtime picker trigger in the footer.
    #[must_use]
    pub fn model_picker(mut self, trigger: ModelPickerTrigger) -> Self {
        self.model_picker = Some(trigger);
        self
    }

    /// A trailing footer control before the send button (voice input).
    #[must_use]
    pub fn trailing(mut self, control: impl Render) -> Self {
        self.trailing.push(control.render());
        self
    }

    /// Replaces the send button. It must stay `type="submit"`.
    #[must_use]
    pub fn send(mut self, button: impl Render) -> Self {
        self.send = Some(button.render());
        self
    }

    /// The send button's accessible name, "Send" by default.
    #[must_use]
    pub fn send_label(mut self, label: impl Into<String>) -> Self {
        self.send_label = label.into();
        self
    }

    /// Initial content of the status region (a failure message after a
    /// plain-form submit).
    #[must_use]
    pub fn status(mut self, status: impl Render) -> Self {
        self.status = Some(status.render());
        self
    }

    /// Content after the form inside the composer region (a panel the
    /// selectors load into).
    #[must_use]
    pub fn after(mut self, after: impl Render) -> Self {
        self.after = Some(after.render());
        self
    }

    /// The text box id.
    #[must_use]
    pub fn input_dom_id(&self) -> String {
        self.input_id
            .clone()
            .unwrap_or_else(|| format!("{}-input", self.id))
    }

    /// The status region id, `{id}-status`, for out-of-band failure messages.
    #[must_use]
    pub fn status_id(&self) -> String {
        format!("{}-status", self.id)
    }
}

impl Render for Composer {
    fn render(&self) -> Markup {
        let input_id = self.input_dom_id();
        let body_id = self
            .body_id
            .clone()
            .unwrap_or_else(|| format!("{}-body", self.id));
        let enhanced = self.enhanced;
        // An absolute selector, not `find ...`: HTMX inherits
        // `hx-disabled-elt` into the selector and picker buttons inside the
        // form, where a relative `find` would resolve against the button.
        let disabled_elt = format!("#{} button[type=submit]", self.id);
        html! {
            section class="oa-composer" aria-label=(self.label) {
                form id=(self.id) class="oa-composer-root" action=(self.action) method="post"
                    data-oa-composer=""
                    hx-post=[enhanced.then_some(self.action.as_str())]
                    hx-swap=[enhanced.then_some("none")]
                    hx-disabled-elt=[enhanced.then_some(disabled_elt.as_str())]
                    hx-sync=[enhanced.then_some("this:drop")]
                    hx-include=[enhanced.then_some(self.hx_include.as_deref()).flatten()] {
                    @for fields in &self.hidden { (fields) }
                    @if let Some(selectors) = &self.selectors {
                        div class="oa-composer-selectors" { (selectors) }
                    }
                    div id=(body_id) class="oa-composer-body" data-composer-body="" {
                        @if let Some(attachments) = &self.attachments {
                            div class="oa-composer-attachments" { (attachments) }
                        }
                        div class="oa-composer-input" {
                            label class="oa-visually-hidden" for=(input_id) { (self.input_label) }
                            textarea id=(input_id) class="oa-rich-text-input" name=(self.name)
                                rows=(self.rows) maxlength=[self.max_chars]
                                placeholder=[(!self.placeholder.is_empty()).then_some(self.placeholder.as_str())]
                                required autofocus[self.autofocus] disabled[self.disabled]
                                aria-describedby=(self.status_id()) {
                                (self.draft)
                            }
                        }
                        div class="oa-composer-footer" {
                            div class="oa-composer-footer-start" {
                                @for control in &self.leading { (control) }
                                @for dropdown in &self.dropdowns { (dropdown) }
                                @if let Some(picker) = &self.model_picker { (picker) }
                            }
                            div class="oa-composer-footer-end" {
                                @for control in &self.trailing { (control) }
                                @if let Some(send) = &self.send {
                                    (send)
                                } @else {
                                    button type="submit" class="oa-composer-send"
                                        aria-label=(self.send_label) title=(self.send_label)
                                        disabled[self.disabled] {
                                        (glyph::arrow_up())
                                    }
                                }
                            }
                        }
                    }
                    div id=(self.status_id()) class="oa-composer-status" role="status"
                        aria-live="polite" {
                        @if let Some(status) = &self.status { (status) }
                    }
                }
                @if let Some(after) = &self.after { (after) }
            }
        }
    }
}

/// A round icon button in the composer footer: "add context" before the
/// dropdowns, voice input before the send button. It never submits the form.
#[derive(Clone, Debug)]
pub struct ComposerAction {
    icon: Markup,
    label: String,
    title: Option<String>,
    hx: Option<HxGet>,
    popover: Option<String>,
}

impl ComposerAction {
    /// A button showing `icon` whose accessible name is `label`.
    #[must_use]
    pub fn new(icon: impl Render, label: impl Into<String>) -> Self {
        Self {
            icon: icon.render(),
            label: label.into(),
            title: None,
            hx: None,
            popover: None,
        }
    }

    /// A tooltip (`title`), when it says more than the label.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Loads a panel with HTMX.
    #[must_use]
    pub fn hx(mut self, hx: HxGet) -> Self {
        self.hx = Some(hx);
        self
    }

    /// Opens a native popover by id instead.
    #[must_use]
    pub fn popover(mut self, id: impl Into<String>) -> Self {
        self.popover = Some(id.into());
        self
    }
}

impl Render for ComposerAction {
    fn render(&self) -> Markup {
        trigger(
            "oa-composer-action",
            Some(&self.label),
            self.title.as_deref(),
            self.hx.as_ref(),
            self.popover.as_deref(),
            html! { span class="oa-composer-action-icon" aria-hidden="true" { (self.icon) } },
        )
    }
}

/// The element the composer's selectors and pickers load their panels into
/// (their [`HxGet::target`]). Put it in [`Composer::after`]; a loaded
/// [`ComposerPanel`] opens above the composer.
#[must_use]
pub fn composer_panel_host(id: &str) -> Markup {
    html! { div id=(id) class="oa-composer-panel-host" {} }
}

/// A panel loaded into the [`composer_panel_host`]: a titled section with a
/// close button and its content. Inside the body, `oa-composer-choice`
/// buttons (a `strong` title and `small` details), `oa-composer-entry` rows
/// (an input and a button), `oa-composer-details` lists and
/// `oa-composer-note` lines are styled.
#[derive(Clone, Debug)]
pub struct ComposerPanel {
    title: String,
    close: Option<HxGet>,
    close_label: String,
    body: Option<Markup>,
}

impl ComposerPanel {
    /// A panel headed `title`; the title also names the region.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            close: None,
            close_label: "Close".to_owned(),
            body: None,
        }
    }

    /// The close button's request (usually one that empties the host).
    #[must_use]
    pub fn close(mut self, hx: HxGet) -> Self {
        self.close = Some(hx);
        self
    }

    /// The close button's accessible name, "Close" by default.
    #[must_use]
    pub fn close_label(mut self, label: impl Into<String>) -> Self {
        self.close_label = label.into();
        self
    }

    /// The panel content.
    #[must_use]
    pub fn body(mut self, body: impl Render) -> Self {
        self.body = Some(body.render());
        self
    }
}

impl Render for ComposerPanel {
    fn render(&self) -> Markup {
        let close = self.close.as_ref();
        html! {
            section class="oa-composer-panel" aria-label=(self.title) {
                header class="oa-composer-panel-header" {
                    h2 class="oa-composer-panel-title" { (self.title) }
                    @if let Some(close) = close {
                        button type="button" class="oa-composer-panel-close"
                            aria-label=(self.close_label) title=(self.close_label)
                            hx-get=(close.url)
                            hx-target=[close.target.as_deref()]
                            hx-include=[close.include.as_deref()]
                            hx-swap=[close.swap.as_deref()]
                            hx-sync=[close.sync.as_deref()] {
                            (glyph::close())
                        }
                    }
                }
                div class="oa-composer-panel-body" {
                    @if let Some(body) = &self.body { (body) }
                }
            }
        }
    }
}
