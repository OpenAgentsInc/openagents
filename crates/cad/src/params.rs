use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

/// Typed scalar units supported by the CAD parameter store.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ScalarUnit {
    #[serde(rename = "mm")]
    Millimeter,
    #[serde(rename = "deg")]
    Degree,
    #[serde(rename = "unitless")]
    Unitless,
}

impl ScalarUnit {
    pub fn parse(value: &str) -> CadResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "mm" => Ok(Self::Millimeter),
            "deg" | "degree" | "degrees" => Ok(Self::Degree),
            "unitless" | "scalar" => Ok(Self::Unitless),
            other => Err(CadError::InvalidParameter {
                name: "unit".to_string(),
                reason: format!("unsupported scalar unit '{other}'"),
            }),
        }
    }
}

/// Scalar CAD parameter with explicit unit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScalarValue {
    pub value: f64,
    pub unit: ScalarUnit,
}

impl ScalarValue {
    pub fn validate(self, name: &str) -> CadResult<()> {
        if !self.value.is_finite() {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: "value must be finite".to_string(),
            });
        }
        Ok(())
    }
}

/// Deterministic CAD parameter store backed by sorted keys.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterStore {
    pub values: BTreeMap<String, ScalarValue>,
}

impl ParameterStore {
    pub fn set(&mut self, name: impl Into<String>, value: ScalarValue) -> CadResult<()> {
        let name = name.into();
        validate_parameter_name(&name)?;
        value.validate(&name)?;
        self.values.insert(name, value);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<ScalarValue> {
        self.values.get(name).copied()
    }

    pub fn get_required_with_unit(&self, name: &str, unit: ScalarUnit) -> CadResult<f64> {
        let Some(parameter) = self.values.get(name) else {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: "parameter not found".to_string(),
            });
        };
        if parameter.unit != unit {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: format!(
                    "unit mismatch: expected {:?}, got {:?}",
                    unit, parameter.unit
                ),
            });
        }
        Ok(parameter.value)
    }
}

/// Deterministic parameter-expression store with named expression payloads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterExpressionStore {
    pub expressions: BTreeMap<String, String>,
}

impl ParameterExpressionStore {
    pub fn set_expression(
        &mut self,
        name: impl Into<String>,
        expression: impl Into<String>,
    ) -> CadResult<()> {
        let name = name.into();
        validate_parameter_name(&name)?;
        let expression = expression.into();
        if expression.trim().is_empty() {
            return Err(CadError::InvalidParameter {
                name,
                reason: "expression must not be empty".to_string(),
            });
        }
        parse_expression(&expression)?;
        self.expressions.insert(name, expression);
        Ok(())
    }

    /// Evaluate a parameter expression with deterministic cycle detection.
    ///
    /// Resolution order:
    /// 1. expression store entry (if present)
    /// 2. base parameter from `ParameterStore`
    pub fn evaluate(
        &self,
        name: &str,
        params: &ParameterStore,
        target_unit: ScalarUnit,
    ) -> CadResult<f64> {
        let mut visiting = Vec::<String>::new();
        self.evaluate_inner(name, params, target_unit, &mut visiting)
    }

    fn evaluate_inner(
        &self,
        name: &str,
        params: &ParameterStore,
        target_unit: ScalarUnit,
        visiting: &mut Vec<String>,
    ) -> CadResult<f64> {
        if visiting.iter().any(|entry| entry == name) {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: format!("expression cycle detected: {}", visiting.join(" -> ")),
            });
        }

        if let Some(expression) = self.expressions.get(name) {
            visiting.push(name.to_string());
            let ast = parse_expression(expression)?;
            let mut resolver =
                |ref_name: &str| self.evaluate_inner(ref_name, params, target_unit, visiting);
            let value = eval_ast(&ast, &mut resolver)?;
            let _ = visiting.pop();
            return Ok(value);
        }

        let Some(parameter) = params.get(name) else {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: "parameter not found".to_string(),
            });
        };

        if parameter.unit != target_unit && parameter.unit != ScalarUnit::Unitless {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: format!(
                    "unit mismatch in expression reference: expected {:?}, got {:?}",
                    target_unit, parameter.unit
                ),
            });
        }

        Ok(parameter.value)
    }
}

