//! Preserve native model evidence before reducing it to loop judgments/actions.
use super::*;

struct Transport<'a, T> {
    host: &'a Host,
    /// Where the reply goes as the model writes it.
    replies: Option<&'a Replies<'a>>,
    inner: T,
    /// The model this route admits.
    model: String,
    /// A usage-limit refusal the last request met.
    refusal: RefCell<Option<Refusal>>,
}
impl<T: codex_transport::Transport> codex_transport::Transport for Transport<'_, T> {
    async fn respond(
        &self,
        request: &codex_transport::Request,
    ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
        let sequence=self.host.effect("codex_request",json!({"model":request.model,"instructions":request.instructions,
            "input":crate::images::redacted(&json!(request.input)),"tools":request.tools,"effort":request.effort,"cache_key":request.cache_key,
            "parallel_tools":request.parallel_tools})).map_err(|error|codex_transport::TransportError::Failed(error.to_string()))?;
        let response = match self.replies {
            Some(replies) => {
                replies.begin();
                self.inner
                    .respond_streaming(request, &mut |text| replies.feed(text))
                    .await
            }
            None => self.inner.respond(request).await,
        };
        if let Err(codex_transport::TransportError::Http { status, body }) = &response
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
            .map_err(|error| codex_transport::TransportError::Failed(error.to_string()))?;
        if let Ok(reply) = &response
            && (reply.model.is_empty() || reply.model != self.model)
        {
            self.host
                .fail("native provider model identity is missing or differs from admission");
            return Err(codex_transport::TransportError::Failed(
                "Native model identity is missing or differs from the grant; refusing its action."
                    .into(),
            ));
        }
        response
    }
}

struct NativeJudge<'a> {
    host: &'a Host,
    /// The client, or why this run has no Jev. Without one every judgment
    /// answers nothing, costs nothing, and names why, and no request is
    /// made.
    client: Result<jev::Client, String>,
    /// Set when the hosted decision service became unavailable mid-run —
    /// unreachable, or refusing this computer's quota: the rest of the run
    /// asks nothing and says why, instead of waiting on every step.
    off: RefCell<Option<String>>,
}
impl Judge for NativeJudge<'_> {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        let unavailable = |why: String| Judgment {
            error: Some(why),
            usd: Some(0.0),
            usd_upper: Some(0.0),
            ..Judgment::default()
        };
        let client = match &self.client {
            Ok(client) => client,
            Err(why) => return unavailable(why.clone()),
        };
        if let Some(why) = self.off.borrow().clone() {
            return unavailable(why);
        }
        let started = std::time::Instant::now();
        let request =
            jev::SystemOneRequest::new(state.clone(), set.questions()).retry(jev::RetryPolicy {
                max_retries: 0,
                ..client.retry().clone()
            });
        let asked = request
            .body(client.default_model())
            .map_or(Value::Null, Value::Object);
        let response = client.system_one(request).await;
        let milliseconds = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        // The call itself, as every Jev caller records one
        // (`openagents.decision-call.v1`): the structured request, the door
        // and how it was reached, the service, latency, and cost.
        let record = jev_hosted::decision_record(
            format!("decision-{}", set.id),
            set.id.clone(),
            client,
            asked,
            response.as_ref(),
            milliseconds,
        );
        let _ = self
            .host
            .append(&Step::called(record.call()).taking(milliseconds));
        match response {
            Ok(response) => {
                let retained=self.host.append(&Step::said(Source::System,"Native decision response retained.")
                    .noting("decision_response",json!({"model":response.model,"request_id":response.raw().request_id(),
                        "status":response.raw().status,"body_bytes":response.raw().bytes,
                        "input_tokens":response.usage.input_tokens,"output_tokens":response.usage.output_tokens,
                        "door":client.base_url(),"via":if client.service().is_some() {"hosted"} else {"direct"},
                        "service":response.service(),"milliseconds":milliseconds})));
                if let Err(error) = retained {
                    return Judgment {
                        error: Some(error.to_string()),
                        cost_unknown: Some("response retention failed".into()),
                        ..Judgment::default()
                    };
                }
                let door = response.service().and_then(|service| {
                    service
                        .get("door")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
                if response.model
                    != served_name(&self.host.configuration().decision_model, door.as_deref())
                {
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
                        json!({"error":error.to_string(),"body":"unavailable","billing":"unknown",
                            "door":client.base_url(),"via":if client.service().is_some() {"hosted"} else {"direct"},
                            "milliseconds":milliseconds}),
                    ),
                );
                if client.service().is_some()
                    && let Some(why) = jev_hosted::unavailable(&error)
                {
                    let _ = self.host.append(
                        &Step::said(
                            Source::System,
                            &format!("Coder runs without Jev's judgments for the rest of this task: {why}"),
                        )
                        .noting("decision_unavailable", json!({"reason":why})),
                    );
                    *self.off.borrow_mut() = Some(why);
                }
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
    /// Where the reply goes as the model writes it.
    replies: &'a Replies<'a>,
    inner: crate::claude::ClaudeGenerator,
    /// A usage or rate-limit refusal the last call met.
    refusal: RefCell<Option<Refusal>>,
}
impl Generate for Claude<'_> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let sequence = match self.host.effect(
            "claude_request",
            json!({"binary":self.inner.binary,"args":self.inner.args(system),"model":self.inner.model,
                "effort":self.inner.effort,"prompt":prompt,
                "images":self.inner.images().iter().map(crate::images::InputImage::record).collect::<Vec<_>>()}),
        ) {
            Ok(sequence) => sequence,
            Err(error) => return refused_generation(&self.inner.model, false, &error.to_string()),
        };
        self.replies.begin();
        let invocation = self
            .inner
            .invoke_streaming(system, prompt, &mut |text| self.replies.feed(text))
            .await;
        *self.refusal.borrow_mut() = invocation.refusal(task::autostart::unix_now());
        let observation = json!({"status":invocation.status,"stdout":invocation.stdout,"stderr":invocation.stderr,
            "stream_events":invocation.stream_events,
            "model":invocation.generated.model,"usd":invocation.generated.usd,"billing":"provider-reported-list-price"});
        if let Err(error) = self.host.result(sequence, "claude_request", observation) {
            return refused_generation(&self.inner.model, true, &error.to_string());
        }
        invocation.generated
    }

    fn warm(&self, system: &str) {
        self.inner.warm(system);
    }
}

