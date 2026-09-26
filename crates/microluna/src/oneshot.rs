//! One request whose answer is one call to one declared tool.
//!
//! Microcoder's step generator and the knowledge base's harvest both ask
//! the model for a single structured value the same way: declare one
//! strict function tool whose parameters are the value's schema, send one
//! request, and read the call's arguments. [`call`] does that on any
//! [`Transport`], retries a transient failure, and prices the reply at list
//! price.
//!
//! # Cost
//!
//! The Codex login reports tokens, not dollars, so the cost is a
//! [`Basis::ListPrice`] estimate. It is known when every attempt either
//! reported its usage or sent nothing:
//!
//! - A reply, or a response that failed or stopped short but reported its
//!   usage, costs that usage at list price.
//! - A request refused before any generation (the login, a connection that
//!   was never made, or an HTTP error status) costs nothing.
//!
//! It is unknown, never zero, when the model has no list price, or when an
//! attempt failed after the request went out without reporting usage (a
//! broken stream, a timeout, or a failed or incomplete response with no
//! usage): that attempt may have consumed tokens no reply reported. On a
//! priced model each such attempt is bounded by [`price::upper_bound`]:
//! the request's bytes as input tokens, plus the model's output cap. The
//! call's [`Called::usd_upper`] is the known part plus those bounds.

use std::time::{Duration, Instant};

use crate::price::{self, Basis};
use crate::transport::{Request, TokenUsage, Transport, TransportError};

/// Retries after a transient failure, at most.
pub const RETRIES: u32 = 3;

/// One tool call's outcome.
#[derive(Clone, Debug)]
pub struct Called {
    /// The call's arguments as the model wrote them, or why there are none.
    pub arguments: Result<String, String>,
    /// The model that answered, or the requested one when none did.
    pub model: String,
    /// The usage of the reply that arrived, if one did.
    pub usage: TokenUsage,
    /// The list-price cost, or `None` when it is unknown.
    pub usd: Option<f64>,
    /// The part of the cost that is known: equal to `usd` when that is
    /// known, and otherwise a lower bound.
    pub known_usd: f64,
    /// Why `usd` is unknown, when it is, with each attempt's bound.
    pub cost_unknown: Option<String>,
    /// The most the call could have cost: `usd` when that is known, else
    /// `known_usd` plus the bound of each attempt that may have consumed
    /// unreported tokens, or `None` when there is no bound (an unpriced
    /// model).
    pub usd_upper: Option<f64>,
    /// How the cost was reached.
    pub basis: Basis,
    pub milliseconds: u64,
}

/// Whether a failed attempt may have consumed tokens that no usage
/// reported: the request went out and generation may have started. A
/// failure that reported its usage ([`TransportError::Reported`]) is priced
/// instead, and one that sent nothing costs nothing.
fn may_have_spent(error: &TransportError) -> bool {
    matches!(
        error,
        TransportError::Stream(_) | TransportError::Failed(_) | TransportError::Incomplete(_)
    )
}

/// Sends `request`, retrying a transient failure up to [`RETRIES`] times
/// with `2^n` seconds between attempts, and returns the arguments of the
/// reply's first call to `tool`.
pub async fn call<T: Transport>(transport: &T, request: &Request, tool: &str) -> Called {
    call_with(transport, request, tool, Duration::from_secs(1)).await
}

