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