/// One admitted route's native generator.
enum Native<'a, T: codex_transport::Transport> {
    Codex(crate::models::CodexGenerator<Transport<'a, T>>),
    Claude(Claude<'a>),
}

impl<T: codex_transport::Transport> Generate for Native<'_, T> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        match self {
            Native::Codex(generator) => {
                generator.transport.refusal.borrow_mut().take();
                generator.generate(system, prompt).await
            }
            Native::Claude(generator) => generator.generate(system, prompt).await,
        }
    }

    fn warm(&self, system: &str) {
        if let Native::Claude(generator) = self {
            generator.warm(system);
        }
    }
}

impl<T: codex_transport::Transport> Lane for Native<'_, T> {
    fn refusal(&self) -> Option<Refusal> {
        match self {
            Native::Codex(generator) => generator.transport.refusal.borrow_mut().take(),
            Native::Claude(generator) => generator.refusal.borrow_mut().take(),
        }
    }
}

/// The admitted decision model as the door that answered names it
/// (#10107): the hosted decision service fails over from TypeSafe to the
/// Vercel AI Gateway and OpenRouter (`jev::doors`), and each names Jev its
/// own way. An answer naming anything else is still refused.
fn served_name(admitted: &str, door: Option<&str>) -> String {
    use jev::doors::{GATEWAY_DOOR, Naming, OPENROUTER_DOOR};
    match door {
        Some(GATEWAY_DOOR) => Naming::Gateway.model(admitted),
        Some(OPENROUTER_DOOR) => Naming::OpenRouter.model(admitted),
        _ => admitted.to_owned(),
    }
}

