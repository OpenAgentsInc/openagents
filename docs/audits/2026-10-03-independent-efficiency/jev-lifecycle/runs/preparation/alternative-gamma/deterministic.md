## Prepared source evidence

Source commit: `c427943a5c84ba5938a3549f24b27de551812a37`. Selections may omit requirements, dependencies, fixtures, and callers. Partial units are labeled; inspect their full spans before relying on completeness. Complete applicable instructions are supplied separately.

### c03 docs/decision-models/2026-09-20-score-contract.md:57-98 — docs/decision-models/2026-09-20-score-contract.md

Role: document; partial_file. Full unit: 1-107. Blob: `593e51099b82e2f905f0759f27122bb8ffb75418`. File SHA-256: `a8c612095723bf3599ae4d618aeff8ae18e3b5a05e868c07f26e623126e20c60`.

```text
| Score | `selected`, the canonical decimal level key | Highest level among maxima in `probabilities`. |

Lev writes `selected` on Noul and Score answers, calibrated or raw. Jev's
Rust SDK decodes it and rejects invalid option names. Gym records it and
reads that option's probability when fitting or applying another map.
Absence means no provenance was reported. It does not prove that an external
door is uncalibrated or reveal its internal estimator. In particular, the
hosted Jev implementation is not inspected here.

Older clients can still read the standard numeric fields. They cannot
recover a fixed categorical selection from an inverted distribution unless
they understand `selected`. No threshold or local argmax can reconstruct
that omitted information in general.

## Versioned measurement rows

New rows use `openagents.gym.eval_row.v2`. The `selected` field identifies
the answer, and `raw_top` records the probability reported on that answer.
Despite its historical name, it can now be below the distribution maximum.
This is a semantic change from v1's maximum, so it receives a new schema
identifier. Older store readers reject v2 instead of silently applying the
old interpretation.

The store also accepts historical v1 rows. Their schema, fields, and receipt
chains are not rewritten. When a v1 row lacks a named selection, readers
retain the historical argmax fallback. That fallback cannot reconstruct an
unrecorded non-argmax Choice or prove whether the distribution was calibrated.
Historical claims require their retained provenance, not an assumption based
on an absent field. An old consumer must upgrade before reading new rows.

The gate's policy is separate from its input schema. This fix changes no
threshold, basis, or criterion in `probability-v1` or `ab::Rule::v2`; their
digests do not change. The [re-derivation inventory](../gym/measurements/2026-09-20-raw-floors-and-mapped-claims.md)
checks the retained records, including their gate verdicts, under the corrected
reader. Unknown historical measurements remain unknown.

## Precision and verification

Kev rounds wire statistics and probabilities to two decimal places. Lev's
raw sample frequencies use a `1/n` grid; calibrated probabilities need not
stay on that grid. Comparisons must use the precision actually reported.
Neither wire rounding nor a tied distribution changes which statistic
```

### c06 .agents/skills/typesafe-ai/SKILL.md:41-71 — .agents/skills/typesafe-ai/SKILL.md

Role: document; partial_file. Full unit: 1-149. Blob: `0109513f9656917dc93cbc5ecddfca465a53ce66`. File SHA-256: `71ea90d7906c6554c4f4c460ef7361b2d26f59116ccdae986dc6d997b9389f52`.

```text
- If the index is unavailable, use the direct links below or the site's navigation.
  If Markdown fetching fails, try the normal page. If live access is unavailable,
  use available local docs or installed SDK types, state that limitation, and avoid
  inventing version-dependent details.

| Task | Start here; follow the relevant details |
| --- | --- |
| Understand the programming model | [System One](https://docs.typesafe.ai/concepts/system-one.md), [building guide](https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md) |
| Explore what to build | [Use-case map](https://docs.typesafe.ai/concepts/use-case-map.md), then relevant cookbooks from the index |
| Prepare inputs and questions | [State](https://docs.typesafe.ai/concepts/state.md), [primitives](https://docs.typesafe.ai/primitives.md), then the chosen primitive's page |
| Decide how to handle uncertainty | [Confidence](https://docs.typesafe.ai/confidence.md) |
| Write API code | [HTTP API](https://docs.typesafe.ai/api.md), [Python SDK](https://docs.typesafe.ai/sdk/python.md), or [JavaScript SDK](https://docs.typesafe.ai/sdk/javascript.md) |
| Update an older integration | [Migration guide](https://docs.typesafe.ai/migrating-to-v1.md) and the installed SDK's current reference |

## Find the useful shape

Start from the behavior the user wants: what will the application show, select,
change, or hand off? Work backward to the judgments it needs. Keep known rules,
calculations, exact lookups, and execution in code. Preserve the user's chosen stack
and scope; add TypeSafe where semantic understanding helps.

When brainstorming or choosing an architecture, consider more than classification.
The patterns below are starting points: combine primitives around the user's goal,
including ideas that do not fit an established recipe.

- **Route and fill known arguments.** A request can select a handler and its typed
  parameters. Ask useful branch-specific questions up front and consume only the
  relevant answers. Explore [function calling](https://docs.typesafe.ai/cookbooks/function_calling.md)
  and [speculative fan-out](https://docs.typesafe.ai/patterns/fan-out.md).
- **Select instead of generate.** Find candidate values or source spans in code,
  use a judgment to select the intended one, then copy or normalize it. Code can
```

