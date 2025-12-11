//! Simple components story showing Phase 2 UI components

use gpui::*;
use ui_oa::{
    Badge, BadgeVariant,
    Avatar, AvatarSize,
    Alert, AlertVariant,
    Toggle, ToggleVariant,
    Collapsible,
    AspectRatio, AspectRatioPreset,
    RadioGroup,
    Slider,
};
use crate::story::Story;

pub struct SimpleComponentsStory;

impl Render for SimpleComponentsStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("Simple Components"))
            .child(Story::description("Phase 2: Badge, Avatar, Alert, Toggle, Collapsible, AspectRatio, RadioGroup, Slider"))

            // Badge
            .child(Story::section()
                .child(Story::section_title("Badge"))
                .child(Story::row()
                    .child(Badge::new("Default"))
                    .child(Badge::new("Secondary").variant(BadgeVariant::Secondary))
                    .child(Badge::new("Outline").variant(BadgeVariant::Outline))
                    .child(Badge::new("Destructive").variant(BadgeVariant::Destructive))))

            // Avatar
            .child(Story::section()
                .child(Story::section_title("Avatar"))
                .child(Story::row()
                    .child(Story::item("Small").child(Avatar::new().fallback("SM").size(AvatarSize::Sm)))
                    .child(Story::item("Default").child(Avatar::new().fallback("JD")))
                    .child(Story::item("Large").child(Avatar::new().fallback("LG").size(AvatarSize::Lg)))))

            // Alert
            .child(Story::section()
                .child(Story::section_title("Alert"))
                .child(Story::column()
                    .gap(px(12.0))
                    .child(Alert::new("Heads up!").description("You can add components to your app using the cli."))
                    .child(Alert::new("Error").variant(AlertVariant::Destructive).description("Your session has expired. Please log in again."))))

            // Toggle
            .child(Story::section()
                .child(Story::section_title("Toggle"))
                .child(Story::row()
                    .child(Story::item("Default").child(Toggle::new("B")))
                    .child(Story::item("Pressed").child(Toggle::new("I").pressed(true)))
                    .child(Story::item("Outline").child(Toggle::new("U").variant(ToggleVariant::Outline)))
                    .child(Story::item("Disabled").child(Toggle::new("S").disabled(true)))))

            // Collapsible
            .child(Story::section()
                .child(Story::section_title("Collapsible"))
                .child(Story::column()
                    .gap(px(8.0))
                    .child(Collapsible::new()
                        .trigger("Closed section")
                        .content(div().child("This content is hidden")))
                    .child(Collapsible::new()
                        .trigger("Open section")
                        .open(true)
                        .content(div().p(px(8.0)).child("This content is visible because open=true")))))

            // AspectRatio
            .child(Story::section()
                .child(Story::section_title("AspectRatio"))
                .child(Story::row()
                    .child(Story::item("16:9").child(
                        AspectRatio::new(AspectRatioPreset::Widescreen)
                            .width(160.0)
                            .child(div().w_full().h_full().bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)).rounded(px(4.0)))))
                    .child(Story::item("4:3").child(
                        AspectRatio::new(AspectRatioPreset::Standard)
                            .width(120.0)
                            .child(div().w_full().h_full().bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)).rounded(px(4.0)))))
                    .child(Story::item("1:1").child(
                        AspectRatio::new(AspectRatioPreset::Square)
                            .width(80.0)
                            .child(div().w_full().h_full().bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)).rounded(px(4.0)))))))

            // RadioGroup
            .child(Story::section()
                .child(Story::section_title("RadioGroup"))
                .child(Story::row()
                    .child(Story::item("Options").child(
                        RadioGroup::new("size")
                            .value("md")
                            .option("Small", "sm")
                            .option("Medium", "md")
                            .option("Large", "lg")))
                    .child(Story::item("Disabled").child(
                        RadioGroup::new("disabled")
                            .value("opt1")
                            .option("Option 1", "opt1")
                            .option("Option 2", "opt2")
                            .disabled(true)))))

            // Slider
            .child(Story::section()
                .child(Story::section_title("Slider"))
                .child(Story::column()
                    .gap(px(16.0))
                    .child(Story::row()
                        .child(Story::label("0%"))
                        .child(Slider::new().value(0.0)))
                    .child(Story::row()
                        .child(Story::label("50%"))
                        .child(Slider::new().value(0.5)))
                    .child(Story::row()
                        .child(Story::label("100%"))
                        .child(Slider::new().value(1.0)))
                    .child(Story::row()
                        .child(Story::label("Disabled"))
                        .child(Slider::new().value(0.75).disabled(true)))))
    }
}
