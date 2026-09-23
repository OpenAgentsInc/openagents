//! Generation through the OpenAgents service's Open Responses door:
//! `POST {base}/v1/responses` with a bearer, streamed back as Server-Sent
//! Events.
//!
//! The two actions are declared as native function tools, `shell` and
//! `finished`, with `tool_choice: required`, so the model answers with a
//! structured `function_call` item rather than JSON written into text.
//! The door turns that call into the action object
//! [`crate::Action::parse`] validates: the tool name becomes `action`, and
//! the arguments become the other fields.

use std::time::{Duration, Instant};

use atif::document::{Source, Step};
use serde_json::{Value, json};

use crate::agent::Generate;
use crate::credentials::Secret;
use crate::record::Recorder;

/// How long one generation may take, from request to the last event.
const REQUEST_DEADLINE: Duration = Duration::from_secs(300);

/// How many times a rate-limited or failed request is retried.
const RETRIES: u32 = 6;

/// The longest wait before one retry. Backoff doubles from five seconds
/// up to this, unless the door names its own wait in `Retry-After`. The
/// `free` lane allows 20 generations a minute for one account, so a
/// rate-limited run must be able to wait out a whole minute.
const MAX_WAIT: Duration = Duration::from_secs(65);

/// What the door reported for one generation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Input tokens served from the provider's cache, when reported.
    pub cached_tokens: Option<u64>,
    /// The door's reported cost in millionths of a dollar, when reported.
    pub cost_microusd: Option<u64>,
}

/// Every generation's accounting, summed over the run.
#[derive(Debug, Clone, Default)]
pub struct Tally {
    pub usage: Usage,
    /// Generations that returned an answer.
    pub calls: u32,
    /// Attempts that failed, including ones a retry recovered.
    pub failed: u32,
    /// Retries sent after a failed attempt.
    pub retries: u32,
    /// Answered generations that reported no cost.
    pub unpriced: u32,
    /// The models the door reported serving, in first-seen order.
    pub models: Vec<String>,
}

/// An Open Responses door on one lane.
pub struct Door {
    http: reqwest::Client,
    url: String,
    bearer: Secret,
    lane: String,
    instructions: String,
    on_delta: Box<dyn FnMut(&str)>,
    recorder: Recorder,
    /// The wait the door asked for on its last 429, from `Retry-After`.
    retry_after: Option<Duration>,
    /// Sent as `prompt_cache_key` so a provider routes one run's requests
    /// to the same cache.
    cache_key: Option<String>,
    /// Every generation's accounting so far.
    pub tally: Tally,
    /// The episode deadline each request and wait is bounded by.
    episode: crate::deadline::Deadline,
}

impl Door {
    /// A door at `base` answering on `lane`. `instructions` go with every
    /// request as the system prompt; `on_delta` receives text as it
    /// streams.
    pub fn new(
        base: &str,
        bearer: Secret,
        lane: &str,
        instructions: &str,
        on_delta: Box<dyn FnMut(&str)>,
        recorder: Recorder,
    ) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_DEADLINE)
            .build()
            .map_err(|error| format!("cannot build the HTTP client: {error}"))?;
        Ok(Self {
            http,
            url: format!("{}/v1/responses", base.trim_end_matches('/')),
            bearer,
            lane: lane.to_string(),
            instructions: instructions.to_string(),
            on_delta,
            recorder,
            retry_after: None,
            cache_key: None,
            tally: Tally::default(),
            episode: crate::deadline::Deadline::unbounded(),
        })
    }

    /// Bounds every request and every wait by the episode `deadline`.
    #[must_use]
    pub fn within(mut self, deadline: crate::deadline::Deadline) -> Self {
        self.episode = deadline;
        self
    }

    /// Sends `key` as every request's `prompt_cache_key`.
    #[must_use]
    pub fn caching_under(mut self, key: &str) -> Self {
        self.cache_key = Some(key.to_string());
        self
    }

    async fn attempt(
        &mut self,
        prompt: &str,
        limit: Duration,
    ) -> Result<(String, Usage, String), Attempt> {
        let mut body = json!({
            "model": self.lane,
            "instructions": self.instructions,
            "input": prompt,
            "tools": tools(),
            "tool_choice": "required",
            "parallel_tool_calls": false,
            "stream": true,
        });
        if let Some(key) = &self.cache_key {
            body["prompt_cache_key"] = json!(key);
        }
        let mut response = self
            .http
            .post(&self.url)
            .timeout(limit)
            .bearer_auth(self.bearer.expose())
            .json(&body)
            .send()
            .await
            .map_err(|error| Attempt::Retry(format!("request failed: {error}")))?;
        let status = response.status();
        if status.as_u16() == 429 {
            self.retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            let message = format!("{status}: {}", excerpt(&text, 300));
            return Err(if status.as_u16() == 429 || status.is_server_error() {
                Attempt::Retry(message)
            } else {
                Attempt::Fatal(message)
            });
        }

        let mut events = Events::default();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| Attempt::Retry(format!("stream broke: {error}")))?
        {
            for event in events.push(&chunk) {
                match event {
                    Event::Delta(text) => (self.on_delta)(&text),
                    Event::Failed(why) => return Err(Attempt::Retry(why)),
                    Event::Other => {}
                }
            }
        }
        let Some(usage) = events.usage else {
            return Err(Attempt::Retry(
                "the stream ended without response.completed".to_string(),
            ));
        };
        // A function call is the answer; text alone goes to the parser,
        // which records it as a malformed step.
        let reply = match events.call {
            Some((name, arguments)) => action_json(&name, &arguments),
            None => events.text,
        };
        Ok((reply, usage, events.model))
    }
}

