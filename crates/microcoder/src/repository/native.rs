//! Preserve native model evidence before reducing it to loop judgments/actions.
use super::*;

struct Transport<'a, T> {
    host: &'a Host,
    inner: T,
    /// The model this route admits.
    model: String,
    /// A usage-limit refusal the last request met.
    refusal: RefCell<Option<Refusal>>,
}
impl<T: microluna::Transport> microluna::Transport for Transport<'_, T> {
    async fn respond(
        &self,
        request: &microluna::Request,
    ) -> Result<microluna::Reply, microluna::TransportError> {
        let sequence=self.host.effect("codex_request",json!({"model":request.model,"instructions":request.instructions,
            "input":request.input,"tools":request.tools,"effort":request.effort,"cache_key":request.cache_key,
            "parallel_tools":request.parallel_tools})).map_err(|error|microluna::TransportError::Failed(error.to_string()))?;
        let response = self.inner.respond(request).await;
        if let Err(microluna::TransportError::Http { status, body }) = &response
            && let Some(refusal) = Refusal::codex(*status, body, task::autostart::unix_now())
        {
            *self.refusal.borrow_mut() = Some(refusal);
        }
        let observation = match &response {
            Ok(reply) => json!({"id":reply.id,"model":reply.model,"items":reply.items,
                "usage":{"input":reply.usage.input,"cached":reply.usage.cached,"output":reply.usage.output,"reasoning":reply.usage.reasoning}}),
            Err(error) => {
                json!({"error":error.to_string(),"native_partial_items":"unavailable","billing":"unknown"})
            }
        };
        self.host
            .result(sequence, "codex_request", observation)
            .map_err(|error| microluna::TransportError::Failed(error.to_string()))?;
        if let Ok(reply) = &response
            && (reply.model.is_empty() || reply.model != self.model)
        {
            self.host
                .fail("native provider model identity is missing or differs from admission");
            return Err(microluna::TransportError::Failed(
                "Native model identity is missing or differs from the grant; refusing its action."
                    .into(),
            ));
        }
        response
    }
}

struct NativeJudge<'a> {
    host: &'a Host,
    client: jev::Client,
}
impl Judge for NativeJudge<'_> {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        let started = std::time::Instant::now();
        let mut questions = jev::Questions::new();
        for question in &set.questions {
            questions = questions.with(question.id.clone(), jev::Noul::new(question.text.clone()));
        }
        let request =
            jev::SystemOneRequest::new(state.clone(), questions).retry(jev::RetryPolicy {
                max_retries: 0,
                ..self.client.retry().clone()
            });
        let response = self.client.system_one(request).await;
        let milliseconds = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        match response {
            Ok(response) => {
                let retained=self.host.append(&Step::said(Source::System,"Native decision response retained.")
                    .noting("decision_response",json!({"model":response.model,"request_id":response.raw().request_id(),
                        "status":response.raw().status,"body_bytes":response.raw().bytes,
                        "input_tokens":response.usage.input_tokens,"output_tokens":response.usage.output_tokens})));
                if let Err(error) = retained {
                    return Judgment {
                        error: Some(error.to_string()),
                        cost_unknown: Some("response retention failed".into()),
                        ..Judgment::default()
                    };
                }
                if response.model != self.host.configuration().decision_model {
                    self.host.fail(
                        "decision provider returned a model different from the admitted model",
                    );
                    return Judgment {
                        error: Some("Decision model identity differs from the grant.".into()),
                        cost_unknown: Some("unaccepted provider model".into()),
                        ..Judgment::default()
                    };
                }
                Judgment {
                    answers: set
                        .questions
                        .iter()
                        .filter_map(|question| {
                            response
                                .noul(&question.id)
                                .ok()
                                .map(|answer| (question.id.clone(), answer.noul))
                        })
                        .collect(),
                    usd: response.usage.input_tokens.map(|tokens| {
                        tokens as f64 * crate::models::JEV_USD_PER_MILLION / 1_000_000.0
                    }),
                    cost_unknown: response
                        .usage
                        .input_tokens
                        .is_none()
                        .then(|| "Jev reported no input tokens".into()),
                    usd_upper: response.usage.input_tokens.map(|tokens| {
                        tokens as f64 * crate::models::JEV_USD_PER_MILLION / 1_000_000.0
                    }),
                    milliseconds,
                    error: None,
                }
            }
            Err(error) => {
                let _ = self.host.append(
                    &Step::said(Source::System, "Decision response was unavailable.").noting(
                        "decision_response",
                        json!({"error":error.to_string(),"body":"unavailable","billing":"unknown"}),
                    ),
                );
                Judgment {
                    error: Some(error.to_string()),
                    cost_unknown: Some("decision request failed; charge is unknown".into()),
                    milliseconds,
                    ..Judgment::default()
                }
            }
        }
    }
}