### c02 crates/jev/tests/blocking.rs:133-167 — the_blocking_call_holds_the_same_whole_call_deadline

Role: test; complete_declaration. Full unit: 133-167. Blob: `02d55d9d443bc32d1127852305e914ba2778a53c`. File SHA-256: `68dd7bf71a2f4bcd41541e39764b181725c126eabb2850cf4ebed8497b5ea09b`.

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

### c05 crates/jev/tests/client.rs:332-349 — a_request_names_the_model_the_call_asks_for

Role: test; complete_declaration. Full unit: 332-349. Blob: `fe4c7346b039c300f34b17886990f6754459a3c6`. File SHA-256: `843f4096427040afd379bc6b03aaa39537a96de45bac1fd45cf04dfc0516fe49`.

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

### c01 crates/jev/src/answers.rs:52-80 — ScoreAnswer

Role: implementation; complete_declaration. Full unit: 52-80. Blob: `11b7359e1784c0b4a1507f7524d90411cbb5dd9d`. File SHA-256: `34dde188f362e772e82e18ef75d1f62eede8cf62ba8f59ac6f9f8cd9a3d41dc0`.

```text
/// Where a Score question placed the state, with the rubric it read.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScoreAnswer {
    /// The probability-weighted mean level, which falls between levels.
    /// A caller that needs a categorical level reads `selected`, or falls
    /// back to the argmax of `probabilities` when the answer lacks it.
    /// `docs/decision-models/2026-09-20-score-contract.md` states the rule.
    pub score: f64,
    /// How sharp the distribution is, as the API reports it.
    pub confidence: f64,
    /// The level the estimator's own distribution named, when the door
    /// reports one.
    ///
    /// A served calibration map rescales the selected level's probability
    /// without replacing the answer, and can leave a runner-up numerically
    /// larger in `probabilities`; this field is then the only place the
    /// estimator's pick survives the wire. When absent, a categorical reader
    /// falls back to the argmax of `probabilities`, with a tied maximum
    /// resolving to the highest level. Absence is not evidence that no
    /// calibration map was served.
    #[serde(default)]
    pub selected: Option<String>,
    /// The rubric, keyed by level.
    pub legend: BTreeMap<u32, Entry>,
    /// A probability for each level. The API leaves this out on some answers,
    /// and the field is then empty.
    #[serde(default)]
    pub probabilities: BTreeMap<u32, f64>,
}
```

### c08 crates/jev/tests/live.rs:34-87 — one_request_reaches_the_api_and_its_answers_read

Role: test; complete_declaration. Full unit: 34-87. Blob: `a55d253c2efa32fa17f84b71689979f656f1cff5`. File SHA-256: `83005a6a9d1d67d10a0b7b58a76f7f766e95c4383e78196023e8ea5d79c4e527`.

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

### c10 crates/jev/tests/answers.rs:235-304 — a_field_that_does_not_read_is_named_by_a_dotted_path

Role: test; complete_declaration. Full unit: 235-304. Blob: `11aa1e38149e1a6ce5f029603ad6238946e5ac35`. File SHA-256: `b1c2c28d1882637813d7699b29cd8656e7156c24c6f46bbca003ef9f7e4bc03e`.

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

### c12 crates/jev/tests/selected_answer.rs:14-24 — a_noul_cannot_name_an_option_outside_yes_and_no

Role: test; complete_declaration. Full unit: 14-24. Blob: `04dcc9de3d41af50d92bc97f5d329632618bc325`. File SHA-256: `183c1b99706243034ba9d882f1849ecb90a325f3c4a9c9fbabf0c845bcde1055`.

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
