//! Button component story showing all variants and sizes

use gpui::*;
use ui::{Button, ButtonVariant, ButtonSize};
use crate::story::Story;

pub struct ButtonStory;

impl Render for ButtonStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("Button"))
            .child(Story::description("A button component with multiple variants and sizes."))

            // Variants section
            .child(Story::section()
                .child(Story::section_title("Variants"))
                .child(Story::row()
                    .child(Story::item("Default")
                        .child(Button::new("Default")))
                    .child(Story::item("Destructive")
                        .child(Button::new("Destructive").variant(ButtonVariant::Destructive)))
                    .child(Story::item("Outline")
                        .child(Button::new("Outline").variant(ButtonVariant::Outline)))
                    .child(Story::item("Secondary")
                        .child(Button::new("Secondary").variant(ButtonVariant::Secondary)))
                    .child(Story::item("Ghost")
                        .child(Button::new("Ghost").variant(ButtonVariant::Ghost)))
                    .child(Story::item("Link")
                        .child(Button::new("Link").variant(ButtonVariant::Link)))))

            // Sizes section
            .child(Story::section()
                .child(Story::section_title("Sizes"))
                .child(Story::row()
                    .child(Story::item("Small")
                        .child(Button::new("Small").size(ButtonSize::Sm)))
                    .child(Story::item("Default")
                        .child(Button::new("Default").size(ButtonSize::Default)))
                    .child(Story::item("Large")
                        .child(Button::new("Large").size(ButtonSize::Lg)))
                    .child(Story::item("Icon")
                        .child(Button::icon("⚙").size(ButtonSize::Icon)))))

            // States section
            .child(Story::section()
                .child(Story::section_title("States"))
                .child(Story::row()
                    .child(Story::item("Enabled")
                        .child(Button::new("Enabled")))
                    .child(Story::item("Disabled")
                        .child(Button::new("Disabled").disabled(true)))
                    .child(Story::item("Destructive Disabled")
                        .child(Button::new("Delete").variant(ButtonVariant::Destructive).disabled(true)))))

            // All variant/size combinations
            .child(Story::section()
                .child(Story::section_title("All Combinations"))
                .child(Story::column()
                    // Default row
                    .child(Story::row()
                        .child(Story::label("Default"))
                        .child(Button::new("Sm").size(ButtonSize::Sm))
                        .child(Button::new("Default"))
                        .child(Button::new("Lg").size(ButtonSize::Lg)))
                    // Destructive row
                    .child(Story::row()
                        .child(Story::label("Destructive"))
                        .child(Button::new("Sm").variant(ButtonVariant::Destructive).size(ButtonSize::Sm))
                        .child(Button::new("Default").variant(ButtonVariant::Destructive))
                        .child(Button::new("Lg").variant(ButtonVariant::Destructive).size(ButtonSize::Lg)))
                    // Outline row
                    .child(Story::row()
                        .child(Story::label("Outline"))
                        .child(Button::new("Sm").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                        .child(Button::new("Default").variant(ButtonVariant::Outline))
                        .child(Button::new("Lg").variant(ButtonVariant::Outline).size(ButtonSize::Lg)))
                    // Secondary row
                    .child(Story::row()
                        .child(Story::label("Secondary"))
                        .child(Button::new("Sm").variant(ButtonVariant::Secondary).size(ButtonSize::Sm))
                        .child(Button::new("Default").variant(ButtonVariant::Secondary))
                        .child(Button::new("Lg").variant(ButtonVariant::Secondary).size(ButtonSize::Lg)))
                    // Ghost row
                    .child(Story::row()
                        .child(Story::label("Ghost"))
                        .child(Button::new("Sm").variant(ButtonVariant::Ghost).size(ButtonSize::Sm))
                        .child(Button::new("Default").variant(ButtonVariant::Ghost))
                        .child(Button::new("Lg").variant(ButtonVariant::Ghost).size(ButtonSize::Lg)))))
    }
}
