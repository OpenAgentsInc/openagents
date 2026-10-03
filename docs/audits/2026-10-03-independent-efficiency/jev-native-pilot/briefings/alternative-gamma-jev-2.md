## Prepared source spans

Source commit: `c427943a5c84ba5938a3549f24b27de551812a37`. Complete applicable instructions are supplied separately. Partial evidence and omissions are explicit; a selected span does not prove requirement or dependency coverage.

### s82 .agents/skills/typesafe-ai/SKILL.md:1-23 — .agents/skills/typesafe-ai/SKILL.md

document; partial_file. Full span: 1-149. Blob: `0109513f9656917dc93cbc5ecddfca465a53ce66`. File SHA-256: `71ea90d7906c6554c4f4c460ef7361b2d26f59116ccdae986dc6d997b9389f52`.

```text
---
name: typesafe-ai
license: MIT
description: >
  Build AI-powered software with TypeSafe: small units of AI intelligence you
  can use like programming primitives. Its System One models, including Jev,
  turn natural language and application state into typed judgments and
  probabilities that code can combine. Use when a feature needs programmable
  common sense, when brainstorming what AI could make possible in an app, or
  when an LLM prompt-and-parse step could become a structured decision.
  Applications include routing, ranking, extraction, verification, and
  interactive experiences; these are starting points, not the limits.
  Read live docs and cookbooks to find useful patterns and discover new combinations.
---

# Build with TypeSafe

TypeSafe makes units of AI intelligence usable like programming primitives: small
judgments you can compose into larger capabilities. Its **System One models** return
fast, focused judgments that software can consume directly. **Jev** is TypeSafe's
flagship and first System One model. It understands natural language and returns
typed answers and probabilities rather
than generating text or reasoning explanations. Code owns the workflow; the model
```

### s81 docs/decision-models/2026-09-20-score-contract.md:1-23 — docs/decision-models/2026-09-20-score-contract.md

document; partial_file. Full span: 1-107. Blob: `593e51099b82e2f905f0759f27122bb8ffb75418`. File SHA-256: `a8c612095723bf3599ae4d618aeff8ae18e3b5a05e868c07f26e623126e20c60`.

```text
# Score positions, selected answers, and calibration

Issues #9394 and #9419 concern two different quantities: a Score answer's
weighted position and the categorical answer used to measure correctness.
Calibration must preserve the identity of the answer whose confidence it
changes.

## Positions and categorical answers

The public `score` field remains the probability-weighted position,
`Σ i · p_i`, over ordered levels. It can fall between levels. Rounding it
is not equivalent to choosing the most probable level. For example,
`{0: 0.5, 1: 0.375, 2: 0.125}` has a weighted position of `0.625` and a
most probable level of `0`.

The contract applies consistently across doors: `score` promises a weighted
position, not a selected level or measured categorical accuracy. The repository
has not established that this position is an accurate ordinal prediction for
every checkpoint.

| Door | `score` field | Published categorical accuracy |
| --- | --- | --- |
| Lev | Mean of raw sample frequencies, or of the calibrated distribution. | Selected level; calibration preserves its identity. Historical rows without selection provenance use their documented argmax convention. |
```

### s25 crates/jev/src/answers.rs:169-198 — SystemOneResponse::decode

implementation; complete_declaration. Full span: 175-198. Blob: `11b7359e1784c0b4a1507f7524d90411cbb5dd9d`. File SHA-256: `34dde188f362e772e82e18ef75d1f62eede8cf62ba8f59ac6f9f8cd9a3d41dc0`.