/// Run the loop over one stage of admitted model routes, failing over
/// among them, and return its state and outcome for the caller to finish.
pub(super) async fn run_stage<T: codex_transport::Transport>(
    host: &Host,
    book: PathBuf,
    clients: Vec<(GrantRoute, Client<T>)>,
    client: Result<jev::Client, String>,
    session: &str,
    images: &[crate::images::InputImage],
) -> Result<(State, crate::run::Outcome), task::Error> {
    let replies = Replies::new(host);
    let replies = &replies;
    let lanes = clients
        .into_iter()
        .map(|(route, client)| {
            let lane = match client {
                Client::Codex(transport) => Native::Codex(crate::models::CodexGenerator {
                    transport: Transport {
                        host,
                        replies: Some(replies),
                        inner: transport,
                        model: route.model.clone(),
                        refusal: RefCell::new(None),
                    },
                    model: route.model.clone(),
                    effort: route.effort.clone(),
                    cache_key: session.to_owned(),
                    images: images.to_vec(),
                }),
                Client::Claude(generator) => Native::Claude(Claude {
                    host,
                    replies,
                    inner: generator.with_images(images.to_vec()),
                    refusal: RefCell::new(None),
                }),
            };
            (route, lane)
        })
        .collect();
    let journal = Transcript(host);
    let generator = failover(host, &journal, book, lanes, task::autostart::unix_now);
    generator.record_start();
    let judge = NativeJudge {
        host,
        client,
        off: RefCell::new(None),
    };
    run_loop(host, &generator, &judge, replies).await
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use codex_transport::Transport as _;

    #[tokio::test]
    async fn without_jev_a_judgment_answers_nothing_costs_nothing_and_says_why() {
        let (_root, store, grant) = super::super::tests::fixture();
        let host = Host::admit(&store, &grant).await.unwrap();
        let why = "OPENAGENTS_JEV_HOSTED=off turns the hosted decision service off";
        let judge = NativeJudge {
            host: &host,
            client: Err(why.to_string()),
            off: RefCell::new(None),
        };
        let set = microcoder_loop::models::question_set();
        let judgment = judge.judge(&set, &json!({"task": "fixture"})).await;
        assert!(judgment.answers.is_empty());
        assert_eq!(judgment.error.as_deref(), Some(why));
        assert_eq!(judgment.usd, Some(0.0));
        assert!(judgment.cost_unknown.is_none());
        assert!(judgment.render(&set).contains(why));
        host.finish("fixture", false, json!({})).unwrap();
    }

    /// A hosted service that cannot be reached is asked once: the run then
    /// says why it has no Jev and asks nothing more, rather than waiting
    /// out every step.
    #[tokio::test]
    async fn an_unreachable_hosted_service_is_asked_once_and_the_run_says_why() {
        let (_root, store, grant) = super::super::tests::fixture();
        let host = Host::admit(&store, &grant).await.unwrap();
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let relay = format!("ws://{}", closed.local_addr().unwrap());
        drop(closed);
        let home = tempfile::tempdir().unwrap();
        let env = |name: &str| (name == jev_hosted::RELAY_VAR).then(|| relay.clone());
        let client = jev_hosted::resolve(
            &env,
            home.path(),
            &jev_hosted::Door {
                url: jev_hosted::DOOR,
                model: "jev-1.13.0",
            },
            &|config| config,
        )
        .unwrap()
        .client;
        let judge = NativeJudge {
            host: &host,
            client: Ok(client),
            off: RefCell::new(None),
        };
        let set = microcoder_loop::models::question_set();
        let first = judge.judge(&set, &json!({"task": "fixture"})).await;
        assert!(
            first
                .error
                .as_deref()
                .unwrap()
                .starts_with("Jev is unreachable")
        );
        let started = std::time::Instant::now();
        let second = judge.judge(&set, &json!({"task": "fixture"})).await;
        assert_eq!(second.error, first.error);
        assert_eq!(second.usd, Some(0.0));
        assert!(started.elapsed() < std::time::Duration::from_millis(50));
        host.finish("fixture", false, json!({})).unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains(
            "Coder runs without Jev's judgments for the rest of this task: Jev is unreachable"
        ));
        assert!(!trace.contains("no Jev key"));
    }

    /// An answer from a backup door names the admitted Jev its own way and
    /// is accepted; any other name, or the gateway's name from another
    /// door, is not (#10107).
    #[test]
    fn a_backup_doors_name_for_the_admitted_jev_is_the_admitted_jev() {
        use jev::doors::{GATEWAY_DOOR, OPENROUTER_DOOR, TYPESAFE_DOOR};
        let admitted = "jev-1.13.0";
        assert_eq!(served_name(admitted, None), admitted);
        assert_eq!(served_name(admitted, Some(TYPESAFE_DOOR)), admitted);
        assert_eq!(served_name(admitted, Some(GATEWAY_DOOR)), "typesafe-ai/jev");
        assert_eq!(
            served_name(admitted, Some(OPENROUTER_DOOR)),
            "typesafe/jev-1.13"
        );
        assert_ne!(
            served_name(admitted, Some(TYPESAFE_DOOR)),
            "typesafe-ai/jev"
        );
        assert_ne!(
            served_name(admitted, Some("https://elsewhere.example")),
            "typesafe-ai/jev"
        );
    }

    #[tokio::test]
    async fn missing_or_mismatched_native_identity_is_retained_but_never_accepted() {
        for model in ["", "other-model"] {
            let (_root, store, grant) = super::super::tests::fixture();
            let host = Host::admit(&store, &grant).await.unwrap();
            let scripted = codex_transport::fake::FakeTransport::default();
            scripted.then(codex_transport::Reply {
                id: Some("unaccepted-fixture".into()),
                model: model.into(),
                items: vec![json!({"type":"function_call","name":"next_action","arguments":"unaccepted action"})],
                usage: codex_transport::TokenUsage::default(),
            });
            let transport = Transport {
                host: &host,
                replies: None,
                inner: scripted,
                model: "fixture-model".into(),
                refusal: RefCell::new(None),
            };
            let request = codex_transport::Request {
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

    /// A transport that streams `text` in `pieces` before its reply.
    struct Streaming {
        pieces: Vec<&'static str>,
    }

    impl codex_transport::Transport for Streaming {
        async fn respond(
            &self,
            request: &codex_transport::Request,
        ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
            self.respond_streaming(request, &mut |_| {}).await
        }

        async fn respond_streaming(
            &self,
            _request: &codex_transport::Request,
            text: &mut dyn FnMut(&str),
        ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
            for piece in &self.pieces {
                text(piece);
            }
            Ok(codex_transport::Reply {
                id: Some("streamed-fixture".into()),
                model: "fixture-model".into(),
                items: vec![
                    json!({"type":"message","content":[{"type":"output_text","text":self.pieces.concat()}]}),
                ],
                usage: codex_transport::TokenUsage::default(),
            })
        }
    }

    #[tokio::test]
    async fn a_streamed_reply_is_shown_a_paragraph_at_a_time_and_once() {
        let (_root, store, grant) = super::super::tests::fixture();
        let host = Host::admit(&store, &grant).await.unwrap();
        let replies = Replies::new(&host);
        let reply = "First paragraph.\n\n```\na\n\nb\n```\n\nLast line.";
        let transport = Transport {
            host: &host,
            replies: Some(&replies),
            inner: Streaming {
                pieces: vec![
                    "{\"reply\": \"First para",
                    "graph.\\n\\n```\\na\\n",
                    "\\nb\\n```\\n\\nLast",
                    " line.\", \"ask\": \"none\", \"rationale\": \"not shown\", \"commands\": [], ",
                    "\"view\": [], \"freeze_tests\": false, \"expand\": [], \"finished\": true}",
                ],
            },
            model: "fixture-model".into(),
            refusal: RefCell::new(None),
        };
        let request = codex_transport::Request {
            text_format: None,
            model: "fixture-model".into(),
            instructions: String::new(),
            input: Vec::new(),
            tools: Vec::new(),
            effort: None,
            cache_key: "fixture".into(),
            parallel_tools: false,
        };
        transport.respond(&request).await.unwrap();
        let action: crate::models::NextAction = serde_json::from_str(
            &[
                "{\"reply\": ",
                &serde_json::to_string(reply).unwrap(),
                ", \"ask\": \"none\", \"rationale\": \"not shown\", \"commands\": [], \"view\": [], \"freeze_tests\": false, \"expand\": [], \"finished\": true}",
            ]
            .concat(),
        )
        .unwrap();
        let generated = crate::models::Generated {
            action: Ok(action),
            model: "fixture-model".into(),
            prompt_tokens: 0,
            completion_tokens: 0,
            usd: Some(0.0),
            known_usd: 0.0,
            cost_unknown: None,
            usd_upper: Some(0.0),
            cost_basis: crate::models::Basis::ListPrice,
            milliseconds: 1,
        };
        let mut events = RecordedEvents {
            host: &host,
            replies: &replies,
        };
        events.event(
            1.0,
            &Event::Generated {
                step: 1,
                prompt_chars: 1,
                generated,
            },
        );
        drop(transport);
        host.finish("fixture_complete", true, json!({})).unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        let shown: Vec<String> = trace
            .lines()
            .filter_map(|line| coder_history::readable_record_full(line.as_bytes()))
            .filter(|readable| readable.role.as_deref() == Some("assistant"))
            .map(|readable| readable.text)
            .collect();
        // Each whole paragraph once, the fence kept whole, and the step's
        // own record adds nothing the parts did not show.
        assert_eq!(
            shown,
            ["First paragraph.", "```\na\n\nb\n```", "Last line."]
        );
        assert!(trace.contains("\"reply_streamed\":"));
    }

    #[tokio::test]
    async fn native_output_items_and_each_attempt_are_retained_before_reduction() {
        let (_root, store, grant) = super::super::tests::fixture();
        let host = Host::admit(&store, &grant).await.unwrap();
        let scripted = codex_transport::fake::FakeTransport::default();
        scripted.then_fail(codex_transport::TransportError::Stream(
            "fixture disconnect".into(),
        ));
        let items = vec![
            json!({"type":"reasoning","summary":[{"type":"summary_text","text":"retained native reasoning"}]}),
            json!({"type":"function_call","name":"next_action","call_id":"call-fixture","arguments":"not valid action JSON"}),
        ];
        scripted.then(codex_transport::Reply {
            id: Some("native-fixture".into()),
            model: "fixture-model".into(),
            items: items.clone(),
            usage: codex_transport::TokenUsage {
                input: 17,
                cached: 3,
                output: 8,
                reasoning: 4,
            },
        });
        let transport = Transport {
            host: &host,
            replies: None,
            inner: scripted,
            model: "fixture-model".into(),
            refusal: RefCell::new(None),
        };
        let request = codex_transport::Request {
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
