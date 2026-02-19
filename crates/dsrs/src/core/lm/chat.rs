use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rig::completion::{AssistantContent, Message as RigMessage, message::UserContent};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    System { content: String },
    User { content: String },
    Assistant { content: String },
}

impl Message {
    pub fn new(role: &str, content: &str) -> Self {
        match role {
            "system" => Message::system(content),
            "user" => Message::user(content),
            "assistant" => Message::assistant(content),
            _ => panic!("Invalid role: {role}"),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Message::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Message::Assistant {
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Message::System {
            content: content.into(),
        }
    }

    pub fn content(&self) -> String {
        match self {
            Message::System { content } => content.clone(),
            Message::User { content } => content.clone(),
            Message::Assistant { content } => content.clone(),
        }
    }

    pub fn get_message_turn(&self) -> RigMessage {
        match self {
            Message::User { content } => RigMessage::user(content.clone()),
            Message::Assistant { content } => RigMessage::assistant(content.clone()),
            _ => panic!("Invalid role: {:?}", self),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Message::System { content } => json!({ "role": "system", "content": content }),
            Message::User { content } => json!({ "role": "user", "content": content }),
            Message::Assistant { content } => json!({ "role": "assistant", "content": content }),
        }
    }
}

impl From<RigMessage> for Message {
    fn from(message: RigMessage) -> Self {
        match message {
            RigMessage::User { content } => {
                let text = content
                    .into_iter()
                    .find_map(|c| {
                        if let UserContent::Text(t) = c {
                            Some(t.text)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                Message::user(text)
            }
            RigMessage::Assistant { content, .. } => {
                let text = content
                    .into_iter()
                    .find_map(|c| {
                        if let AssistantContent::Text(t) = c {
                            Some(t.text)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                Message::assistant(text)
            }
        }
    }
}

pub struct RigChatMessage {
    pub system: String,
    pub conversation: Vec<RigMessage>,
    pub prompt: RigMessage,
}

#[derive(Clone, Debug)]
pub struct Chat {
    pub messages: Vec<Message>,
}

impl Chat {
    pub fn new(messages: Vec<Message>) -> Self {
        Self { messages }
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn push(&mut self, role: &str, content: &str) {
        self.messages.push(Message::new(role, content));
    }

    pub fn push_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn push_all(&mut self, chat: &Chat) {
        self.messages.extend(chat.messages.clone());
    }

    pub fn pop(&mut self) -> Option<Message> {
        self.messages.pop()
    }

    pub fn from_json(&self, json_dump: Value) -> Result<Self> {
        let messages = json_dump.as_array().unwrap();
        let messages = messages
            .iter()
            .map(|message| {
                Message::new(
                    message["role"].as_str().unwrap(),
                    message["content"].as_str().unwrap(),
                )
            })
            .collect();
        Ok(Self { messages })
    }

    pub fn to_json(&self) -> Value {
        let messages = self
            .messages
            .iter()
            .map(|message| message.to_json())
            .collect::<Vec<Value>>();
        json!(messages)
    }

    pub fn get_rig_messages(&self) -> RigChatMessage {
        let system: String = self.messages[0].content();
        let conversation: Vec<RigMessage> = if self.messages.len() > 2 {
            self.messages[1..self.messages.len() - 1]
                .iter()
                .map(|message| message.get_message_turn())
                .collect::<Vec<RigMessage>>()
        } else {
            vec![]
        };
        let prompt = self.messages.last().unwrap().get_message_turn();

        RigChatMessage {
            system,
            conversation,
            prompt,
        }
    }
}
