//! Unit Runtime Story: Demonstrates unit execution and data flow
//!
//! Tests all system units (arithmetic, logic, comparison, control) by
//! running them with real inputs and displaying outputs.

use gpui_oa::{Context, Render, Window, div, prelude::*, px};
use unit::{Unit, system};
use theme_oa::{accent, status, text, FONT_FAMILY};

use crate::story::Story;

pub struct UnitRuntimeStory;

impl UnitRuntimeStory {
    /// Helper to run an arithmetic unit and return the result
    fn run_binary_op<U: Unit>(mut unit: U, a: f64, b: f64) -> String {
        unit.play();
        unit.push_input("a", Box::new(a)).unwrap();
        unit.push_input("b", Box::new(b)).unwrap();
        match unit.take_output("result") {
            Some(result) => {
                if let Ok(val) = result.downcast::<f64>() {
                    format!("{}", *val)
                } else {
                    "type error".to_string()
                }
            }
            None => "no output".to_string()
        }
    }

    /// Helper to run a unary unit and return the result
    fn run_unary_op<U: Unit>(mut unit: U, x: f64) -> String {
        unit.play();
        unit.push_input("x", Box::new(x)).unwrap();
        match unit.take_output("result") {
            Some(result) => {
                if let Ok(val) = result.downcast::<f64>() {
                    format!("{}", *val)
                } else {
                    "type error".to_string()
                }
            }
            None => "no output".to_string()
        }
    }

    /// Helper to run a logic unit and return the result
    fn run_logic_op<U: Unit>(mut unit: U, a: bool, b: bool) -> String {
        unit.play();
        unit.push_input("a", Box::new(a)).unwrap();
        unit.push_input("b", Box::new(b)).unwrap();
        match unit.take_output("result") {
            Some(result) => {
                if let Ok(val) = result.downcast::<bool>() {
                    format!("{}", *val)
                } else {
                    "type error".to_string()
                }
            }
            None => "no output".to_string()
        }
    }

    /// Helper to run a comparison unit and return the result
    fn run_comparison<U: Unit>(mut unit: U, a: f64, b: f64) -> String {
        unit.play();
        unit.push_input("a", Box::new(a)).unwrap();
        unit.push_input("b", Box::new(b)).unwrap();
        match unit.take_output("result") {
            Some(result) => {
                if let Ok(val) = result.downcast::<bool>() {
                    format!("{}", *val)
                } else {
                    "type error".to_string()
                }
            }
            None => "no output".to_string()
        }
    }
}

impl Render for UnitRuntimeStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Arithmetic results
        let add_result = Self::run_binary_op(system::Add::new(), 10.0, 5.0);
        let sub_result = Self::run_binary_op(system::Subtract::new(), 10.0, 5.0);
        let mul_result = Self::run_binary_op(system::Multiply::new(), 10.0, 5.0);
        let div_result = Self::run_binary_op(system::Divide::new(), 10.0, 5.0);
        let mod_result = Self::run_binary_op(system::Modulo::new(), 17.0, 5.0);
        let neg_result = Self::run_unary_op(system::Negate::new(), 42.0);
        let inc_result = Self::run_unary_op(system::Increment::new(), 10.0);
        let dec_result = Self::run_unary_op(system::Decrement::new(), 10.0);

        // Logic results
        let and_tt = Self::run_logic_op(system::And::new(), true, true);
        let and_tf = Self::run_logic_op(system::And::new(), true, false);
        let or_ff = Self::run_logic_op(system::Or::new(), false, false);
        let or_tf = Self::run_logic_op(system::Or::new(), true, false);

        // Comparison results
        let lt_result = Self::run_comparison(system::LessThan::new(), 3.0, 5.0);
        let gt_result = Self::run_comparison(system::GreaterThan::new(), 5.0, 3.0);
        let eq_result = Self::run_comparison(system::Equal::new(), 5.0, 5.0);
        let ne_result = Self::run_comparison(system::NotEqual::new(), 5.0, 3.0);

        // Control unit tests
        let identity_result = Self::run_unary_op(system::Identity::new(), 42.0);

        // Select unit test
        let mut select = system::Select::new();
        select.play();
        select.push_input("condition", Box::new(true)).unwrap();
        select.push_input("a", Box::new(10.0f64)).unwrap();
        select.push_input("b", Box::new(20.0f64)).unwrap();
        let select_result = select.take_output("result")
            .map(|r| format!("{}", *r.downcast::<f64>().unwrap()))
            .unwrap_or_else(|| "no output".to_string());

        // Registry test
        let registry = system::system_registry();
        let registry_count = registry.type_ids().len();

        Story::container()
            .child(Story::title("Unit Runtime"))
            .child(Story::description("Live execution of system units with real data"))
            .child(
                Story::section()
                    .child(Story::section_title("Arithmetic Units"))
                    .child(Story::description("Basic math operations on f64 values"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(result_row("Add(10, 5)", &add_result, "15"))
                            .child(result_row("Subtract(10, 5)", &sub_result, "5"))
                            .child(result_row("Multiply(10, 5)", &mul_result, "50"))
                            .child(result_row("Divide(10, 5)", &div_result, "2"))
                            .child(result_row("Modulo(17, 5)", &mod_result, "2"))
                            .child(result_row("Negate(42)", &neg_result, "-42"))
                            .child(result_row("Increment(10)", &inc_result, "11"))
                            .child(result_row("Decrement(10)", &dec_result, "9"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Logic Units"))
                    .child(Story::description("Boolean operations"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(result_row("And(true, true)", &and_tt, "true"))
                            .child(result_row("And(true, false)", &and_tf, "false"))
                            .child(result_row("Or(false, false)", &or_ff, "false"))
                            .child(result_row("Or(true, false)", &or_tf, "true"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Comparison Units"))
                    .child(Story::description("Compare f64 values, return bool"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(result_row("LessThan(3, 5)", &lt_result, "true"))
                            .child(result_row("GreaterThan(5, 3)", &gt_result, "true"))
                            .child(result_row("Equal(5, 5)", &eq_result, "true"))
                            .child(result_row("NotEqual(5, 3)", &ne_result, "true"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Control Units"))
                    .child(Story::description("Data routing and flow control"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(result_row("Identity(42)", &identity_result, "42"))
                            .child(result_row("Select(true, 10, 20)", &select_result, "10"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Unit Registry"))
                    .child(Story::description("Factory for creating units from type IDs"))
                    .child(
                        div().flex().flex_col().gap(px(8.0))
                            .child(
                                div().flex().flex_row().gap(px(8.0))
                                    .child(div().text_sm().text_color(text::SECONDARY).child("Registered unit types:"))
                                    .child(div().text_sm().font_weight(gpui_oa::FontWeight::BOLD)
                                        .text_color(accent::PRIMARY)
                                        .child(format!("{}", registry_count)))
                            )
                    )
            )
    }
}

/// Helper to render a result row with expression, actual result, and expected
fn result_row(expr: &str, actual: &str, expected: &str) -> gpui_oa::Div {
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
                .w(px(180.0))
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
                .w(px(80.0))
                .text_sm()
                .font_weight(gpui_oa::FontWeight::BOLD)
                .text_color(status_color)
                .child(actual.to_string())
        )
        .child(
            div()
                .text_sm()
                .text_color(text::MUTED)
                .child(if passed { "" } else { "(expected: " })
        )
        .child(
            div()
                .text_sm()
                .text_color(text::MUTED)
                .child(if passed { "".to_string() } else { format!("{})", expected) })
        )
}
