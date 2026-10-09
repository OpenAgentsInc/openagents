use maud::{Markup, PreEscaped, Render, html};

use super::{Align, Side, check_icon};
use crate::forms::{self, ControlSize, FieldAria};

/// SelectControl trigger look (`data-variant`); Apps SDK UI's default is
/// outline. The no-JavaScript native select shows ghost as outline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectVariant {
    Soft,
    #[default]
    Outline,
    Ghost,
}

impl SelectVariant {
    fn as_str(self) -> &'static str {
        match self {
            SelectVariant::Soft => "soft",
            SelectVariant::Outline => "outline",
            SelectVariant::Ghost => "ghost",
        }
    }

    fn native(self) -> forms::Variant {
        match self {
            SelectVariant::Soft => forms::Variant::Soft,
            SelectVariant::Outline | SelectVariant::Ghost => forms::Variant::Outline,
        }
    }
}

#[derive(Clone, Debug)]
struct Choice {
    value: String,
    label: String,
    disabled: bool,
}

/// The rich select: a trigger button and a popover `role="listbox"` with
/// optional search and multiple selection. For a plain choice use
/// [`forms::Select`].
///
/// It renders a [`forms::Select`] (the no-JavaScript form, and the form
/// value either way) plus a hidden trigger and listbox. `oaSelect` swaps
/// them: it hides the native control, shows the trigger, moves any
/// `<label for=id>` to it, and keeps the native options in sync, firing
/// `input` and `change` on the `<select>` so form and HTMX listeners keep
/// working. Keys: arrows, Home/End, Enter, Space, type-ahead (without
/// search), Escape and Tab to close.
///
/// ```
/// use maud::Render;
/// use openagents_ui::overlays::SelectControl;
/// let html = SelectControl::new("repo", "repo")
///     .option("oa", "openagents")
///     .option("psionic", "psionic")
///     .searchable("Search repos")
///     .render()
///     .into_string();
/// assert!(html.contains(r#"role="listbox""#));
/// ```
#[derive(Clone, Debug)]
pub struct SelectControl {
    id: String,
    name: String,
    options: Vec<Choice>,
    selected: Vec<String>,
    placeholder: Option<String>,
    label: Option<String>,
    aria: FieldAria,
    variant: SelectVariant,
    size: ControlSize,
    pill: bool,
    block: bool,
    disabled: bool,
    multiple: bool,
    search: Option<String>,
    empty_text: String,
    side: Side,
    align: Align,
}

impl SelectControl {
    /// `id` is the native `<select>` id (for `<label for>`); `name` its
    /// form field.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            options: Vec::new(),
            selected: Vec::new(),
            placeholder: None,
            label: None,
            aria: FieldAria::default(),
            variant: SelectVariant::default(),
            size: ControlSize::default(),
            pill: true,
            block: false,
            disabled: false,
            multiple: false,
            search: None,
            empty_text: "No results".to_string(),
            side: Side::default(),
            align: Align::default(),
        }
    }

    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(Choice {
            value: value.into(),
            label: label.into(),
            disabled: false,
        });
        self
    }

    /// An option that cannot be chosen.
    pub fn disabled_option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(Choice {
            value: value.into(),
            label: label.into(),
            disabled: true,
        });
        self
    }

    /// Select a value; call again with `multiple(true)` to select several.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected.push(value.into());
        self
    }

    /// Shown when nothing is selected.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Accessible name when no `<label for=id>` names the control.
    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Wiring from a [`forms::Field`] (`aria-describedby`, invalid,
    /// required). The id stays the one given to [`Self::new`].
    pub fn aria(mut self, aria: FieldAria) -> Self {
        self.aria = aria;
        self
    }

    pub fn variant(mut self, variant: SelectVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Fully rounded ends (default true, as in Apps SDK UI).
    pub fn pill(mut self, pill: bool) -> Self {
        self.pill = pill;
        self
    }

    /// Full width.
    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.aria.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.aria.required = required;
        self
    }

    /// Allow several values (`<select multiple>`, `aria-multiselectable`).
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// Adds a search box with this placeholder above the options.
    pub fn searchable(mut self, placeholder: impl Into<String>) -> Self {
        self.search = Some(placeholder.into());
        self
    }

    /// Text when the search matches nothing (default "No results").
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    fn is_selected(&self, value: &str) -> bool {
        self.selected.iter().any(|selected| selected == value)
    }

    /// The no-JavaScript form: the same options as a native select.
    fn native(&self) -> forms::Select {
        let mut aria = self.aria.clone();
        aria.id = Some(self.id.clone());
        let mut select = forms::Select::new(self.name.clone())
            .multiple(self.multiple)
            .aria(aria)
            .variant(self.variant.native())
            .size(self.size)
            .pill(self.pill)
            .block(self.block)
            .disabled(self.disabled);
        if let Some(placeholder) = &self.placeholder {
            select = select.placeholder(placeholder.clone());
        }
        if let Some(label) = &self.label {
            select = select.aria_label(label.clone());
        }
        for choice in &self.options {
            select = if choice.disabled {
                select.disabled_option(choice.value.clone(), choice.label.clone())
            } else {
                select.option(choice.value.clone(), choice.label.clone())
            };
        }
        for value in &self.selected {
            select = select.selected(value.clone());
        }
        select
    }
}

