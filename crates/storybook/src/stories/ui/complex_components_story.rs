//! Interactive complex components story showing Phase 4 UI components

use gpui_oa::*;
use gpui_oa::prelude::FluentBuilder;
use ui_oa::{
    Select, SelectOption,
    Popover,
    Dialog, DialogHeader, DialogTitle, DialogDescription, DialogContent, DialogFooter,
    DropdownMenu, DropdownMenuItem,
    Sheet, SheetSide, SheetHeader, SheetTitle, SheetDescription, SheetContent, SheetFooter,
    Command, CommandItem,
    Button, ButtonVariant,
};
use crate::story::Story;

pub struct ComplexComponentsStory {
    // Select state
    select_open: bool,
    select_value: Option<SharedString>,

    // Popover state
    popover_open: bool,

    // Dropdown state
    dropdown_open: bool,

    // Dialog state
    dialog_open: bool,

    // Sheet state
    sheet_open: bool,
    sheet_side: SheetSide,

    // Command state
    command_open: bool,

    // Status message
    last_action: SharedString,
}

impl ComplexComponentsStory {
    pub fn new() -> Self {
        Self {
            select_open: false,
            select_value: None,
            popover_open: false,
            dropdown_open: false,
            dialog_open: false,
            sheet_open: false,
            sheet_side: SheetSide::Right,
            command_open: false,
            last_action: "None".into(),
        }
    }
}

