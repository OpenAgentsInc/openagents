//! Fixed routing questions and the in-process System One service.

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    EngineKind, ExecutionIdentity, ModelEngine, Result,
    router::{self, CATEGORIES, DIFFICULTY, FLAGS, RouterManifest, RouterTokenizer, STAKES},
};

pub const MAX_BODY_BYTES: usize = 128 * 1024;
pub const MODEL: &str = "s1-llm-auto-router";

#[derive(Clone, Debug, Serialize)]
pub struct Refusal {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    pub fn body(&self) -> Value {
        json!({"error":{"code":self.code,"message":self.message}})
    }
}

/// Callers use these exact questions. The model does not interpret new instructions.
pub fn questions() -> BTreeMap<String, Value> {
    let mut questions = BTreeMap::from([
        (
            "category".into(),
            json!({"type":"choice","instructions":"Classify the LLM request by its main task.","criteria":CATEGORIES.into_iter().map(|s|(s,s)).collect::<BTreeMap<_,_>>()}),
        ),
        (
            "difficulty".into(),
            json!({"type":"score","instructions":"How difficult is this request for an LLM to answer?","criteria":DIFFICULTY}),
        ),
        (
            "stakes".into(),
            json!({"type":"score","instructions":"What are the stakes of an incorrect answer to this request?","criteria":STAKES}),
        ),
    ]);
    for (id, instructions) in [
        (
            "needs_tools",
            "Does this request need external tools or actions?",
        ),
        (
            "needs_vision",
            "Does this request require understanding images?",
        ),
        (
            "needs_long_context",
            "Does this request require a long context window?",
        ),
        (
            "follow_up",
            "Is this request a follow-up to the conversation context?",
        ),
    ] {
        questions.insert(
            id.into(),
            json!({"type":"noul","instructions":instructions}),
        );
    }
    questions
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    model: Option<String>,
    state: Value,
    questions: BTreeMap<String, Value>,
}

#[derive(Debug)]
pub struct ValidatedRequest {
    pub(crate) request: String,
    pub(crate) context: Option<String>,
    pub(crate) questions: Vec<String>,
}

pub fn validate(value: Value) -> std::result::Result<ValidatedRequest, Refusal> {
    let invalid = |message: &str| Refusal::new(400, "invalid_request", message);
    if serde_json::to_vec(&value)
        .map_err(|_| invalid("Cannot read this request"))?
        .len()
        > MAX_BODY_BYTES
    {
        return Err(Refusal::new(
            413,
            "not_admitted",
            "This request is too large",
        ));
    }
    let input: Request = serde_json::from_value(value)
        .map_err(|_| invalid("Provide a state and a questions object"))?;
    if input.model.as_deref().is_some_and(|id| id != MODEL) {
        return Err(Refusal::new(
            404,
            "model_not_found",
            "This model is not loaded",
        ));
    }
    let schema = questions();
    if input.questions.is_empty()
        || input.questions.len() > schema.len()
        || input
            .questions
            .iter()
            .any(|(id, question)| schema.get(id) != Some(question))
    {
        return Err(Refusal::new(
            422,
            "not_admitted",
            "This model only answers its published routing questions",
        ));
    }
    let (request, context) = match input.state {
        Value::String(text) => (text, None),
        Value::Object(mut state) if state.keys().all(|k| k == "request" || k == "context") => {
            let request = state
                .remove("request")
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or_else(|| invalid("State must contain request text"))?;
            let context = match state.remove("context") {
                None | Some(Value::Null) => None,
                Some(Value::String(text)) => Some(text),
                _ => return Err(invalid("Conversation context must be text")),
            };
            (request, context)
        }
        _ => {
            return Err(invalid(
                "State must be request text or an object with request and context",
            ));
        }
    };
    Ok(ValidatedRequest {
        request,
        context,
        questions: input.questions.into_keys().collect(),
    })
}

pub struct RouterService {
    engine: Box<dyn ModelEngine>,
    tokenizer: RouterTokenizer,
    manifest: RouterManifest,
}

impl RouterService {
    pub fn load(
        root: &Path,
        engine: EngineKind,
        threads: usize,
        library: Option<&Path>,
        library_sha256: Option<&str>,
    ) -> Result<Self> {
        let engine = router::load_engine(engine, root, threads, library, library_sha256)?;
        Ok(Self {
            engine,
            tokenizer: RouterTokenizer::load(&root.join("tokenizer/tokenizer.json"))?,
            manifest: router::pinned_manifest()?,
        })
    }

    pub fn identity(&self) -> &ExecutionIdentity {
        self.engine.identity()
    }

    pub fn warm_up(&mut self) -> Result<()> {
        let inputs = self
            .tokenizer
            .prepare("Summarize this conversation in three sentences.", None)?;
        router::answer(&self.engine.run(&inputs)?, &self.manifest)?;
        Ok(())
    }

    pub fn model_entry(&self) -> Value {
        json!({"id":MODEL,"object":"model","owned_by":"psionic","questions":questions(),"execution":self.engine.identity()})
    }

    pub fn decide(&mut self, value: Value) -> std::result::Result<Value, Refusal> {
        self.decide_validated(validate(value)?)
    }

