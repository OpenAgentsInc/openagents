//! UI Kitchen Sink - All UI components in one comprehensive view

use gpui::*;
use ui::{
    Button, ButtonVariant, ButtonSize,
    Label, Separator, Kbd, Skeleton, Spinner,
    Progress, Checkbox, Switch,
};
use crate::story::Story;

pub struct UiKitchenSinkStory;

impl Render for UiKitchenSinkStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("UI Kitchen Sink"))
            .child(Story::description("All shadcn-style UI components for GPUI."))

            // Button showcase
            .child(Story::section()
                .child(Story::section_title("Buttons"))
                .child(Story::column()
                    .gap(px(16.0))
                    // All variants
                    .child(Story::row()
                        .child(Button::new("Default"))
                        .child(Button::new("Destructive").variant(ButtonVariant::Destructive))
                        .child(Button::new("Outline").variant(ButtonVariant::Outline))
                        .child(Button::new("Secondary").variant(ButtonVariant::Secondary))
                        .child(Button::new("Ghost").variant(ButtonVariant::Ghost))
                        .child(Button::new("Link").variant(ButtonVariant::Link)))
                    // All sizes
                    .child(Story::row()
                        .child(Button::new("Small").size(ButtonSize::Sm))
                        .child(Button::new("Default"))
                        .child(Button::new("Large").size(ButtonSize::Lg))
                        .child(Button::icon("⚡").size(ButtonSize::Icon)))
                    // Disabled
                    .child(Story::row()
                        .child(Button::new("Disabled").disabled(true))
                        .child(Button::new("Delete").variant(ButtonVariant::Destructive).disabled(true)))))

            // Form controls
            .child(Story::section()
                .child(Story::section_title("Form Controls"))
                .child(Story::row()
                    .gap(px(32.0))
                    // Labels
                    .child(Story::column()
                        .child(Story::label("Labels"))
                        .child(Label::new("Email address"))
                        .child(Label::new("Disabled").disabled(true)))
                    // Checkboxes
                    .child(Story::column()
                        .child(Story::label("Checkboxes"))
                        .child(Story::row()
                            .child(Checkbox::new())
                            .child(Checkbox::new().checked(true))
                            .child(Checkbox::new().disabled(true))))
                    // Switches
                    .child(Story::column()
                        .child(Story::label("Switches"))
                        .child(Story::row()
                            .child(Switch::new())
                            .child(Switch::new().on(true))
                            .child(Switch::new().disabled(true))))))

            // Status indicators
            .child(Story::section()
                .child(Story::section_title("Status Indicators"))
                .child(Story::row()
                    .gap(px(32.0))
                    // Progress
                    .child(Story::column()
                        .child(Story::label("Progress"))
                        .child(div().w(px(200.0)).child(Progress::new().value(0.33)))
                        .child(div().w(px(200.0)).child(Progress::new().value(0.66)))
                        .child(div().w(px(200.0)).child(Progress::new().value(1.0))))
                    // Spinners
                    .child(Story::column()
                        .child(Story::label("Spinners"))
                        .child(Story::row()
                            .child(Spinner::sm())
                            .child(Spinner::md())
                            .child(Spinner::lg())))))

            // Loading states
            .child(Story::section()
                .child(Story::section_title("Loading States (Skeleton)"))
                .child(Story::row()
                    .gap(px(24.0))
                    // Card skeleton
                    .child(Story::column()
                        .gap(px(8.0))
                        .child(Skeleton::new().w(px(200.0)).h(px(120.0)))
                        .child(Skeleton::new().w(px(200.0)).h(px(16.0)))
                        .child(Skeleton::new().w(px(150.0)).h(px(12.0))))
                    // Avatar + text skeleton
                    .child(Story::row()
                        .child(Skeleton::new().w(px(48.0)).h(px(48.0)).rounded_full())
                        .child(Story::column()
                            .gap(px(4.0))
                            .child(Skeleton::new().w(px(120.0)).h(px(16.0)))
                            .child(Skeleton::new().w(px(80.0)).h(px(12.0)))))))

            // Typography & Layout
            .child(Story::section()
                .child(Story::section_title("Typography & Layout"))
                .child(Story::column()
                    .gap(px(12.0))
                    // Keyboard shortcuts
                    .child(Story::row()
                        .child(Story::label("Keyboard:"))
                        .child(Kbd::new("⌘"))
                        .child(Kbd::new("K"))
                        .child(Story::label("or"))
                        .child(Kbd::new("Ctrl"))
                        .child(Kbd::new("P")))
                    // Separator
                    .child(Separator::horizontal())
                    .child(Story::row()
                        .h(px(24.0))
                        .child(Story::label("Item 1"))
                        .child(Separator::vertical())
                        .child(Story::label("Item 2"))
                        .child(Separator::vertical())
                        .child(Story::label("Item 3")))))

            // Example form
            .child(Story::section()
                .child(Story::section_title("Example: Form Actions"))
                .child(Story::column()
                    .gap(px(16.0))
                    .child(Story::row()
                        .child(Label::new("Would you like to continue?"))
                        .child(Checkbox::new().checked(true))
                        .child(Label::new("Remember my choice")))
                    .child(Separator::horizontal())
                    .child(Story::row()
                        .justify_end()
                        .child(Button::new("Cancel").variant(ButtonVariant::Ghost))
                        .child(Button::new("Save Changes")))))
    }
}
