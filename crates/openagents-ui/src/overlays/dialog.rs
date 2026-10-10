use maud::{Markup, Render, html};

use super::Trigger;

/// Dialog width (`data-size`): 360px, the `--dialog-max-width` default, or
/// 640px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl DialogSize {
    fn as_str(self) -> &'static str {
        match self {
            DialogSize::Sm => "sm",
            DialogSize::Md => "md",
            DialogSize::Lg => "lg",
        }
    }
}

/// A native modal `<dialog>` (`role="dialog"` implicit, `aria-modal` from
/// `showModal()`), labelled by its title. Escape closes it natively, the
/// close button is a `method="dialog"` form (no JavaScript needed), and
/// `closedby="any"` closes it on a backdrop click when [`Self::dismissible`]
/// (the default). `oaDialog` sets `data-state`, emulates `closedby` and
/// invoker commands where missing, and returns focus to the opener.
///
/// Open it with a [`DialogTrigger`], or from script with `showModal()`.
///
/// ```
/// use openagents_ui::overlays::{Dialog, DialogTrigger};
/// use maud::Render;
/// let open = DialogTrigger::new("confirm", "Delete").render().into_string();
/// let dialog = Dialog::new("confirm", "Delete this run?", maud::html! { p { "This cannot be undone." } })
///     .render()
///     .into_string();
/// assert!(open.contains(r#"command="show-modal""#));
/// assert!(dialog.contains(r#"aria-labelledby="confirm-title""#));
/// ```
#[derive(Clone, Debug)]
pub struct Dialog {
    id: String,
    title: String,
    description: Option<String>,
    body: Markup,
    footer: Option<Markup>,
    size: DialogSize,
    dismissible: bool,
    close_label: String,
    open_on_load: bool,
}

impl Dialog {
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Render) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            body: body.render(),
            footer: None,
            size: DialogSize::default(),
            dismissible: true,
            close_label: "Close".to_string(),
            open_on_load: false,
        }
    }

    /// A line under the title, wired to `aria-describedby`.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Actions row at the bottom, for example a form's buttons. A button in
    /// a `<form method="dialog">` here closes the dialog without script.
    pub fn footer(mut self, footer: impl Render) -> Self {
        self.footer = Some(footer.render());
        self
    }

    pub fn size(mut self, size: DialogSize) -> Self {
        self.size = size;
        self
    }

    /// Close on a backdrop click (`closedby="any"`, default true). When
    /// false only Escape and the buttons close it.
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Open as soon as it is on the page, and leave the page when closed
    /// (`data-oa-open`): for a dialog loaded on demand, such as a form that
    /// needs fresh tickets. Needs `oaDialog` (the component script).
    pub fn open_on_load(mut self, open: bool) -> Self {
        self.open_on_load = open;
        self
    }

    /// Accessible name of the close button (default "Close").
    pub fn close_label(mut self, label: impl Into<String>) -> Self {
        self.close_label = label.into();
        self
    }
}

impl Render for Dialog {
    fn render(&self) -> Markup {
        let title_id = format!("{}-title", self.id);
        let description_id = format!("{}-description", self.id);
        html! {
            dialog id=(self.id) class="oa-dialog" x-data="oaDialog" data-state="closed"
                aria-labelledby=(title_id)
                aria-describedby=[self.description.as_ref().map(|_| description_id.as_str())]
                closedby=(if self.dismissible { "any" } else { "closerequest" })
                data-size=(self.size.as_str()) data-oa-open[self.open_on_load] {
                div class="oa-dialog-container" {
                    div class="oa-dialog-header" {
                        h2 id=(title_id) class="oa-dialog-title" { (self.title) }
                        form method="dialog" class="oa-dialog-close-form" {
                            button type="submit" class="oa-dialog-close"
                                aria-label=(self.close_label) {
                                svg width="16" height="16" viewBox="0 0 16 16" fill="none"
                                    aria-hidden="true" {
                                    path d="M4 4l8 8M12 4l-8 8" stroke="currentColor"
                                        stroke-width="1.75" stroke-linecap="round" {}
                                }
                            }
                        }
                    }
                    @if let Some(description) = &self.description {
                        p id=(description_id) class="oa-dialog-description" { (description) }
                    }
                    div class="oa-dialog-body" { (self.body) }
                    @if let Some(footer) = &self.footer {
                        div class="oa-dialog-footer" { (footer) }
                    }
                }
            }
        }
    }
}

/// A button that opens the [`Dialog`] with this id: `commandfor` plus
/// `command="show-modal"` (no JavaScript where invoker commands ship), and
/// `data-oa-dialog` for the `oaDialog` fallback.
#[derive(Clone, Debug)]
pub struct DialogTrigger {
    dialog_id: String,
    trigger: Trigger,
}

impl DialogTrigger {
    pub fn new(dialog_id: impl Into<String>, content: impl Render) -> Self {
        Self {
            dialog_id: dialog_id.into(),
            trigger: Trigger::new(content),
        }
    }

    /// Extra classes on the button.
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.trigger.class(class);
        self
    }

    /// Accessible name, for icon-only content.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.trigger.label(label);
        self
    }

    /// An extra attribute, such as `data-variant`.
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.trigger.attr(name, value);
        self
    }
}

impl Render for DialogTrigger {
    fn render(&self) -> Markup {
        self.trigger.render(
            "oa-overlay-trigger",
            &[
                ("commandfor", &self.dialog_id),
                ("command", "show-modal"),
                ("aria-haspopup", "dialog"),
                ("aria-controls", &self.dialog_id),
                ("data-oa-dialog", &self.dialog_id),
                ("data-state", "closed"),
            ],
        )
    }
}
