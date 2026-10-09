//! Actions: buttons, links, badges, indicators, avatars, alerts, the empty
//! message, image and shimmer text.

use maud::{Markup, html};

use super::{Pane, caption, row, specimen, stack};
use crate::actions::{
    Alert, AlertActionsPlacement, Avatar, AvatarGroup, AvatarSize, AvatarStack, Badge, BadgeSize,
    Button, ButtonLink, ButtonVariant, CircularProgress, Color, ControlSize, CopyButton,
    EmptyMessage, EmptyMessageFill, EmptyMessageIconSize, Image, LoadingDots, LoadingIndicator,
    ShimmerTag, ShimmerText, TextLink, Variant,
};
use crate::icons::Icon;

const BUTTON_COLORS: [(Color, &str); 8] = [
    (Color::Primary, "Primary"),
    (Color::Secondary, "Secondary"),
    (Color::Danger, "Danger"),
    (Color::Success, "Success"),
    (Color::Warning, "Warning"),
    (Color::Caution, "Caution"),
    (Color::Discovery, "Discovery"),
    (Color::Info, "Info"),
];

const BUTTON_VARIANTS: [(ButtonVariant, &str); 5] = [
    (ButtonVariant::Solid, "Solid"),
    (ButtonVariant::Soft, "Soft"),
    (ButtonVariant::Outline, "Outline"),
    (ButtonVariant::Ghost, "Ghost"),
    (ButtonVariant::Transparent, "Transparent"),
];

const SIZES: [(ControlSize, &str); 9] = [
    (ControlSize::Xs3, "3xs"),
    (ControlSize::Xs2, "2xs"),
    (ControlSize::Xs, "xs"),
    (ControlSize::Sm, "sm"),
    (ControlSize::Md, "md"),
    (ControlSize::Lg, "lg"),
    (ControlSize::Xl, "xl"),
    (ControlSize::Xl2, "2xl"),
    (ControlSize::Xl3, "3xl"),
];

const VARIANTS: [(Variant, &str); 3] = [
    (Variant::Solid, "Solid"),
    (Variant::Soft, "Soft"),
    (Variant::Outline, "Outline"),
];

pub(super) fn buttons(pane: Pane) -> Markup {
    html! {
        @for (variant, name) in BUTTON_VARIANTS {
            (specimen("Button", &format!("{name} by color"), row(html! {
                @for (color, label) in BUTTON_COLORS {
                    (Button::new(label).variant(variant).color(color))
                }
            })))
        }
        (specimen("Button", "Sizes", row(html! {
            @for (size, label) in SIZES {
                (Button::new(label).size(size).color(Color::Secondary).variant(ButtonVariant::Soft))
            }
        })))
        (specimen("Button", "Shape, icons and states", row(html! {
            (Button::new("Pill"))
            (Button::new("Square").pill(false))
            (Button::new("New task").icon_start(Icon::Plus))
            (Button::new("Continue").icon_end(Icon::ArrowRight).variant(ButtonVariant::Outline).color(Color::Secondary))
            (Button::icon(Icon::Settings, "Settings").variant(ButtonVariant::Ghost).color(Color::Secondary))
            (Button::icon(Icon::Trash, "Delete").variant(ButtonVariant::Soft).color(Color::Danger))
            (Button::new("Saving").loading(true))
            (Button::new("Disabled").disabled(true))
            (Button::new("Selected").variant(ButtonVariant::Outline).color(Color::Secondary).selected(true))
        })))
        (specimen("Button", "Block", Button::new("Full width").block(true).color(Color::Secondary).variant(ButtonVariant::Soft)))
        (specimen("ButtonLink", "Links styled as buttons", row(html! {
            (ButtonLink::new("Open docs", "/docs").icon_start(Icon::Code))
            (ButtonLink::new("GitHub", "https://github.com/OpenAgentsInc/openagents").external(true).variant(ButtonVariant::Outline).color(Color::Secondary))
            (ButtonLink::icon(Icon::Download, "Download", "/download").variant(ButtonVariant::Soft).color(Color::Secondary))
        })))
        (specimen("CopyButton", "Copy to clipboard", row(html! {
            (CopyButton::new("cargo test -p openagents-ui").label("Copy command").variant(ButtonVariant::Outline).color(Color::Secondary))
            (CopyButton::new("oa_example_token").label("Copy").copied_label("Copied").size(ControlSize::Sm).variant(ButtonVariant::Soft).color(Color::Secondary))
            span class="oa-catalog-labelled" {
                code id=(pane.id("copy-source")) { "openagents issue claim 11021" }
                (CopyButton::new("").copy_from(pane.id("copy-source")).uniform(true).copy_icon(Icon::Copy).copied_icon(Icon::Check).aria_label("Copy issue command").variant(ButtonVariant::Ghost).color(Color::Secondary))
            }
        })))
    }
}

