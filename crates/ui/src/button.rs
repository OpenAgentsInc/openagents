//! Button component using Tailwind CSS with semantic color tokens.

use maud::{Markup, html};

/// Button size variants.
#[derive(Debug, Clone, Copy, Default)]
pub enum ButtonSize {
    Small,
    #[default]
    Default,
    Large,
}

/// Button style variants.
#[derive(Debug, Clone, Copy, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
}

/// Button component builder.
///
/// # Examples
///
/// ```
/// use ui::{Button, ButtonVariant, ButtonSize};
///
/// // Primary button
/// let btn = Button::new("Click me")
///     .variant(ButtonVariant::Primary)
///     .size(ButtonSize::Default)
///     .render();
///
/// // Disabled secondary button
/// let btn = Button::new("Submit")
///     .variant(ButtonVariant::Secondary)
///     .disabled(true)
///     .render();
///
/// // Small ghost button
/// let btn = Button::new("Cancel")
///     .variant(ButtonVariant::Ghost)
///     .size(ButtonSize::Small)
///     .render();
/// ```
pub struct Button {
    label: String,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
}

impl Button {
    /// Create a new button with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            disabled: false,
        }
    }

    /// Set the button variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the button size.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Set the button as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Render the button.
    pub fn render(self) -> Markup {
        let base = "inline-flex items-center gap-2 font-mono cursor-pointer transition-colors select-none";

        let size = match self.size {
            ButtonSize::Small => "px-2 py-1 text-xs",
            ButtonSize::Default => "px-4 py-2 text-sm",
            ButtonSize::Large => "px-6 py-3 text-base",
        };

        let variant = match self.variant {
            ButtonVariant::Primary => "bg-primary text-primary-foreground border border-primary hover:opacity-90",
            ButtonVariant::Secondary => "bg-secondary text-secondary-foreground border border-border hover:bg-accent",
            ButtonVariant::Ghost => "bg-transparent text-muted-foreground border border-border hover:bg-accent hover:text-accent-foreground",
        };

        let disabled = if self.disabled {
            "opacity-50 cursor-not-allowed pointer-events-none"
        } else {
            ""
        };

        let class = format!("{base} {size} {variant} {disabled}");

        html! {
            button class=(class) disabled[self.disabled] {
                (self.label)
            }
        }
    }
}
