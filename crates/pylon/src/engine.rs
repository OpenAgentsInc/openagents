//! The model a pylon serves. The provider asks an [`Engine`] for one chat
//! completion; [`Psionic`] is the engine it uses in production, a
//! `psionic-openai-server` on loopback, and [`Echo`] is the fake for tests.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde_json::{Value, json};

/// Explicit text sampling settings, independent of the model server's defaults.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "SamplingInput")]
pub struct Sampling {
    temperature: f64,
    top_p: f64,
    seed: u32,
}

// Construction and deserialization both exclude NaN, so equality is reflexive.
impl Eq for Sampling {}

#[derive(serde::Deserialize)]
struct SamplingInput {
    temperature: f64,
    top_p: f64,
    seed: u32,
}

impl TryFrom<SamplingInput> for Sampling {
    type Error = String;
    fn try_from(value: SamplingInput) -> Result<Self, Self::Error> {
        Self::new(value.temperature, value.top_p, value.seed)
    }
}

impl Sampling {
    /// Validate sampling settings before starting a provider.
    ///
    /// # Errors
    ///
    /// When temperature is outside 0–2 or top-p is outside (0, 1].
    pub fn new(temperature: f64, top_p: f64, seed: u32) -> Result<Self, String> {
        if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
            return Err("--temperature must be between 0 and 2".into());
        }
        if !top_p.is_finite() || !(0.0 < top_p && top_p <= 1.0) {
            return Err("--top-p must be greater than 0 and at most 1".into());
        }
        Ok(Self {
            temperature,
            top_p,
            seed,
        })
    }
}

/// One message of a conversation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Turn {
    /// `system`, `user`, or `assistant`.
    pub role: String,
    pub content: String,
}

/// A finished completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub text: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub model: String,
    /// Why the answer ended.
    pub finish: Finish,
    /// The model's reasoning before its answer, when the engine keeps it
    /// apart (`reasoning_content`, or the text before `</think>`). The job
    /// result carries it as `reasoning`, capped ([`cap_reasoning`]).
    pub reasoning: Option<String>,
}

/// The most reasoning a job result carries, in bytes.
pub const REASONING_MAX_BYTES: usize = 32 * 1024;

/// `reasoning` held to [`REASONING_MAX_BYTES`], keeping its end: the
/// newest thinking is what led to the answer. `None` for empty text.
#[must_use]
pub fn cap_reasoning(reasoning: &str) -> Option<String> {
    let reasoning = reasoning.trim();
    if reasoning.is_empty() {
        return None;
    }
    if reasoning.len() <= REASONING_MAX_BYTES {
        return Some(reasoning.to_string());
    }
    let mut start = reasoning.len() - REASONING_MAX_BYTES + "…".len();
    while !reasoning.is_char_boundary(start) {
        start += 1;
    }
    Some(format!("…{}", &reasoning[start..]))
}

/// Why an answer ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// The model ended its answer.
    Stop,
    /// The output limit cut the answer off.
    Length,
}

/// A boxed future, so the trait stays object safe.
pub type Pending<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The marker an engine's error starts with when the request is larger than
/// the engine serves: a longer prompt or answer than its context, or state
/// too large for its stages. The provider answers the buyer with
/// `limit_exceeded` and the rest of the error, and the engine stays healthy.
const REQUEST_LIMIT: &str = "request_limit: ";

/// An engine error that says the request exceeds what the engine serves.
#[must_use]
pub fn request_limit(reason: &str) -> String {
    format!("{REQUEST_LIMIT}{reason}")
}

/// Why the request was too large, when an engine error says so.
#[must_use]
pub fn request_limit_reason(error: &str) -> Option<&str> {
    error.strip_prefix(REQUEST_LIMIT)
}

/// A model a pylon can run jobs on.
pub trait Engine: Send + Sync {
    /// Whether the last request ended without confirming worker completion.
    fn recovery_required(&self) -> bool {
        false
    }
    /// The model identifier the beacon advertises.
    fn model(&self) -> &str;
    /// Whether the engine answers right now.
    fn healthy(&self) -> Pending<'_, bool>;
    /// Complete `turns` with at most `max_tokens` new tokens.
    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>>;
    /// Resolves once the work of a dropped [`Engine::generate`] has stopped,
    /// so the provider frees its slot only then. Immediate unless the engine
    /// stops asynchronously, as a model group does.
    fn settled(&self) -> Pending<'_, ()> {
        Box::pin(async {})
    }
}

