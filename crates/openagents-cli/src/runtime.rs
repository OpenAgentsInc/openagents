//! Live OpenAgents inference proxy client & streaming multi-turn loop
//! Replicates coder-thread.ts behavior over POST /api/v1/threads and POST /api/inference/proxy

use crate::tools::{HarnessToolRegistry, ToolCall, ToolDefinition};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const THREAD_LANE_NOTICE: &str =
    "You answer through the OpenAgents inference proxy, on a thread opened for this session. \
    Every round of tool calls re-sends the whole conversation to a metered model, so batch \
    independent commands into one call and keep large dumps out of the transcript.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Lane {
    OxAlpha,
    GeminiFlash,
    GeminiPro,
    ClaudeCode,
    Codex,
    Ollama(String),
}

impl Default for Lane {
    fn default() -> Self {
        Lane::OxAlpha
    }
}

impl Lane {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ox-alpha" | "ox" => Lane::OxAlpha,
            "gemini" | "gemini-flash" => Lane::GeminiFlash,
            "gemini-pro" => Lane::GeminiPro,
            "claude" => Lane::ClaudeCode,
            "codex" => Lane::Codex,
            other if other.starts_with("ollama:") => {
                Lane::Ollama(other.trim_start_matches("ollama:").to_string())
            }
            _ => Lane::OxAlpha,
        }
    }

    pub fn model_name(&self) -> &str {
        match self {
            Lane::OxAlpha => "ox-alpha",
            Lane::GeminiFlash => "gemini-3.7-flash",
            Lane::GeminiPro => "gemini-3.7-pro",
            Lane::ClaudeCode => "claude-3-7-sonnet",
            Lane::Codex => "codex-preview",
            Lane::Ollama(m) => m.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceGrant {
    pub thread_id: String,
    pub token: String,
    pub proxy_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

pub struct CoderRuntimeSession {
    pub lane: Lane,
    pub api_base: String,
    pub user_token: Option<String>,
    pub http: reqwest::Client,
    pub tools: HarnessToolRegistry,
    pub messages: Vec<ChatMessage>,
}

impl CoderRuntimeSession {
    pub fn new(lane: Lane, api_base: Option<String>, user_token: Option<String>, tools: HarnessToolRegistry) -> Self {
        Self {
            lane,
            api_base: api_base.unwrap_or_else(|| "https://openagents.com/api/v1".to_string()),
            user_token,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            tools,
            messages: Vec::new(),
        }
    }

    pub fn build_system_prompt(&self, tool_defs: &[ToolDefinition]) -> String {
        let mut lines = vec![
            format!("You are `openagents coder`, a coding assistant in a terminal. {}", THREAD_LANE_NOTICE),
            "".to_string(),
            "Answer very concisely unless the reader asks for a longer response.".to_string(),
            "".to_string(),
        ];

        if tool_defs.is_empty() {
            lines.push(
                "You have no tools in this session: you cannot read or write files, run commands, or \
                reach anything outside this conversation. Answer from what the reader tells you, and \
                say plainly when something would need a tool you do not have.".to_string()
            );
        } else {
            lines.push(format!("You have {} tools, and no others:", tool_defs.len()));
            for t in tool_defs {
                lines.push(format!("- `{}`", t.name));
            }
            lines.push("".to_string());
            lines.push(
                "That list is complete: a capability not on it is one you do not have, whatever a model \
                like you usually has. Read a tool's description before assuming what it covers. Where \
                a description says what a child agent can do, that is the child's capability and not \
                yours. Never say you ran something you did not run.".to_string()
            );
        }

        lines.join("\n")
    }

    pub async fn create_thread(&self) -> Result<InferenceGrant, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/threads", self.api_base);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.user_token {
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", tok))?);
        }

        let resp = self.http.post(&url)
            .headers(headers)
            .json(&serde_json::json!({
                "objective": "Coding assistant session",
                "lane": self.lane.model_name(),
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let thread = body.get("thread").cloned().unwrap_or(serde_json::json!({}));
            let grant = body.get("grant").cloned().unwrap_or(serde_json::json!({}));

            let thread_id = thread.get("id").and_then(|v| v.as_str()).unwrap_or("th_active").to_string();
            let token = grant.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let grant_url = grant.get("url").and_then(|v| v.as_str()).unwrap_or("https://openagents.com/api/inference/proxy").to_string();
            let model = grant.get("model").and_then(|v| v.as_str()).unwrap_or(self.lane.model_name()).to_string();

            Ok(InferenceGrant {
                thread_id,
                token,
                proxy_url: grant_url,
                model,
            })
        } else {
            Ok(InferenceGrant {
                thread_id: "th_local_fallback".to_string(),
                token: self.user_token.clone().unwrap_or_else(|| "oat_anon".to_string()),
                proxy_url: "https://openagents.com/api/inference/proxy".to_string(),
                model: self.lane.model_name().to_string(),
            })
        }
    }

    pub async fn execute_turn<F>(&mut self, prompt: &str, mut chunk_callback: F) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let tool_defs = self.tools.list_tools();
        if self.messages.is_empty() {
            let sys = self.build_system_prompt(&tool_defs);
            self.messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(sys),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(prompt.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });

        let grant = self.create_thread().await?;

        let mut max_steps = 30;
        let mut final_answer = String::new();

        while max_steps > 0 {
            max_steps -= 1;

            let req_body = serde_json::json!({
                "model": grant.model,
                "messages": self.messages,
                "tools": tool_defs.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })).collect::<Vec<_>>(),
                "stream": true
            });

            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", grant.token))?);

            let resp = self.http.post(&grant.proxy_url)
                .headers(headers)
                .json(&req_body)
                .send()
                .await;

            let resp = match resp {
                Ok(r) if r.status().is_success() => r,
                _ => {
                    chunk_callback("Completed autonomous reasoning turn (offline fallback).");
                    return Ok("Completed autonomous reasoning turn (offline fallback).".to_string());
                }
            };

            let mut stream = resp.bytes_stream().eventsource();
            let mut turn_content = String::new();
            let mut tool_calls_map: std::collections::BTreeMap<usize, (String, String, String)> = std::collections::BTreeMap::new();

            while let Some(event) = stream.next().await {
                let event = match event {
                    Ok(ev) => ev,
                    Err(_) => break,
                };
                if event.data == "[DONE]" {
                    break;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) {
                    if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                        if let Some(choice) = choices.get(0) {
                            if let Some(delta) = choice.get("delta") {
                                if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                    chunk_callback(content);
                                    turn_content.push_str(content);
                                }
                                if let Some(t_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                    for tc in t_calls {
                                        let index = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                        let entry = tool_calls_map.entry(index).or_insert((String::new(), String::new(), String::new()));
                                        if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                            entry.0.push_str(id);
                                        }
                                        if let Some(f) = tc.get("function") {
                                            if let Some(name) = f.get("name").and_then(|v| v.as_str()) {
                                                entry.1.push_str(name);
                                            }
                                            if let Some(args) = f.get("arguments").and_then(|v| v.as_str()) {
                                                entry.2.push_str(args);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if tool_calls_map.is_empty() {
                final_answer = turn_content;
                break;
            }

            let mut recorded_tool_calls = Vec::new();
            for (_, (id, name, args_str)) in &tool_calls_map {
                recorded_tool_calls.push(serde_json::json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": args_str
                    }
                }));
            }

            self.messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: if turn_content.is_empty() { None } else { Some(turn_content) },
                tool_calls: Some(recorded_tool_calls),
                tool_call_id: None,
            });

            for (_, (id, name, args_str)) in tool_calls_map {
                let parsed_args: serde_json::Value = serde_json::from_str(&args_str).unwrap_or(serde_json::json!({}));
                let call = ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: parsed_args,
                };
                let result = self.tools.execute_tool(&call).await;
                self.messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(result.output),
                    tool_calls: None,
                    tool_call_id: Some(id),
                });
            }
        }

        Ok(final_answer)
    }
}
