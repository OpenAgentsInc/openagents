//! Alert, ported from Apps SDK UI `src/components/Alert` (MIT).
//! Styles: `static/components/alert.css`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag};
use super::{Color, Variant, glyphs};

/// Where the actions sit. React measures the actions and picks `Bottom`
/// when they take more than a third of the width; server-rendered alerts
/// default to `End`, so choose `Bottom` for wide action rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AlertActionsPlacement {
    #[default]
    End,
    Bottom,
}

impl AlertActionsPlacement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Clone, Debug)]
enum Indicator {
    Default,
    Custom(Markup),
    Hidden,
}

/// A message banner with an indicator icon, title, description and actions.
/// Danger alerts carry `role="alert"`, as in React. Colors: Primary
/// (default), Danger, Success, Info, Discovery, Caution, Warning. Variants:
/// Outline (default), Soft, Solid.
#[derive(Clone, Debug)]
pub struct Alert {
    color: Color,
    variant: Variant,
    title: Option<Markup>,
    description: Option<Markup>,
    actions: Option<Markup>,
    placement: AlertActionsPlacement,
    indicator: Indicator,
    attrs: Attrs,
}

impl Default for Alert {
    fn default() -> Self {
        Self::new()
    }
}

impl Alert {
    pub fn new() -> Self {
        Self {
            color: Color::Primary,
            variant: Variant::Outline,
            title: None,
            description: None,
            actions: None,
            placement: AlertActionsPlacement::End,
            indicator: Indicator::Default,
            attrs: Attrs::default(),
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.title = Some(html! { (title) });
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(html! { (description) });
        self
    }

    /// Rich description markup (for example text with a TextLink).
    pub fn description_markup(mut self, description: impl Render) -> Self {
        self.description = Some(description.render());
        self
    }

    /// Action buttons.
    pub fn actions(mut self, actions: impl Render) -> Self {
        self.actions = Some(actions.render());
        self
    }

    pub fn actions_placement(mut self, placement: AlertActionsPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Replaces the default indicator (Info, Warning for danger, caution and
    /// warning, CheckCircle for success).
    pub fn indicator(mut self, indicator: impl Render) -> Self {
        self.indicator = Indicator::Custom(indicator.render());
        self
    }

    /// Shows no indicator.
    pub fn no_indicator(mut self) -> Self {
        self.indicator = Indicator::Hidden;
        self
    }
}

impl_attrs!(Alert);

fn default_indicator(color: Color) -> Markup {
    match color {
        Color::Warning | Color::Caution | Color::Danger => glyphs::warning(),
        Color::Success => glyphs::check_circle(),
        _ => glyphs::info(),
    }
}

impl Render for Alert {
    fn render(&self) -> Markup {
        let indicator = match &self.indicator {
            Indicator::Default => Some(default_indicator(self.color)),
            Indicator::Custom(markup) => Some(markup.clone()),
            Indicator::Hidden => None,
        };
        Tag::new("div", "oa-alert", &self.attrs)
            .attr("data-variant", self.variant.as_str())
            .attr("data-color", self.color.as_str())
            .attr_opt("role", (self.color == Color::Danger).then_some("alert"))
            .attr("data-actions-placement", self.placement.as_str())
            .extra(&self.attrs)
            .close(html! {
                @if let Some(indicator) = indicator {
                    div class="oa-alert-indicator" { (indicator) }
                }
                div class="oa-alert-content" {
                    div class="oa-alert-message" {
                        @if let Some(title) = &self.title {
                            div class="oa-alert-title" { (title) }
                        }
                        @if let Some(description) = &self.description {
                            div class="oa-alert-description" { (description) }
                        }
                    }
                    @if let Some(actions) = &self.actions {
                        div class="oa-alert-actions" { (actions) }
                    }
                }
            })
    }
}
