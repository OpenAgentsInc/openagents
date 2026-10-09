//! Forms: Field and every control. Ids and names carry the pane prefix so
//! the two theme copies stay independent (radio groups with one name would
//! otherwise share a selection across panes).

use maud::{Markup, html};

use super::{Pane, row, specimen, stack};
use crate::forms::{
    Checkbox, ControlSize, DatePicker, DateRangePicker, Direction, Field, Input, InputType,
    LabelPosition, RadioGroup, SegmentedControl, Select, Slider, Switch, TagInput, Textarea,
    Variant,
};
use crate::icons::Icon;

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

pub(super) fn text_fields(pane: Pane) -> Markup {
    let email = Field::new(pane.id("email"), "Email")
        .description("We only use it for receipts.")
        .required(true);
    let email_input = Input::new(pane.id("email"))
        .input_type(InputType::Email)
        .placeholder("you@example.com")
        .autocomplete("email")
        .aria(email.aria());
    let handle = Field::new(pane.id("handle"), "Handle").error("That handle is taken");
    let handle_input = Input::new(pane.id("handle"))
        .value("openagents")
        .aria(handle.aria());
    let notes = Field::new(pane.id("notes"), "Notes").optional(true);
    let notes_input = Textarea::new(pane.id("notes"))
        .placeholder("Anything the agent should know")
        .rows(3)
        .aria(notes.aria());
    html! {
        (specimen("Field Input", "Field with description, required", email.control(email_input)))
        (specimen("Field Input", "Field with error", handle.control(handle_input)))
        (specimen("Input", "Types", stack(html! {
            (Input::new(pane.id("search")).input_type(InputType::Search).placeholder("Search").aria_label("Search"))
            (Input::new(pane.id("password")).input_type(InputType::Password).value("hunter22").aria_label("Password"))
            (Input::new(pane.id("url")).input_type(InputType::Url).placeholder("https://").aria_label("URL"))
            (Input::new(pane.id("tel")).input_type(InputType::Tel).placeholder("+1 555 0100").aria_label("Phone"))
            (Input::new(pane.id("number")).input_type(InputType::Number).min("0").max("100").step("5").value("40").aria_label("Number"))
        })))
        (specimen("Input", "Variants and states", stack(html! {
            (Input::new(pane.id("outline")).placeholder("Outline").aria_label("Outline"))
            (Input::new(pane.id("soft")).variant(Variant::Soft).placeholder("Soft").aria_label("Soft"))
            (Input::new(pane.id("pill")).pill(true).placeholder("Pill").aria_label("Pill"))
            (Input::new(pane.id("disabled")).disabled(true).value("Disabled").aria_label("Disabled"))
            (Input::new(pane.id("readonly")).readonly(true).value("Read only").aria_label("Read only"))
            (Input::new(pane.id("invalid")).invalid(true).value("Invalid").aria_label("Invalid"))
        })))
        (specimen("Input", "Sizes", stack(html! {
            @for (size, name) in SIZES {
                (Input::new(pane.id(&format!("size-{name}"))).size(size).placeholder(name).aria_label(name))
            }
        })))
        (specimen("Field Textarea", "Textarea in a field", notes.control(notes_input)))
        (specimen("Textarea", "Textarea variants", stack(html! {
            (Textarea::new(pane.id("autogrow")).auto_grow(true).min_rows(2).max_rows(6).placeholder("Grows as you type").aria_label("Auto-grow"))
            (Textarea::new(pane.id("soft-text")).variant(Variant::Soft).value("Soft variant").aria_label("Soft"))
            (Textarea::new(pane.id("limited")).maxlength(140).size(ControlSize::Sm).placeholder("Up to 140 characters").aria_label("Limited"))
            (Textarea::new(pane.id("readonly-text")).readonly(true).value("Read only").aria_label("Read only"))
            (Textarea::new(pane.id("disabled-text")).disabled(true).value("Disabled").aria_label("Disabled"))
            (Textarea::new(pane.id("invalid-text")).invalid(true).value("Invalid").aria_label("Invalid"))
        })))
    }
}

