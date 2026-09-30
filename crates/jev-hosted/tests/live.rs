//! One judgment from the deployed hosted decision service. Opt in:
//! `cargo test -p jev-hosted --test live -- --ignored --nocapture`, with no
//! TypeSafe key in the environment (a temporary `HOME` keeps the decision
//! key it makes out of the real one).

#[tokio::test]
#[ignore = "reaches the deployed decision worker on wss://relay.openagents.com"]
async fn live_hosted_decision_answers() {
    let home = std::env::var_os("HOME").expect("HOME");
    let dir = std::path::PathBuf::from(home).join(".openagents");
    let env = |name: &str| std::env::var(name).ok();
    assert!(
        jev_hosted::local_key(&env, &dir).is_none(),
        "run this with no TypeSafe key, so the hosted service answers"
    );
    let resolved = jev_hosted::resolve(
        &env,
        &dir,
        &jev_hosted::Door {
            url: jev_hosted::DOOR,
            model: "jev-1.13.0",
        },
        &|config| config,
    )
    .expect("the hosted service resolves");
    println!("via: {}", resolved.via);
    let started = std::time::Instant::now();
    let response = resolved
        .client
        .system_one(jev::SystemOneRequest::new(
            "cargo test ran 3 tests: 3 passed, 0 failed.",
            jev::Questions::new().with("passed", jev::Noul::new("Did every test pass?")),
        ))
        .await
        .expect("the hosted decision service answers");
    println!(
        "model {} passed={:.3} service={} input_tokens={:?} in {} ms",
        response.model,
        response.noul("passed").unwrap().noul,
        response.service().unwrap_or_default(),
        response.usage.input_tokens,
        started.elapsed().as_millis()
    );
    assert_eq!(response.model, "jev-1.13.0");
    assert!(response.service().is_some());
}

/// A structured decision from the deployed worker (NIP-DEC): object
/// `state`, a `choice` whose options are `what` / `not_for` / `examples`
/// rubrics, and a `noul` whose `true` and `false` are objects. Opt in the
/// same way: `cargo test -p jev-hosted --test live -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "reaches the deployed decision worker on wss://relay.openagents.com"]
async fn live_hosted_structured_decision_answers() {
    use serde_json::json;
    let home = std::env::var_os("HOME").expect("HOME");
    let dir = std::path::PathBuf::from(home).join(".openagents");
    let env = |name: &str| std::env::var(name).ok();
    assert!(
        jev_hosted::local_key(&env, &dir).is_none(),
        "run this with no TypeSafe key, so the hosted service answers"
    );
    let resolved = jev_hosted::resolve(
        &env,
        &dir,
        &jev_hosted::Door {
            url: jev_hosted::DOOR,
            model: "jev-1.13.0",
        },
        &|config| config,
    )
    .expect("the hosted service resolves");
    let state = jev::Entry::from(json!({
        "channel": "support email",
        "message": "I was charged twice for my March invoice. Please send the extra $49 back to my card.",
        "account": {"plan": "team", "seats": 12}
    }));
    let choice = jev::Choice::new(
        json!({
            "question": "Which queue should handle this message?",
            "focus": "the customer's request, not their tone"
        }),
        Default::default(),
    )
    .option(
        "billing",
        json!({
            "what": "Charges, invoices, refunds, and payment methods.",
            "not_for": "Questions about plan features or seat limits.",
            "examples": ["I was billed twice", "Update my card"]
        }),
    )
    .option(
        "technical",
        json!({
            "what": "Something in the product is broken or behaves unexpectedly.",
            "not_for": "Money questions, even when a bug caused them.",
            "examples": ["The export button does nothing", "Login loops"]
        }),
    )
    .option(
        "sales",
        json!({
            "what": "Buying more, upgrading, or pricing for a new purchase.",
            "not_for": "Disputes about money already paid.",
            "examples": ["Can we add 20 seats?", "Do you have an annual plan?"]
        }),
    );
    let noul = jev::Noul::with_criteria(
        "Does the customer ask for money back?",
        jev::NoulCriteria::new()
            .when_true(json!({
                "what": "The message asks for a refund, credit, or reversal of a charge.",
                "examples": ["Please refund the duplicate charge", "Credit my account"]
            }))
            .when_false(json!({
                "what": "The message asks for anything else, including explanations of a charge.",
                "examples": ["Why was I charged $49?", "Send me the invoice PDF"]
            })),
    );
    let started = std::time::Instant::now();
    let response = resolved
        .client
        .system_one(jev::SystemOneRequest::new(
            state,
            jev::Questions::new()
                .with("queue", choice)
                .with("refund", noul),
        ))
        .await
        .expect("the hosted decision service answers a structured request");
    let queue = response.choice("queue").expect("a choice answer");
    let refund = response.noul("refund").expect("a noul answer");
    println!(
        "model {} queue={} confidence={:.3} probabilities={:?} refund={:.3} service={} input_tokens={:?} in {} ms",
        response.model,
        queue.choice,
        queue.confidence,
        queue.probabilities,
        refund.noul,
        response.service().unwrap_or_default(),
        response.usage.input_tokens,
        started.elapsed().as_millis()
    );
    assert_eq!(queue.choice, "billing");
    assert!(refund.noul > 0.5);
}