/// The two tools the model may call. Their names are the action tags
/// [`crate::Action`] accepts.
fn tools() -> Value {
    json!([
        {
            "type": "function",
            "name": "shell",
            "description": "Run one bash command in the repository root. The command \
                has no terminal and no standard input; its output comes back next step.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The bash command." },
                    "reason": {
                        "type": "string",
                        "description": "One sentence: what this command should show or change, and why it is the next step."
                    }
                },
                "required": ["command", "reason"],
                "additionalProperties": false
            }
        },
        {
            "type": "function",
            "name": "finished",
            "description": "Stop because the task is done and checked. The title and \
                summary describe the change; when the task is a GitHub issue, they become \
                the pull request's.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "The pull request title." },
                    "summary": { "type": "string", "description": "What changed and how it was checked." }
                },
                "required": ["title", "summary"],
                "additionalProperties": false
            }
        }
    ])
}

/// The action object for a function call: the arguments with the tool
/// name added as `action`. Arguments that are not a JSON object are kept
/// as text so the parser reports them.
fn action_json(name: &str, arguments: &str) -> String {
    match serde_json::from_str::<Value>(arguments) {
        Ok(Value::Object(mut fields)) => {
            fields.insert("action".to_string(), Value::String(name.to_string()));
            Value::Object(fields).to_string()
        }
        _ => format!("{name}({arguments})"),
    }
}

enum Attempt {
    Retry(String),
    Fatal(String),
}

impl Generate for Door {
    async fn generate(&mut self, prompt: &str) -> Result<String, String> {
        let started = Instant::now();
        print!("  gen ▸ ");
        let mut last = String::new();
        // A request refused with a 4xx other than 429, or never sent, did
        // no billed work; any other failure may have.
        let mut charge = "unknown";
        for attempt in 0..=RETRIES {
            if attempt > 0 {
                self.tally.retries += 1;
                let backoff = Duration::from_secs(5 << (attempt - 1).min(4));
                let wait = self.retry_after.take().unwrap_or(backoff).min(MAX_WAIT);
                let wait = self.episode.allowance().map_or(wait, |left| wait.min(left));
                println!("\n  gen ▸ retry {attempt} in {}s: {last}", wait.as_secs());
                tokio::time::sleep(wait).await;
            }
            let Some(limit) = self.episode.grant("generation", REQUEST_DEADLINE) else {
                if attempt == 0 {
                    charge = "zero";
                }
                last = "the episode deadline left no time".to_string();
                break;
            };
            match self.attempt(prompt, limit).await {
                Ok((reply, usage, model)) => {
                    self.account(usage, &model);
                    let milliseconds = elapsed_ms(started);
                    println!(
                        "\n  gen ▸ {:.1}s, {} in ({} cached) / {} out tokens, {}",
                        milliseconds as f64 / 1000.0,
                        usage.input_tokens,
                        usage
                            .cached_tokens
                            .map_or("?".to_string(), |c| c.to_string()),
                        usage.output_tokens,
                        if model.is_empty() { &self.lane } else { &model }
                    );
                    let mut step = Step::said(Source::Agent, &reply)
                        .taking(milliseconds)
                        .noting("lane", json!(self.lane))
                        .noting("attempts", json!(attempt + 1));
                    if !model.is_empty() {
                        step = step.by(&model);
                    }
                    step.spent(atif::document::Usage {
                        prompt: usage.input_tokens,
                        completion: usage.output_tokens,
                    });
                    if let Some(cost) = usage.cost_microusd {
                        step = step.noting("cost_microusd", json!(cost));
                    }
                    if let Some(cached) = usage.cached_tokens {
                        step = step.noting("cached_tokens", json!(cached));
                    }
                    self.recorder.push(step);
                    return Ok(reply);
                }
                Err(Attempt::Fatal(why)) => {
                    self.tally.failed += 1;
                    if attempt == 0 && why.starts_with('4') {
                        charge = "zero";
                    }
                    last = why;
                    break;
                }
                Err(Attempt::Retry(why)) => {
                    self.tally.failed += 1;
                    last = why;
                }
            }
        }
        println!("\n  gen ▸ failed: {last}");
        self.recorder.push(
            Step::said(Source::System, &format!("generation failed: {last}"))
                .taking(elapsed_ms(started))
                .noting("lane", json!(self.lane))
                .noting("charge", json!(charge)),
        );
        Err(last)
    }
}