impl Render for ComplexComponentsStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get entity handle for callbacks
        let entity = cx.entity().clone();

        let select_open = self.select_open;
        let select_value = self.select_value.clone();
        let popover_open = self.popover_open;
        let dropdown_open = self.dropdown_open;
        let dialog_open = self.dialog_open;
        let sheet_open = self.sheet_open;
        let sheet_side = self.sheet_side;
        let command_open = self.command_open;
        let last_action = self.last_action.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Main scrollable content
                Story::container()
                    .child(Story::title("Interactive Components"))
                    .child(Story::description("Click buttons to test interactions. Phase 4: Select, Popover, Dialog, DropdownMenu, Sheet, Command"))

                    // Status bar
                    .child(
                        div()
                            .mb(px(16.0))
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(6.0))
                            .bg(theme_oa::bg::SURFACE)
                            .border_1()
                            .border_color(theme_oa::border::DEFAULT)
                            .text_sm()
                            .child(format!("Last action: {}", last_action))
                    )

                    // Select
                    .child(Story::section()
                        .child(Story::section_title("Select"))
                        .child(Story::description("Click to open dropdown, select an option"))
                        .child(
                            div().h(px(180.0)).w(px(220.0)).child({
                                let entity = entity.clone();
                                let mut select = Select::new()
                                    .placeholder("Select a fruit...")
                                    .option(SelectOption::new("apple", "Apple"))
                                    .option(SelectOption::new("banana", "Banana"))
                                    .option(SelectOption::new("orange", "Orange"))
                                    .option(SelectOption::new("grape", "Grape"))
                                    .open(select_open)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.select_value = Some(value.clone());
                                            this.select_open = false;
                                            this.last_action = format!("Selected: {}", value).into();
                                            cx.notify();
                                        });
                                    });

                                if let Some(ref val) = select_value {
                                    select = select.value(val.clone());
                                }

                                select
                            })
                        )
                        .child({
                            let entity = entity.clone();
                            Button::new("Toggle Select")
                                .variant(ButtonVariant::Outline)
                                .on_click(move |_, _window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.select_open = !this.select_open;
                                        this.last_action = format!("Select open: {}", this.select_open).into();
                                        cx.notify();
                                    });
                                })
                        }))

                    // Popover
                    .child(Story::section()
                        .child(Story::section_title("Popover"))
                        .child(Story::description("Click button to toggle popover"))
                        .child(
                            div().h(px(150.0)).child({
                                let entity = entity.clone();
                                Popover::new()
                                    .trigger(
                                        Button::new(if popover_open { "Close Popover" } else { "Open Popover" })
                                            .on_click(move |_, _window, cx| {
                                                entity.update(cx, |this, cx| {
                                                    this.popover_open = !this.popover_open;
                                                    this.last_action = format!("Popover open: {}", this.popover_open).into();
                                                    cx.notify();
                                                });
                                            })
                                    )
                                    .content(
                                        div()
                                            .w(px(200.0))
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Popover Content"))
                                            .child(div().text_xs().text_color(theme_oa::text::MUTED).child("This is the popover body. Click the button again to close."))
                                    )
                                    .open(popover_open)
                            })
                        ))

                    // DropdownMenu
                    .child(Story::section()
                        .child(Story::section_title("DropdownMenu"))
                        .child(Story::description("Click button to toggle menu, click items to trigger actions"))
                        .child(
                            div().h(px(280.0)).child({
                                let entity_trigger = entity.clone();
                                let entity_profile = entity.clone();
                                let entity_settings = entity.clone();
                                let entity_shortcuts = entity.clone();
                                let entity_logout = entity.clone();
                                DropdownMenu::new()
                                    .trigger(
                                        Button::new(if dropdown_open { "Close Menu" } else { "Open Menu" })
                                            .on_click(move |_, _window, cx| {
                                                entity_trigger.update(cx, |this, cx| {
                                                    this.dropdown_open = !this.dropdown_open;
                                                    this.last_action = format!("Dropdown open: {}", this.dropdown_open).into();
                                                    cx.notify();
                                                });
                                            })
                                    )
                                    .item(DropdownMenuItem::label("My Account"))
                                    .item(DropdownMenuItem::item("Profile").shortcut("⌘P").on_select(move |_window, cx| {
                                        entity_profile.update(cx, |this, cx| {
                                            this.dropdown_open = false;
                                            this.last_action = "Clicked: Profile".into();
                                            cx.notify();
                                        });
                                    }))
                                    .item(DropdownMenuItem::item("Settings").shortcut("⌘,").on_select(move |_window, cx| {
                                        entity_settings.update(cx, |this, cx| {
                                            this.dropdown_open = false;
                                            this.last_action = "Clicked: Settings".into();
                                            cx.notify();
                                        });
                                    }))
                                    .item(DropdownMenuItem::item("Keyboard shortcuts").shortcut("⌘K").on_select(move |_window, cx| {
                                        entity_shortcuts.update(cx, |this, cx| {
                                            this.dropdown_open = false;
                                            this.last_action = "Clicked: Keyboard shortcuts".into();
                                            cx.notify();
                                        });
                                    }))
                                    .item(DropdownMenuItem::separator())
                                    .item(DropdownMenuItem::item("Log out").on_select(move |_window, cx| {
                                        entity_logout.update(cx, |this, cx| {
                                            this.dropdown_open = false;
                                            this.last_action = "Clicked: Log out".into();
                                            cx.notify();
                                        });
                                    }))
                                    .open(dropdown_open)
                            })
                        ))

                    // Dialog
                    .child(Story::section()
                        .child(Story::section_title("Dialog"))
                        .child(Story::description("Click to open modal dialog"))
                        .child({
                            let entity = entity.clone();
                            Button::new("Open Dialog")
                                .on_click(move |_, _window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.dialog_open = true;
                                        this.last_action = "Dialog opened".into();
                                        cx.notify();
                                    });
                                })
                        }))

                    // Sheet
                    .child(Story::section()
                        .child(Story::section_title("Sheet"))
                        .child(Story::description("Click to open side panel"))
                        .child(Story::row()
                            .child({
                                let entity = entity.clone();
                                Button::new("Open Right Sheet")
                                    .on_click(move |_, _window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.sheet_side = SheetSide::Right;
                                            this.sheet_open = true;
                                            this.last_action = "Sheet opened (right)".into();
                                            cx.notify();
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Button::new("Open Left Sheet")
                                    .variant(ButtonVariant::Outline)
                                    .on_click(move |_, _window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.sheet_side = SheetSide::Left;
                                            this.sheet_open = true;
                                            this.last_action = "Sheet opened (left)".into();
                                            cx.notify();
                                        });
                                    })
                            })))

                    // Command
                    .child(Story::section()
                        .child(Story::section_title("Command Palette"))
                        .child(Story::description("Click to open command palette"))
                        .child({
                            let entity = entity.clone();
                            Button::new("Open Command Palette")
                                .on_click(move |_, _window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.command_open = true;
                                        this.last_action = "Command palette opened".into();
                                        cx.notify();
                                    });
                                })
                        }))
            )
            // Overlays rendered on top
            .when(dialog_open, |el| {
                let entity_cancel = entity.clone();
                let entity_save = entity.clone();
                el.child(
                    Dialog::new()
                        .open(true)
                        .child(DialogHeader::new()
                            .child(DialogTitle::new("Edit Profile"))
                            .child(DialogDescription::new("Make changes to your profile here. Click save when you're done.")))
                        .child(DialogContent::new()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .child(div().text_sm().child("Name: John Doe"))
                                    .child(div().text_sm().child("Email: john@example.com"))
                            ))
                        .child(DialogFooter::new()
                            .child(
                                Button::new("Cancel")
                                    .variant(ButtonVariant::Outline)
                                    .on_click(move |_, _window, cx| {
                                        entity_cancel.update(cx, |this, cx| {
                                            this.dialog_open = false;
                                            this.last_action = "Dialog cancelled".into();
                                            cx.notify();
                                        });
                                    })
                            )
                            .child(
                                Button::new("Save Changes")
                                    .on_click(move |_, _window, cx| {
                                        entity_save.update(cx, |this, cx| {
                                            this.dialog_open = false;
                                            this.last_action = "Dialog saved".into();
                                            cx.notify();
                                        });
                                    })
                            ))
                )
            })
            .when(sheet_open, |el| {
                let entity_close = entity.clone();
                let entity_save = entity.clone();
                el.child(
                    Sheet::new()
                        .open(true)
                        .side(sheet_side)
                        .child(SheetHeader::new()
                            .child(SheetTitle::new("Settings"))
                            .child(SheetDescription::new("Adjust your application preferences here.")))
                        .child(SheetContent::new()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("General"))
                                    .child(div().text_sm().text_color(theme_oa::text::MUTED).child("Theme: Dark"))
                                    .child(div().text_sm().text_color(theme_oa::text::MUTED).child("Language: English"))
                            ))
                        .child(SheetFooter::new()
                            .child(
                                Button::new("Close")
                                    .variant(ButtonVariant::Outline)
                                    .on_click(move |_, _window, cx| {
                                        entity_close.update(cx, |this, cx| {
                                            this.sheet_open = false;
                                            this.last_action = "Sheet closed".into();
                                            cx.notify();
                                        });
                                    })
                            )
                            .child(
                                Button::new("Save")
                                    .on_click(move |_, _window, cx| {
                                        entity_save.update(cx, |this, cx| {
                                            this.sheet_open = false;
                                            this.last_action = "Sheet saved".into();
                                            cx.notify();
                                        });
                                    })
                            ))
                )
            })
            .when(command_open, |el| {
                let entity = entity.clone();
                el.child(
                    Command::new()
                        .open(true)
                        .placeholder("Type a command or search...")
                        .item(CommandItem::new("new-file", "New File").shortcut("⌘N").group("File"))
                        .item(CommandItem::new("open-file", "Open File").shortcut("⌘O").group("File"))
                        .item(CommandItem::new("save", "Save").shortcut("⌘S").group("File"))
                        .item(CommandItem::new("settings", "Settings").shortcut("⌘,").group("Preferences"))
                        .item(CommandItem::new("theme", "Change Theme").group("Preferences"))
                        .on_select(move |id, _window, cx| {
                            entity.update(cx, |this, cx| {
                                this.command_open = false;
                                this.last_action = format!("Command: {}", id).into();
                                cx.notify();
                            });
                        })
                )
            })
    }
}