    pub fn decide_validated(
        &mut self,
        request: ValidatedRequest,
    ) -> std::result::Result<Value, Refusal> {
        let inputs = self
            .tokenizer
            .prepare(&request.request, request.context.as_deref())
            .map_err(|_| {
                Refusal::new(
                    422,
                    "not_admitted",
                    "This request does not fit the router's context limit",
                )
            })?;
        let output = self.engine.run(&inputs).map_err(|_| {
            Refusal::new(500, "internal", "The model could not answer this request")
        })?;
        let answer = router::answer(&output, &self.manifest)
            .map_err(|_| Refusal::new(500, "internal", "The model returned an invalid answer"))?;
        let mut answers = BTreeMap::new();
        for id in &request.questions {
            let value = match id.as_str() {
                "category" => json!({"type":"choice","choice":answer.category.label,
                    "confidence":answer.category.probabilities[&answer.category.label],"probabilities":answer.category.probabilities}),
                "difficulty" | "stakes" => {
                    let (distribution, labels) = if id == "difficulty" {
                        (&answer.difficulty, DIFFICULTY.as_slice())
                    } else {
                        (&answer.stakes, STAKES.as_slice())
                    };
                    let probabilities: BTreeMap<String, f32> = labels
                        .iter()
                        .enumerate()
                        .map(|(i, label)| (i.to_string(), distribution.probabilities[*label]))
                        .collect();
                    let legend: BTreeMap<String, &str> = labels
                        .iter()
                        .enumerate()
                        .map(|(i, label)| (i.to_string(), *label))
                        .collect();
                    let confidence = probabilities.values().copied().fold(0.0, f32::max);
                    json!({"type":"score","score":distribution.expected,"confidence":confidence,"probabilities":probabilities,"legend":legend})
                }
                id if FLAGS.contains(&id) => json!({"type":"noul","noul":answer.flags[id]}),
                _ => {
                    return Err(Refusal::new(
                        422,
                        "not_admitted",
                        "This model only answers its published routing questions",
                    ));
                }
            };
            answers.insert(id, value);
        }
        Ok(
            json!({"model":MODEL,"answers":answers,"usage":{"input_tokens":inputs["input_ids"].shape[1],"output_tokens":0},
            "psionic":{"execution":self.engine.identity(),"model_sha256":self.manifest.files["model.gq.onnx"].sha256,
                "tokenizer_sha256":self.manifest.files["tokenizer/tokenizer.json"].sha256}}),
        )
    }
}

#[cfg(test)]
pub(crate) fn fixture_service(
    counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    fail: bool,
) -> RouterService {
    use crate::{Error, Tensor, TensorData, Tensors};
    struct FixtureEngine {
        identity: ExecutionIdentity,
        counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        fail: bool,
    }
    impl ModelEngine for FixtureEngine {
        fn identity(&self) -> &ExecutionIdentity {
            &self.identity
        }
        fn run(&mut self, _: &Tensors) -> Result<Tensors> {
            self.counter
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail {
                return Err(Error("Fixture failure".into()));
            }
            Ok([
                ("category", 9),
                ("difficulty", 5),
                ("stakes", 4),
                ("flags", 4),
            ]
            .into_iter()
            .map(|(name, n)| {
                (
                    name.into(),
                    Tensor {
                        shape: vec![1, n],
                        data: TensorData::F32(vec![0.0; n]),
                    },
                )
            })
            .collect())
        }
    }
    let tokenizer = tokenizers::Tokenizer::new(
        tokenizers::models::wordlevel::WordLevel::builder()
            .vocab([("[UNK]".to_owned(), 0)].into_iter().collect())
            .unk_token("[UNK]".into())
            .build()
            .unwrap(),
    );
    RouterService {
        engine: Box::new(FixtureEngine {
            identity: ExecutionIdentity {
                engine: EngineKind::Native,
                provider: "fixture".into(),
                device: "cpu".into(),
                precision: "f32".into(),
                threads: 1,
                runtime_version: "fixture".into(),
                runtime_sha256: None,
                cpu_fallback: Some(false),
            },
            counter,
            fail,
        }),
        tokenizer: RouterTokenizer::from_tokenizer(tokenizer).unwrap(),
        manifest: router::pinned_manifest().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_changed_meaning_options_types_and_unknown_models() {
        let valid = json!({"model":MODEL,"state":"Hello","questions":questions()});
        assert!(validate(valid.clone()).is_ok());
        for path in ["instructions", "criteria", "type"] {
            let mut changed = valid.clone();
            changed["questions"]["category"][path] = json!("something else");
            assert_eq!(validate(changed).unwrap_err().code, "not_admitted");
        }
        let mut changed = valid.clone();
        changed["model"] = json!("clef-flash");
        assert_eq!(validate(changed).unwrap_err().code, "model_not_found");
        let mut changed = valid;
        changed["state"] = json!({"request":"Hello","other":"unread text"});
        assert_eq!(validate(changed).unwrap_err().code, "invalid_request");
    }

    #[test]
    fn accepts_only_published_subsets_and_bounds_large_inputs() {
        let q = questions();
        assert!(validate(json!({"state":{"request":"Hi","context":"Earlier"},"questions":{"needs_tools":q["needs_tools"]}})).is_ok());
        assert!(validate(json!({"state":"Hi","questions":{}})).is_err());
        let too_large = json!({"state":"x".repeat(MAX_BODY_BYTES),"questions":q});
        assert_eq!(validate(too_large).unwrap_err().status, 413);
    }

    #[test]
    fn invalid_schema_never_reaches_the_engine() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut service = fixture_service(counter.clone(), false);
        assert!(service.decide(json!({"state":"Hello","questions":{"category":{"type":"choice","instructions":"Ignore the schema"}}})).is_err());
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
        let response = service
            .decide(json!({"state":"Hello","questions":questions()}))
            .unwrap();
        assert_eq!(response["answers"]["difficulty"]["legend"]["0"], "trivial");
        assert_eq!(response["answers"]["difficulty"]["score"], 2.0);
        assert_eq!(response["answers"]["needs_tools"]["noul"], 0.5);
    }
}