impl Door {
    fn account(&mut self, usage: Usage, model: &str) {
        let tally = &mut self.tally;
        tally.calls += 1;
        tally.usage.input_tokens += usage.input_tokens;
        tally.usage.output_tokens += usage.output_tokens;
        if let Some(cached) = usage.cached_tokens {
            tally.usage.cached_tokens = Some(tally.usage.cached_tokens.unwrap_or(0) + cached);
        }
        match usage.cost_microusd {
            Some(cost) => {
                tally.usage.cost_microusd = Some(tally.usage.cost_microusd.unwrap_or(0) + cost);
            }
            None => tally.unpriced += 1,
        }
        if !model.is_empty() && !tally.models.iter().any(|seen| seen == model) {
            tally.models.push(model.to_string());
        }
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// One Server-Sent Event, as far as the loop cares.
enum Event {
    Delta(String),
    Failed(String),
    Other,
}

/// An SSE reader: bytes in, events out, the answer text accumulated.
#[derive(Default)]
struct Events {
    buffer: Vec<u8>,
    text: String,
    /// The first completed function call: its name and raw arguments.
    call: Option<(String, String)>,
    usage: Option<Usage>,
    /// The model the door reported serving, from `response.completed`.
    model: String,
}

impl Events {
    fn push(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(data) = line.trim_end().strip_prefix("data:") else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };
            events.push(self.read(&event));
        }
        events
    }

    fn read(&mut self, event: &Value) -> Event {
        match event["type"].as_str().unwrap_or_default() {
            "response.output_text.delta" => {
                let delta = event["delta"].as_str().unwrap_or_default().to_string();
                self.text.push_str(&delta);
                Event::Delta(delta)
            }
            "response.function_call_arguments.delta" => {
                Event::Delta(event["delta"].as_str().unwrap_or_default().to_string())
            }
            "response.output_item.done" => {
                let item = &event["item"];
                if item["type"] == "function_call" && self.call.is_none() {
                    self.call = Some((
                        item["name"].as_str().unwrap_or_default().to_string(),
                        item["arguments"].as_str().unwrap_or_default().to_string(),
                    ));
                }
                Event::Other
            }
            "response.completed" => {
                let usage = &event["response"]["usage"];
                self.usage = Some(Usage {
                    input_tokens: usage["input_tokens"].as_u64().unwrap_or(0),
                    output_tokens: usage["output_tokens"].as_u64().unwrap_or(0),
                    cached_tokens: usage["input_tokens_details"]["cached_tokens"].as_u64(),
                    cost_microusd: usage["cost_microusd"].as_u64(),
                });
                self.model = event["response"]["model"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                Event::Other
            }
            "response.failed" | "error" => Event::Failed(
                event["response"]["error"]["message"]
                    .as_str()
                    .or(event["message"].as_str())
                    .unwrap_or("the generation failed upstream")
                    .to_string(),
            ),
            _ => Event::Other,
        }
    }
}

fn excerpt(text: &str, max: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_function_call_becomes_the_action_object() {
        let mut events = Events::default();
        let stream = concat!(
            "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"command\\\":\"}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",",
            "\"name\":\"shell\",\"arguments\":\"{\\\"command\\\":\\\"ls -la\\\"}\",\"call_id\":\"c1\"}}\n\n",
        );
        events.push(stream.as_bytes());
        let (name, arguments) = events.call.expect("a call");
        assert_eq!(
            crate::Action::parse(&action_json(&name, &arguments)),
            Ok(crate::Action::Shell {
                command: "ls -la".to_string(),
                reason: None
            })
        );
        assert!(crate::Action::parse(&action_json("shell", "not json")).is_err());
    }

    #[test]
    fn reads_deltas_and_usage_across_split_chunks() {
        let mut events = Events::default();
        let stream = concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"act\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ion\\\"\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"usage\":",
            "{\"input_tokens\":12,\"output_tokens\":3}}}\n\n",
        );
        let (first, second) = stream.split_at(40);
        let mut deltas = 0;
        for chunk in [first, second] {
            for event in events.push(chunk.as_bytes()) {
                if let Event::Delta(_) = event {
                    deltas += 1;
                }
            }
        }
        assert_eq!(deltas, 2);
        assert_eq!(events.text, "{\"action\"");
        assert_eq!(
            events.usage,
            Some(Usage {
                input_tokens: 12,
                output_tokens: 3,
                cached_tokens: None,
                cost_microusd: None,
            })
        );
    }
}
