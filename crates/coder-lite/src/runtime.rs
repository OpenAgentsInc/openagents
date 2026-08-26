//! Open Responses streaming client for coder-lite.

use futures::StreamExt;
use openresponses_rust::{
    CreateResponseBody, FunctionOutput, Input, Item, StreamingClient, StreamingEvent, Tool,
};
use std::env;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use crate::acp::Agent;
use crate::acp_harness::{AcpEvent, AcpHarness};

const SYSTEM_INSTRUCTIONS: &str = "You are OpenAgents Coder. Do not say you are from Google, Anthropic, OpenAI, or any other company. Do not mention your model, training, or architecture. Respond as a neutral, terse terminal: no greetings, no \"As an AI\", no explanations of your role, and no unnecessary padding. Use short sentences and dense, factual output. Answer questions directly. Output only code and minimal context when asked for code.";

pub enum Control {
    Chunk(String),
    Done,
    Tool {
        function_name: String,
        arguments: String,
        title: String,
    },
    ToolTitle(String),
    ToolText(String),
    ToolDone,
}

pub struct CoderRuntimeSession {
    pub api_key: String,
    pub base_url: String,
    pub history: Vec<Item>,
    pub agents: Vec<Agent>,
}

impl CoderRuntimeSession {
    pub fn new() -> Self {
        Self {
            api_key: env::var("OPENAGENTS_API_KEY").unwrap_or_default(),
            base_url: env::var("OPENAGENTS_BASE_URL")
                .unwrap_or_else(|_| "https://openagents.com/api/v1".to_string()),
            history: vec![Item::system_message(SYSTEM_INSTRUCTIONS)],
            agents: Vec::new(),
        }
    }

    pub async fn execute_turn(
        &mut self,
        prompt: &str,
        tx: Sender<Control>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.api_key.is_empty() {
            let _ = tx.send(Control::Chunk(
                "[error: OPENAGENTS_API_KEY is not set]".to_string(),
            ));
            let _ = tx.send(Control::Done);
            return Err("OPENAGENTS_API_KEY is not set".into());
        }

        self.history.push(Item::user_message(prompt));

        let client = StreamingClient::with_base_url(&self.api_key, &self.base_url);
        let tools = self.delegate_tool();

        loop {
            let request = CreateResponseBody {
                model: env::var("OPENAGENTS_MODEL").ok(),
                input: Some(Input::Items(self.history.clone())),
                tools: tools.clone(),
                stream: Some(true),
                ..Default::default()
            };

            let mut stream = match client.stream_response(request).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Control::Chunk(format!("[error: {}]", e)));
                    let _ = tx.send(Control::Done);
                    return Err(e.into());
                }
            };

            let mut collected = String::new();
            let mut pending_tool: Option<(String, String, String, String)> = None;

            while let Some(event) = stream.next().await {
                match event {
                    Ok(StreamingEvent::OutputTextDelta { delta, .. }) => {
                        collected.push_str(&delta);
                        let _ = tx.send(Control::Chunk(delta));
                    }
                    Ok(StreamingEvent::ReasoningDelta { delta, .. }) => {
                        let _ = tx.send(Control::Chunk(delta));
                    }
                    Ok(StreamingEvent::OutputItemDone {
                        item: Some(Item::FunctionCall {
                            call_id,
                            name,
                            arguments,
                            ..
                        }),
                        ..
                    }) if name == "delegate" => {
                        let args = serde_json::from_str::<serde_json::Value>(&arguments)
                            .unwrap_or(serde_json::json!({}));
                        let agent = args
                            .get("agent")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let task = args
                            .get("prompt")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        pending_tool = Some((call_id, agent, task, arguments));
                    }
                    Ok(StreamingEvent::Error { error, .. }) => {
                        let msg = format!("[error: {:?}]", error);
                        let _ = tx.send(Control::Chunk(msg));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let _ = tx.send(Control::Chunk(format!("[error: {}]", e)));
                        let _ = tx.send(Control::Done);
                        return Err(e.into());
                    }
                }
            }

            if let Some((call_id, agent_id, task, raw_args)) = pending_tool.take() {
                if let Some(agent) = self.agents.iter().find(|a| a.id == agent_id).cloned() {
                    let _ = tx.send(Control::Tool {
                        function_name: "delegate".to_string(),
                        arguments: raw_args,
                        title: task.clone(),
                    });

                    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    let mut header_sent = false;
                    let result = {
                        let tx = tx.clone();
                        AcpHarness {
                            command: agent.command,
                            args: agent.args,
                        }
                        .run(&task, &cwd, |event| {
                            match event {
                                AcpEvent::Tool { title, .. } => {
                                    header_sent = true;
                                    let _ = tx.send(Control::ToolTitle(title));
                                }
                                AcpEvent::Text { chunk } => {
                                    let _ = tx.send(Control::ToolText(chunk));
                                }
                                _ => {}
                            }
                        })
                        .await
                    };

                    match &result {
                        Ok(_) if !header_sent => {
                            let _ = tx.send(Control::ToolTitle("completed".to_string()));
                        }
                        Err(_) => {
                            let _ = tx.send(Control::ToolTitle("error".to_string()));
                            let _ = tx.send(Control::ToolText(
                                result.as_ref().err().unwrap().to_string(),
                            ));
                        }
                        _ => {}
                    }

                    let _ = tx.send(Control::ToolDone);
                    let output = result.unwrap_or_else(|e| e.to_string());
                    self.history.push(Item::FunctionCallOutput {
                        id: None,
                        call_id,
                        output: FunctionOutput::Text(output),
                        status: None,
                    });
                    continue;
                } else {
                    let msg = format!("unknown ACP agent: {}", agent_id);
                    let _ = tx.send(Control::Chunk(msg.clone()));
                    self.history.push(Item::FunctionCallOutput {
                        id: None,
                        call_id,
                        output: FunctionOutput::Text(msg),
                        status: None,
                    });
                    continue;
                }
            }

            if !collected.is_empty() {
                self.history.push(Item::assistant_message(collected));
            }
            break;
        }

        let _ = tx.send(Control::Done);
        Ok(())
    }

    fn delegate_tool(&self) -> Option<Vec<Tool>> {
        if self.agents.is_empty() {
            return None;
        }
        let ids: Vec<String> = self.agents.iter().map(|a| a.id.clone()).collect();
        let tool = Tool::function("delegate")
            .with_description("Delegate a coding task to an ACP agent on this machine.")
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "string",
                        "enum": ids,
                        "description": "the ACP agent to delegate to"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "the task for the child agent"
                    }
                },
                "required": ["agent", "prompt"]
            }));
        Some(vec![tool])
    }
}
