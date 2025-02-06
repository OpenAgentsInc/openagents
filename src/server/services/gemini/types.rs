use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiRequest {
    pub contents: Vec<Content>,
    pub safety_settings: Vec<SafetySetting>,
    pub generation_config: GenerationConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub candidate_count: i32,
    pub stop_sequences: Vec<String>,
    pub max_output_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub content: Content,
    pub finish_reason: String,
    pub safety_ratings: Vec<SafetyRating>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 1.0,
            top_k: 1,
            candidate_count: 1,
            stop_sequences: vec![],
            max_output_tokens: 2048,
        }
    }
}

impl Default for GeminiRequest {
    fn default() -> Self {
        Self {
            contents: vec![],
            safety_settings: vec![],
            generation_config: GenerationConfig::default(),
        }
    }
}