fn validate_parameter_name(name: &str) -> CadResult<()> {
    if name.is_empty() {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name must not be empty".to_string(),
        });
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name must not be empty".to_string(),
        });
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name must start with ascii letter or '_'".to_string(),
        });
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')) {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name contains unsupported characters".to_string(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
enum ExprNode {
    Literal(f64),
    Reference(String),
    UnaryMinus(Box<ExprNode>),
    Add(Box<ExprNode>, Box<ExprNode>),
    Sub(Box<ExprNode>, Box<ExprNode>),
    Mul(Box<ExprNode>, Box<ExprNode>),
    Div(Box<ExprNode>, Box<ExprNode>),
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Identifier(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn parse_expression(input: &str) -> CadResult<ExprNode> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let expression = parser.parse_expr()?;
    if !parser.is_eof() {
        return Err(CadError::ParseFailed {
            reason: "unexpected trailing expression tokens".to_string(),
        });
    }
    Ok(expression)
}

fn tokenize(input: &str) -> CadResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        let ch = chars[idx];
        if ch.is_ascii_whitespace() {
            idx += 1;
            continue;
        }
        match ch {
            '+' => {
                tokens.push(Token::Plus);
                idx += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                idx += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                idx += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                idx += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                idx += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                idx += 1;
            }
            _ if ch.is_ascii_digit() || ch == '.' => {
                let start = idx;
                idx += 1;
                while idx < chars.len() && (chars[idx].is_ascii_digit() || chars[idx] == '.') {
                    idx += 1;
                }
                let slice: String = chars[start..idx].iter().collect();
                let value = slice
                    .parse::<f64>()
                    .map_err(|error| CadError::ParseFailed {
                        reason: format!("invalid numeric literal '{slice}': {error}"),
                    })?;
                if !value.is_finite() {
                    return Err(CadError::ParseFailed {
                        reason: format!("non-finite numeric literal '{slice}'"),
                    });
                }
                tokens.push(Token::Number(value));
            }
            _ if ch.is_ascii_alphabetic() || ch == '_' => {
                let start = idx;
                idx += 1;
                while idx < chars.len()
                    && (chars[idx].is_ascii_alphanumeric() || matches!(chars[idx], '_' | '.' | '-'))
                {
                    idx += 1;
                }
                let ident: String = chars[start..idx].iter().collect();
                tokens.push(Token::Identifier(ident));
            }
            _ => {
                return Err(CadError::ParseFailed {
                    reason: format!("unsupported token '{ch}' in expression"),
                });
            }
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn is_eof(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn parse_expr(&mut self) -> CadResult<ExprNode> {
        let mut node = self.parse_term()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    let _ = self.next();
                    let rhs = self.parse_term()?;
                    node = ExprNode::Add(Box::new(node), Box::new(rhs));
                }
                Some(Token::Minus) => {
                    let _ = self.next();
                    let rhs = self.parse_term()?;
                    node = ExprNode::Sub(Box::new(node), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_term(&mut self) -> CadResult<ExprNode> {
        let mut node = self.parse_factor()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    let _ = self.next();
                    let rhs = self.parse_factor()?;
                    node = ExprNode::Mul(Box::new(node), Box::new(rhs));
                }
                Some(Token::Slash) => {
                    let _ = self.next();
                    let rhs = self.parse_factor()?;
                    node = ExprNode::Div(Box::new(node), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_factor(&mut self) -> CadResult<ExprNode> {
        match self.next() {
            Some(Token::Number(value)) => Ok(ExprNode::Literal(value)),
            Some(Token::Identifier(name)) => Ok(ExprNode::Reference(name)),
            Some(Token::Minus) => {
                let inner = self.parse_factor()?;
                Ok(ExprNode::UnaryMinus(Box::new(inner)))
            }
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                match self.next() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err(CadError::ParseFailed {
                        reason: "missing ')' in expression".to_string(),
                    }),
                }
            }
            token => Err(CadError::ParseFailed {
                reason: format!("unexpected token in expression: {:?}", token),
            }),
        }
    }
}

fn eval_ast(
    node: &ExprNode,
    resolve_ref: &mut impl FnMut(&str) -> CadResult<f64>,
) -> CadResult<f64> {
    match node {
        ExprNode::Literal(value) => Ok(*value),
        ExprNode::Reference(name) => resolve_ref(name),
        ExprNode::UnaryMinus(inner) => Ok(-eval_ast(inner, resolve_ref)?),
        ExprNode::Add(left, right) => {
            Ok(eval_ast(left, resolve_ref)? + eval_ast(right, resolve_ref)?)
        }
        ExprNode::Sub(left, right) => {
            Ok(eval_ast(left, resolve_ref)? - eval_ast(right, resolve_ref)?)
        }
        ExprNode::Mul(left, right) => {
            Ok(eval_ast(left, resolve_ref)? * eval_ast(right, resolve_ref)?)
        }
        ExprNode::Div(left, right) => {
            let divisor = eval_ast(right, resolve_ref)?;
            if divisor.abs() <= f64::EPSILON {
                return Err(CadError::InvalidParameter {
                    name: "expression".to_string(),
                    reason: "divide by zero in parameter expression".to_string(),
                });
            }
            Ok(eval_ast(left, resolve_ref)? / divisor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ParameterExpressionStore, ParameterStore, ScalarUnit, ScalarValue};

    #[test]
    fn setting_and_getting_valid_parameter_is_deterministic() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "thickness_mm",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Millimeter,
            },
        );
        assert!(set.is_ok(), "valid parameter should set successfully");
        assert_eq!(
            params.get("thickness_mm"),
            Some(ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Millimeter
            })
        );
    }

    #[test]
    fn invalid_parameter_name_is_rejected() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "9thickness",
            ScalarValue {
                value: 3.0,
                unit: ScalarUnit::Millimeter,
            },
        );
        assert!(set.is_err(), "invalid parameter name should be rejected");
    }