pub(super) fn links(_pane: Pane) -> Markup {
    html! {
        (specimen("TextLink", "Variants", stack(html! {
            p { "Read the " (TextLink::new("adoption plan", "/docs")) " before you start." }
            p { "A " (TextLink::new("primary link", "/docs").primary(true)) " and an " (TextLink::new("underlined one", "/docs").underline(true)) "." }
            p { "External: " (TextLink::new("Apps SDK UI", "https://github.com/openai/apps-sdk-ui")) }
            p { "Forced external: " (TextLink::new("download page", "/download").force_external(true)) }
            p { "Without href: " (TextLink::without_href("not a link yet")) }
            p { "With content: " (TextLink::with_content(html! { (Icon::Globe) " openagents.com" }, "/")) }
        })))
    }
}

pub(super) fn badges(_pane: Pane) -> Markup {
    html! {
        @for (variant, name) in VARIANTS {
            (specimen("Badge", name, row(html! {
                @for (color, label) in BUTTON_COLORS {
                    (Badge::new(label).variant(variant).color(color))
                }
            })))
        }
        (specimen("Badge", "Sizes, pill and icons", row(html! {
            (Badge::new("Small").size(BadgeSize::Sm))
            (Badge::new("Medium").size(BadgeSize::Md))
            (Badge::new("Large").size(BadgeSize::Lg))
            (Badge::new("Pill").pill(true).color(Color::Info))
            (Badge::new("Verified").icon_start(Icon::Check).color(Color::Success).variant(Variant::Soft))
            (Badge::new("Beta").icon_end(Icon::Sparkles).color(Color::Discovery).variant(Variant::Outline))
        })))
    }
}

pub(super) fn indicators(_pane: Pane) -> Markup {
    html! {
        (specimen("LoadingIndicator", "Loading indicator", row(html! {
            (LoadingIndicator::new())
            (LoadingIndicator::new().label("Loading results"))
            span class="oa-catalog-labelled" { (LoadingIndicator::new().decorative()) (caption("decorative")) }
        })))
        (specimen("LoadingDots", "Loading dots", row(html! {
            (LoadingDots::new())
            (LoadingDots::new().label("Agent is typing"))
            span class="oa-catalog-labelled" { (LoadingDots::new().decorative()) (caption("decorative")) }
        })))
        (specimen("CircularProgress", "Circular progress", row(html! {
            @for progress in [0.0_f32, 25.0, 50.0, 75.0, 100.0] {
                span class="oa-catalog-labelled" {
                    (CircularProgress::new(progress).label(format!("{progress}% done")))
                    (caption(&format!("{progress}%")))
                }
            }
            (CircularProgress::new(60.0).decorative())
        })))
    }
}

