//! Overlays: Popover, Menu, Tooltip, SelectControl, Dialog.

use maud::{Markup, html};

use super::{Pane, row, specimen, stack};
use crate::actions::{Button, ButtonVariant, Color, ControlSize};
use crate::forms::{ControlSize as FormSize, Field};
use crate::icons::Icon;
use crate::overlays::{
    Align, Dialog, DialogSize, DialogTrigger, Menu, MenuItem, Popover, SelectControl,
    SelectVariant, Side, Tooltip, TooltipGutter,
};

pub(super) fn popover(pane: Pane) -> Markup {
    html! {
        (specimen("Popover", "Sides and alignment", row(html! {
            (Popover::new(pane.id("share"), "Share", html! {
                p { "Anyone with the link can view this run." }
                (Button::new("Copy link").size(ControlSize::Sm).icon_start(Icon::Copy))
            }).label("Share options"))
            (Popover::new(pane.id("top"), "Top end", html! { p { "Opens above, aligned to the end." } })
                .side(Side::Top).align(Align::End).label("Top end"))
            (Popover::new(pane.id("right"), html! { (Icon::Info) " Right" }, html! { p { "Opens to the right." } })
                .side(Side::Right).align(Align::Center).label("Right").trigger_label("More information"))
            (Popover::new(pane.id("styled"), "Styled trigger", html! { p { "The trigger takes extra classes and attributes." } })
                .trigger_class("oa-catalog-trigger").trigger_attr("data-catalog-trigger", "popover")
                .side(Side::Left).label("Left"))
        })))
    }
}

pub(super) fn menu(pane: Pane) -> Markup {
    html! {
        (specimen("Menu MenuItem", "Menu", row(html! {
            (Menu::new(pane.id("account"), html! { (Icon::User) " Account" })
                .label("Account")
                .item(MenuItem::heading("Signed in"))
                .item(MenuItem::link("Settings", "/settings").icon(Icon::Settings))
                .item(MenuItem::link("Billing", "/billing").disabled(true))
                .item(MenuItem::separator())
                .item(MenuItem::button("Compact mode").checked(true).value("compact").keep_open())
                .item(MenuItem::button("Show tips").checked(false).value("tips").keep_open())
                .item(MenuItem::separator())
                .item(MenuItem::button("Sign out").submit().form(pane.id("logout")).name("action").value("out")))
            (Menu::new(pane.id("sort"), "Sort")
                .label("Sort by").side(Side::Top).align(Align::End)
                .trigger_class("oa-catalog-trigger").trigger_label("Sort runs").trigger_attr("data-catalog-trigger", "menu")
                .items([
                    MenuItem::button("Newest").checked(true),
                    MenuItem::button("Oldest").checked(false),
                    MenuItem::button("Cost").checked(false),
                ]))
        })))
    }
}

pub(super) fn tooltip(pane: Pane) -> Markup {
    let trigger = |label: &str| {
        Button::new(label)
            .variant(ButtonVariant::Outline)
            .color(Color::Secondary)
            .size(ControlSize::Sm)
    };
    html! {
        (specimen("Tooltip", "Sides", row(html! {
            (Tooltip::new(pane.id("tip-top"), trigger("Top"), "Shown above"))
            (Tooltip::new(pane.id("tip-bottom"), trigger("Bottom"), "Shown below").side(Side::Bottom))
            (Tooltip::new(pane.id("tip-left"), trigger("Left"), "Shown left").side(Side::Left))
            (Tooltip::new(pane.id("tip-right"), trigger("Right"), "Shown right").side(Side::Right).align(Align::Start))
        })))
        (specimen("Tooltip", "Compact, gutters, delay and decoration", row(html! {
            (Tooltip::new(pane.id("tip-compact"), trigger("Compact"), "Compact").compact(true))
            (Tooltip::new(pane.id("tip-sm"), trigger("Small gutter"), "Small gutter").gutter(TooltipGutter::Sm))
            (Tooltip::new(pane.id("tip-lg"), trigger("Large gutter"), html! { strong { "Large gutter" } " with markup" }).gutter(TooltipGutter::Lg))
            (Tooltip::new(pane.id("tip-slow"), trigger("Slow"), "Waits 600 ms").delay_ms(600))
            (Tooltip::new(pane.id("tip-deco"), trigger("Decorated"), "With an arrow").decorated(true).align(Align::End))
        })))
    }
}