/// The `claude` binary's native output, retained before reduction: what
/// the binary was asked, what it printed, and how it exited.
struct Claude<'a> {
    host: &'a Host,
    inner: crate::claude::ClaudeGenerator,
    /// A usage or rate-limit refusal the last call met.
    refusal: RefCell<Option<Refusal>>,
}
impl Generate for Claude<'_> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let sequence = match self.host.effect(
            "claude_request",
            json!({"binary":self.inner.binary,"args":self.inner.args(system),"model":self.inner.model,
                "effort":self.inner.effort,"prompt":prompt}),
        ) {
            Ok(sequence) => sequence,
            Err(error) => return refused_generation(&self.inner.model, false, &error.to_string()),
        };
        let invocation = self.inner.invoke(system, prompt).await;
        *self.refusal.borrow_mut() = Refusal::claude(
            invocation.api_error_status.is_some(),
            invocation.api_error_status,
            task::autostart::unix_now(),
        );
        let observation = json!({"status":invocation.status,"stdout":invocation.stdout,"stderr":invocation.stderr,
            "model":invocation.generated.model,"usd":invocation.generated.usd,"billing":"provider-reported-list-price"});
        if let Err(error) = self.host.result(sequence, "claude_request", observation) {
            return refused_generation(&self.inner.model, true, &error.to_string());
        }
        invocation.generated
    }
}

/// One admitted route's native generator.
enum Native<'a, T: microluna::Transport> {
    Codex(crate::models::CodexGenerator<Transport<'a, T>>),
    Claude(Claude<'a>),
}

impl<T: microluna::Transport> Generate for Native<'_, T> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        match self {
            Native::Codex(generator) => {
                generator.transport.refusal.borrow_mut().take();
                generator.generate(system, prompt).await
            }
            Native::Claude(generator) => generator.generate(system, prompt).await,
        }
    }
}

impl<T: microluna::Transport> Lane for Native<'_, T> {
    fn refusal(&self) -> Option<Refusal> {
        match self {
            Native::Codex(generator) => generator.transport.refusal.borrow_mut().take(),
            Native::Claude(generator) => generator.refusal.borrow_mut().take(),
        }
    }
}