/// A Psionic model server's OpenAI-compatible `/v1/chat/completions`, built
/// from `crates/psionic` and listening on loopback.
pub struct Psionic {
    base: String,
    model: String,
    client: reqwest::Client,
    /// The name the server knows the model by, read once from its
    /// `/v1/models` (see [`served_name`]).
    served: tokio::sync::OnceCell<String>,
    stop: Vec<String>,
    sampling: Option<Sampling>,
    uncertain: std::sync::atomic::AtomicBool,
}

/// The id in a `/v1/models` body to request `model` by. Psionic lists a
/// model by its file name (`Qwen3.5-0.8B-Q8_0.gguf`) and matches requests
/// case-sensitively, while a beacon names it `qwen3.5-0.8b-q8_0`; so pick
/// the listed id that equals `model` ignoring case and a `.gguf` ending,
/// else the only model listed, else `model` itself.
#[must_use]
pub fn served_name(model: &str, models: &Value) -> Option<String> {
    let plain = |name: &str| {
        let lower = name.to_lowercase();
        lower.strip_suffix(".gguf").unwrap_or(&lower).to_string()
    };
    let ids: Vec<&str> = models["data"]
        .as_array()?
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();
    ids.iter()
        .find(|id| plain(id) == plain(model))
        .or_else(|| (ids.len() == 1).then(|| &ids[0]))
        .map(|id| (*id).to_string())
}

impl Psionic {
    /// The server at `base` (such as `http://127.0.0.1:18080`) serving
    /// `model`.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn new(base: &str, model: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client,
            served: tokio::sync::OnceCell::new(),
            stop: Vec::new(),
            sampling: None,
            uncertain: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Stop before any configured transcript boundary.
    #[must_use]
    pub fn with_stops(mut self, stop: Vec<String>) -> Self {
        self.stop = stop;
        self
    }

    /// Send the same explicit sampling settings with every text request.
    #[must_use]
    pub fn with_sampling(mut self, sampling: Sampling) -> Self {
        self.sampling = Some(sampling);
        self
    }

    fn request_body(&self, model: &str, messages: Vec<Value>, max_tokens: u32) -> Value {
        let mut body = json!({"model": model, "messages": messages, "max_tokens": max_tokens, "stream": false, "n": 1});
        if !self.stop.is_empty() {
            body["stop"] = json!(self.stop);
        }
        if let Some(sampling) = self.sampling {
            body["temperature"] = json!(sampling.temperature);
            body["top_p"] = json!(sampling.top_p);
            body["seed"] = json!(sampling.seed);
        }
        body
    }

    /// The name to request the model by: [`served_name`] over the
    /// server's list, remembered once read; `model` while it cannot be read.
    async fn request_name(&self) -> String {
        let read = self
            .served
            .get_or_try_init(|| async {
                let body: Value = self
                    .client
                    .get(format!("{}/v1/models", self.base))
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                    .map_err(|_| ())?
                    .json()
                    .await
                    .map_err(|_| ())?;
                served_name(&self.model, &body).ok_or(())
            })
            .await;
        read.map_or_else(|()| self.model.clone(), Clone::clone)
    }
}

impl Engine for Psionic {
    fn recovery_required(&self) -> bool {
        self.uncertain.load(std::sync::atomic::Ordering::Acquire)
    }
    fn model(&self) -> &str {
        &self.model
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async move {
            self.client
                .get(format!("{}/v1/models", self.base))
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
        })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let messages: Vec<Value> = turns
                .iter()
                .map(|t| json!({"role": t.role, "content": t.content}))
                .collect();
            let request = self.request_name().await;
            self.uncertain
                .store(true, std::sync::atomic::Ordering::Release);
            let response = self
                .client
                .post(format!("{}/v1/chat/completions", self.base))
                .json(&self.request_body(&request, messages, max_tokens))
                .send()
                .await
                .map_err(|e| format!("model server unreachable: {e}"))?;
            let status = response.status();
            if !status.is_success() {
                self.uncertain.store(
                    status.is_server_error() || status.as_u16() == 429,
                    std::sync::atomic::Ordering::Release,
                );
                return Err(format!("model server refused with {status}"));
            }
            let body: Value = response
                .json()
                .await
                .map_err(|e| format!("model server answered with no JSON: {e}"))?;
            self.uncertain
                .store(false, std::sync::atomic::Ordering::Release);
            let text = finished_text(&body)?;
            let reasoning = body["choices"][0]["message"]["reasoning_content"]
                .as_str()
                .and_then(cap_reasoning);
            Ok(Generation {
                text,
                reasoning,
                input_tokens: body["usage"]["prompt_tokens"].as_u64(),
                output_tokens: body["usage"]["completion_tokens"].as_u64(),
                model: self.model.clone(),
                // `finished_text` refuses every answer that didn't stop.
                finish: Finish::Stop,
            })
        })
    }
}

