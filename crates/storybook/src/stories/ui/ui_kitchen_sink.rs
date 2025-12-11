//! UI Kitchen Sink - All shadcn-style UI components in one comprehensive view

use gpui::*;
use ui::{
    // Phase 1: Primitives
    Button, ButtonVariant, ButtonSize,
    Label, Separator, Kbd, Skeleton, Spinner,
    Progress, Checkbox, Switch,
    // Phase 2: Simple
    Badge, BadgeVariant,
    Avatar, AvatarSize,
    Alert, AlertVariant,
    Toggle, ToggleVariant,
    Collapsible,
    AspectRatio, AspectRatioPreset,
    RadioGroup,
    Slider,
    // Phase 3: Medium
    Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter,
    Tabs, TabItem,
    Accordion, AccordionItem,
    Tooltip,
    Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
    Breadcrumb, BreadcrumbItem,
    Pagination,
    // Phase 4: Complex (shown inline, not interactive)
    Select, SelectOption,
};
use crate::story::Story;

pub struct UiKitchenSinkStory;

impl Render for UiKitchenSinkStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("UI Kitchen Sink"))
            .child(Story::description("All 33 shadcn-style UI components for GPUI across 4 phases."))

            // ================================================================
            // PHASE 1: PRIMITIVES
            // ================================================================
            .child(div()
                .mt(px(24.0))
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text::PRIMARY)
                .child("Phase 1: Primitives"))

            // Buttons
            .child(Story::section()
                .child(Story::section_title("Button"))
                .child(Story::row()
                    .child(Button::new("Default"))
                    .child(Button::new("Destructive").variant(ButtonVariant::Destructive))
                    .child(Button::new("Outline").variant(ButtonVariant::Outline))
                    .child(Button::new("Secondary").variant(ButtonVariant::Secondary))
                    .child(Button::new("Ghost").variant(ButtonVariant::Ghost))
                    .child(Button::new("Link").variant(ButtonVariant::Link)))
                .child(Story::row()
                    .child(Button::new("Small").size(ButtonSize::Sm))
                    .child(Button::new("Default"))
                    .child(Button::new("Large").size(ButtonSize::Lg))
                    .child(Button::icon("⚡").size(ButtonSize::Icon))
                    .child(Button::new("Disabled").disabled(true))))

            // Label
            .child(Story::section()
                .child(Story::section_title("Label"))
                .child(Story::row()
                    .child(Label::new("Normal label"))
                    .child(Label::new("Disabled label").disabled(true))))

            // Separator
            .child(Story::section()
                .child(Story::section_title("Separator"))
                .child(Separator::horizontal())
                .child(Story::row()
                    .h(px(32.0))
                    .child(div().child("Left"))
                    .child(Separator::vertical())
                    .child(div().child("Right"))))

            // Kbd
            .child(Story::section()
                .child(Story::section_title("Kbd"))
                .child(Story::row()
                    .child(Kbd::new("⌘"))
                    .child(Kbd::new("K"))
                    .child(Kbd::new("Enter"))
                    .child(Kbd::new("Shift"))))

            // Skeleton & Spinner
            .child(Story::section()
                .child(Story::section_title("Skeleton & Spinner"))
                .child(Story::row()
                    .child(Skeleton::new().w(px(100.0)).h(px(20.0)))
                    .child(Skeleton::new().w(px(40.0)).h(px(40.0)).rounded_full())
                    .child(Spinner::sm())
                    .child(Spinner::md())
                    .child(Spinner::lg())))

            // Progress
            .child(Story::section()
                .child(Story::section_title("Progress"))
                .child(Story::column()
                    .gap(px(8.0))
                    .child(div().w(px(200.0)).child(Progress::new().value(0.25)))
                    .child(div().w(px(200.0)).child(Progress::new().value(0.75)))))

            // Checkbox & Switch
            .child(Story::section()
                .child(Story::section_title("Checkbox & Switch"))
                .child(Story::row()
                    .child(Checkbox::new())
                    .child(Checkbox::new().checked(true))
                    .child(Checkbox::new().disabled(true))
                    .child(Switch::new())
                    .child(Switch::new().on(true))
                    .child(Switch::new().disabled(true))))

            // ================================================================
            // PHASE 2: SIMPLE COMPONENTS
            // ================================================================
            .child(div()
                .mt(px(24.0))
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text::PRIMARY)
                .child("Phase 2: Simple Components"))

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
                    .child(Avatar::new().fallback("SM").size(AvatarSize::Sm))
                    .child(Avatar::new().fallback("JD"))
                    .child(Avatar::new().fallback("LG").size(AvatarSize::Lg))))

            // Alert
            .child(Story::section()
                .child(Story::section_title("Alert"))
                .child(Story::column()
                    .gap(px(8.0))
                    .child(Alert::new("Info").description("This is an informational alert."))
                    .child(Alert::new("Error").variant(AlertVariant::Destructive).description("Something went wrong."))))

            // Toggle
            .child(Story::section()
                .child(Story::section_title("Toggle"))
                .child(Story::row()
                    .child(Toggle::new("B"))
                    .child(Toggle::new("I").pressed(true))
                    .child(Toggle::new("U").variant(ToggleVariant::Outline))
                    .child(Toggle::new("S").disabled(true))))

            // Collapsible
            .child(Story::section()
                .child(Story::section_title("Collapsible"))
                .child(Story::column()
                    .gap(px(8.0))
                    .child(Collapsible::new().trigger("Closed").content(div().child("Hidden content")))
                    .child(Collapsible::new().trigger("Open").open(true).content(div().child("Visible content")))))

            // AspectRatio
            .child(Story::section()
                .child(Story::section_title("AspectRatio"))
                .child(Story::row()
                    .child(AspectRatio::new(AspectRatioPreset::Square).width(60.0).child(div().w_full().h_full().bg(theme::bg::HOVER).rounded(px(4.0))))
                    .child(AspectRatio::new(AspectRatioPreset::Widescreen).width(120.0).child(div().w_full().h_full().bg(theme::bg::HOVER).rounded(px(4.0))))))

            // RadioGroup
            .child(Story::section()
                .child(Story::section_title("RadioGroup"))
                .child(RadioGroup::new("size").value("md").option("Small", "sm").option("Medium", "md").option("Large", "lg")))

            // Slider
            .child(Story::section()
                .child(Story::section_title("Slider"))
                .child(Story::column()
                    .gap(px(8.0))
                    .child(Slider::new().value(0.3))
                    .child(Slider::new().value(0.7))))

            // ================================================================
            // PHASE 3: MEDIUM COMPONENTS
            // ================================================================
            .child(div()
                .mt(px(24.0))
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text::PRIMARY)
                .child("Phase 3: Medium Components"))

            // Card
            .child(Story::section()
                .child(Story::section_title("Card"))
                .child(
                    div().w(px(300.0)).child(
                        Card::new()
                            .child(CardHeader::new()
                                .child(CardTitle::new("Card Title"))
                                .child(CardDescription::new("Card description.")))
                            .child(CardContent::new().child(div().child("Card content area.")))
                            .child(CardFooter::new()
                                .child(Button::new("Cancel").variant(ButtonVariant::Outline))
                                .child(Button::new("Save"))))))

            // Tabs
            .child(Story::section()
                .child(Story::section_title("Tabs"))
                .child(Tabs::new()
                    .tab(TabItem::new("tab1", "Account"))
                    .tab(TabItem::new("tab2", "Password"))
                    .tab(TabItem::new("tab3", "Settings"))
                    .active("tab1")))

            // Accordion
            .child(Story::section()
                .child(Story::section_title("Accordion"))
                .child(
                    div().w(px(350.0)).child(
                        Accordion::new()
                            .child(AccordionItem::new("a1", "Section 1").content(div().child("Content 1")).open(true))
                            .child(AccordionItem::new("a2", "Section 2").content(div().child("Content 2"))))))

            // Tooltip
            .child(Story::section()
                .child(Story::section_title("Tooltip"))
                .child(Tooltip::new("Helpful tooltip text").child(Button::new("Hover me"))))

            // Table
            .child(Story::section()
                .child(Story::section_title("Table"))
                .child(
                    div().w(px(400.0)).child(
                        Table::new()
                            .child(TableHeader::new()
                                .child(TableRow::new()
                                    .child(TableHead::new("Name"))
                                    .child(TableHead::new("Status"))
                                    .child(TableHead::new("Amount"))))
                            .child(TableBody::new()
                                .child(TableRow::new()
                                    .child(TableCell::new("INV001"))
                                    .child(TableCell::new("Paid"))
                                    .child(TableCell::new("$100")))
                                .child(TableRow::new()
                                    .child(TableCell::new("INV002"))
                                    .child(TableCell::new("Pending"))
                                    .child(TableCell::new("$200")))))))

            // Breadcrumb
            .child(Story::section()
                .child(Story::section_title("Breadcrumb"))
                .child(Breadcrumb::new()
                    .item(BreadcrumbItem::new("Home").href("/"))
                    .item(BreadcrumbItem::new("Products").href("/products"))
                    .item(BreadcrumbItem::new("Electronics"))))

            // Pagination
            .child(Story::section()
                .child(Story::section_title("Pagination"))
                .child(Pagination::new().current_page(3).total_pages(10)))

            // ================================================================
            // PHASE 4: COMPLEX COMPONENTS
            // ================================================================
            .child(div()
                .mt(px(24.0))
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text::PRIMARY)
                .child("Phase 4: Complex Components"))

            .child(Story::section()
                .child(Story::section_title("Select, Dialog, Sheet, Command"))
                .child(Story::description("See the ComplexComponents story for interactive versions"))
                .child(Story::row()
                    .child(
                        div().w(px(180.0)).child(
                            Select::new()
                                .placeholder("Select option...")
                                .option(SelectOption::new("a", "Option A"))
                                .option(SelectOption::new("b", "Option B"))
                        )
                    )
                    .child(Button::new("Open Dialog").variant(ButtonVariant::Outline))
                    .child(Button::new("Open Sheet").variant(ButtonVariant::Outline))
                    .child(Button::new("⌘K Command").variant(ButtonVariant::Secondary))))
    }
}
