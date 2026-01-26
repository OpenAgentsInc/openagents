use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const DEFAULT_CONTINUE_PROMPT: &str =
    "Continue immediately. Do not ask for confirmation or pause. If errors occur, recover and keep going.";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FullAutoState {
    pub enabled: bool,
    pub thread_id: Option<String>,
    pub continue_prompt: String,
    pub last_turn_id: Option<String>,
}

pub type FullAutoMap = Arc<Mutex<HashMap<String, FullAutoState>>>;

impl FullAutoState {
    pub fn new(thread_id: Option<String>, continue_prompt: Option<String>) -> Self {
        Self {
            enabled: true,
            thread_id,
            continue_prompt: normalize_prompt(continue_prompt),
            last_turn_id: None,
        }
    }

    pub fn matches_thread(&self, thread_id: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }
        match (&self.thread_id, thread_id) {
            (Some(expected), Some(actual)) => expected == actual,
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    pub fn adopt_thread(&mut self, thread_id: &str) {
        if self.thread_id.is_none() {
            self.thread_id = Some(thread_id.to_string());
        }
    }

    pub fn set_continue_prompt(&mut self, prompt: Option<String>) {
        if prompt.is_some() {
            self.continue_prompt = normalize_prompt(prompt);
        }
    }

    pub fn should_continue(&mut self, thread_id: Option<&str>, turn_id: Option<&str>) -> bool {
        if !self.matches_thread(thread_id) {
            return false;
        }

        if let Some(thread_id) = thread_id {
            self.adopt_thread(thread_id);
        }

        if let Some(turn_id) = turn_id {
            if self.last_turn_id.as_deref() == Some(turn_id) {
                return false;
            }
            self.last_turn_id = Some(turn_id.to_string());
        }

        true
    }
}

fn normalize_prompt(prompt: Option<String>) -> String {
    let trimmed = prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    trimmed
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_CONTINUE_PROMPT.to_string())
}