impl Render for SelectControl {
    fn render(&self) -> Markup {
        let list_id = format!("{}-list", self.id);
        let panel_id = format!("{}-panel", self.id);
        let placeholder = self.placeholder.clone().unwrap_or_default();
        let labels: Vec<&str> = self
            .options
            .iter()
            .filter(|choice| self.is_selected(&choice.value))
            .map(|choice| choice.label.as_str())
            .collect();
        let text = if labels.is_empty() {
            placeholder.clone()
        } else {
            labels.join(", ")
        };
        let flag = |on: bool| on.then_some("");
        html! {
            div class="oa-select-root" x-data="oaSelect" data-state="closed"
                data-block=[flag(self.block)] {
                span class="oa-select-native" data-oa-native { (self.native()) }
                button type="button" class="oa-select-control" hidden data-oa-trigger
                    popovertarget=(panel_id) aria-haspopup="listbox" aria-controls=(panel_id)
                    aria-label=[self.label.as_deref()]
                    aria-describedby=[self.aria.described_by.as_deref()]
                    aria-invalid=[self.aria.invalid.then_some("true")]
                    disabled[self.disabled] data-state="closed" data-placeholder=(placeholder)
                    data-selected=(if labels.is_empty() { "false" } else { "true" })
                    data-variant=(self.variant.as_str()) data-size=(self.size.as_str())
                    data-pill=[flag(self.pill)] data-block=[flag(self.block)]
                    data-invalid=[flag(self.aria.invalid)] data-disabled=[flag(self.disabled)] {
                    span class="oa-select-control-text" data-oa-text { (text) }
                    span class="oa-select-control-indicators" { (dropdown_icon()) }
                }
                div id=(panel_id) class="oa-select-list" popover="auto" data-oa-panel
                    data-state="closed" data-match-width data-side=(self.side.as_str())
                    data-align=(self.align.as_str()) {
                    div class="oa-select-list-inner" {
                        @if let Some(search) = &self.search {
                            div class="oa-select-search" {
                                input type="search" data-oa-search role="combobox"
                                    aria-expanded="true" aria-controls=(list_id)
                                    aria-autocomplete="list" autocomplete="off"
                                    placeholder=(search) aria-label=(search);
                            }
                        }
                        div id=(list_id) class="oa-select-options" role="listbox" tabindex="-1"
                            aria-multiselectable=[self.multiple.then_some("true")]
                            aria-label=[self.label.as_deref().or(self.placeholder.as_deref())] {
                            @for (index, choice) in self.options.iter().enumerate() {
                                div id=(format!("{}-opt-{index}", self.id))
                                    class="oa-select-option" role="option"
                                    data-value=(choice.value) data-label=(choice.label)
                                    aria-selected=(if self.is_selected(&choice.value) { "true" } else { "false" })
                                    aria-disabled=[choice.disabled.then_some("true")] {
                                    span class="oa-select-option-inner" {
                                        span { (choice.label) }
                                        span class="oa-select-option-check" { (check_icon()) }
                                    }
                                }
                            }
                        }
                        @if self.search.is_some() {
                            div class="oa-select-empty" data-oa-empty hidden { (self.empty_text) }
                        }
                    }
                }
            }
        }
    }
}

/// Apps SDK UI's DropdownVector (MIT, see NOTICE).
fn dropdown_icon() -> Markup {
    html! {
        svg class="oa-select-control-icon" width="1em" height="1em" viewBox="0 0 10 16"
            fill="currentColor" aria-hidden="true" {
            (PreEscaped(r#"<path fill-rule="evenodd" clip-rule="evenodd" d="M4.34151 0.747423C4.71854 0.417526 5.28149 0.417526 5.65852 0.747423L9.65852 4.24742C10.0742 4.61111 10.1163 5.24287 9.75259 5.6585C9.38891 6.07414 8.75715 6.11626 8.34151 5.75258L5.00001 2.82877L1.65852 5.75258C1.24288 6.11626 0.61112 6.07414 0.247438 5.6585C-0.116244 5.24287 -0.0741267 4.61111 0.34151 4.24742L4.34151 0.747423ZM0.246065 10.3578C0.608879 9.94139 1.24055 9.89795 1.65695 10.2608L5.00001 13.1737L8.34308 10.2608C8.75948 9.89795 9.39115 9.94139 9.75396 10.3578C10.1168 10.7742 10.0733 11.4058 9.65695 11.7687L5.65695 15.2539C5.28043 15.582 4.7196 15.582 4.34308 15.2539L0.343082 11.7687C-0.0733128 11.4058 -0.116749 10.7742 0.246065 10.3578Z"/>"#))
        }
    }
}