pub(super) fn choices(pane: Pane) -> Markup {
    let plan = Field::new(pane.id("plan"), "Plan").group(true);
    let plan_group = RadioGroup::new(pane.id("plan"))
        .option_with_description("free", "Free", "Local agents only")
        .option_with_description("pro", "Pro", "Cloud agents and more credits")
        .disabled_option("team", "Team (soon)")
        .selected("pro")
        .aria(plan.aria());
    html! {
        (specimen("Checkbox", "Checkbox", stack(html! {
            (Checkbox::new(pane.id("terms"), "I accept the terms"))
            (Checkbox::new(pane.id("updates"), "Product updates").checked(true).description("About once a month."))
            (Checkbox::new(pane.id("label-first"), "Label first").label_first(true))
            (Checkbox::new(pane.id("locked"), "Disabled").checked(true).disabled(true))
            (Checkbox::new(pane.id("required"), "Required, invalid").required(true).invalid(true))
        })))
        (specimen("Field RadioGroup", "Radio group in a field", plan.control(plan_group)))
        (specimen("RadioGroup", "Row direction", RadioGroup::new(pane.id("speed"))
            .option("fast", "Fast").option("balanced", "Balanced").option("thorough", "Thorough")
            .selected("balanced").direction(Direction::Row).aria_label("Speed")))
        (specimen("RadioGroup", "Disabled", RadioGroup::new(pane.id("locked-radio"))
            .option("a", "Option A").option("b", "Option B").selected("a")
            .direction(Direction::Col).disabled(true).aria_label("Locked")))
        (specimen("Switch", "Switch", stack(html! {
            (Switch::new(pane.id("notify")).label("Notifications"))
            (Switch::new(pane.id("auto")).label("Auto-approve edits").checked(true).description("Agents apply edits without asking."))
            (Switch::new(pane.id("start")).label("Label at start").label_position(LabelPosition::Start))
            (Switch::new(pane.id("off")).label("Disabled").disabled(true))
            (Switch::new(pane.id("bare")).aria_label("Unlabelled switch").checked(true))
        })))
        (specimen("SegmentedControl", "Segmented control", stack(html! {
            (SegmentedControl::new(pane.id("view"))
                .option("list", "List").option("board", "Board").option("timeline", "Timeline")
                .selected("board").aria_label("View"))
            (SegmentedControl::new(pane.id("mode"))
                .option_with_icon("chat", "Chat", Icon::Sparkles)
                .option_with_icon("code", "Code", Icon::Code)
                .disabled_option("voice", "Voice")
                .selected("code").size(ControlSize::Sm).aria_label("Mode"))
            (SegmentedControl::new(pane.id("icons"))
                .icon_option("search", "Search", Icon::Search)
                .icon_option("settings", "Settings", Icon::Settings)
                .selected("search").pill(false).aria_label("Tool"))
            (SegmentedControl::new(pane.id("block")).option("day", "Day").option("week", "Week").option("month", "Month")
                .selected("week").block(true).aria_label("Range"))
            (SegmentedControl::new(pane.id("off-seg")).option("on", "On").option("off", "Off")
                .selected("on").disabled(true).aria_label("Disabled"))
        })))
    }
}

