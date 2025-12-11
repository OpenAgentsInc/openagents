//! Medium components story showing Phase 3 UI components

use gpui::*;
use ui_oa::{
    Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter,
    Tabs, TabItem,
    Accordion, AccordionItem,
    Tooltip,
    Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
    Breadcrumb, BreadcrumbItem,
    Pagination,
    Button,
};
use crate::story::Story;

pub struct MediumComponentsStory;

impl Render for MediumComponentsStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("Medium Components"))
            .child(Story::description("Phase 3: Card, Tabs, Accordion, Tooltip, Table, Breadcrumb, Pagination"))

            // Card
            .child(Story::section()
                .child(Story::section_title("Card"))
                .child(
                    div().w(px(350.0)).child(
                        Card::new()
                            .child(CardHeader::new()
                                .child(CardTitle::new("Card Title"))
                                .child(CardDescription::new("Card description goes here.")))
                            .child(CardContent::new()
                                .child(div().child("This is the card content area.")))
                            .child(CardFooter::new()
                                .child(Button::new("Cancel").variant(ui_oa::ButtonVariant::Outline))
                                .child(Button::new("Save")))
                    )
                ))

            // Tabs
            .child(Story::section()
                .child(Story::section_title("Tabs"))
                .child(
                    Tabs::new()
                        .tab(TabItem::new("account", "Account"))
                        .tab(TabItem::new("password", "Password"))
                        .tab(TabItem::new("settings", "Settings"))
                        .active("account")
                ))

            // Accordion
            .child(Story::section()
                .child(Story::section_title("Accordion"))
                .child(
                    div().w(px(400.0)).child(
                        Accordion::new()
                            .child(AccordionItem::new("item1", "Is it accessible?")
                                .content(div().child("Yes. It adheres to the WAI-ARIA design pattern."))
                                .open(true))
                            .child(AccordionItem::new("item2", "Is it styled?")
                                .content(div().child("Yes. It comes with default styles that match the other components.")))
                            .child(AccordionItem::new("item3", "Is it animated?")
                                .content(div().child("Yes. It's animated by default, but you can disable it.")))
                    )
                ))

            // Tooltip
            .child(Story::section()
                .child(Story::section_title("Tooltip"))
                .child(Story::row()
                    .child(
                        Tooltip::new("This is a helpful tooltip")
                            .child(Button::new("Hover me"))
                    )))

            // Table
            .child(Story::section()
                .child(Story::section_title("Table"))
                .child(
                    div().w(px(500.0)).child(
                        Table::new()
                            .child(TableHeader::new()
                                .child(TableRow::new()
                                    .child(TableHead::new("Invoice"))
                                    .child(TableHead::new("Status"))
                                    .child(TableHead::new("Amount"))))
                            .child(TableBody::new()
                                .child(TableRow::new()
                                    .child(TableCell::new("INV001"))
                                    .child(TableCell::new("Paid"))
                                    .child(TableCell::new("$250.00")))
                                .child(TableRow::new()
                                    .child(TableCell::new("INV002"))
                                    .child(TableCell::new("Pending"))
                                    .child(TableCell::new("$150.00")))
                                .child(TableRow::new()
                                    .child(TableCell::new("INV003"))
                                    .child(TableCell::new("Unpaid"))
                                    .child(TableCell::new("$350.00"))))
                    )
                ))

            // Breadcrumb
            .child(Story::section()
                .child(Story::section_title("Breadcrumb"))
                .child(
                    Breadcrumb::new()
                        .item(BreadcrumbItem::new("Home").href("/"))
                        .item(BreadcrumbItem::new("Products").href("/products"))
                        .item(BreadcrumbItem::new("Electronics").href("/products/electronics"))
                        .item(BreadcrumbItem::new("Phones"))
                ))

            // Pagination
            .child(Story::section()
                .child(Story::section_title("Pagination"))
                .child(Story::column()
                    .gap(px(16.0))
                    .child(Story::row()
                        .child(Story::label("Page 1 of 10"))
                        .child(Pagination::new().current_page(1).total_pages(10)))
                    .child(Story::row()
                        .child(Story::label("Page 5 of 10"))
                        .child(Pagination::new().current_page(5).total_pages(10)))
                    .child(Story::row()
                        .child(Story::label("Page 10 of 10"))
                        .child(Pagination::new().current_page(10).total_pages(10)))))
    }
}
