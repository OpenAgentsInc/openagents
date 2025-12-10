//! Unit Chains Story: Demonstrates chaining units together
//!
//! Shows data flowing through multiple connected units to perform
//! complex computations.

use gpui::{Context, Render, Window, div, hsla, prelude::*, px};
use unit::{Unit, system};

use crate::story::Story;

pub struct UnitChainsStory;

impl Render for UnitChainsStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Chain 1: (2 + 3) * 4 = 20
        let chain1_result = {
            let mut add = system::Add::new();
            let mut mul = system::Multiply::new();
            add.play();
            mul.play();

            add.push_input("a", Box::new(2.0f64)).unwrap();
            add.push_input("b", Box::new(3.0f64)).unwrap();
            let sum = add.take_output("result").unwrap();

            mul.push_input("a", sum).unwrap();
            mul.push_input("b", Box::new(4.0f64)).unwrap();
            mul.take_output("result")
                .map(|r| format!("{}", *r.downcast::<f64>().unwrap()))
                .unwrap_or_else(|| "error".to_string())
        };

        // Chain 2: (10 - 3) > 5 = true
        let chain2_result = {
            let mut sub = system::Subtract::new();
            let mut gt = system::GreaterThan::new();
            sub.play();
            gt.play();

            sub.push_input("a", Box::new(10.0f64)).unwrap();
            sub.push_input("b", Box::new(3.0f64)).unwrap();
            let diff = sub.take_output("result").unwrap();

            gt.push_input("a", diff).unwrap();
            gt.push_input("b", Box::new(5.0f64)).unwrap();
            gt.take_output("result")
                .map(|r| format!("{}", *r.downcast::<bool>().unwrap()))
                .unwrap_or_else(|| "error".to_string())
        };

        // Chain 3: Select based on comparison: (5 < 10) ? 100 : 200 = 100
        let chain3_result = {
            let mut lt = system::LessThan::new();
            let mut sel = system::Select::new();
            lt.play();
            sel.play();

            lt.push_input("a", Box::new(5.0f64)).unwrap();
            lt.push_input("b", Box::new(10.0f64)).unwrap();
            let cond = lt.take_output("result").unwrap();

            sel.push_input("condition", cond).unwrap();
            sel.push_input("a", Box::new(100.0f64)).unwrap();
            sel.push_input("b", Box::new(200.0f64)).unwrap();
            sel.take_output("result")
                .map(|r| format!("{}", *r.downcast::<f64>().unwrap()))
                .unwrap_or_else(|| "error".to_string())
        };

        // Chain 4: Complex math: ((10 / 2) + 3) * -1 = -8
        let chain4_result = {
            let mut div = system::Divide::new();
            let mut add = system::Add::new();
            let mut neg = system::Negate::new();
            div.play();
            add.play();
            neg.play();

            div.push_input("a", Box::new(10.0f64)).unwrap();
            div.push_input("b", Box::new(2.0f64)).unwrap();
            let quotient = div.take_output("result").unwrap();

            add.push_input("a", quotient).unwrap();
            add.push_input("b", Box::new(3.0f64)).unwrap();
            let sum = add.take_output("result").unwrap();

            neg.push_input("x", sum).unwrap();
            neg.take_output("result")
                .map(|r| format!("{}", *r.downcast::<f64>().unwrap()))
                .unwrap_or_else(|| "error".to_string())
        };

        // Chain 5: Logic chain: (true AND false) OR true = true
        let chain5_result = {
            let mut and = system::And::new();
            let mut or = system::Or::new();
            and.play();
            or.play();

            and.push_input("a", Box::new(true)).unwrap();
            and.push_input("b", Box::new(false)).unwrap();
            let and_result = and.take_output("result").unwrap();

            or.push_input("a", and_result).unwrap();
            or.push_input("b", Box::new(true)).unwrap();
            or.take_output("result")
                .map(|r| format!("{}", *r.downcast::<bool>().unwrap()))
                .unwrap_or_else(|| "error".to_string())
        };

        // Chain 6: Gate with comparison: Gate(5 > 3, 42) = 42
        let chain6_result = {
            let mut gt = system::GreaterThan::new();
            let mut gate = system::Gate::new();
            gt.play();
            gate.play();

            gt.push_input("a", Box::new(5.0f64)).unwrap();
            gt.push_input("b", Box::new(3.0f64)).unwrap();
            let enable = gt.take_output("result").unwrap();

            gate.push_input("enable", enable).unwrap();
            gate.push_input("value", Box::new(42.0f64)).unwrap();
            gate.take_output("result")
                .map(|r| format!("{}", *r.downcast::<f64>().unwrap()))
                .unwrap_or_else(|| "no output (gated)".to_string())
        };

        Story::container()
            .child(Story::title("Unit Chains"))
            .child(Story::description("Connecting units together to build complex computations"))
            .child(
                Story::section()
                    .child(Story::section_title("Arithmetic Chains"))
                    .child(Story::description("Math operations flowing through multiple units"))
                    .child(
                        div().flex().flex_col().gap(px(12.0))
                            .child(chain_row(
                                "Add -> Multiply",
                                "(2 + 3) * 4",
                                &chain1_result,
                                "20"
                            ))
                            .child(chain_row(
                                "Divide -> Add -> Negate",
                                "((10 / 2) + 3) * -1",
                                &chain4_result,
                                "-8"
                            ))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Mixed Type Chains"))
                    .child(Story::description("Arithmetic flowing into comparisons or logic"))
                    .child(
                        div().flex().flex_col().gap(px(12.0))
                            .child(chain_row(
                                "Subtract -> GreaterThan",
                                "(10 - 3) > 5",
                                &chain2_result,
                                "true"
                            ))
                            .child(chain_row(
                                "LessThan -> Select",
                                "(5 < 10) ? 100 : 200",
                                &chain3_result,
                                "100"
                            ))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Logic Chains"))
                    .child(Story::description("Boolean operations chained together"))
                    .child(
                        div().flex().flex_col().gap(px(12.0))
                            .child(chain_row(
                                "And -> Or",
                                "(true AND false) OR true",
                                &chain5_result,
                                "true"
                            ))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Control Flow Chains"))
                    .child(Story::description("Using comparisons to control data flow"))
                    .child(
                        div().flex().flex_col().gap(px(12.0))
                            .child(chain_row(
                                "GreaterThan -> Gate",
                                "Gate(5 > 3, 42)",
                                &chain6_result,
                                "42"
                            ))
                    )
            )
    }
}

fn chain_row(chain_name: &str, expression: &str, actual: &str, expected: &str) -> gpui::Div {
    let passed = actual == expected;
    let status_color = if passed {
        hsla(0.33, 0.8, 0.5, 1.0)
    } else {
        hsla(0.0, 0.8, 0.5, 1.0)
    };

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(12.0))
        .bg(hsla(0.0, 0.0, 0.15, 1.0))
        .rounded(px(6.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(hsla(0.55, 0.6, 0.7, 1.0))
                        .child(chain_name.to_string())
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(status_color)
                        .child(format!("= {}", actual))
                )
        )
        .child(
            div()
                .text_sm()
                .font_family("Berkeley Mono")
                .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                .child(expression.to_string())
        )
}