pub(super) fn avatars(_pane: Pane) -> Markup {
    let sizes = [
        AvatarSize::Px16,
        AvatarSize::Px20,
        AvatarSize::Px24,
        AvatarSize::Px28,
        AvatarSize::Px32,
        AvatarSize::Px36,
        AvatarSize::Px40,
        AvatarSize::Px48,
        AvatarSize::Px56,
        AvatarSize::Px64,
        AvatarSize::Px80,
        AvatarSize::Px96,
    ];
    let people = ["Ada", "Grace", "Linus", "Margaret", "Ken"];
    html! {
        (specimen("Avatar", "Initials by color", row(html! {
            @for (color, label) in BUTTON_COLORS {
                (Avatar::new().name(label).color(color))
            }
        })))
        (specimen("Avatar", "Solid and soft", row(html! {
            (Avatar::new().name("Solid").variant(Variant::Solid).color(Color::Info))
            (Avatar::new().name("Soft").variant(Variant::Soft).color(Color::Info))
            (Avatar::new().name("Image").image_url("/favicon.svg"))
            (Avatar::new().name("Icon").icon(Icon::User))
            (Avatar::new().overflow_count(12))
            (Avatar::new().overflow_count(2400))
            (Avatar::new().name("Interactive").interactive(true))
        })))
        (specimen("Avatar", "Sizes", row(html! {
            @for size in sizes {
                span class="oa-catalog-labelled" { (Avatar::new().name("OpenAgents").size(size)) (caption(size.as_str())) }
            }
        })))
        (specimen("AvatarGroup", "Groups", stack(html! {
            (AvatarGroup::new().avatars(people.iter().map(|name| Avatar::new().name(*name))))
            (AvatarGroup::new().stack(AvatarStack::End).size(AvatarSize::Px32)
                .avatars(people.iter().take(3).map(|name| Avatar::new().name(*name).variant(Variant::Solid)))
                .avatar(Avatar::new().overflow_count(9)))
        })))
    }
}

pub(super) fn alerts(_pane: Pane) -> Markup {
    html! {
        @for (variant, name) in VARIANTS {
            (specimen("Alert", name, stack(html! {
                @for (color, label) in BUTTON_COLORS {
                    (Alert::new().variant(variant).color(color).title(format!("{label} alert"))
                        .description("Something happened that you should know about."))
                }
            })))
        }
        (specimen("Alert", "Actions and indicators", stack(html! {
            (Alert::new().color(Color::Warning).variant(Variant::Soft).title("Usage at 80%")
                .description("Add credits to keep agents running.")
                .actions(Button::new("Add credits").size(ControlSize::Sm).color(Color::Warning)))
            (Alert::new().color(Color::Info).variant(Variant::Outline).title("New version")
                .description_markup(html! { "See the " (TextLink::new("release notes", "/docs")) "." })
                .actions(html! {
                    (Button::new("Later").size(ControlSize::Sm).variant(ButtonVariant::Ghost).color(Color::Secondary))
                    (Button::new("Update").size(ControlSize::Sm).color(Color::Info))
                })
                .actions_placement(AlertActionsPlacement::Bottom))
            (Alert::new().color(Color::Discovery).variant(Variant::Soft).title("Custom indicator").indicator(Icon::Sparkles))
            (Alert::new().color(Color::Secondary).variant(Variant::Outline).description("No indicator, description only.").no_indicator())
        })))
    }
}

pub(super) fn empty_message(_pane: Pane) -> Markup {
    html! {
        (specimen("EmptyMessage", "Default", EmptyMessage::new().fill(EmptyMessageFill::None)
            .icon(Icon::Folder).title("No tasks yet")
            .description("Start a task and it will show up here.")
            .actions(Button::new("New task").icon_start(Icon::Plus).size(ControlSize::Sm))))
        (specimen("EmptyMessage", "Danger, small icon", EmptyMessage::new().fill(EmptyMessageFill::None)
            .icon(Icon::Info).icon_size(EmptyMessageIconSize::Sm).icon_color(Color::Danger)
            .title("Could not load runs").title_color(Color::Danger)
            .description("Check your connection and try again.")))
        (specimen("EmptyMessage", "Warning, medium icon", EmptyMessage::new().fill(EmptyMessageFill::None)
            .icon(Icon::Bell).icon_size(EmptyMessageIconSize::Md).icon_color(Color::Warning)
            .title("Notifications paused").title_color(Color::Warning)))
    }
}

pub(super) fn media(_pane: Pane) -> Markup {
    html! {
        (specimen("Image", "Image", row(html! {
            (Image::new("/favicon.svg", "OpenAgents mark").width(64).height(64))
            (Image::new("/favicon.svg", "OpenAgents mark, lazy").width(32).height(32).lazy().draggable(false))
        })))
        (specimen("ShimmerText", "Shimmer text", stack(html! {
            (ShimmerText::new("Thinking about the plan"))
            (ShimmerText::new("Reading 12 files").tag(ShimmerTag::P))
            p { "Inline: " (ShimmerText::new("searching").tag(ShimmerTag::Span)) }
            (ShimmerText::new("Idle shimmer").idle(true))
        })))
    }
}
