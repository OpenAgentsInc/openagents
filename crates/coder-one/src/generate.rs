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

use std::time::Duration;

use serde_json::{Value, json};

use crate::agent::Generate;
use crate::credentials::Secret;

/// How long one generation may take, from request to the last event.
const REQUEST_DEADLINE: Duration = Duration::from_secs(300);

/// How many times a rate-limited or failed request is retried.
const RETRIES: u32 = 3;

/// Token counts the door reported for one generation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// An Open Responses door on one lane.
pub struct Door {
    http: reqwest::Client,
    url: String,
    bearer: Secret,
    lane: String,
    instructions: String,
    on_delta: Box<dyn FnMut(&str)>,
    /// The usage of every generation so far, summed.
    pub usage: Usage,
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
            usage: Usage::default(),
        })
    }

    async fn attempt(&mut self, prompt: &str) -> Result<(String, Usage), Attempt> {
        let body = json!({
            "model": self.lane,
            "instructions": self.instructions,
            "input": prompt,
            "tools": tools(),
            "tool_choice": "required",
            "parallel_tool_calls": false,
            "stream": true,
        });
        let mut response = self
            .http
            .post(&self.url)
            .bearer_auth(self.bearer.expose())
            .json(&body)
            .send()
            .await
            .map_err(|error| Attempt::Retry(format!("request failed: {error}")))?;
        let status = response.status();
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
        Ok((reply, usage))
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
            "description": "Stop because the issue is resolved and checked. The host \
                commits the changes and uses the title and summary for the pull request.",
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
        let started = std::time::Instant::now();
        let before = self.usage;
        let result = self.generate_retrying(prompt).await;
        println!(
            "\n  gen ▸ {:.1}s, {} in / {} out tokens",
            started.elapsed().as_secs_f64(),
            self.usage.input_tokens - before.input_tokens,
            self.usage.output_tokens - before.output_tokens
        );
        result
    }
}

impl Door {
    async fn generate_retrying(&mut self, prompt: &str) -> Result<String, String> {
        print!("  gen ▸ ");
        let mut last = String::new();
        for attempt in 0..=RETRIES {
            if attempt > 0 {
                let wait = Duration::from_secs(5 * u64::from(attempt));
                println!("\n  gen ▸ retry {attempt} in {}s: {last}", wait.as_secs());
                tokio::time::sleep(wait).await;
            }
            match self.attempt(prompt).await {
                Ok((text, usage)) => {
                    self.usage.input_tokens += usage.input_tokens;
                    self.usage.output_tokens += usage.output_tokens;
                    return Ok(text);
                }
                Err(Attempt::Fatal(why)) => return Err(why),
                Err(Attempt::Retry(why)) => last = why,
            }
        }
        Err(last)
    }
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
                });
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
                output_tokens: 3
            })
        );
    }
}
