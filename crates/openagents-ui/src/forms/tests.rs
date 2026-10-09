use maud::Render;

use super::*;

fn html(value: impl Render) -> String {
    value.render().into_string()
}

#[test]
fn field_wires_label_description_and_error() {
    let field = Field::new("email", "Email")
        .description("Used for receipts")
        .error("Enter an email address")
        .required(true);
    let input = Input::new("email")
        .input_type(InputType::Email)
        .aria(field.aria());
    let out = html(field.control(input));
    assert!(out.contains(r#"<label class="oa-field__label" id="email-label" for="email">"#));
    assert!(out.contains(r#"id="email-description""#));
    assert!(out.contains(r#"id="email-error""#));
    assert!(out.contains(r#"type="email" id="email" name="email""#));
    assert!(out.contains(r#"aria-describedby="email-description email-error""#));
    assert!(out.contains(r#"aria-invalid="true""#));
    assert!(out.contains(" required"));
    assert!(out.contains("data-invalid"));
}

#[test]
fn field_without_error_is_not_invalid() {
    let field = Field::new("name", "Name");
    let aria = field.aria();
    assert_eq!(aria.described_by, None);
    assert!(!aria.invalid);
    let out = html(field.control(Input::new("name").aria(aria)));
    assert!(!out.contains("aria-invalid"));
    assert!(!out.contains("aria-describedby"));
}

#[test]
fn group_field_uses_fieldset_and_legend() {
    let field = Field::new("plan", "Plan").group(true).error("Pick a plan");
    let group = RadioGroup::new("plan")
        .option("free", "Free")
        .option("pro", "Pro")
        .selected("pro")
        .aria(field.aria());
    let out = html(field.control(group));
    assert!(out.starts_with(r#"<fieldset class="oa-field" data-invalid>"#));
    assert!(out.contains(r#"<legend class="oa-field__label" id="plan-label">"#));
    assert!(out.contains(r#"role="radiogroup""#));
    assert!(out.contains(r#"aria-labelledby="plan-label""#));
    assert!(out.contains(r#"aria-describedby="plan-error""#));
    assert!(out.contains(r#"type="radio" id="plan-1" name="plan" value="pro" checked"#));
    assert!(!out.contains(r#"value="free" checked"#));
}

#[test]
fn input_renders_variant_size_and_adornments() {
    let out = html(
        Input::new("q")
            .input_type(InputType::Search)
            .variant(Variant::Soft)
            .size(ControlSize::Xl2)
            .pill(true)
            .placeholder("Search")
            .start_adornment(maud::html! { svg {} }),
    );
    assert!(out.contains(r#"data-variant="soft""#));
    assert!(out.contains(r#"data-size="2xl""#));
    assert!(out.contains("data-pill"));
    assert!(out.contains("data-has-start-adornment"));
    assert!(!out.contains("data-has-end-adornment"));
    assert!(out.contains(r#"type="search""#));
}

#[test]
fn textarea_auto_grow_uses_alpine_component_name_only() {
    let out = html(
        Textarea::new("notes")
            .value("<b>hi</b>")
            .auto_grow(true)
            .min_rows(2)
            .max_rows(8),
    );
    assert!(out.contains(r#"x-data="oaTextareaAutogrow""#));
    assert!(out.contains(r#"data-min-rows="2" data-max-rows="8""#));
    assert!(!out.contains(" style="));
    let clamped = html(Textarea::new("long").min_rows(40).max_rows(90));
    assert!(clamped.contains(r#"data-min-rows="12" data-max-rows="24""#));
    assert!(clamped.contains(r#"rows="40""#));
    assert!(out.contains("&lt;b&gt;hi&lt;/b&gt;"));
    let plain = html(Textarea::new("notes"));
    assert!(!plain.contains("x-data"));
}

#[test]
fn checkbox_is_native_and_labelled() {
    let out = html(
        Checkbox::new("terms", "I agree")
            .description("Required to continue")
            .checked(true)
            .required(true),
    );
    assert!(out.contains(r#"<label class="oa-checkbox" for="terms">"#));
    assert!(out.contains(r#"type="checkbox" id="terms" name="terms""#));
    assert!(out.contains(r#"aria-describedby="terms-description""#));
    assert!(out.contains(" checked"));
    assert!(out.contains(" required"));
}

#[test]
fn switch_has_switch_role() {
    let out = html(
        Switch::new("notify")
            .label("Email me")
            .checked(true)
            .label_position(LabelPosition::Start),
    );
    assert!(out.contains(r#"type="checkbox" role="switch" id="notify" name="notify""#));
    assert!(out.contains(r#"data-label-position="start""#));
    assert!(out.contains(" checked"));
}

#[test]
fn segmented_control_is_a_radio_group() {
    let out = html(
        SegmentedControl::new("view")
            .option("list", "List")
            .icon_option("grid", "Grid", maud::html! { svg {} })
            .selected("grid")
            .aria_label("View"),
    );
    assert!(out.contains(r#"class="oa-segmented-control" id="view" role="radiogroup""#));
    assert!(out.contains(r#"aria-label="View""#));
    assert!(out.contains(r#"value="grid" aria-label="Grid" checked"#));
    assert_eq!(out.matches(r#"type="radio""#).count(), 2);
}

#[test]
fn slider_is_a_native_range() {
    let out = html(
        Slider::new("temp")
            .range(0.0, 2.0)
            .step(0.1)
            .value(0.5)
            .label("Temperature"),
    );
    assert!(out.contains(r#"x-data="oaSlider""#));
    assert!(
        out.contains(
            r#"type="range" id="temp" name="temp" min="0" max="2" step="0.1" value="0.5""#
        )
    );
    assert!(out.contains(r#"data-fill="25""#));
    assert!(!out.contains(" style="));
    assert!(html(Slider::new("t").value(33.0)).contains(r#"data-fill="35""#));
    assert!(out.contains(r#"<label for="temp">Temperature</label>"#));
    assert!(out.contains(r#"<output class="oa-slider__value" for="temp">"#));
}

#[test]
fn select_is_native_with_placeholder_and_groups() {
    let out = html(
        Select::new("model")
            .placeholder("Choose a model")
            .option("a", "Model A")
            .group("Legacy", [("b", "Model B")])
            .required(true),
    );
    assert!(
        out.contains(r#"<select class="oa-select__control" id="model" name="model" required>"#)
    );
    assert!(out.contains(r#"<option value="" disabled selected>Choose a model</option>"#));
    assert!(
        out.contains(r#"<optgroup label="Legacy"><option value="b">Model B</option></optgroup>"#)
    );

    let chosen = html(
        Select::new("model")
            .placeholder("Pick")
            .option("a", "A")
            .selected("a"),
    );
    assert!(chosen.contains(r#"<option value="" disabled>Pick</option>"#));
    assert!(chosen.contains(r#"<option value="a" selected>A</option>"#));
}

#[test]
fn date_pickers_are_native_date_inputs() {
    let out = html(DatePicker::new("due").value("2026-10-08").min("2026-01-01"));
    assert!(out.contains(r#"type="date" id="due" name="due" value="2026-10-08" min="2026-01-01""#));

    let field = Field::new("period", "Period").group(true);
    let range = DateRangePicker::new("from", "to")
        .start("2026-10-01")
        .end("2026-10-31")
        .aria(field.aria());
    let out = html(field.control(range));
    assert!(out.contains(r#"id="period" name="from" value="2026-10-01""#));
    assert!(out.contains(r#"max="2026-10-31""#));
    assert!(out.contains(r#"id="period-end" name="to" value="2026-10-31" min="2026-10-01""#));
    assert!(out.contains(r#"aria-label="Start date""#));
    assert!(out.contains(r#"aria-label="End date""#));
    assert!(out.contains(r#"aria-labelledby="period-label""#));
}

#[test]
fn tag_input_degrades_to_a_text_field() {
    let out = html(TagInput::new("labels").tags(["bug", "ui"]).max(5));
    assert!(out.contains(r#"x-data="oaTagInput""#));
    assert!(out.contains(r#"data-delimiter=",""#));
    assert!(out.contains(r#"data-max="5""#));
    assert!(out.contains(r#"type="text" id="labels" name="labels" value="bug, ui""#));
    assert_eq!(
        TagInput::parse(" bug, ui,,bug , docs ", ','),
        vec!["bug", "ui", "docs"]
    );
}

#[test]
fn markup_has_no_inline_alpine_expressions() {
    let all = [
        html(Textarea::new("a").auto_grow(true)),
        html(Slider::new("b")),
        html(TagInput::new("c")),
    ]
    .join("");
    for forbidden in ["x-on:", "@click", "@input", "x-bind:", "x-model", ":class="] {
        assert!(!all.contains(forbidden), "found {forbidden}");
    }
    for name in ["oaTagInput", "oaSlider", "oaTextareaAutogrow"] {
        assert!(SCRIPT.contains(&format!("Alpine.data(\"{name}\"")));
    }
}

#[test]
fn stylesheets_use_oa_classes_and_apps_sdk_tokens() {
    assert_eq!(STYLESHEETS.len(), 10);
    for (name, css) in STYLESHEETS {
        assert!(css.contains(".oa-"), "{name} has no oa- classes");
        assert!(!css.contains("@mixin"), "{name} has an unexpanded mixin");
        assert!(
            !css.contains("spacing("),
            "{name} uses the spacing() function"
        );
        // Theme colors use light-dark(), resolved by color-scheme, so the
        // system theme works too (see the crate-level data-theme test).
        assert!(
            !css.contains("[data-theme=\"dark\"]"),
            "{name} keys colors on data-theme"
        );
        assert_eq!(
            css.matches('{').count(),
            css.matches('}').count(),
            "{name} braces"
        );
    }
    let input = STYLESHEETS
        .iter()
        .find(|(n, _)| *n == "input.css")
        .unwrap()
        .1;
    assert!(input.contains("var(--input-outline-border-color)"));
    assert!(input.contains("var(--control-size-md)"));
}

#[test]
fn secret_fields_turn_off_autocomplete_and_spellcheck() {
    let area = html(Textarea::new("key").autocomplete("off").spellcheck(false));
    assert!(
        area.contains(r#"autocomplete="off" spellcheck="false""#),
        "{area}"
    );
    let input = html(Input::new("key").autocomplete("off").spellcheck(false));
    assert!(
        input.contains(r#"autocomplete="off" spellcheck="false""#),
        "{input}"
    );
    let plain = html(Textarea::new("notes"));
    assert!(!plain.contains("spellcheck"), "{plain}");
    assert!(!plain.contains("autocomplete"), "{plain}");
}
