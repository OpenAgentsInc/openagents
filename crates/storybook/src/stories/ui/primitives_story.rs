//! Primitives story showing all primitive UI components

use gpui_oa::*;
use ui_oa::{
    Button, ButtonVariant,
    Label, Separator, Kbd, Skeleton, Spinner,
    Progress, Checkbox, Switch,
};
use crate::story::Story;

pub struct PrimitivesStory;

impl Render for PrimitivesStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("UI Primitives"))
            .child(Story::description("All primitive UI components in one view."))

            // Button
            .child(Story::section()
                .child(Story::section_title("Button"))
                .child(Story::row()
                    .child(Button::new("Default"))
                    .child(Button::new("Destructive").variant(ButtonVariant::Destructive))
                    .child(Button::new("Outline").variant(ButtonVariant::Outline))
                    .child(Button::new("Secondary").variant(ButtonVariant::Secondary))
                    .child(Button::new("Ghost").variant(ButtonVariant::Ghost))
                    .child(Button::new("Link").variant(ButtonVariant::Link))))

            // Label
            .child(Story::section()
                .child(Story::section_title("Label"))
                .child(Story::row()
                    .child(Label::new("Normal label"))
                    .child(Label::new("Disabled label").disabled(true))))

            // Separator
            .child(Story::section()
                .child(Story::section_title("Separator"))
                .child(Story::column()
                    .child(Story::label("Horizontal"))
                    .child(Separator::horizontal())
                    .child(Story::row()
                        .h(px(40.0))
                        .child(Story::label("Vertical"))
                        .child(Separator::vertical())
                        .child(Story::label("Between items")))))

            // Kbd
            .child(Story::section()
                .child(Story::section_title("Kbd (Keyboard)"))
                .child(Story::row()
                    .child(Kbd::new("⌘"))
                    .child(Kbd::new("K"))
                    .child(Kbd::new("Enter"))
                    .child(Kbd::new("Ctrl"))
                    .child(Kbd::new("Shift"))
                    .child(Kbd::new("⌫"))))

            // Skeleton
            .child(Story::section()
                .child(Story::section_title("Skeleton"))
                .child(Story::column()
                    .child(Story::row()
                        .child(Skeleton::new().w(px(40.0)).h(px(40.0)).rounded_full())
                        .child(Story::column()
                            .child(Skeleton::new().w(px(200.0)).h(px(16.0)))
                            .child(Skeleton::new().w(px(150.0)).h(px(12.0)))))
                    .child(Skeleton::new().w(px(300.0)).h(px(100.0)))))

            // Spinner
            .child(Story::section()
                .child(Story::section_title("Spinner"))
                .child(Story::row()
                    .child(Story::item("Small").child(Spinner::sm()))
                    .child(Story::item("Medium").child(Spinner::md()))
                    .child(Story::item("Large").child(Spinner::lg()))))

            // Progress
            .child(Story::section()
                .child(Story::section_title("Progress"))
                .child(Story::column()
                    .gap(px(12.0))
                    .child(Story::row()
                        .child(Story::label("0%"))
                        .child(div().w(px(200.0)).child(Progress::new().value(0.0))))
                    .child(Story::row()
                        .child(Story::label("25%"))
                        .child(div().w(px(200.0)).child(Progress::new().value(0.25))))
                    .child(Story::row()
                        .child(Story::label("50%"))
                        .child(div().w(px(200.0)).child(Progress::new().value(0.50))))
                    .child(Story::row()
                        .child(Story::label("75%"))
                        .child(div().w(px(200.0)).child(Progress::new().value(0.75))))
                    .child(Story::row()
                        .child(Story::label("100%"))
                        .child(div().w(px(200.0)).child(Progress::new().value(1.0))))))

            // Checkbox
            .child(Story::section()
                .child(Story::section_title("Checkbox"))
                .child(Story::row()
                    .child(Story::item("Unchecked").child(Checkbox::new()))
                    .child(Story::item("Checked").child(Checkbox::new().checked(true)))
                    .child(Story::item("Disabled").child(Checkbox::new().disabled(true)))
                    .child(Story::item("Checked Disabled").child(Checkbox::new().checked(true).disabled(true)))))

            // Switch
            .child(Story::section()
                .child(Story::section_title("Switch"))
                .child(Story::row()
                    .child(Story::item("Off").child(Switch::new()))
                    .child(Story::item("On").child(Switch::new().on(true)))
                    .child(Story::item("Disabled Off").child(Switch::new().disabled(true)))
                    .child(Story::item("Disabled On").child(Switch::new().on(true).disabled(true)))))
    }
}