```text
    /// Read one response body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ResponseValidation`] naming the first field the SDK
    /// cannot read.
    pub fn decode(raw: RawResponse) -> Result<Self> {
        // Serde reads a struct from a JSON array as well as from an object, and
        // a response is an object, so the first byte is checked here.
        if raw.bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err(validation(&raw, "body".to_string()));
        }
        let wire: Wire = serde_json::from_slice(&raw.bytes)
            .map_err(|_| validation(&raw, blame_body(&raw.bytes)))?;
        let model = wire
            .model
            .ok_or_else(|| validation(&raw, "model".to_string()))?;
        let mut answers = IndexMap::with_capacity(wire.answers.len());
        for (id, body) in &wire.answers {
            if let Some(answer) = decode_answer(&raw, id, body)? {
                answers.insert(id.clone(), answer);
            }
        }
        Ok(Self {
            model,
            answers,
            usage: wire.usage.unwrap_or_default(),
            raw,
        })
    }
```

### s51 crates/jev/src/answers.rs:303-347 — decode_answer

implementation; complete_declaration. Full span: 304-347. Blob: `11b7359e1784c0b4a1507f7524d90411cbb5dd9d`. File SHA-256: `34dde188f362e772e82e18ef75d1f62eede8cf62ba8f59ac6f9f8cd9a3d41dc0`.

```text
/// One answer, or `None` when its type is one this crate does not model.
fn decode_answer(raw: &RawResponse, id: &str, body: &RawValue) -> Result<Option<Answer>> {
    let tag: Tag = serde_json::from_str(body.get())
        .map_err(|_| validation(raw, format!("answers.{id}.type")))?;
    let kind = tag
        .r#type
        .ok_or_else(|| validation(raw, format!("answers.{id}.type")))?;
    match kind.as_str() {
        "noul" => {
            let answer: NoulAnswer = read(raw, id, body, NOUL_FIELDS)?;
            if answer
                .selected
                .as_deref()
                .is_some_and(|option| !matches!(option, "no" | "yes"))
            {
                return Err(validation(raw, format!("answers.{id}.selected")));
            }
            Ok(Some(Answer::Noul(answer)))
        }
        "choice" => read(raw, id, body, CHOICE_FIELDS).map(|answer| Some(Answer::Choice(answer))),
        "score" => {
            let answer: ScoreAnswer = read(raw, id, body, SCORE_FIELDS)?;
            if let Some(selected) = &answer.selected {
                let valid = selected.parse::<u32>().ok().is_some_and(|level| {
                    level.to_string() == *selected && answer.probabilities.contains_key(&level)
                });
                if !valid {
                    return Err(validation(raw, format!("answers.{id}.selected")));
                }
            }
            Ok(Some(Answer::Score(answer)))
        }
        other => {
            // A newer API may answer with a type this crate does not model. The
            // rest of the response still reads, and the bytes stay on `raw()`.
            tracing::warn!(
                target: "jev",
                answer = id,
                answer_type = other,
                "skipping an answer of a type this client does not read"
            );
            Ok(None)
        }
    }
}
```

### s65 crates/jev/tests/selected_answer.rs:14-24 — a_noul_cannot_name_an_option_outside_yes_and_no

test; complete_declaration. Full span: 15-24. Blob: `04dcc9de3d41af50d92bc97f5d329632618bc325`. File SHA-256: `183c1b99706243034ba9d882f1849ecb90a325f3c4a9c9fbabf0c845bcde1055`.

```text
#[test]
fn a_noul_cannot_name_an_option_outside_yes_and_no() {
    for selected in ["", "maybe", "YES"] {
        let error = decode(json!({"type":"noul","noul":0.25,"selected":selected})).unwrap_err();
        assert!(
            matches!(error, Error::ResponseValidation { field_path, .. } if field_path == "answers.q.selected")
        );
    }
    let answer = decode(json!({"type":"noul","noul":0.25,"selected":"yes"})).unwrap();
    assert_eq!(answer.noul("q").unwrap().selected.as_deref(), Some("yes"));
}
```

### s66 crates/jev/tests/answers.rs:235-304 — a_field_that_does_not_read_is_named_by_a_dotted_path

test; complete_declaration. Full span: 236-304. Blob: `11aa1e38149e1a6ce5f029603ad6238946e5ac35`. File SHA-256: `b1c2c28d1882637813d7699b29cd8656e7156c24c6f46bbca003ef9f7e4bc03e`.

```text
#[test]
fn a_field_that_does_not_read_is_named_by_a_dotted_path() -> Outcome {
    let cases = [
        (
            json!({"model": "m", "answers": {"tone": {"type": "score", "score": 1.0, "confidence": "high", "legend": {}}}}),
            "answers.tone.confidence",
        ),
        (
            json!({"model": "m", "answers": {"tone": {"type": "noul"}}}),
            "answers.tone.noul",
        ),
        (
            json!({"model": "m", "answers": {"tone": {"type": "choice", "choice": "a", "confidence": 1.0}}}),
            "answers.tone.probabilities",
        ),
        (
            json!({"model": "m", "answers": {"tone": {"noul": 0.5}}}),
            "answers.tone.type",
        ),
        (json!({"answers": {}}), "model"),
        (json!({"model": 7, "answers": {}}), "model"),
        (json!({"model": "m", "answers": 7}), "answers"),
        (json!(["not an object"]), "body"),
        (
            json!({"model": "m", "answers": {"c": "not-a-mapping"}}),
            "answers.c.type",
        ),
        (
            json!({"model": "m", "answers": {"c": {"type": "choice", "choice": "a", "probabilities": {}}}}),
            "answers.c.confidence",
        ),
        (
            json!({"model": "m", "answers": {"n": {"type": "noul", "noul": "0.5"}}}),
            "answers.n.noul",
        ),
        // A malformed map entry names the entry, inside `legend` or
        // `probabilities`.
        (
            json!({"model": "m", "answers": {"s": {"type": "score", "score": 1.0, "confidence": 1.0, "legend": [], "probabilities": {}}}}),
            "answers.s.legend",
        ),
        (
            json!({"model": "m", "answers": {"s": {"type": "score", "score": 1.0, "confidence": 1.0, "legend": {"x": "bad"}, "probabilities": {}}}}),
            "answers.s.legend.x",
        ),
        (
            json!({"model": "m", "answers": {"s": {"type": "score", "score": 1.0, "confidence": 1.0, "legend": {}, "probabilities": {"x": 0.5}}}}),
            "answers.s.probabilities.x",
        ),
        (
            json!({"model": "m", "answers": {"s": {"type": "score", "score": 1.0, "confidence": 1.0, "legend": {}, "probabilities": {"0": "high"}}}}),
            "answers.s.probabilities.0",
        ),
    ];
    for (body, expected) in cases {
        let Err(Error::ResponseValidation {
            status,
            field_path,
            request_id,
            ..
        }) = SystemOneResponse::decode(raw(&body)?)
        else {
            unreachable!("{expected} does not read");
        };
        assert_eq!(field_path, expected);
        assert_eq!(status, 200);
        assert_eq!(request_id.as_deref(), Some("req_01test"));
    }
    Ok(())
}
```

### s67 crates/jev/tests/blocking.rs:133-167 — the_blocking_call_holds_the_same_whole_call_deadline

test; complete_declaration. Full span: 137-167. Blob: `02d55d9d443bc32d1127852305e914ba2778a53c`. File SHA-256: `68dd7bf71a2f4bcd41541e39764b181725c126eabb2850cf4ebed8497b5ea09b`.

```text
/// The blocking entry point runs the same loop, so the same whole-call
/// deadline holds: a reply that arrives past the budget cannot succeed, and
/// the attempt's own timeout gives way to the time the call has left.
#[test]
fn the_blocking_call_holds_the_same_whole_call_deadline() -> Outcome {
    let (base, _seen) = serve_after(RECORDED_RESPONSE, Duration::from_millis(200));
    let client = BlockingClient::new(
        Config::new()
            .api_key("ts-test-key")
            .base_url(&base)
            .timeout(Duration::from_secs(5))
            .retry(RetryPolicy {
                max_retries: 0,
                budget: Some(Duration::from_millis(50)),
                ..RetryPolicy::default()
            }),
    )?;
    let began = std::time::Instant::now();
    let Err(jev::Error::Timeout { timeout }) = client.system_one(SystemOneRequest::new(
        json!({"subject": "Double charge"}),
        Questions::new().with("refund", Noul::new("Is a refund asked for?")),
    )) else {
        unreachable!("a reply past the call's deadline cannot succeed");
    };
    assert!(
        timeout <= Duration::from_millis(50),
        "the attempt was capped by the remaining budget: {timeout:?}"
    );
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "the budget, not the attempt's own timeout, ended the call: {:?}",
        began.elapsed()
    );
    Ok(())
}
```

### s68 crates/jev/tests/live.rs:34-87 — one_request_reaches_the_api_and_its_answers_read

test; complete_declaration. Full span: 35-87. Blob: `a55d253c2efa32fa17f84b71689979f656f1cff5`. File SHA-256: `83005a6a9d1d67d10a0b7b58a76f7f766e95c4383e78196023e8ea5d79c4e527`.

```text
#[tokio::test]
async fn one_request_reaches_the_api_and_its_answers_read() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let client = Client::from_env()?;
    let questions = Questions::new()
        .with("refund", Noul::new("Does the customer ask for money back?"))
        .with(
            "department",
            Choice::new("Which team should handle this?", IndexMap::new())
                .option("billing", "Charges, invoices, and refunds")
                .option("technical", "Bugs and outages")
                .bare_option("other"),
        )
        .with(
            "severity",
            Score::new("How severe is the issue?", Vec::new())
                .level("Cosmetic; the product works")
                .level("Impaired; a workaround exists")
                .level("Blocking; no workaround"),
        );
    let response = client
        .system_one(SystemOneRequest::new(
            "The same order was charged to my card twice, and I want the second charge back.",
            questions,
        ))
        .await?;

    println!("model: {}", response.model);
    println!("request id: {}", response.request_id().unwrap_or("-"));
    println!(
        "usage: {:?} input, {:?} output",
        response.usage.input_tokens, response.usage.output_tokens
    );
    for (id, answer) in &response.answers {
        println!("{id}: {answer:?}");
    }

    let refund = response.noul("refund")?;
    assert!((0.0..=1.0).contains(&refund.noul));
    let department = response.choice("department")?;
    assert!(
        ["billing", "technical", "other"].contains(&department.choice.as_str()),
        "{}",
        department.choice
    );
    let total: f64 = department.probabilities.values().sum();
    assert!((total - 1.0).abs() <= 0.02, "{total}");
    let severity = response.score("severity")?;
    assert!((0.0..=2.0).contains(&severity.score), "{}", severity.score);
    assert_eq!(severity.legend.len(), 3);
    Ok(())
}
```

### s69 crates/jev/tests/client.rs:332-349 — a_request_names_the_model_the_call_asks_for

test; complete_declaration. Full span: 333-349. Blob: `fe4c7346b039c300f34b17886990f6754459a3c6`. File SHA-256: `843f4096427040afd379bc6b03aaa39537a96de45bac1fd45cf04dfc0516fe49`.

```text
#[tokio::test]
async fn a_request_names_the_model_the_call_asks_for() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(format!("{base}/"))
            .default_model("jev-1.12")
            .retry(once()),
    )?;
    assert_eq!(client.base_url(), base, "a trailing slash is dropped");
    assert_eq!(client.default_model(), "jev-1.12");
    client.system_one(asking().model("jev-1.13.0")).await?;
    let seen = seen.lock().await;
    let body = seen.first().ok_or("the server read one request")?.json()?;
    assert_eq!(body["model"], json!("jev-1.13.0"));
    Ok(())
}
```

### s33 crates/jev/src/answers.rs:145-151 — RawResponse::request_id

implementation; complete_declaration. Full span: 147-151. Blob: `11b7359e1784c0b4a1507f7524d90411cbb5dd9d`. File SHA-256: `34dde188f362e772e82e18ef75d1f62eede8cf62ba8f59ac6f9f8cd9a3d41dc0`.

```text
    /// The request id, when the response carried one.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.headers
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
    }
```
