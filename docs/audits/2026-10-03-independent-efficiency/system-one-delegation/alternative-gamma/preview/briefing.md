## Prepared source context

Source commit: `c427943a5c84ba5938a3549f24b27de551812a37`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/coder/tests/program_run.rs:585-615 — a_bound_this_host_cannot_enforce_refuses_before_anything_runs

File SHA-256: `a2b709fec83667ad422d9cb4bf05f24ab34272f4b1ade78130ae0a76e6a534f8`

```rust
/// A step whose bounds this host cannot enforce does not run, and neither
/// does the program that carried it.
#[tokio::test]
async fn a_bound_this_host_cannot_enforce_refuses_before_anything_runs() {
    let machine = machine();
    let root = machine.path();
    let runtime = runtime(root).await;

    for (bounds, expected) in [
        // A shape nobody here can make. Running it in the shared
        // directory instead is the substitution the rule forbids.
        (json!({"isolation": "vm", "minutes": 60}), "vm checkout"),
        // A bound key this host keeps nothing for.
        (json!({"budget_cents": 500}), "cannot enforce budget_cents"),
        // A key it knows, carrying a value it cannot hold to.
        (json!({"concurrent_max": 0}), "count above zero"),
    ] {
        let program = program_with(root, bounds.clone());
        let refused = runtime
            .admit(&program)
            .expect_err(&format!("{bounds} is not enforceable here"));
        assert_eq!(refused.step, "fan_out");
        assert_eq!(refused.code, "bound_unenforceable");
        assert!(refused.reason.contains(expected), "{refused}");

        let run = runtime.run(&program, &inputs(), None).await;
        assert!(run.steps.is_empty(), "nothing ran: {:?}", run.step_names());
        assert!(run.delegations.is_empty());
        assert_eq!(run.stopped, Some(refused));
    }
}
```

### crates/gym/src/gate.rs:2920-2944 — tests::the_decision_bound_records_what_it_does_not_cover

File SHA-256: `59edaff2c4680b90a554c33d6d1933636066e4e76da277442eb0b4800f866920`

```rust
    #[test]
    fn the_decision_bound_records_what_it_does_not_cover() {
        // The gate cannot honestly carry an absolute effect-size floor until
        // openagents#9370 measures the suite's trial-to-trial spread. That
        // gap is on the gate and inside the digest, so filling it produces
        // decision-v2.
        let gate = decision();
        let Rule::Decision(rule) = &gate.rule else {
            panic!("decision-v1 carries a decision rule");
        };
        assert_eq!(rule.variance_basis, VarianceBasis::ItemSampling);
        assert_eq!(rule.gain_standard_errors.basis, Basis::Convention);
        let pending = rule
            .pending_measurement
            .as_ref()
            .expect("the gap is recorded");
        assert_eq!(pending.issue.as_deref(), Some("openagents#9370"));

        let mut measured = gate.clone();
        if let Rule::Decision(rule) = &mut measured.rule {
            rule.variance_basis = VarianceBasis::ItemSamplingAndTrialResampling;
            rule.pending_measurement = None;
        }
        assert_ne!(gate.digest(), measured.digest());
    }
```

### docs/audits/2026-09-19-codebase-audit/calibration-wire.rs:8-54 — main

File SHA-256: `c58d6ead3dff8fce9dfd8ef5489267ec7f1ca918da6196c728b3c63b3ee86736`

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let map = Map::fit(&[Observation::new(0.8, false)], 1);
    let cases = [
        (
            "choice",
            Kind::Choice,
            vec![("yes", 0.8), ("no", 0.2)],
            "no",
        ),
        ("noul", Kind::Noul, vec![("no", 0.2), ("yes", 0.8)], "no"),
        (
            "score",
            Kind::Score,
            vec![("0", 0.8), ("1", 0.15), ("2", 0.05)],
            "1",
        ),
    ];
    for (name, kind, pairs, truth) in cases {
        let raw = pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
        let selected = lev::estimator::argmax(&raw)?;
        let mapped = map.apply_distribution(&raw);
        let typed = lev::estimator::answer(kind, &mapped, &Default::default(), &selected)?;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "model": "audit-local", "answers": {"q": typed}, "usage": {}
        }))?;
        let response = jev::SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes,
        })?;
        let Disposition::Answered { chosen, .. } = read_answer(&response.answers["q"]) else {
            return Err("the served answer was not decoded as an answer".into());
        };
        let row = Row::new("audit", "audit", name, "audit").scored(raw, selected == truth);
        let observations = mapped_observations(&[row], &map);
        let observation = observations.first().ok_or("no mapped observation")?;
        println!(
            "kind={name} raw_selected={selected} mapped_metric_correct={} wire_selected={chosen} wire_correct={}",
            observation.correct,
            chosen == truth,
        );
    }
    Ok(())
}
```

### crates/coder/src/generate.rs:729-805 — ResponsesDoor::once

File SHA-256: `403cbfb6ab4cc42d161580e5c02124e77b7a879fbd9405c3932fc86011b64680`

```rust
    /// One streaming attempt: the request plus the SSE read. On failure the
    /// error carries whatever text streamed before it died, so the caller
    /// can tell whether anything user-visible arrived.
    async fn once(
        &self,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(String, Option<Usage>), (String, GenerateError)> {
        let ends = Instant::now() + self.patience.whole;
        let sent = self
            .http
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth(&self.key)
            .json(&self.body(instructions, input))
            .send();
        let response = match tokio::time::timeout(self.patience.first_word, sent).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => return Err((String::new(), GenerateError::Transport(error))),
            // No headers, so nothing was heard and the request may go
            // again. The attempt count is added where the attempts are
            // counted.
            Err(_) => {
                return Err((
                    String::new(),
                    GenerateError::Quiet {
                        heard: false,
                        reason: format!(
                            "no response headers in {} seconds",
                            self.patience.first_word.as_secs()
                        ),
                    },
                ));
            }
        };
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err((
                String::new(),
                GenerateError::Status(status.as_u16(), clip(&body, 400)),
            ));
        }

        let mut reader = Reader::default();
        let mut stream = response.bytes_stream();
        // The quiet clock runs from the last event rather than from the
        // last byte, so a door that dribbles bytes without completing an
        // event is still quiet.
        // The whole-attempt clock runs alongside it, so a door that never
        // stops talking ends too.
        let mut spoke = Instant::now();
        loop {
            let left = self
                .patience
                .quiet
                .saturating_sub(spoke.elapsed())
                .min(ends.saturating_duration_since(Instant::now()));
            let chunk = match tokio::time::timeout(left, stream.next()).await {
                Ok(Some(Ok(chunk))) => chunk,
                Ok(Some(Err(error))) => {
                    return Err((reader.text, GenerateError::Transport(error)));
                }
                Ok(None) => break,
                Err(_) if Instant::now() >= ends => {
                    return Err((reader.text.clone(), self.ran_long(&reader)));
                }
                Err(_) => return Err((reader.text.clone(), self.went_quiet(&reader))),
            };
            match reader.push(&chunk, sink) {
                Ok(0) => {}
                Ok(_) => spoke = Instant::now(),
                Err(error) => return Err((reader.text, error)),
            }
        }
        reader.finish(sink)
    }
