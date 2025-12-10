//! Value Types Story: Demonstrates the Value enum with JS-like coercion
//!
//! Tests type coercion, deep access, and JSON interoperability.

use gpui::{Context, Render, Window, div, prelude::*, px};
use unit::Value;
use theme::{accent, status, text, FONT_FAMILY};

use crate::story::Story;

pub struct ValueTypesStory;

impl Render for ValueTypesStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Type coercion examples
        let num = Value::Number(42.0);
        let str_val = Value::String("hello".to_string());
        let bool_val = Value::Boolean(true);
        let null_val = Value::Null;
        let arr = Value::array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
        let obj = Value::object(vec![
            ("name".to_string(), Value::String("test".to_string())),
            ("value".to_string(), Value::Number(100.0)),
        ]);

        // to_boolean coercion
        let num_to_bool = Value::Number(0.0).to_boolean();
        let str_to_bool = Value::String("".to_string()).to_boolean();
        let str_nonempty_to_bool = Value::String("x".to_string()).to_boolean();

        // to_number coercion
        let str_to_num = Value::String("42".to_string()).to_number();
        let bool_to_num = Value::Boolean(true).to_number();
        let null_to_num = Value::Null.to_number();

        // to_string coercion
        let num_to_str = Value::Number(3.14159).to_string_value();
        let bool_to_str = Value::Boolean(false).to_string_value();

        // Deep access
        let nested = Value::object(vec![
            ("user".to_string(), Value::object(vec![
                ("profile".to_string(), Value::object(vec![
                    ("name".to_string(), Value::String("Alice".to_string())),
                ])),
            ])),
        ]);
        let deep_name = nested.deep_get("user.profile.name")
            .map(|v| v.to_string_value())
            .unwrap_or_else(|| "not found".to_string());

        // Array access (using string index)
        let arr_elem = arr.get("1")
            .map(|v| v.to_string_value())
            .unwrap_or_else(|| "not found".to_string());

        Story::container()
            .child(Story::title("Value Types"))
            .child(Story::description("Dynamic Value type with JavaScript-like coercion semantics"))
            .child(
                Story::section()
                    .child(Story::section_title("Value Constructors"))
                    .child(Story::description("Creating different value types"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(value_row("Number", &format!("{:?}", num), "Number(42.0)"))
                            .child(value_row("String", &format!("{:?}", str_val), "String(\"hello\")"))
                            .child(value_row("Boolean", &format!("{:?}", bool_val), "Boolean(true)"))
                            .child(value_row("Null", &format!("{:?}", null_val), "Null"))
                            .child(value_row("Array", &format!("{}", arr), "[1, 2, 3]"))
                            .child(value_row("Object", &format!("{}", obj), "{...}"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("to_boolean() Coercion"))
                    .child(Story::description("Falsy: 0, \"\", null, false. Everything else is truthy."))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(coerce_row("Number(0).to_boolean()", num_to_bool, false))
                            .child(coerce_row("String(\"\").to_boolean()", str_to_bool, false))
                            .child(coerce_row("String(\"x\").to_boolean()", str_nonempty_to_bool, true))
                            .child(coerce_row("Null.to_boolean()", Value::Null.to_boolean(), false))
                            .child(coerce_row("Array([]).to_boolean()", Value::Array(vec![]).to_boolean(), true))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("to_number() Coercion"))
                    .child(Story::description("Strings parse to numbers, bools become 0/1"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(number_row("String(\"42\").to_number()", str_to_num, 42.0))
                            .child(number_row("Boolean(true).to_number()", bool_to_num, 1.0))
                            .child(number_row("Null.to_number()", null_to_num, 0.0))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("to_string() Coercion"))
                    .child(Story::description("Everything can become a string"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(string_row("Number(3.14159).to_string()", &num_to_str, "3.14159"))
                            .child(string_row("Boolean(false).to_string()", &bool_to_str, "false"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Deep Access"))
                    .child(Story::description("Navigate nested structures with dot-separated paths"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(string_row("deep_get(\"user.profile.name\")", &deep_name, "Alice"))
                            .child(string_row("array.get(\"1\")", &arr_elem, "2"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Type Identification"))
                    .child(Story::description("type_name() returns the variant name"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(string_row("Number(42).type_name()", Value::Number(42.0).type_name(), "number"))
                            .child(string_row("String(\"\").type_name()", Value::String(String::new()).type_name(), "string"))
                            .child(string_row("Array([]).type_name()", Value::Array(vec![]).type_name(), "array"))
                            .child(string_row("Object({}).type_name()", Value::Object(Default::default()).type_name(), "object"))
                    )
            )
    }
}

fn value_row(label: &str, actual: &str, _expected: &str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(
            div()
                .w(px(100.0))
                .text_sm()
                .text_color(text::SECONDARY)
                .child(label.to_string())
        )
        .child(
            div()
                .text_sm()
                .font_family(FONT_FAMILY)
                .text_color(accent::SECONDARY)
                .child(actual.to_string())
        )
}

fn coerce_row(expr: &str, actual: bool, expected: bool) -> gpui::Div {
    let passed = actual == expected;
    let status_color = if passed {
        status::SUCCESS
    } else {
        status::ERROR
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(
            div()
                .w(px(280.0))
                .text_sm()
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(expr.to_string())
        )
        .child(
            div()
                .text_sm()
                .text_color(text::MUTED)
                .child("=")
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(status_color)
                .child(format!("{}", actual))
        )
}

fn number_row(expr: &str, actual: f64, expected: f64) -> gpui::Div {
    let passed = (actual - expected).abs() < f64::EPSILON;
    let status_color = if passed {
        status::SUCCESS
    } else {
        status::ERROR
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(
            div()
                .w(px(280.0))
                .text_sm()
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(expr.to_string())
        )
        .child(
            div()
                .text_sm()
                .text_color(text::MUTED)
                .child("=")
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(status_color)
                .child(format!("{}", actual))
        )
}

fn string_row(expr: &str, actual: &str, expected: &str) -> gpui::Div {
    let passed = actual == expected;
    let status_color = if passed {
        status::SUCCESS
    } else {
        status::ERROR
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(
            div()
                .w(px(280.0))
                .text_sm()
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(expr.to_string())
        )
        .child(
            div()
                .text_sm()
                .text_color(text::MUTED)
                .child("=")
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(status_color)
                .child(format!("\"{}\"", actual))
        )
}