/// [`call`] with `unit` in place of one second between retries.
pub async fn call_with<T: Transport>(
    transport: &T,
    request: &Request,
    tool: &str,
    unit: Duration,
) -> Called {
    let started = Instant::now();
    let mut attempt = 0u32;
    // Failed attempts that may have consumed tokens no usage reported.
    let mut spent: Vec<String> = Vec::new();
    // Usage that failed attempts reported.
    let mut reported = TokenUsage::default();
    let mut reported_priced = Some(0.0);
    let bound = price::upper_bound(&request.model, request.text_bytes(), None);
    let reply = loop {
        match transport.respond(request).await {
            Ok(reply) => break Ok(reply),
            Err(error) => {
                if let TransportError::Reported { usage, .. } = &error {
                    reported.add(*usage);
                    reported_priced = reported_priced
                        .zip(price::cost(&request.model, *usage))
                        .map(|(a, b)| a + b);
                } else if may_have_spent(&error) {
                    spent.push(format!(
                        "attempt {}: {error}{}",
                        attempt + 1,
                        bound.map_or(String::new(), |b| format!(" (at most ${b:.6})"))
                    ));
                }
                if error.transient() && attempt < RETRIES {
                    attempt += 1;
                    tokio::time::sleep(unit * 2u32.pow(attempt)).await;
                    continue;
                }
                break Err(error.to_string());
            }
        }
    };
    let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let (arguments, model, mut usage) = match reply {
        Ok(reply) => {
            let model = if reply.model.is_empty() {
                request.model.clone()
            } else {
                reply.model.clone()
            };
            let arguments = reply
                .calls()
                .into_iter()
                .find(|c| c.name == tool)
                .map(|c| c.arguments)
                .ok_or_else(|| {
                    format!(
                        "the reply called no {tool} tool; it said: {}",
                        reply.text().chars().take(300).collect::<String>()
                    )
                });
            (arguments, model, reply.usage)
        }
        Err(error) => (Err(error), request.model.clone(), TokenUsage::default()),
    };
    let priced = price::cost(&request.model, usage)
        .zip(reported_priced)
        .map(|(a, b)| a + b);
    usage.add(reported);
    let known_usd = priced.unwrap_or(0.0);
    let cost_unknown = if !spent.is_empty() {
        Some(format!(
            "{} failed after the request was sent and may have consumed tokens no reply reported ({})",
            if spent.len() == 1 {
                "an attempt".to_string()
            } else {
                format!("{} attempts", spent.len())
            },
            spent.join("; ")
        ))
    } else if priced.is_none() {
        Some(format!("{} has no known list price", request.model))
    } else {
        None
    };
    let usd = if cost_unknown.is_none() { priced } else { None };
    let usd_upper = match usd {
        Some(usd) => Some(usd),
        None => priced
            .zip(bound)
            .map(|(known, bound)| known + bound * spent.len() as f64),
    };
    Called {
        arguments,
        model,
        usage,
        usd,
        known_usd,
        cost_unknown,
        usd_upper,
        basis: Basis::ListPrice,
        milliseconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeTransport;
    use crate::transport::Reply;
    use serde_json::json;

    fn request(model: &str) -> Request {
        Request {
            model: model.to_string(),
            instructions: String::new(),
            input: Vec::new(),
            tools: Vec::new(),
            effort: None,
            cache_key: String::new(),
            parallel_tools: false,
        }
    }

    fn reply(tool: &str) -> Reply {
        Reply {
            id: None,
            model: "gpt-6-luna".to_string(),
            items: vec![
                json!({"type": "function_call", "call_id": "c", "name": tool, "arguments": "{}"}),
            ],
            usage: TokenUsage {
                input: 1_000_000,
                cached: 0,
                output: 0,
                reasoning: 0,
            },
        }
    }

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn a_priced_reply_costs_its_list_price() {
        let t = FakeTransport::new(vec![reply("go")]);
        let out = run(call(&t, &request("gpt-6-luna"), "go"));
        assert_eq!(out.arguments.unwrap(), "{}");
        assert_eq!(out.usd, Some(0.1));
        assert_eq!(out.basis, Basis::ListPrice);
        assert!(out.cost_unknown.is_none());
    }

    #[test]
    fn an_unpriced_model_is_unknown_not_zero() {
        let t = FakeTransport::new(vec![reply("go")]);
        let out = run(call(&t, &request("gpt-9-mystery"), "go"));
        assert_eq!(out.usd, None);
        assert!(out.cost_unknown.unwrap().contains("no known list price"));
    }

    #[test]
    fn a_failed_response_is_unknown_not_zero() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Failed("server_error".to_string()));
        let out = run(call(&t, &request("gpt-6-luna"), "go"));
        assert!(out.arguments.is_err());
        assert_eq!(out.usd, None);
        assert_eq!(out.known_usd, 0.0);
        assert!(out.cost_unknown.unwrap().contains("may have consumed"));
    }

    #[test]
    fn a_refused_request_costs_nothing() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Http {
            status: 400,
            body: "bad".to_string(),
        });
        let out = run(call(&t, &request("gpt-6-luna"), "go"));
        assert!(out.arguments.is_err());
        assert_eq!(out.usd, Some(0.0));
    }

    #[test]
    fn a_broken_stream_before_a_reply_leaves_a_lower_bound() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Stream("reset".to_string()));
        t.then(reply("go"));
        let out = run(call_with(
            &t,
            &request("gpt-6-luna"),
            "go",
            Duration::from_millis(1),
        ));
        assert_eq!(out.arguments.unwrap(), "{}");
        assert_eq!(out.usd, None);
        assert!((out.known_usd - 0.1).abs() < 1e-12);
        assert!(out.cost_unknown.unwrap().contains("attempt 1"));
    }

    #[test]
    fn a_failed_response_is_bounded_by_its_request_and_the_output_cap() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Failed("server_error".to_string()));
        let mut req = request("gpt-6-luna");
        req.instructions = "x".repeat(10_000);
        let bound = crate::price::upper_bound("gpt-6-luna", req.text_bytes(), None).unwrap();
        let out = run(call(&t, &req, "go"));
        assert_eq!(out.usd, None);
        assert_eq!(out.usd_upper, Some(bound));
        assert!(bound > 0.064, "the whole output cap is counted: {bound}");
        assert!(out.cost_unknown.unwrap().contains("at most $"));
    }

    #[test]
    fn a_retried_timeout_adds_its_bound_to_the_reply_it_got() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Stream("operation timed out".to_string()));
        t.then(reply("go"));
        let req = request("gpt-6-luna");
        let bound = crate::price::upper_bound("gpt-6-luna", req.text_bytes(), None).unwrap();
        let out = run(call_with(&t, &req, "go", Duration::from_millis(1)));
        assert_eq!(out.usd, None);
        assert!((out.known_usd - 0.1).abs() < 1e-12);
        assert!((out.usd_upper.unwrap() - (0.1 + bound)).abs() < 1e-12);
    }

    #[test]
    fn an_unsent_request_costs_nothing_and_is_retried() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Unsent("connection refused".to_string()));
        t.then(reply("go"));
        let out = run(call_with(
            &t,
            &request("gpt-6-luna"),
            "go",
            Duration::from_millis(1),
        ));
        assert_eq!(out.usd, Some(0.1));
        assert_eq!(out.usd_upper, Some(0.1));
        assert!(out.cost_unknown.is_none());
    }

    #[test]
    fn a_failure_that_reported_usage_is_priced_not_unknown() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Reported {
            error: Box::new(TransportError::Failed("server_error".to_string())),
            usage: TokenUsage {
                input: 1_000_000,
                cached: 0,
                output: 0,
                reasoning: 0,
            },
        });
        let out = run(call(&t, &request("gpt-6-luna"), "go"));
        assert!(out.arguments.is_err());
        assert_eq!(out.usd, Some(0.1));
        assert_eq!(out.usage.input, 1_000_000);
        assert!(out.cost_unknown.is_none());
    }

    #[test]
    fn an_unpriced_model_has_no_bound() {
        let t = FakeTransport::new(Vec::new());
        t.then_fail(TransportError::Failed("server_error".to_string()));
        let out = run(call(&t, &request("gpt-9-mystery"), "go"));
        assert_eq!(out.usd, None);
        assert_eq!(out.usd_upper, None);
    }

    #[test]
    fn a_reply_without_the_tool_says_what_it_said() {
        let mut r = reply("other");
        r.items =
            vec![json!({"type": "message", "content": [{"type": "output_text", "text": "hi"}]})];
        let t = FakeTransport::new(vec![r]);
        let out = run(call(&t, &request("gpt-6-luna"), "go"));
        assert!(out.arguments.unwrap_err().contains("called no go tool"));
        assert_eq!(out.usd, Some(0.1));
    }
}