/// Pylon delivers one final text answer. A token limit (even with some final
/// text), unfinished thought or tool request is not a completed answer.
fn finished_text(body: &Value) -> Result<String, String> {
    let choices = body["choices"]
        .as_array()
        .filter(|choices| choices.len() == 1)
        .ok_or("model server must return exactly one answer")?;
    let choice = &choices[0];
    match choice["finish_reason"].as_str() {
        Some("stop") => {}
        Some("length") => {
            return Err("model reached its output token limit before finishing".into());
        }
        Some("tool_calls" | "function_call") => {
            return Err("model requested a tool instead of a final text answer".into());
        }
        Some("content_filter") => return Err("model declined to answer".into()),
        _ => return Err("model server did not confirm a finished answer".into()),
    }
    let message = &choice["message"];
    if (!message["tool_calls"].is_null()
        && !message["tool_calls"].as_array().is_some_and(Vec::is_empty))
        || !message["function_call"].is_null()
    {
        return Err("model requested a tool instead of a final text answer".into());
    }
    if message["refusal"]
        .as_str()
        .is_some_and(|text| !text.trim().is_empty())
    {
        return Err("model declined to answer".into());
    }
    let text = message["content"].as_str().unwrap_or_default();
    if text.trim().is_empty() {
        return Err(if message["reasoning_content"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty())
        {
            "model returned reasoning without a final answer"
        } else {
            "model server returned no final text"
        }
        .into());
    }
    Ok(text.to_string())
}

/// A fake engine that answers with the last user message reversed, for
/// tests that need no model.
pub struct Echo;