```

### docs/audits/2026-09-19-codebase-audit/reproduce.rs:7-28 — streamed

File SHA-256: `03d69e2c3be756f64f157b838105c552325e59deaa3b07c21e82780293e89723`

```rust
async fn streamed(chunks: Vec<Vec<u8>>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 16384];
        let _ = stream.read(&mut buf).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
        for chunk in chunks {
            write!(stream, "{:x}\r\n", chunk.len()).unwrap();
            stream.write_all(&chunk).unwrap();
            stream.write_all(b"\r\n").unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(150));
        }
        stream.write_all(b"0\r\n\r\n").unwrap();
    });
    let door = ResponsesDoor::new(format!("http://{addr}"), "audit", "unused-local-test");
    let result = door.generate("", &[], &mut |_| {}, &mut |_| {}).await;
    server.join().unwrap();
    format!("{result:?}")
}
```

### crates/jev/tests/client.rs:821-846 — the_budget_bounds_the_raw_call_the_same_way

File SHA-256: `843f4096427040afd379bc6b03aaa39537a96de45bac1fd45cf04dfc0516fe49`

```rust
/// The deadline holds on the unread path too: `system_one_raw` hands the
/// response back with its body unread, and the reply still cannot arrive
/// past the budget.
#[tokio::test]
async fn the_budget_bounds_the_raw_call_the_same_way() -> Outcome {
    let (base, _) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).after(Duration::from_millis(200)),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_secs(1))
            .retry(RetryPolicy {
                max_retries: 0,
                budget: Some(Duration::from_millis(50)),
                ..RetryPolicy::default()
            }),
    )?;
    let Err(Error::Timeout { timeout }) = client.system_one_raw(asking()).await else {
        unreachable!("a reply past the call's deadline cannot succeed");
    };
    assert!(timeout <= Duration::from_millis(50), "{timeout:?}");
    Ok(())
}
```

### crates/coder/src/generate.rs:849-908 — ResponsesDoor::generate

File SHA-256: `403cbfb6ab4cc42d161580e5c02124e77b7a879fbd9405c3932fc86011b64680`

```rust
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        // Two kinds of attempt are counted, because two kinds of failure
        // earn another one.
        //
        // A stream that fails before showing anything — empty, or holding
        // only a hidden plan — is safe to redo: the user saw nothing and
        // the request is idempotent. Upstream flakes like a model-side
        // malformed function call are transient, and the retry is
        // invisible.
        //
        // A door that never sent response headers is the other kind. It
        // is indistinguishable from one that dropped the connection, so
        // the request goes again after `Patience::retry_wait`, which
        // doubles on the third attempt.
        //
        // Nothing else is redone. A door that sent headers and then went
        // quiet has already shown the caller where the turn got to, and a
        // second attempt would repeat it. A door that keeps failing after
        // the last attempt surfaces its error — hidden retries, honest
        // failures.
        let mut empty: usize = 0;
        let mut unanswered: u32 = 0;
        loop {
            let (partial, error) = match self.once(instructions, input, sink).await {
                Ok(done) => return Ok(done),
                Err(failed) => failed,
            };
            match &error {
                GenerateError::Quiet { heard: false, .. } => {
                    unanswered += 1;
                    if unanswered >= HEADER_ATTEMPTS {
                        return Err(GenerateError::Quiet {
                            heard: false,
                            reason: format!(
                                "no response headers in {} seconds, over {unanswered} attempts",
                                self.patience.first_word.as_secs()
                            ),
                        });
                    }
                    tokio::time::sleep(self.patience.retry_wait * (1 << (unanswered - 1))).await;
                }
                GenerateError::Stream(_) | GenerateError::Transport(_)
                    if partial.is_empty() || planish(&partial) =>
                {
                    empty += 1;
                    if empty >= EMPTY_STREAM_ATTEMPTS {
                        return Err(error);
                    }
                    tokio::time::sleep(Duration::from_millis(300 * empty as u64)).await;
                }
                _ => return Err(error),
            }
        }
    }
```

### crates/jev/tests/live.rs:34-87 — one_request_reaches_the_api_and_its_answers_read

File SHA-256: `83005a6a9d1d67d10a0b7b58a76f7f766e95c4383e78196023e8ea5d79c4e527`

```rust
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