pub(super) fn select_control(pane: Pane) -> Markup {
    let model = Field::new(pane.id("sc-model"), "Model");
    let model_select = SelectControl::new(pane.id("sc-model"), pane.id("sc-model"))
        .option("mini", "Mini")
        .option("max", "Max")
        .disabled_option("ultra", "Ultra (soon)")
        .selected("max")
        .aria(model.aria());
    html! {
        (specimen("Field SelectControl", "In a field", model.control(model_select)))
        (specimen("SelectControl", "Variants and options", stack(html! {
            (SelectControl::new(pane.id("sc-soft"), pane.id("sc-soft")).variant(SelectVariant::Soft)
                .placeholder("Soft").option("a", "Alpha").option("b", "Beta").aria_label("Soft"))
            (SelectControl::new(pane.id("sc-ghost"), pane.id("sc-ghost")).variant(SelectVariant::Ghost)
                .option("a", "Ghost").selected("a").aria_label("Ghost").pill(false))
            (SelectControl::new(pane.id("sc-search"), pane.id("sc-search"))
                .searchable("Search languages").empty_text("No languages")
                .option("rust", "Rust").option("ts", "TypeScript").option("go", "Go").option("py", "Python")
                .multiple(true).selected("rust").selected("ts").block(true).aria_label("Languages")
                .side(Side::Top).align(Align::End))
            (SelectControl::new(pane.id("sc-small"), pane.id("sc-small")).size(FormSize::Sm)
                .option("a", "Small").selected("a").aria_label("Small"))
            (SelectControl::new(pane.id("sc-off"), pane.id("sc-off")).disabled(true)
                .option("a", "Disabled").selected("a").aria_label("Disabled"))
            (SelectControl::new(pane.id("sc-bad"), pane.id("sc-bad")).invalid(true).required(true)
                .placeholder("Required").option("a", "A").aria_label("Invalid"))
        })))
    }
}

pub(super) fn dialog(pane: Pane) -> Markup {
    let sizes = [
        (DialogSize::Sm, "sm", "Small"),
        (DialogSize::Md, "md", "Medium"),
        (DialogSize::Lg, "lg", "Large"),
    ];
    let confirm = pane.id("dialog-confirm");
    let locked = pane.id("dialog-locked");
    html! {
        (specimen("Dialog DialogTrigger", "Sizes", row(html! {
            @for (size, slug, name) in sizes {
                @let id = pane.id(&format!("dialog-{slug}"));
                (DialogTrigger::new(id.clone(), format!("{name} dialog")).class("oa-catalog-trigger"))
                (Dialog::new(id, format!("{name} dialog"), html! { p { "Dialogs use the native dialog element." } })
                    .size(size).description("Escape or the close button dismisses it."))
            }
        })))
        (specimen("Dialog DialogTrigger", "Footer actions, not dismissible", row(html! {
            (DialogTrigger::new(confirm.clone(), "Delete run").label("Delete this run").attr("data-catalog-trigger", "dialog"))
            (Dialog::new(confirm.clone(), "Delete this run?", html! { p { "This removes its logs and artifacts." } })
                .footer(html! {
                    form method="dialog" class="oa-catalog-row" {
                        (Button::new("Cancel").variant(ButtonVariant::Ghost).color(Color::Secondary).value("cancel"))
                        (Button::new("Delete").color(Color::Danger).value("delete"))
                    }
                })
                .close_label("Close dialog"))
            (DialogTrigger::new(locked.clone(), "Locked dialog"))
            (Dialog::new(locked, "Choose to continue", html! { p { "This dialog has no close button; use the footer." } })
                .dismissible(false)
                .footer(html! { form method="dialog" { (Button::new("Continue")) } }))
        })))
    }
}
