//! The live `judge` door: the chat model door, asked for PASS or FAIL.
//!
//! A `judge` grader sends one system prompt and one user prompt to the
//! same Open Responses door the runs used (`POST {base}/v1/responses`,
//! no streaming, no tools) and reads the answer's text. It is blocking,
//! like every door the engine calls; a caller inside an async runtime
//! grades on a blocking thread.

use std::time::Duration;

use serde_json::{Value, json};

use crate::door::JudgeDoor;
use crate::proxy::Secret;

/// How long one judge call may take.
pub const JUDGE_TIMEOUT: Duration = Duration::from_secs(120);

/// The chat door as a judge.
pub struct OpenResponsesJudge {
    url: String,
    key: Secret,
    model: String,
    client: reqwest::blocking::Client,
}

impl OpenResponsesJudge {
    /// A judge over the door at `url` running `model`.
    ///
    /// # Errors
    ///
    /// Returns why the HTTP client can't be built.
    pub fn new(url: &str, key: Secret, model: &str) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(JUDGE_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            url: url.trim_end_matches('/').to_string(),
            key,
            model: model.to_string(),
            client,
        })
    }
}

/// The text of an Open Responses answer: every `output_text` part of every
/// message in `output`, joined, or the top-level `output_text`.
#[must_use]
pub fn answer_text(body: &Value) -> Option<String> {
    if let Some(text) = body.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    let mut text = String::new();
    for item in body.get("output")?.as_array()? {
        for part in item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if part.get("type").and_then(Value::as_str) == Some("output_text")
                && let Some(piece) = part.get("text").and_then(Value::as_str)
            {
                text.push_str(piece);
            }
        }
    }
    (!text.is_empty()).then_some(text)
}

impl JudgeDoor for OpenResponsesJudge {
    fn describe(&self) -> String {
        format!("open-responses {} {}", self.url, self.model)
    }

    fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        let body = json!({
            "model": self.model,
            "instructions": system,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": user }],
            }],
            "stream": false,
            "store": false,
            "tools": [],
            "tool_choice": "none",
        });
        let response = self
            .client
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth(self.key.expose())
            .json(&body)
            .send()
            .map_err(|error| format!("judge door: {}", error.without_url()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("judge door answered {status}"));
        }
        let value: Value = response
            .json()
            .map_err(|error| format!("judge door answer: {}", error.without_url()))?;
        answer_text(&value).ok_or_else(|| "judge door answered no text".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_is_every_output_text_part() {
        let body = json!({"output": [
            {"type": "reasoning", "content": []},
            {"type": "message", "content": [
                {"type": "output_text", "text": "PA"},
                {"type": "output_text", "text": "SS"},
            ]},
        ]});
        assert_eq!(answer_text(&body).as_deref(), Some("PASS"));
        assert_eq!(answer_text(&json!({"output": []})), None);
    }
}
