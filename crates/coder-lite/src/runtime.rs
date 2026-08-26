//! Open Responses streaming client for coder-lite.

use futures::StreamExt;
use openresponses_rust::{CreateResponseBody, Input, Item, StreamingClient, StreamingEvent};
use std::env;
use std::sync::mpsc::Sender;

const SYSTEM_INSTRUCTIONS: &str = "You are OpenAgents Coder. Do not say you are from Google, Anthropic, OpenAI, or any other company. Do not mention your model, training, or architecture. Respond as a neutral, terse terminal: no greetings, no \"As an AI\", no explanations of your role, and no unnecessary padding. Use short sentences and dense, factual output. Answer questions directly. Output only code and minimal context when asked for code.";

pub enum Control {
    Chunk(String),
    Done,
}

pub struct CoderRuntimeSession {
    pub api_key: String,
    pub base_url: String,
    pub history: Vec<Item>,
}

impl CoderRuntimeSession {
    pub fn new() -> Self {
        Self {
            api_key: env::var("OPENAGENTS_API_KEY").unwrap_or_default(),
            base_url: env::var("OPENAGENTS_BASE_URL")
                .unwrap_or_else(|_| "https://openagents.com/api/v1".to_string()),
            history: vec![Item::system_message(SYSTEM_INSTRUCTIONS)],
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

        let request = CreateResponseBody {
            model: env::var("OPENAGENTS_MODEL").ok(),
            input: Some(Input::Items(self.history.clone())),
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

        while let Some(event) = stream.next().await {
            match event {
                Ok(StreamingEvent::OutputTextDelta { delta, .. }) => {
                    collected.push_str(&delta);
                    let _ = tx.send(Control::Chunk(delta));
                }
                Ok(StreamingEvent::ReasoningDelta { delta, .. }) => {
                    collected.push_str(&delta);
                    let _ = tx.send(Control::Chunk(delta));
                }
                Ok(StreamingEvent::RefusalDelta { delta, .. }) => {
                    let _ = tx.send(Control::Chunk(delta));
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

        if !collected.is_empty() {
            self.history.push(Item::assistant_message(collected));
        }

        let _ = tx.send(Control::Done);
        Ok(())
    }
}
