//! Preserve native model evidence before reducing it to loop judgments/actions.
use super::*;

struct Transport<'a, T> {
    host: &'a Host,
    inner: T,
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
            && (reply.model.is_empty() || reply.model != self.host.configuration().model)
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

pub(super) async fn run<T: microluna::Transport>(
    host: Host,
    transport: T,
    client: jev::Client,
    session: String,
) -> Result<task::Task, task::Error> {
    let (state, outcome) = {
        let configuration = host.configuration();
        let generator = crate::models::CodexGenerator {
            transport: Transport {
                host: &host,
                inner: transport,
            },
            model: configuration.model.clone(),
            effort: configuration.effort.clone(),
            cache_key: session,
        };
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
            };
            let request = microluna::Request {
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
        };
        let request = microluna::Request {
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
