use std::iter::Peekable;
use std::str::Chars;

use wgpui::{Bounds, Component, InputEvent, PaintContext, Point, theme};

use crate::app_state::CalculatorPaneInputs;
use crate::app_state::{PaneKind, RenderState};
use crate::pane_system::{calculator_expression_input_bounds, pane_content_bounds};

pub fn paint(content_bounds: Bounds, inputs: &mut CalculatorPaneInputs, paint: &mut PaintContext) {
    let input_bounds = calculator_expression_input_bounds(content_bounds);
    inputs
        .expression
        .set_max_width(input_bounds.size.width.max(80.0));
    inputs.expression.paint(input_bounds, paint);

    paint.scene.draw_text(paint.text.layout(
        "Expression",
        Point::new(input_bounds.origin.x, input_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let expression = inputs.expression.get_value();
    let (result_label, result_color) = if expression.trim().is_empty() {
        (
            "Enter an expression to calculate.".to_string(),
            theme::text::MUTED,
        )
    } else {
        match evaluate_expression(expression) {
            Ok(value) => (format_result(value), theme::text::PRIMARY),
            Err(error) => (format!("Error: {error}"), theme::status::ERROR),
        }
    };

    let result_y = input_bounds.max_y() + 18.0;
    paint.scene.draw_text(paint.text.layout(
        "Result",
        Point::new(input_bounds.origin.x, result_y),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &result_label,
        Point::new(input_bounds.origin.x, result_y + 14.0),
        12.0,
        result_color,
    ));

    let hint_y = result_y + 40.0;
    paint.scene.draw_text(paint.text.layout(
        "Supports +, -, *, /, and parentheses.",
        Point::new(input_bounds.origin.x, hint_y),
        10.0,
        theme::text::MUTED,
    ));
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::Calculator)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    let input_bounds = calculator_expression_input_bounds(pane_content_bounds(bounds));
    state
        .calculator_inputs
        .expression
        .event(event, input_bounds, &mut state.event_context)
        .is_handled()
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn precedence(self) -> u8 {
        match self {
            Op::Add | Op::Sub => 1,
            Op::Mul | Op::Div => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Token {
    Number(f64),
    Op(Op),
    LParen,
    RParen,
}

fn format_result(value: f64) -> String {
    let mut formatted = format!("{value:.6}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    formatted
}

fn evaluate_expression(expression: &str) -> Result<f64, String> {
    let tokens = tokenize_expression(expression)?;
    if tokens.is_empty() {
        return Err("expression is empty".to_string());
    }

    let mut values: Vec<f64> = Vec::new();
    let mut ops: Vec<Token> = Vec::new();

    for token in tokens {
        match token {
            Token::Number(value) => values.push(value),
            Token::Op(op) => {
                while let Some(Token::Op(top)) = ops.last().copied() {
                    if top.precedence() >= op.precedence() {
                        ops.pop();
                        apply_op(&mut values, top)?;
                    } else {
                        break;
                    }
                }
                ops.push(Token::Op(op));
            }
            Token::LParen => ops.push(Token::LParen),
            Token::RParen => {
                let mut matched = false;
                while let Some(top) = ops.pop() {
                    match top {
                        Token::Op(op) => apply_op(&mut values, op)?,
                        Token::LParen => {
                            matched = true;
                            break;
                        }
                        Token::RParen | Token::Number(_) => {}
                    }
                }
                if !matched {
                    return Err("mismatched parentheses".to_string());
                }
            }
        }
    }

    while let Some(token) = ops.pop() {
        match token {
            Token::Op(op) => apply_op(&mut values, op)?,
            Token::LParen | Token::RParen => {
                return Err("mismatched parentheses".to_string());
            }
            Token::Number(_) => {}
        }
    }

    if values.len() == 1 {
        Ok(values[0])
    } else {
        Err("invalid expression".to_string())
    }
}

fn tokenize_expression(expression: &str) -> Result<Vec<Token>, String> {
    let mut chars = expression.chars().peekable();
    let mut tokens = Vec::new();
    let mut expect_number = true;

    while let Some(ch) = chars.peek().copied() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        match ch {
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
                expect_number = true;
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
                expect_number = false;
            }
            '+' | '-' => {
                if expect_number {
                    let sign = chars.next().unwrap_or('+');
                    skip_whitespace(&mut chars);
                    if chars.peek().copied() == Some('(') {
                        tokens.push(Token::Number(0.0));
                        tokens.push(Token::Op(if sign == '+' { Op::Add } else { Op::Sub }));
                        expect_number = true;
                        continue;
                    }
                    let number = read_number(&mut chars, Some(sign))?;
                    tokens.push(Token::Number(number));
                    expect_number = false;
                } else {
                    tokens.push(Token::Op(if ch == '+' { Op::Add } else { Op::Sub }));
                    chars.next();
                    expect_number = true;
                }
            }
            '*' | '/' => {
                tokens.push(Token::Op(if ch == '*' { Op::Mul } else { Op::Div }));
                chars.next();
                expect_number = true;
            }
            _ => {
                if ch.is_ascii_digit() || ch == '.' {
                    let number = read_number(&mut chars, None)?;
                    tokens.push(Token::Number(number));
                    expect_number = false;
                } else {
                    return Err(format!("unexpected character '{ch}'"));
                }
            }
        }
    }

    Ok(tokens)
}

fn skip_whitespace(chars: &mut Peekable<Chars<'_>>) {
    while chars.peek().is_some_and(|value| value.is_whitespace()) {
        chars.next();
    }
}

fn read_number(chars: &mut Peekable<Chars<'_>>, leading: Option<char>) -> Result<f64, String> {
    let mut buffer = String::new();
    if let Some(ch) = leading {
        buffer.push(ch);
    }

    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_digit() || ch == '.' {
            buffer.push(ch);
            chars.next();
        } else {
            break;
        }
    }

    let trimmed = buffer.trim();
    if trimmed.is_empty()
        || trimmed == "+"
        || trimmed == "-"
        || trimmed == "."
        || trimmed == "+."
        || trimmed == "-."
    {
        return Err("expected number".to_string());
    }

    trimmed
        .parse::<f64>()
        .map_err(|_| "invalid number".to_string())
}

fn apply_op(values: &mut Vec<f64>, op: Op) -> Result<(), String> {
    if values.len() < 2 {
        return Err("invalid expression".to_string());
    }
    let right = values.pop().unwrap_or(0.0);
    let left = values.pop().unwrap_or(0.0);
    let result = match op {
        Op::Add => left + right,
        Op::Sub => left - right,
        Op::Mul => left * right,
        Op::Div => {
            if right.abs() <= f64::EPSILON {
                return Err("division by zero".to_string());
            }
            left / right
        }
    };
    values.push(result);
    Ok(())
}
