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

### c13 crates/jev/src/error.rs:64-136 — Error

Role: implementation; complete_declaration. Full unit: 64-136. Blob: `9cee344140702bff0860b789cea82ca85331811f`. File SHA-256: `816c6d2bfa73b037a2e0be77fd7213bf7890cd87ebe8aeada6d6c6efccc4e11b`.

```text
/// What a failed request failed at.
#[derive(Debug, Error)]
pub enum Error {
    /// A setting is missing or out of range. Raised before any request.
    #[error("{0}")]
    Config(String),

    /// A question set failed the checks the SDK runs before any request.
    #[error("{}", describe_question(id, message))]
    Question {
        /// The question id, or an empty string when the whole set is at fault.
        id: String,
        /// What is wrong with it.
        message: String,
    },

    /// The API answered with a status outside the 2xx range.
    ///
    /// The error is boxed because it carries the response headers and body, and
    /// a `Result` whose failure is that wide costs every call that returns one.
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The request never reached the API, or its response never arrived.
    #[error("{message}")]
    Connection {
        /// What the transport reported.
        message: String,
        /// The transport error, when there is one to carry.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// One attempt ran past its timeout.
    #[error("the request timed out after {}ms", timeout.as_millis())]
    Timeout {
        /// The per-attempt timeout that expired.
        timeout: Duration,
    },

    /// A 2xx body was missing a field, or carried one the SDK cannot read.
    #[error("invalid response data at {field_path:?}")]
    ResponseValidation {
        /// The response status.
        status: u16,
        /// A dotted path to the field, such as `answers.tone.confidence`.
        field_path: String,
        /// The body the SDK read, when it read one. It is boxed so the enum
        /// stays narrow under every feature combination a dependent crate's
        /// `serde_json` features give `Value`.
        body: Option<Box<ResponseBody>>,
        /// The request id, when the response carried one.
        request_id: Option<String>,
    },

    /// A typed accessor asked for one answer type and found another.
    #[error("answer {id:?} is a {found} answer, not a {expected} answer")]
    AnswerType {
        /// The question id.
        id: String,
        /// The type the caller asked for.
        expected: &'static str,
        /// The type the response carried.
        found: &'static str,
    },

    /// A typed accessor named an answer the response does not carry.
    #[error("the response carries no answer named {id:?}")]
    MissingAnswer {
        /// The question id.
        id: String,
    },
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

### c04 crates/jev/src/questions.rs:432-456 — Questions::validate

Role: implementation; complete_declaration. Full unit: 432-456. Blob: `ae89c2bdb8fad2f93e2cef3cb9d5e57ce324f22a`. File SHA-256: `dfa7fb6681c7915bfd7401f4c451325614f5f85d6d656767cd701dd1655f6c83`.

```text
    /// Check the set the way both official SDKs check theirs, and the two
    /// limits the API documents.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Question`] when the set is empty, a Score names fewer
    /// than two or more than ten levels, or a Choice names more than 255
    /// options. The error names the question at fault.
    pub fn validate(&self) -> Result<()> {
        if self.0.is_empty() {
            return Err(Error::Question {
                id: String::new(),
                message: "a request asks at least one question".to_string(),
            });
        }
        for (id, question) in self.iter() {
            match question {
                Question::Score(score) => check_score(id, score.criteria.len())?,
                Question::Choice(choice) => check_choice(id, choice.criteria.len())?,
                Question::Noul(_) => {}
                Question::Raw(value) => check_raw(id, value)?,
            }
        }
        Ok(())
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

### c09 crates/jev/src/retry.rs:120-148 — RetryPolicy::validate

Role: implementation; complete_declaration. Full unit: 120-148. Blob: `ac26bf44bd147f50cacf5bad9a240f098abbe516`. File SHA-256: `7248982511c6bfe6183f479f1788284378ad702b04bc416819b6f4a58f76e9ba`.

```text
    /// Check every field, the way both official SDKs check theirs.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the jitter falls outside 0 to 1, a
    /// status falls outside 100 to 999, or the budget is zero.
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.backoff_jitter) {
            return Err(Error::Config(format!(
                "`retry.backoff_jitter` must be between 0 and 1, got {}",
                self.backoff_jitter
            )));
        }
        if let Some(status) = self
            .http_statuses
            .iter()
            .find(|status| !(100..=999).contains(*status))
        {
            return Err(Error::Config(format!(
                "`retry.http_statuses` must hold HTTP status codes, got {status}"
            )));
        }
        if self.budget == Some(Duration::ZERO) {
            return Err(Error::Config(
                "`retry.budget` must be a positive duration".to_string(),
            ));
        }
        Ok(())
    }
```