    #[test]
    fn invalid_parameter_value_is_rejected() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "thickness_mm",
            ScalarValue {
                value: f64::NAN,
                unit: ScalarUnit::Millimeter,
            },
        );
        assert!(set.is_err(), "non-finite values should be rejected");
    }

    #[test]
    fn invalid_unit_string_is_rejected() {
        let parsed = ScalarUnit::parse("meters");
        assert!(parsed.is_err(), "unsupported unit should be rejected");
    }

    #[test]
    fn unit_mismatch_is_rejected() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "draft_angle",
            ScalarValue {
                value: 5.0,
                unit: ScalarUnit::Degree,
            },
        );
        assert!(set.is_ok(), "parameter setup should succeed");

        let value = params.get_required_with_unit("draft_angle", ScalarUnit::Millimeter);
        assert!(value.is_err(), "unit mismatch should be rejected");
    }

    #[test]
    fn expression_evaluation_supports_arithmetic_and_refs() {
        let mut params = ParameterStore::default();
        params
            .set(
                "width_mm",
                ScalarValue {
                    value: 100.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("width should set");
        params
            .set(
                "clearance_mm",
                ScalarValue {
                    value: 2.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("clearance should set");

        let mut expressions = ParameterExpressionStore::default();
        expressions
            .set_expression("slot_width_mm", "width_mm - clearance_mm * 2")
            .expect("expression should parse");

        let value = expressions
            .evaluate("slot_width_mm", &params, ScalarUnit::Millimeter)
            .expect("expression should evaluate");
        assert!((value - 96.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn expression_cycle_is_rejected() {
        let params = ParameterStore::default();
        let mut expressions = ParameterExpressionStore::default();
        expressions
            .set_expression("a_mm", "b_mm + 1")
            .expect("expression should set");
        expressions
            .set_expression("b_mm", "a_mm + 1")
            .expect("expression should set");

        let value = expressions.evaluate("a_mm", &params, ScalarUnit::Millimeter);
        assert!(
            value.is_err(),
            "cyclic expression dependencies must be rejected"
        );
    }

    #[test]
    fn expression_divide_by_zero_is_rejected() {
        let mut params = ParameterStore::default();
        params
            .set(
                "x",
                ScalarValue {
                    value: 5.0,
                    unit: ScalarUnit::Unitless,
                },
            )
            .expect("x should set");
        let mut expressions = ParameterExpressionStore::default();
        expressions
            .set_expression("boom", "10 / (x - x)")
            .expect("expression should parse");

        let value = expressions.evaluate("boom", &params, ScalarUnit::Unitless);
        assert!(value.is_err(), "divide-by-zero must be rejected");
    }
}