impl Engine for Echo {
    fn model(&self) -> &str {
        "echo"
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        _max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let last = turns
                .iter()
                .rev()
                .find(|t| t.role == "user")
                .ok_or("no user message")?;
            Ok(Generation {
                text: format!("echo: {}", last.content.chars().rev().collect::<String>()),
                input_tokens: Some(last.content.len() as u64),
                output_tokens: Some(last.content.len() as u64),
                model: "echo".into(),
                finish: Finish::Stop,
                reasoning: Some("I reverse the last message.".into()),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_limit_is_marked_and_read_back() {
        let error = request_limit("the prompt is 9 tokens; this group takes at most 4");
        assert_eq!(
            request_limit_reason(&error),
            Some("the prompt is 9 tokens; this group takes at most 4")
        );
        assert_eq!(request_limit_reason("the model failed"), None);
    }

    #[test]
    fn only_a_finished_final_answer_is_deliverable() {
        let cases = [
            (json!({"choices": []}), "exactly one"),
            (
                json!({"choices": [{"message": {"content": "partial"}}]}),
                "did not confirm",
            ),
            (
                json!({"choices": [{"finish_reason": "length", "message": {"content": "partial", "reasoning_content": "thought"}}]}),
                "token limit",
            ),
            (
                json!({"choices": [{"finish_reason": "stop", "message": {"content": "", "reasoning_content": "unfinished thought"}}]}),
                "reasoning without",
            ),
            (
                json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": "calling tool"}}]}),
                "requested a tool",
            ),
            (
                json!({"choices": [{"finish_reason": "stop", "message": {"content": "calling tool", "tool_calls": [{"id": "x"}]}}]}),
                "requested a tool",
            ),
            (
                json!({"choices": [{"finish_reason": "content_filter", "message": {"content": "partial"}}]}),
                "declined",
            ),
            (
                json!({"choices": [{"finish_reason": "stop", "message": {"refusal": "no"}}]}),
                "declined",
            ),
            (
                json!({"choices": [{"finish_reason": "stop", "message": {"content": "  "}}]}),
                "no final text",
            ),
        ];
        for (body, expected) in cases {
            let error = finished_text(&body).unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("unfinished thought"));
        }
        let body = json!({"choices": [{"finish_reason": "stop", "message": {"content": "  2\n", "reasoning_content": "private thought"}}]});
        assert_eq!(finished_text(&body).unwrap(), "  2\n");
    }

    #[tokio::test]
    async fn completion_sends_stops_and_rejects_truncation_without_retry() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for finish in [None, Some("length"), Some("stop")] {
                let completion = finish.is_some();
                let (stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                assert!(line.starts_with(if completion {
                    "POST /v1/chat/completions "
                } else {
                    "GET /v1/models "
                }));
                let mut length = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).await.unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).await.unwrap();
                let response = if completion {
                    let request: Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(request["stop"], json!(["<extra_id_1>", "next,turn"]));
                    assert_eq!(request["model"], "actual.gguf");
                    assert_eq!(request["max_tokens"], 32);
                    assert_eq!(request["stream"], false);
                    assert_eq!(request["n"], 1);
                    assert_eq!(request["temperature"], 0.6);
                    assert_eq!(request["top_p"], 0.95);
                    assert_eq!(request["seed"], 1);
                    json!({"choices":[{"finish_reason":finish,"message":{"content":"\n\nREADY\n","reasoning_content":"private thought"}}],"usage":{"prompt_tokens":12,"completion_tokens":1}})
                } else {
                    json!({"data":[{"id":"actual.gguf"}]})
                }.to_string();
                reader.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).as_bytes()).await.unwrap();
            }
        });
        let engine = Psionic::new(&address, "advertised")
            .unwrap()
            .with_stops(vec!["<extra_id_1>".into(), "next,turn".into()])
            .with_sampling(Sampling::new(0.6, 0.95, 1).unwrap());
        let error = engine
            .generate(
                &[Turn {
                    role: "user".into(),
                    content: "Reply READY".into(),
                }],
                32,
            )
            .await
            .unwrap_err();
        assert!(error.contains("token limit"));
        let answer = engine
            .generate(
                &[Turn {
                    role: "user".into(),
                    content: "Reply READY".into(),
                }],
                32,
            )
            .await
            .unwrap();
        assert_eq!(answer.text, "\n\nREADY\n");
        // The reasoning stays apart from the answer, for the result's
        // optional `reasoning` field.
        assert_eq!(answer.reasoning.as_deref(), Some("private thought"));
        server.await.unwrap();
    }

    #[test]
    fn reasoning_is_capped_from_the_start() {
        assert_eq!(cap_reasoning("  \n"), None);
        assert_eq!(cap_reasoning(" why ").as_deref(), Some("why"));
        let long = format!("{}END", "é".repeat(REASONING_MAX_BYTES));
        let capped = cap_reasoning(&long).unwrap();
        assert!(capped.len() <= REASONING_MAX_BYTES);
        assert!(capped.starts_with('…') && capped.ends_with("END"));
    }

    #[test]
    fn the_served_name_matches_the_beacon_name_by_file_name() {
        let listed = json!({"data": [{"id": "Qwen3.5-0.8B-Q8_0.gguf"}]});
        assert_eq!(
            served_name("qwen3.5-0.8b-q8_0", &listed).as_deref(),
            Some("Qwen3.5-0.8B-Q8_0.gguf")
        );
        let two = json!({"data": [{"id": "a.gguf"}, {"id": "Qwen3.5-0.8B-Q8_0"}]});
        assert_eq!(
            served_name("qwen3.5-0.8b-q8_0", &two).as_deref(),
            Some("Qwen3.5-0.8B-Q8_0")
        );
        // One model listed under another name: that one.
        assert_eq!(
            served_name("m", &listed).as_deref(),
            Some("Qwen3.5-0.8B-Q8_0.gguf")
        );
        // Several and none matching: ask by the given name.
        assert_eq!(served_name("m", &two), None);
        assert_eq!(served_name("m", &json!({})), None);
    }
}