pub(super) fn select(pane: Pane) -> Markup {
    let region = Field::new(pane.id("region"), "Region").description("Where your agents run.");
    let region_select = Select::new(pane.id("region"))
        .placeholder("Choose a region")
        .option("us", "United States")
        .option("eu", "Europe")
        .disabled_option("ap", "Asia Pacific (soon)")
        .aria(region.aria());
    html! {
        (specimen("Field Select", "Native select in a field", region.control(region_select)))
        (specimen("Select", "Groups, variants and states", stack(html! {
            (Select::new(pane.id("model")).group("Fast", [("mini", "Mini"), ("nano", "Nano")])
                .group("Thorough", [("max", "Max")]).selected("max").aria_label("Model"))
            (Select::new(pane.id("soft-select")).variant(Variant::Soft).pill(true)
                .option("a", "Soft pill").aria_label("Soft"))
            (Select::new(pane.id("small-select")).size(ControlSize::Sm).option("a", "Small").aria_label("Small"))
            (Select::new(pane.id("block-select")).block(true).option("a", "Block").aria_label("Block"))
            (Select::new(pane.id("multi")).multiple(true).option("rust", "Rust").option("ts", "TypeScript")
                .option("go", "Go").selected("rust").selected("go").aria_label("Languages"))
            (Select::new(pane.id("disabled-select")).disabled(true).option("a", "Disabled").aria_label("Disabled"))
            (Select::new(pane.id("invalid-select")).invalid(true).required(true).placeholder("Required").option("a", "A").aria_label("Invalid"))
        })))
    }
}

pub(super) fn slider(pane: Pane) -> Markup {
    html! {
        (specimen("Slider", "Slider", stack(html! {
            (Slider::new(pane.id("effort")).label("Effort").value(50.0))
            (Slider::new(pane.id("budget")).label("Budget").range(0.0, 500.0).step(25.0).value(150.0).unit("credits"))
            (Slider::new(pane.id("level")).label("Level").range(1.0, 3.0).step(1.0).value(2.0).marks(["Low", "Medium", "High"]))
            (Slider::new(pane.id("locked-slider")).label("Disabled").value(30.0).disabled(true))
        })))
    }
}

pub(super) fn dates(pane: Pane) -> Markup {
    let due = Field::new(pane.id("due"), "Due date");
    let due_picker = DatePicker::new(pane.id("due"))
        .value("2026-10-08")
        .aria(due.aria());
    html! {
        (specimen("Field DatePicker", "Date picker in a field", due.control(due_picker)))
        (specimen("DatePicker", "Variants", row(html! {
            (DatePicker::new(pane.id("soft-date")).variant(Variant::Soft).pill(true).min("2026-01-01").max("2026-12-31").aria_label("Soft pill"))
            (DatePicker::new(pane.id("small-date")).size(ControlSize::Sm).aria_label("Small"))
            (DatePicker::new(pane.id("off-date")).disabled(true).value("2026-10-08").aria_label("Disabled"))
            (DatePicker::new(pane.id("bad-date")).invalid(true).required(true).aria_label("Invalid"))
        })))
        (specimen("DateRangePicker", "Date range", stack(html! {
            (DateRangePicker::new(pane.id("from"), pane.id("to")).start("2026-10-01").end("2026-10-08")
                .labels("Start date", "End date"))
            (DateRangePicker::new(pane.id("soft-from"), pane.id("soft-to")).variant(Variant::Soft)
                .min("2026-01-01").max("2026-12-31").size(ControlSize::Sm))
            (DateRangePicker::new(pane.id("off-from"), pane.id("off-to")).disabled(true))
        })))
    }
}

pub(super) fn tags(pane: Pane) -> Markup {
    let labels =
        Field::new(pane.id("labels"), "Labels").description("Press Enter or comma to add.");
    let labels_input = TagInput::new(pane.id("labels"))
        .tags(["ui", "catalog"])
        .placeholder("Add a label")
        .aria(labels.aria());
    html! {
        (specimen("Field TagInput", "Tag input in a field", labels.control(labels_input)))
        (specimen("TagInput", "Variants", stack(html! {
            (TagInput::new(pane.id("emails")).delimiter(';').max(3).size(ControlSize::Sm)
                .tags(["ada@example.com"]).aria_label("Emails"))
            (TagInput::new(pane.id("off-tags")).tags(["locked"]).disabled(true).aria_label("Disabled"))
            (TagInput::new(pane.id("bad-tags")).invalid(true).required(true).aria_label("Invalid"))
        })))
    }
}