pub(super) async fn run<T: microluna::Transport>(
    host: Host,
    book: PathBuf,
    clients: Vec<(GrantRoute, Client<T>)>,
    client: jev::Client,
    session: String,
) -> Result<task::Task, task::Error> {
    let (state, outcome) = {
        let lanes = clients
            .into_iter()
            .map(|(route, client)| {
                let lane = match client {
                    Client::Codex(transport) => Native::Codex(crate::models::CodexGenerator {
                        transport: Transport {
                            host: &host,
                            inner: transport,
                            model: route.model.clone(),
                            refusal: RefCell::new(None),
                        },
                        model: route.model.clone(),
                        effort: route.effort.clone(),
                        cache_key: session.clone(),
                    }),
                    Client::Claude(generator) => Native::Claude(Claude {
                        host: &host,
                        inner: generator,
                        refusal: RefCell::new(None),
                    }),
                };
                (route, lane)
            })
            .collect();
        let journal = Transcript(&host);
        let generator = failover(&host, &journal, book, lanes, task::autostart::unix_now);
        generator.record_start();
        let judge = NativeJudge {
            host: &host,
            client,
        };
        run_loop(&host, &generator, &judge).await?
    };
    finish(host, state, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use microluna::Transport as _;

    #[tokio::test]
    async fn missing_or_mismatched_native_identity_is_retained_but_never_accepted() {
        for model in ["", "other-model"] {
            let (_root, store, grant) = super::super::tests::fixture();
            let host = Host::admit(&store, &grant).await.unwrap();
            let scripted = microluna::fake::FakeTransport::default();
            scripted.then(microluna::Reply {
                id: Some("unaccepted-fixture".into()),
                model: model.into(),
                items: vec![json!({"type":"function_call","name":"next_action","arguments":"unaccepted action"})],
                usage: microluna::TokenUsage::default(),
            });
            let transport = Transport {
                host: &host,
                inner: scripted,
                model: "fixture-model".into(),
                refusal: RefCell::new(None),
            };
            let request = microluna::Request {
                text_format: None,
                model: "fixture-model".into(),
                instructions: String::new(),
                input: Vec::new(),
                tools: Vec::new(),
                effort: None,
                cache_key: "fixture".into(),
                parallel_tools: false,
            };
            assert!(transport.respond(&request).await.is_err());
            assert!(host.effect("command", json!({})).is_err());
            drop(transport);
            host.finish("identity_refused", false, json!({})).unwrap();
            let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
            assert!(trace.contains("unaccepted action"));
            assert!(trace.contains("unaccepted-fixture"));
            assert!(!trace.contains("Supervised command started"));
        }
    }

    #[tokio::test]
    async fn native_output_items_and_each_attempt_are_retained_before_reduction() {
        let (_root, store, grant) = super::super::tests::fixture();
        let host = Host::admit(&store, &grant).await.unwrap();
        let scripted = microluna::fake::FakeTransport::default();
        scripted.then_fail(microluna::TransportError::Stream(
            "fixture disconnect".into(),
        ));
        let items = vec![
            json!({"type":"reasoning","summary":[{"type":"summary_text","text":"retained native reasoning"}]}),
            json!({"type":"function_call","name":"next_action","call_id":"call-fixture","arguments":"not valid action JSON"}),
        ];
        scripted.then(microluna::Reply {
            id: Some("native-fixture".into()),
            model: "fixture-model".into(),
            items: items.clone(),
            usage: microluna::TokenUsage {
                input: 17,
                cached: 3,
                output: 8,
                reasoning: 4,
            },
        });
        let transport = Transport {
            host: &host,
            inner: scripted,
            model: "fixture-model".into(),
            refusal: RefCell::new(None),
        };
        let request = microluna::Request {
            text_format: None,
            model: "fixture-model".into(),
            instructions: "exact fixture instructions".into(),
            input: vec![json!({"type":"message","role":"user","content":"exact fixture input"})],
            tools: Vec::new(),
            effort: Some("medium".into()),
            cache_key: "fixture-cache".into(),
            parallel_tools: false,
        };
        assert!(transport.respond(&request).await.is_err());
        assert_eq!(transport.respond(&request).await.unwrap().items, items);
        drop(transport);
        host.finish("fixture_complete", false, json!({})).unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        for expected in [
            "exact fixture instructions",
            "exact fixture input",
            "fixture disconnect",
            "retained native reasoning",
            "not valid action JSON",
        ] {
            assert!(trace.contains(expected), "missing {expected}");
        }
        assert_eq!(trace.matches("\"kind\":\"codex_request\"").count(), 4);
    }
}
